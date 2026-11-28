//! Kvóta- és breaker-kapu integrációs tesztek (Task 8).
//! Determinisztikus idő: fix bázis-időpont, nincs élő hálózat.

use chrono::{DateTime, Duration, SubsecRound, Utc};
use football_domain::policy::{BreakerState, FetchOutcome, SourceMode, SourcePolicy, Transport};
use football_storage::repositories::worker::{self as repo, BreakerRow};
use football_storage::test_support::test_pool;
use football_worker::{breaker, quota};
use sqlx::PgPool;
use uuid::Uuid;

fn base_time() -> DateTime<Utc> {
    DateTime::from_timestamp(1_763_000_000, 0).unwrap()
}

async fn seed_policy(pool: &PgPool, daily_budget: i32, min_delay_ms: i64) -> SourcePolicy {
    let policy = SourcePolicy {
        source_slug: "demo-synthetic".into(),
        transport: Transport::Fixture,
        mode: SourceMode::Shadow,
        daily_budget,
        min_delay_ms,
        jitter_ms: 0,
        breaker_threshold: 2,
        breaker_cooldown_secs: 60,
        adapter_version: "1.0".into(),
        parser_version: "1.0".into(),
        domains: vec![],
        events: vec![],
        enabled: true,
        max_bytes: 5_242_880,
    };
    repo::upsert_policy(pool, &policy).await.unwrap();
    policy
}

async fn is_allowed(pool: &PgPool, policy: &SourcePolicy, now: DateTime<Utc>) -> bool {
    matches!(
        breaker::allow(pool, policy, now).await.unwrap(),
        breaker::BreakerDecision::Allowed(_)
    )
}

async fn acquire_probe(pool: &PgPool, policy: &SourcePolicy, now: DateTime<Utc>) -> Uuid {
    match breaker::allow(pool, policy, now).await.unwrap() {
        breaker::BreakerDecision::Allowed(Some(token)) => token,
        other => panic!("nem érkezett probe engedély: {other:?}"),
    }
}

async fn ledger(pool: &PgPool, at: DateTime<Utc>, outcome: FetchOutcome) {
    repo::insert_ledger(
        pool,
        &repo::LedgerEntry {
            source_slug: "demo-synthetic".into(),
            request_key: "k".into(),
            attempted_at: at,
            outcome,
            http_status: matches!(outcome, FetchOutcome::Success).then_some(200),
            byte_size: Some(10),
            latency_ms: Some(5),
            artifact_hash: None,
            job_id: None,
        },
    )
    .await
    .unwrap();
}

async fn leased_job(pool: &PgPool, entity_key: &str) -> football_domain::policy::LeasedJob {
    let due = Utc::now() - Duration::seconds(1);
    repo::insert_job(
        pool,
        &repo::NewJob {
            source_slug: "demo-synthetic".into(),
            domain: "match".into(),
            entity_key: entity_key.into(),
            event_type: "discovery".into(),
            time_window: due.date_naive().to_string(),
            due_at: due,
            priority: 0,
        },
    )
    .await
    .unwrap();
    repo::claim_due_jobs(pool, uuid::Uuid::now_v7(), 60, 1)
        .await
        .unwrap()
        .remove(0)
}

#[tokio::test]
async fn quota_blocks_after_budget() {
    let pool = test_pool().await;
    seed_policy(&pool, 2, 0).await;
    let leased = leased_job(&pool, "quota-budget").await;
    for index in 0..2 {
        let decision = quota::reserve(
            &pool,
            "demo-synthetic",
            &format!("budget-{index}"),
            &leased.lease,
        )
        .await
        .unwrap();
        assert!(matches!(decision, quota::QuotaDecision::Allowed(_)));
    }
    let decision = quota::reserve(&pool, "demo-synthetic", "budget-blocked", &leased.lease)
        .await
        .unwrap();
    assert_eq!(decision, quota::QuotaDecision::QuotaBlocked);
}

#[tokio::test]
async fn quota_blocked_ledger_rows_do_not_count() {
    let pool = test_pool().await;
    seed_policy(&pool, 2, 0).await;
    let leased = leased_job(&pool, "quota-blocked-ledger").await;
    let now = Utc::now();
    assert!(matches!(
        quota::reserve(&pool, "demo-synthetic", "first", &leased.lease)
            .await
            .unwrap(),
        quota::QuotaDecision::Allowed(_)
    ));
    for _ in 0..3 {
        ledger(&pool, now, FetchOutcome::QuotaBlocked).await;
    }
    // 1 valódi kísérlet + 3 quota_blocked: a számláló 1, budget 2 → Allowed
    assert!(matches!(
        quota::reserve(&pool, "demo-synthetic", "second", &leased.lease)
            .await
            .unwrap(),
        quota::QuotaDecision::Allowed(_)
    ));
    assert_eq!(
        quota::reserve(&pool, "demo-synthetic", "third", &leased.lease)
            .await
            .unwrap(),
        quota::QuotaDecision::QuotaBlocked
    );
}

