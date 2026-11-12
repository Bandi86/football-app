//! Kvóta- és breaker-kapu integrációs tesztek (Task 8).
//! Determinisztikus idő: fix bázis-időpont, nincs élő hálózat.

use chrono::{DateTime, Duration, Utc};
use football_domain::policy::{BreakerState, FetchOutcome, SourceMode, SourcePolicy, Transport};
use football_storage::repositories::worker as repo;
use football_storage::test_support::test_pool;
use football_worker::{breaker, quota};
use sqlx::PgPool;

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

#[tokio::test]
async fn quota_blocks_after_budget() {
    let pool = test_pool().await;
    let policy = seed_policy(&pool, 2, 0).await;
    let now = base_time();
    // üres ledger: engedélyezett
    let decision = quota::guard(&pool, &policy, now).await.unwrap();
    assert!(matches!(decision, quota::QuotaDecision::Allowed));
    for _ in 0..2 {
        ledger(&pool, now, FetchOutcome::Success).await;
    }
    let decision = quota::guard(&pool, &policy, now).await.unwrap();
    assert!(matches!(decision, quota::QuotaDecision::QuotaBlocked));
}

#[tokio::test]
async fn quota_blocked_ledger_rows_do_not_count() {
    let pool = test_pool().await;
    let policy = seed_policy(&pool, 2, 0).await;
    let now = base_time();
    ledger(&pool, now, FetchOutcome::Success).await;
    for _ in 0..3 {
        ledger(&pool, now, FetchOutcome::QuotaBlocked).await;
    }
    // 1 valódi kísérlet + 3 quota_blocked: a számláló 1, budget 2 → Allowed
    let decision = quota::guard(&pool, &policy, now).await.unwrap();
    assert!(matches!(decision, quota::QuotaDecision::Allowed));
    ledger(&pool, now, FetchOutcome::Success).await;
    let decision = quota::guard(&pool, &policy, now).await.unwrap();
    assert!(matches!(decision, quota::QuotaDecision::QuotaBlocked));
}

#[tokio::test]
async fn delay_blocks_until_min_delay_elapsed() {
    let pool = test_pool().await;
    let policy = seed_policy(&pool, 10, 60_000).await;
    let now = base_time();
    ledger(&pool, now - Duration::seconds(30), FetchOutcome::Success).await;
    // last + 60s = now + 30s: addig DelayBlocked
    let decision = quota::guard(&pool, &policy, now).await.unwrap();
    assert!(matches!(decision, quota::QuotaDecision::DelayBlocked));
    let decision = quota::guard(&pool, &policy, now + Duration::seconds(29))
        .await
        .unwrap();
    assert!(matches!(decision, quota::QuotaDecision::DelayBlocked));
    // határon (last + min_delay == now) már engedélyezett
    let decision = quota::guard(&pool, &policy, now + Duration::seconds(30))
        .await
        .unwrap();
    assert!(matches!(decision, quota::QuotaDecision::Allowed));
}

#[tokio::test]
async fn auth_failure_opens_breaker_and_flags_policy_review() {
    let pool = test_pool().await;
    let policy = seed_policy(&pool, 10, 0).await;
    let now = base_time();
    breaker::on_auth_failure(&pool, &policy, now).await.unwrap();
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
    assert!(breaker::allow(&pool, &policy, now).await.unwrap());
    // küszöb alatti hiba: marad closed
    breaker::on_failure(&pool, &policy, now).await.unwrap();
    let row = repo::get_breaker(&pool, "demo-synthetic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::Closed);
    assert_eq!(row.failure_count, 1);
    assert!(breaker::allow(&pool, &policy, now).await.unwrap());
    // második hiba (threshold=2): open, cooldown 60s
    breaker::on_failure(&pool, &policy, now).await.unwrap();
    let row = repo::get_breaker(&pool, "demo-synthetic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::Open);
    assert_eq!(row.next_probe_at, Some(now + Duration::seconds(60)));
    assert!(!breaker::allow(&pool, &policy, now).await.unwrap());
    assert!(
        !breaker::allow(&pool, &policy, now + Duration::seconds(59))
            .await
            .unwrap()
    );
    // probe-idő elérkezett: half_open átmenet + egyetlen probe
    let probe_at = now + Duration::seconds(60);
    assert!(breaker::allow(&pool, &policy, probe_at).await.unwrap());
    let row = repo::get_breaker(&pool, "demo-synthetic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::HalfOpen);
    // half_open alatt nincs második probe
    assert!(!breaker::allow(&pool, &policy, probe_at).await.unwrap());
    // sikertelen probe: azonnal újra open, új cooldown
    breaker::on_failure(&pool, &policy, probe_at).await.unwrap();
    let row = repo::get_breaker(&pool, "demo-synthetic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::Open);
    assert_eq!(row.next_probe_at, Some(probe_at + Duration::seconds(60)));
    assert!(!breaker::allow(&pool, &policy, probe_at).await.unwrap());
    // sikeres probe után closed, számláló nulla
    breaker::on_success(&pool, "demo-synthetic").await.unwrap();
    let row = repo::get_breaker(&pool, "demo-synthetic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::Closed);
    assert_eq!(row.failure_count, 0);
    assert!(breaker::allow(&pool, &policy, probe_at).await.unwrap());
}