#[tokio::test]
async fn delay_blocks_until_min_delay_elapsed() {
    let pool = test_pool().await;
    seed_policy(&pool, 10, 60_000).await;
    let leased = leased_job(&pool, "delay-boundary").await;
    let db_now: DateTime<Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await
        .unwrap();
    ledger(&pool, db_now - Duration::seconds(30), FetchOutcome::Success).await;
    assert_eq!(
        quota::reserve(&pool, "demo-synthetic", "blocked", &leased.lease)
            .await
            .unwrap(),
        quota::QuotaDecision::DelayBlocked
    );

    sqlx::query(
        "UPDATE ops.fetch_ledger
         SET attempted_at = clock_timestamp() - interval '61 seconds'
         WHERE source_slug='demo-synthetic' AND outcome='success'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        quota::reserve(&pool, "demo-synthetic", "allowed", &leased.lease)
            .await
            .unwrap(),
        quota::QuotaDecision::Allowed(_)
    ));
}

#[tokio::test]
async fn min_delay_survives_utc_midnight() {
    let pool = test_pool().await;
    let mut policy = seed_policy(&pool, 10, 0).await;
    let leased = leased_job(&pool, "midnight-delay").await;
    let db_now: DateTime<Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let midnight = db_now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
    let last = midnight - Duration::seconds(1);
    ledger(&pool, last, FetchOutcome::Success).await;
    policy.min_delay_ms = db_now.signed_duration_since(last).num_milliseconds() + 60_000;
    repo::upsert_policy(&pool, &policy).await.unwrap();

    let decision = quota::reserve(&pool, "demo-synthetic", "after-midnight", &leased.lease)
        .await
        .unwrap();
    assert_eq!(
        decision,
        quota::QuotaDecision::DelayBlocked,
        "az éjfél nem törölheti az előző nap globális min-delay óráját"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reservations_never_exceed_daily_budget() {
    let pool = test_pool().await;
    seed_policy(&pool, 3, 0).await;
    let due = Utc::now() - Duration::seconds(1);
    repo::insert_job(
        &pool,
        &repo::NewJob {
            source_slug: "demo-synthetic".into(),
            domain: "match".into(),
            entity_key: "quota-race".into(),
            event_type: "discovery".into(),
            time_window: due.date_naive().to_string(),
            due_at: due,
            priority: 0,
        },
    )
    .await
    .unwrap();
    let leased = repo::claim_due_jobs(&pool, uuid::Uuid::now_v7(), 60, 1)
        .await
        .unwrap()
        .remove(0);
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(12));
    let handles: Vec<_> = (0..12)
        .map(|index| {
            let pool = pool.clone();
            let lease = leased.lease;
            let barrier = barrier.clone();
            tokio::spawn(async move {
                barrier.wait().await;
                quota::reserve(&pool, "demo-synthetic", &format!("race-{index}"), &lease)
                    .await
                    .unwrap()
            })
        })
        .collect();

    let mut allowed = 0;
    let mut blocked = 0;
    for handle in handles {
        match handle.await.unwrap() {
            quota::QuotaDecision::Allowed(_) => allowed += 1,
            quota::QuotaDecision::QuotaBlocked => blocked += 1,
            other => panic!("váratlan kvótadöntés: {other:?}"),
        }
    }
    assert_eq!((allowed, blocked), (3, 9));
    let reserved: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.fetch_ledger
         WHERE source_slug='demo-synthetic' AND outcome='reserved'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reserved, 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reservations_obey_min_delay_and_reserved_survives_crash() {
    let pool = test_pool().await;
    seed_policy(&pool, 10, 60_000).await;
    let due = Utc::now() - Duration::seconds(1);
    repo::insert_job(
        &pool,
        &repo::NewJob {
            source_slug: "demo-synthetic".into(),
            domain: "match".into(),
            entity_key: "delay-race".into(),
            event_type: "discovery".into(),
            time_window: due.date_naive().to_string(),
            due_at: due,
            priority: 0,
        },
    )
    .await
    .unwrap();
    let leased = repo::claim_due_jobs(&pool, uuid::Uuid::now_v7(), 60, 1)
        .await
        .unwrap()
        .remove(0);
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|index| {
            let pool = pool.clone();
            let lease = leased.lease;
            let barrier = barrier.clone();
            tokio::spawn(async move {
                barrier.wait().await;
                quota::reserve(&pool, "demo-synthetic", &format!("delay-{index}"), &lease)
                    .await
                    .unwrap()
            })
        })
        .collect();

    let mut allowed = 0;
    let mut delayed = 0;
    for handle in handles {
        match handle.await.unwrap() {
            quota::QuotaDecision::Allowed(_) => allowed += 1,
            quota::QuotaDecision::DelayBlocked => delayed += 1,
            other => panic!("váratlan delay-döntés: {other:?}"),
        }
    }
    assert_eq!((allowed, delayed), (1, 7));

    let next = quota::reserve(&pool, "demo-synthetic", "after-crash", &leased.lease)
        .await
        .unwrap();
    assert!(matches!(next, quota::QuotaDecision::DelayBlocked));
    let reserved: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.fetch_ledger
         WHERE source_slug='demo-synthetic' AND outcome='reserved'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reserved, 1);
}

#[tokio::test]
async fn reservation_rejects_a_lost_lease_without_consuming_quota() {
    let pool = test_pool().await;
    seed_policy(&pool, 1, 0).await;
    let now = Utc::now().trunc_subsecs(6);
    repo::insert_job(
        &pool,
        &repo::NewJob {
            source_slug: "demo-synthetic".into(),
            domain: "match".into(),
            entity_key: "lost-lease".into(),
            event_type: "discovery".into(),
            time_window: now.date_naive().to_string(),
            due_at: now - Duration::seconds(1),
            priority: 0,
        },
    )
    .await
    .unwrap();
    let leased = repo::claim_due_jobs(&pool, uuid::Uuid::now_v7(), 60, 1)
        .await
        .unwrap()
        .remove(0);
    sqlx::query(
        "UPDATE ops.job SET leased_until = clock_timestamp() - interval '1 second' WHERE id = $1",
    )
    .bind(leased.job.id)
    .execute(&pool)
    .await
    .unwrap();

    let decision = quota::reserve(&pool, "demo-synthetic", "lost-lease", &leased.lease)
        .await
        .unwrap();
    assert!(matches!(decision, quota::QuotaDecision::LeaseLost));
    let reserved: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.fetch_ledger
         WHERE source_slug='demo-synthetic' AND outcome='reserved'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reserved, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn only_one_concurrent_half_open_probe_is_allowed() {
    let pool = test_pool().await;
    let policy = seed_policy(&pool, 10, 0).await;
    let now = base_time();
    repo::update_breaker_locked(&pool, &policy.source_slug, |_| {
        (
            (),
            repo::BreakerWrite::Set(BreakerRow {
                state: BreakerState::Open,
                failure_count: 2,
                opened_at: Some(now - Duration::seconds(60)),
                next_probe_at: Some(now),
                probe_token: None,
            }),
        )
    })
    .await
    .unwrap();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(12));
    let handles: Vec<_> = (0..12)
        .map(|_| {
            let pool = pool.clone();
            let policy = policy.clone();
            let barrier = barrier.clone();
            tokio::spawn(async move {
                barrier.wait().await;
                is_allowed(&pool, &policy, now).await
            })
        })
        .collect();

    let mut probes = 0;
    for handle in handles {
        if handle.await.unwrap() {
            probes += 1;
        }
    }
    assert_eq!(probes, 1);
    let row = repo::get_breaker(&pool, &policy.source_slug)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::HalfOpen);
}

#[tokio::test]
async fn unused_probe_release_preserves_open_state_and_allows_retry() {
    let pool = test_pool().await;
    let policy = seed_policy(&pool, 10, 0).await;
    let now = base_time();
    let open = BreakerRow {
        state: BreakerState::Open,
        failure_count: 2,
        opened_at: Some(now - Duration::seconds(60)),
        next_probe_at: Some(now),
        probe_token: None,
    };
    repo::update_breaker_locked(&pool, &policy.source_slug, |_| {
        ((), repo::BreakerWrite::Set(open.clone()))
    })
    .await
    .unwrap();
    let token = acquire_probe(&pool, &policy, now).await;

    breaker::release_probe(&pool, &policy.source_slug, token)
        .await
        .unwrap();

    let row = repo::get_breaker(&pool, &policy.source_slug)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::Open);
    assert_eq!(row.failure_count, open.failure_count);
    assert_eq!(row.opened_at, open.opened_at);
    assert_eq!(row.next_probe_at, open.next_probe_at);
    assert_eq!(row.probe_token, None);
    assert!(is_allowed(&pool, &policy, now).await);
}

#[tokio::test]
async fn unused_probe_release_does_not_undo_a_newer_transition() {
    let pool = test_pool().await;
    let policy = seed_policy(&pool, 10, 0).await;
    let now = base_time();
    repo::update_breaker_locked(&pool, &policy.source_slug, |_| {
        (
            (),
            repo::BreakerWrite::Set(BreakerRow {
                state: BreakerState::Open,
                failure_count: 2,
                opened_at: Some(now - Duration::seconds(60)),
                next_probe_at: Some(now),
                probe_token: None,
            }),
        )
    })
    .await
    .unwrap();
    let token = acquire_probe(&pool, &policy, now).await;
    breaker::on_success(&pool, &policy.source_slug, Some(token))
        .await
        .unwrap();

    breaker::release_probe(&pool, &policy.source_slug, token)
        .await
        .unwrap();

    let row = repo::get_breaker(&pool, &policy.source_slug)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::Closed);
    assert_eq!(row.failure_count, 0);
}

#[tokio::test]
async fn stale_probe_release_cannot_reopen_newer_half_open_generation() {
    let pool = test_pool().await;
    let policy = seed_policy(&pool, 10, 0).await;
    let now = base_time();
    repo::update_breaker_locked(&pool, &policy.source_slug, |_| {
        (
            (),
            repo::BreakerWrite::Set(BreakerRow {
                state: BreakerState::Open,
                failure_count: 2,
                opened_at: Some(now - Duration::seconds(60)),
                next_probe_at: Some(now),
                probe_token: None,
            }),
        )
    })
    .await
    .unwrap();

    let old_token = acquire_probe(&pool, &policy, now).await;
    breaker::release_probe(&pool, &policy.source_slug, old_token)
        .await
        .unwrap();
    let new_token = acquire_probe(&pool, &policy, now).await;
    breaker::release_probe(&pool, &policy.source_slug, old_token)
        .await
        .unwrap();

    let row = repo::get_breaker(&pool, &policy.source_slug)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        row.state,
        BreakerState::HalfOpen,
        "a régi probe felszabadítása nem nyithatja vissza az újabb generációt"
    );
    assert_eq!(row.probe_token, Some(new_token));
}

#[tokio::test]
async fn stale_probe_completion_cannot_change_newer_generation() {
    let pool = test_pool().await;
    let policy = seed_policy(&pool, 10, 0).await;
    let now = base_time();
    repo::update_breaker_locked(&pool, &policy.source_slug, |_| {
        (
            (),
            repo::BreakerWrite::Set(BreakerRow {
                state: BreakerState::Open,
                failure_count: 2,
                opened_at: Some(now - Duration::seconds(60)),
                next_probe_at: Some(now),
                probe_token: None,
            }),
        )
    })
    .await
    .unwrap();

    let old_token = acquire_probe(&pool, &policy, now).await;
    breaker::release_probe(&pool, &policy.source_slug, old_token)
        .await
        .unwrap();
    let new_token = acquire_probe(&pool, &policy, now).await;

    breaker::on_success(&pool, &policy.source_slug, Some(old_token))
        .await
        .unwrap();
    breaker::on_failure(&pool, &policy, now, Some(old_token))
        .await
        .unwrap();
    breaker::on_auth_failure(&pool, &policy, now, Some(old_token))
        .await
        .unwrap();

    let row = repo::get_breaker(&pool, &policy.source_slug)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::HalfOpen);
    assert_eq!(row.failure_count, 2);
    assert_eq!(row.probe_token, Some(new_token));
}

#[tokio::test]
async fn cancelled_probe_owner_releases_half_open_generation() {
    let pool = test_pool().await;
    let policy = seed_policy(&pool, 10, 0).await;
    let now = base_time();
    repo::update_breaker_locked(&pool, &policy.source_slug, |_| {
        (
            (),
            repo::BreakerWrite::Set(BreakerRow {
                state: BreakerState::Open,
                failure_count: 2,
                opened_at: Some(now - Duration::seconds(60)),
                next_probe_at: Some(now),
                probe_token: None,
            }),
        )
    })
    .await
    .unwrap();

    let owner_pool = pool.clone();
    let owner_policy = policy.clone();
    let owner = tokio::spawn(async move {
        let decision = breaker::allow(&owner_pool, &owner_policy, now)
            .await
            .unwrap();
        let _guard = breaker::ProbeGuard::new(&owner_pool, &owner_policy.source_slug, decision);
        std::future::pending::<()>().await;
    });
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let row = repo::get_breaker(&pool, &policy.source_slug)
            .await
            .unwrap()
            .unwrap();
        if row.state == BreakerState::HalfOpen {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "a probe tulajdonosa nem érte el a half_open állapotot"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    owner.abort();
    assert!(owner.await.unwrap_err().is_cancelled());
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let row = repo::get_breaker(&pool, &policy.source_slug)
            .await
            .unwrap()
            .unwrap();
        if row.state == BreakerState::Open {
            assert_eq!(row.probe_token, None);
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "a megszakított probe nem szabadult fel"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_breaker_failures_do_not_lose_updates() {
    let pool = test_pool().await;
    let policy = seed_policy(&pool, 10, 0).await;
    let now = base_time();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(10));
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let pool = pool.clone();
            let policy = policy.clone();
            let barrier = barrier.clone();
            tokio::spawn(async move {
                barrier.wait().await;
                breaker::on_failure(&pool, &policy, now, None)
                    .await
                    .unwrap()
            })
        })
        .collect();
    for handle in handles {
        handle.await.unwrap();
    }
    let row = repo::get_breaker(&pool, &policy.source_slug)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.failure_count, 10);
    assert_eq!(row.state, BreakerState::Open);
}

#[tokio::test]
async fn auth_failure_opens_breaker_and_flags_policy_review() {
    let pool = test_pool().await;
    let policy = seed_policy(&pool, 10, 0).await;
    let now = base_time();
    breaker::on_auth_failure(&pool, &policy, now, None)
        .await
        .unwrap();
    let row = repo::get_breaker(&pool, "demo-synthetic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::Open);
    assert_eq!(row.failure_count, 1);
    assert_eq!(row.opened_at, Some(now));
    assert_eq!(row.next_probe_at, Some(now + Duration::seconds(60)));
    let audits: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ops.audit_event WHERE action='policy_review'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(audits, 1);
    let (actor, subject_kind, slug, status): (String, String, String, String) = sqlx::query_as(
        "SELECT actor, subject_kind, detail->>'slug', detail->>'status'
         FROM ops.audit_event WHERE action='policy_review'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(actor, "worker");
    assert_eq!(subject_kind, "source");
    assert_eq!(slug, "demo-synthetic");
    assert_eq!(status, "auth_failure");
}

#[tokio::test]
async fn breaker_opens_at_threshold_and_probes_once() {
    let pool = test_pool().await;
    let policy = seed_policy(&pool, 10, 0).await;
    let now = base_time();
    // nincs breaker-sor → closed-ként kezel → true
    assert!(is_allowed(&pool, &policy, now).await);
    // küszöb alatti hiba: marad closed
    breaker::on_failure(&pool, &policy, now, None)
        .await
        .unwrap();
    let row = repo::get_breaker(&pool, "demo-synthetic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::Closed);
    assert_eq!(row.failure_count, 1);
    assert!(is_allowed(&pool, &policy, now).await);
    // második hiba (threshold=2): open, cooldown 60s
    breaker::on_failure(&pool, &policy, now, None)
        .await
        .unwrap();
    let row = repo::get_breaker(&pool, "demo-synthetic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::Open);
    assert_eq!(row.next_probe_at, Some(now + Duration::seconds(60)));
    assert!(!is_allowed(&pool, &policy, now).await);
    assert!(!is_allowed(&pool, &policy, now + Duration::seconds(59)).await);
    // probe-idő elérkezett: half_open átmenet + egyetlen probe
    let probe_at = now + Duration::seconds(60);
    let token = acquire_probe(&pool, &policy, probe_at).await;
    let row = repo::get_breaker(&pool, "demo-synthetic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::HalfOpen);
    assert_eq!(row.probe_token, Some(token));
    // half_open alatt nincs második probe
    assert!(!is_allowed(&pool, &policy, probe_at).await);
    // sikertelen probe: azonnal újra open, új cooldown
    breaker::on_failure(&pool, &policy, probe_at, Some(token))
        .await
        .unwrap();
    let row = repo::get_breaker(&pool, "demo-synthetic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::Open);
    assert_eq!(row.next_probe_at, Some(probe_at + Duration::seconds(60)));
    assert!(!is_allowed(&pool, &policy, probe_at).await);
    // sikeres újabb probe után closed, számláló nulla
    let retry_at = probe_at + Duration::seconds(60);
    let retry_token = acquire_probe(&pool, &policy, retry_at).await;
    breaker::on_success(&pool, "demo-synthetic", Some(retry_token))
        .await
        .unwrap();
    let row = repo::get_breaker(&pool, "demo-synthetic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::Closed);
    assert_eq!(row.failure_count, 0);
    assert!(is_allowed(&pool, &policy, retry_at).await);
}
