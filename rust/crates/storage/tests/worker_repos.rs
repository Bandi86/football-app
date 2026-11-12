use chrono::{Duration, Utc};
use football_domain::policy::*;
use football_storage::repositories::worker as repo;
use football_storage::test_support::test_pool;
use uuid::Uuid;

fn policy(slug: &str) -> SourcePolicy {
    SourcePolicy {
        source_slug: slug.into(),
        transport: Transport::Fixture,
        mode: SourceMode::Shadow,
        daily_budget: 2,
        min_delay_ms: 0,
        jitter_ms: 0,
        breaker_threshold: 3,
        breaker_cooldown_secs: 60,
        adapter_version: "1.0".into(),
        parser_version: "1.0".into(),
        domains: vec!["match".into()],
        events: vec![],
        enabled: true,
        max_bytes: 5_242_880,
    }
}

async fn seed_policy(pool: &sqlx::PgPool, slug: &str) {
    repo::upsert_policy(pool, &policy(slug)).await.unwrap();
}

fn ledger(
    slug: &str,
    attempted_at: chrono::DateTime<Utc>,
    outcome: FetchOutcome,
) -> repo::LedgerEntry {
    repo::LedgerEntry {
        source_slug: slug.into(),
        request_key: "k".into(),
        attempted_at,
        outcome,
        http_status: None,
        byte_size: None,
        latency_ms: None,
        artifact_hash: None,
        job_id: None,
    }
}

fn new_job(
    slug: &str,
    entity_key: &str,
    due_at: chrono::DateTime<Utc>,
    priority: i32,
) -> repo::NewJob {
    repo::NewJob {
        source_slug: slug.into(),
        domain: "match".into(),
        entity_key: entity_key.into(),
        event_type: "discovery".into(),
        time_window: due_at.date_naive().to_string(),
        due_at,
        priority,
    }
}

#[tokio::test]
async fn policy_upsert_get_and_list() {
    let pool = test_pool().await;

    // hiányzó policy: None
    assert!(
        repo::get_policy(&pool, "nincs-ilyen")
            .await
            .unwrap()
            .is_none()
    );

    let mut a = policy("demo-a");
    a.events = vec![EventTemplate {
        event_type: "discovery".into(),
        offset_minutes: Some(-30),
        interval_minutes: None,
    }];
    repo::upsert_policy(&pool, &a).await.unwrap();

    let got = repo::get_policy(&pool, "demo-a").await.unwrap().unwrap();
    assert_eq!(got.source_slug, "demo-a");
    assert_eq!(got.transport, Transport::Fixture);
    assert_eq!(got.mode, SourceMode::Shadow);
    assert_eq!(got.daily_budget, 2);
    assert_eq!(got.breaker_threshold, 3);
    assert_eq!(got.breaker_cooldown_secs, 60);
    assert_eq!(got.domains, vec!["match".to_string()]);
    assert_eq!(got.events.len(), 1);
    assert_eq!(got.events[0].event_type, "discovery");
    assert_eq!(got.events[0].offset_minutes, Some(-30));
    assert_eq!(got.events[0].interval_minutes, None);
    assert!(got.enabled);
    assert_eq!(got.max_bytes, 5_242_880);

    // upsert ugyanarra a kulcsra: frissít, nem duplikál
    a.daily_budget = 9;
    a.enabled = false;
    repo::upsert_policy(&pool, &a).await.unwrap();
    let updated = repo::get_policy(&pool, "demo-a").await.unwrap().unwrap();
    assert_eq!(updated.daily_budget, 9);
    assert!(!updated.enabled);

    repo::upsert_policy(&pool, &policy("demo-b")).await.unwrap();
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.source_policy")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 2);

    // csak az engedélyezett policy-k listázódnak
    let enabled = repo::list_enabled_policies(&pool).await.unwrap();
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].source_slug, "demo-b");
}

#[tokio::test]
async fn ledger_counts_only_fetch_attempts() {
    let pool = test_pool().await;
    seed_policy(&pool, "demo-synthetic").await;
    let now = Utc::now();
    for outcome in [
        FetchOutcome::Success,
        FetchOutcome::Timeout,
        FetchOutcome::QuotaBlocked,
    ] {
        repo::insert_ledger(&pool, &ledger("demo-synthetic", now, outcome))
            .await
            .unwrap();
    }
    // a quota_blocked nem fetch-kísérlet: nem számít a kvótába
    let n = repo::count_fetch_attempts_today(&pool, "demo-synthetic", now.date_naive())
        .await
        .unwrap();
    assert_eq!(n, 2);
}

#[tokio::test]
async fn ledger_day_filter_fields_and_last_attempt() {
    let pool = test_pool().await;
    seed_policy(&pool, "demo-synthetic").await;
    let now = Utc::now();
    let yesterday = now - Duration::days(1);

    // tegnapi kísérlet nem számít a mai kvótába
    repo::insert_ledger(
        &pool,
        &ledger("demo-synthetic", yesterday, FetchOutcome::Success),
    )
    .await
    .unwrap();
    let job_id = Uuid::now_v7();
    let mut full = ledger("demo-synthetic", now, FetchOutcome::HttpError);
    full.http_status = Some(503);
    full.byte_size = Some(1234);
    full.latency_ms = Some(42);
    full.artifact_hash = Some("abc123".into());
    full.job_id = Some(job_id);
    let id = repo::insert_ledger(&pool, &full).await.unwrap();

    let n = repo::count_fetch_attempts_today(&pool, "demo-synthetic", now.date_naive())
        .await
        .unwrap();
    assert_eq!(n, 1);

    // a beszúrás visszaadja az azonosítót, a mezők pontosan tárolódnak
    let row = sqlx::query(
        "SELECT request_key, outcome, http_status, byte_size, latency_ms, artifact_hash, job_id
         FROM ops.fetch_ledger WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    use sqlx::Row;
    assert_eq!(row.try_get::<String, _>("request_key").unwrap(), "k");
    assert_eq!(row.try_get::<String, _>("outcome").unwrap(), "http_error");
    assert_eq!(
        row.try_get::<Option<i16>, _>("http_status").unwrap(),
        Some(503)
    );
    assert_eq!(
        row.try_get::<Option<i64>, _>("byte_size").unwrap(),
        Some(1234)
    );
    assert_eq!(
        row.try_get::<Option<i64>, _>("latency_ms").unwrap(),
        Some(42)
    );
    assert_eq!(
        row.try_get::<Option<String>, _>("artifact_hash")
            .unwrap()
            .as_deref(),
        Some("abc123")
    );
    assert_eq!(
        row.try_get::<Option<Uuid>, _>("job_id").unwrap(),
        Some(job_id)
    );

    // last_attempt_at a legutóbbi kísérlet időpontja; ismeretlen forrásra None
    let last = repo::last_attempt_at(&pool, "demo-synthetic")
        .await
        .unwrap();
    assert_eq!(last, Some(now));
    assert!(
        repo::last_attempt_at(&pool, "nincs-ilyen")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn last_attempt_at_ignores_non_fetch_outcomes() {
    let pool = test_pool().await;
    seed_policy(&pool, "demo-synthetic").await;
    let now = Utc::now();

    // Valódi fetch-kísérlet T-ben, majd későbbi, nem számláló blokk-sorok:
    // a min_delay óráját NEM tolhatják ki (DelayBlocked livelock regresszió).
    repo::insert_ledger(&pool, &ledger("demo-synthetic", now, FetchOutcome::Success))
        .await
        .unwrap();
    for (i, outcome) in [
        FetchOutcome::QuotaBlocked,
        FetchOutcome::RobotsBlocked,
        FetchOutcome::BreakerOpen,
    ]
    .into_iter()
    .enumerate()
    {
        repo::insert_ledger(
            &pool,
            &ledger(
                "demo-synthetic",
                now + Duration::seconds(i as i64 + 1),
                outcome,
            ),
        )
        .await
        .unwrap();
    }
    let last = repo::last_attempt_at(&pool, "demo-synthetic")
        .await
        .unwrap();
    assert_eq!(last, Some(now));

    // Csak blokk-sorokkal rendelkező forrás: nincs valódi kísérlet → None.
    seed_policy(&pool, "csak-blokkolt").await;
    repo::insert_ledger(
        &pool,
        &ledger("csak-blokkolt", now, FetchOutcome::QuotaBlocked),
    )
    .await
    .unwrap();
    assert!(
        repo::last_attempt_at(&pool, "csak-blokkolt")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn claim_lease_and_requeue() {
    let pool = test_pool().await;
    seed_policy(&pool, "demo-synthetic").await;
    let now = Utc::now();
    repo::insert_job(
        &pool,
        &new_job("demo-synthetic", "day", now - Duration::minutes(1), 10),
    )
    .await
    .unwrap();

    let worker = Uuid::now_v7();
    let claimed = repo::claim_due_jobs(&pool, worker, now, 60, 10)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);
    // második claim ugyanarra a jobra nem ad vissza semmit (leased)
    assert!(
        repo::claim_due_jobs(&pool, worker, now, 60, 10)
            .await
            .unwrap()
            .is_empty()
    );
    // lejárt lease visszasorolása
    let later = now + Duration::seconds(120);
    assert_eq!(repo::requeue_expired_leases(&pool, later).await.unwrap(), 1);
    let reclaimed = repo::claim_due_jobs(&pool, Uuid::now_v7(), later, 60, 10)
        .await
        .unwrap();
    assert_eq!(reclaimed.len(), 1);
    assert_eq!(reclaimed[0].id, claimed[0].id);
}

#[tokio::test]
async fn job_dedup_priority_limit_and_attempts() {
    let pool = test_pool().await;
    seed_policy(&pool, "demo-synthetic").await;
    let now = Utc::now();
    let due = now - Duration::minutes(1);

    // duplikált generálás nem szúr be második sort (ON CONFLICT DO NOTHING)
    repo::insert_job(&pool, &new_job("demo-synthetic", "day", due, 10))
        .await
        .unwrap();
    repo::insert_job(&pool, &new_job("demo-synthetic", "day", due, 10))
        .await
        .unwrap();
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.job")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 1);

    // kisebb priorityszám előre kerül; a limit tiszteletben tartva
    repo::insert_job(&pool, &new_job("demo-synthetic", "urgent", due, 5))
        .await
        .unwrap();
    let claimed = repo::claim_due_jobs(&pool, Uuid::now_v7(), now, 60, 1)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].entity_key, "urgent");
    assert_eq!(claimed[0].priority, 5);

    // claim növeli az attempts számlálót
    let attempts: i32 = sqlx::query_scalar("SELECT attempts FROM ops.job WHERE id = $1")
        .bind(claimed[0].id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempts, 1);

    // jövőben esedékes job nem claimelhető
    repo::insert_job(
        &pool,
        &new_job("demo-synthetic", "future", now + Duration::hours(1), 1),
    )
    .await
    .unwrap();
    let rest = repo::claim_due_jobs(&pool, Uuid::now_v7(), now, 60, 10)
        .await
        .unwrap();
    assert_eq!(rest.len(), 1);
    assert_eq!(rest[0].entity_key, "day");
}

#[tokio::test]
async fn claim_returns_jobs_sorted_by_priority_and_due_at() {
    let pool = test_pool().await;
    seed_policy(&pool, "demo-synthetic").await;
    let now = Utc::now();
    let due = now - Duration::minutes(5);

    // beszúrási sorrend szándékosan nem egyezik a prioritás-sorrenddel:
    // az UPDATE ... RETURNING nyers sorrendje tetszőleges, a repo rendez
    repo::insert_job(&pool, &new_job("demo-synthetic", "p30", due, 30))
        .await
        .unwrap();
    repo::insert_job(&pool, &new_job("demo-synthetic", "p10", due, 10))
        .await
        .unwrap();
    repo::insert_job(&pool, &new_job("demo-synthetic", "p20", due, 20))
        .await
        .unwrap();
    // azonos prioritásnál a korábbi due_at kerül előre (due_at tie-break)
    repo::insert_job(
        &pool,
        &new_job("demo-synthetic", "tie-late", due + Duration::minutes(1), 15),
    )
    .await
    .unwrap();
    repo::insert_job(&pool, &new_job("demo-synthetic", "tie-early", due, 15))
        .await
        .unwrap();

    let claimed = repo::claim_due_jobs(&pool, Uuid::now_v7(), now, 60, 10)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 5);
    let priorities: Vec<i32> = claimed.iter().map(|j| j.priority).collect();
    assert_eq!(priorities, [10, 15, 15, 20, 30]);
    let keys: Vec<&str> = claimed.iter().map(|j| j.entity_key.as_str()).collect();
    assert_eq!(keys, ["p10", "tie-early", "tie-late", "p20", "p30"]);
}

#[tokio::test]
async fn insert_job_counted_returns_inserted_rows() {
    let pool = test_pool().await;
    seed_policy(&pool, "demo-synthetic").await;
    let due = Utc::now() - Duration::minutes(1);

    // első beszúrás: 1 új sor
    let job = new_job("demo-synthetic", "day", due, 10);
    assert_eq!(repo::insert_job_counted(&pool, &job).await.unwrap(), 1);

    // ugyanarra az egyedi kulcsra: 0 (ON CONFLICT DO NOTHING)
    assert_eq!(repo::insert_job_counted(&pool, &job).await.unwrap(), 0);
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.job")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 1);

    // másik entity_key: megint 1 új sor
    assert_eq!(
        repo::insert_job_counted(&pool, &new_job("demo-synthetic", "other", due, 10))
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn breaker_set_get_roundtrip() {
    let pool = test_pool().await;
    seed_policy(&pool, "demo-synthetic").await;

    assert!(
        repo::get_breaker(&pool, "demo-synthetic")
            .await
            .unwrap()
            .is_none()
    );

    let now = Utc::now();
    let open = repo::BreakerRow {
        state: BreakerState::Open,
        failure_count: 3,
        opened_at: Some(now),
        next_probe_at: Some(now + Duration::seconds(60)),
    };
    repo::set_breaker(&pool, "demo-synthetic", &open)
        .await
        .unwrap();
    let got = repo::get_breaker(&pool, "demo-synthetic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.state, BreakerState::Open);
    assert_eq!(got.failure_count, 3);
    assert_eq!(got.opened_at, Some(now));
    assert_eq!(got.next_probe_at, Some(now + Duration::seconds(60)));

    // upsert: zárás felülírja a nyitott állapotot, egy sor marad
    let closed = repo::BreakerRow {
        state: BreakerState::Closed,
        failure_count: 0,
        opened_at: None,
        next_probe_at: None,
    };
    repo::set_breaker(&pool, "demo-synthetic", &closed)
        .await
        .unwrap();
    let got = repo::get_breaker(&pool, "demo-synthetic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.state, BreakerState::Closed);
    assert_eq!(got.failure_count, 0);
    assert!(got.opened_at.is_none());
    assert!(got.next_probe_at.is_none());
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.breaker_state")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
}

#[tokio::test]
async fn coverage_upsert_updates_state_and_job() {
    let pool = test_pool().await;
    seed_policy(&pool, "demo-synthetic").await;
    let window = Utc::now().date_naive().to_string();

    repo::upsert_coverage(
        &pool,
        "demo-synthetic",
        "match",
        "schedule",
        &window,
        CoverageState::Pending,
        None,
    )
    .await
    .unwrap();
    let row = sqlx::query(
        "SELECT state, last_job_id FROM ops.coverage_slot
         WHERE source_slug='demo-synthetic' AND entity_type='match'
           AND data_kind='schedule' AND time_window=$1",
    )
    .bind(&window)
    .fetch_one(&pool)
    .await
    .unwrap();
    use sqlx::Row;
    assert_eq!(row.try_get::<String, _>("state").unwrap(), "pending");
    assert!(
        row.try_get::<Option<Uuid>, _>("last_job_id")
            .unwrap()
            .is_none()
    );

    // upsert ugyanarra a slotra: állapot és job frissül, nem duplikál
    let job_id = Uuid::now_v7();
    repo::upsert_coverage(
        &pool,
        "demo-synthetic",
        "match",
        "schedule",
        &window,
        CoverageState::Done,
        Some(job_id),
    )
    .await
    .unwrap();
    let row = sqlx::query(
        "SELECT state, last_job_id FROM ops.coverage_slot
         WHERE source_slug='demo-synthetic' AND entity_type='match'
           AND data_kind='schedule' AND time_window=$1",
    )
    .bind(&window)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.try_get::<String, _>("state").unwrap(), "done");
    assert_eq!(
        row.try_get::<Option<Uuid>, _>("last_job_id").unwrap(),
        Some(job_id)
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.coverage_slot")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
}

#[tokio::test]
async fn robots_cache_put_get_roundtrip() {
    let pool = test_pool().await;
    assert!(
        repo::get_robots_cache(&pool, "example.invalid")
            .await
            .unwrap()
            .is_none()
    );

    let now = Utc::now();
    let row = repo::RobotsCacheRow {
        host: "example.invalid".into(),
        fetched_at: now,
        verdict: "allowed".into(),
        rules_text: "User-agent: *\nDisallow: /admin".into(),
        expires_at: now + Duration::hours(1),
    };
    repo::put_robots_cache(&pool, &row).await.unwrap();
    let got = repo::get_robots_cache(&pool, "example.invalid")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.host, "example.invalid");
    assert_eq!(got.fetched_at, now);
    assert_eq!(got.verdict, "allowed");
    assert_eq!(got.rules_text, "User-agent: *\nDisallow: /admin");
    assert_eq!(got.expires_at, now + Duration::hours(1));

    // upsert: új verdict felülír, egy sor marad
    let mut updated = row.clone();
    updated.verdict = "blocked".into();
    repo::put_robots_cache(&pool, &updated).await.unwrap();
    let got = repo::get_robots_cache(&pool, "example.invalid")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.verdict, "blocked");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.robots_cache")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
}

#[tokio::test]
async fn complete_job_and_release_worker_leases() {
    let pool = test_pool().await;
    seed_policy(&pool, "demo-synthetic").await;
    let now = Utc::now();
    let due = now - Duration::minutes(1);
    repo::insert_job(&pool, &new_job("demo-synthetic", "elso", due, 10))
        .await
        .unwrap();
    repo::insert_job(&pool, &new_job("demo-synthetic", "masodik", due, 10))
        .await
        .unwrap();

    let worker = Uuid::now_v7();
    let claimed = repo::claim_due_jobs(&pool, worker, now, 60, 10)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 2);

    // sikeres befejezés: státusz done, lease törölve
    repo::complete_job(&pool, claimed[0].id, JobStatus::Done, None)
        .await
        .unwrap();
    // sikertelen befejezés: státusz failed, hibaüzenet tárolva
    repo::complete_job(
        &pool,
        claimed[1].id,
        JobStatus::Failed,
        Some("kapcsolati hiba"),
    )
    .await
    .unwrap();

    use sqlx::Row;
    let row = sqlx::query(
        "SELECT status, worker_id, leased_until, last_error FROM ops.job WHERE id = $1",
    )
    .bind(claimed[0].id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.try_get::<String, _>("status").unwrap(), "done");
    assert!(
        row.try_get::<Option<Uuid>, _>("worker_id")
            .unwrap()
            .is_none()
    );
    assert!(
        row.try_get::<Option<chrono::DateTime<Utc>>, _>("leased_until")
            .unwrap()
            .is_none()
    );
    assert!(
        row.try_get::<Option<String>, _>("last_error")
            .unwrap()
            .is_none()
    );

    let row = sqlx::query("SELECT status, last_error FROM ops.job WHERE id = $1")
        .bind(claimed[1].id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.try_get::<String, _>("status").unwrap(), "failed");
    assert_eq!(
        row.try_get::<Option<String>, _>("last_error")
            .unwrap()
            .as_deref(),
        Some("kapcsolati hiba")
    );

    // worker összeomlása: a még leased jobok visszasorolódnak, a befejezettek nem
    repo::insert_job(&pool, &new_job("demo-synthetic", "harmadik", due, 10))
        .await
        .unwrap();
    let held = repo::claim_due_jobs(&pool, worker, now, 60, 10)
        .await
        .unwrap();
    assert_eq!(held.len(), 1);
    assert_eq!(repo::release_worker_leases(&pool, worker).await.unwrap(), 1);
    let row = sqlx::query("SELECT status, worker_id, leased_until FROM ops.job WHERE id = $1")
        .bind(held[0].id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.try_get::<String, _>("status").unwrap(), "pending");
    assert!(
        row.try_get::<Option<Uuid>, _>("worker_id")
            .unwrap()
            .is_none()
    );
    assert!(
        row.try_get::<Option<chrono::DateTime<Utc>>, _>("leased_until")
            .unwrap()
            .is_none()
    );
    // idegen worker release-e nem nyúl a befejezett jobokhoz
    assert_eq!(
        repo::release_worker_leases(&pool, Uuid::now_v7())
            .await
            .unwrap(),
        0
    );
}

// --- Task 10: persist_raw / update_ledger_outcome / record_auth_failure /
// --- count_duplicate_matches repository tesztek.

fn stored_artifact(hash_char: char) -> football_storage::object_store::StoredArtifact {
    let sha256: String = std::iter::repeat_n(hash_char, 64).collect();
    football_storage::object_store::StoredArtifact {
        storage_key: format!("{}/{}/{}.json.zst", &sha256[0..2], &sha256[2..4], sha256),
        sha256,
        byte_size: 42,
        compression: "zstd",
    }
}

fn raw_capture(request_key: &str) -> repo::RawCapture<'_> {
    let now = Utc::now();
    repo::RawCapture {
        endpoint_key: "match",
        request_url: "fixture://demo-synthetic/2026-11-12",
        request_key,
        requested_at: now,
        received_at: now,
        http_status: 200,
        adapter_version: "1.0",
        parser_version: "1.0",
    }
}

#[tokio::test]
async fn persist_raw_upserts_source_artifact_capture_idempotently() {
    let pool = test_pool().await;
    let artifact = stored_artifact('a');
    let capture = raw_capture("demo-synthetic:match:2026-11-12");

    let (source_id, capture_id) =
        repo::persist_raw(&pool, "demo-synthetic", "Demó forrás", &artifact, &capture)
            .await
            .unwrap();

    // Ugyanaz a tartalom + request_key: nem keletkezik új artifact/capture.
    let (source_id2, capture_id2) = repo::persist_raw(
        &pool,
        "demo-synthetic",
        "Demó forrás v2",
        &artifact,
        &capture,
    )
    .await
    .unwrap();
    assert_eq!(source_id, source_id2);
    assert_eq!(capture_id, capture_id2);

    let (artifacts, captures, sources): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM raw.artifact),
                (SELECT count(*) FROM raw.capture),
                (SELECT count(*) FROM ops.source)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((artifacts, captures, sources), (1, 1, 1));

    // A display_name upsert frissült.
    let display_name: String =
        sqlx::query_scalar("SELECT display_name FROM ops.source WHERE id=$1")
            .bind(source_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(display_name, "Demó forrás v2");

    // Új request_key ugyanazzal az artifacttal: új capture, ugyanaz az artifact.
    let other = raw_capture("demo-synthetic:match:2026-11-13");
    let (_, capture_id3) =
        repo::persist_raw(&pool, "demo-synthetic", "Demó forrás v2", &artifact, &other)
            .await
            .unwrap();
    assert_ne!(capture_id, capture_id3);
    let (artifacts, captures): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM raw.artifact), (SELECT count(*) FROM raw.capture)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((artifacts, captures), (1, 2));
}

#[tokio::test]
async fn update_ledger_outcome_finalizes_placeholder() {
    use sqlx::Row;

    let pool = test_pool().await;
    seed_policy(&pool, "demo-synthetic").await;
    let now = Utc::now();
    let id = repo::insert_ledger(
        &pool,
        &ledger("demo-synthetic", now, FetchOutcome::TransportError),
    )
    .await
    .unwrap();

    repo::update_ledger_outcome(
        &pool,
        id,
        FetchOutcome::Success,
        Some(200),
        Some(1234),
        Some(56),
        Some("abc123"),
    )
    .await
    .unwrap();

    let row = sqlx::query(
        "SELECT outcome, http_status, byte_size, latency_ms, artifact_hash, attempted_at
         FROM ops.fetch_ledger WHERE id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.try_get::<String, _>("outcome").unwrap(), "success");
    assert_eq!(
        row.try_get::<Option<i16>, _>("http_status").unwrap(),
        Some(200)
    );
    assert_eq!(
        row.try_get::<Option<i64>, _>("byte_size").unwrap(),
        Some(1234)
    );
    assert_eq!(
        row.try_get::<Option<i64>, _>("latency_ms").unwrap(),
        Some(56)
    );
    assert_eq!(
        row.try_get::<Option<String>, _>("artifact_hash").unwrap(),
        Some("abc123".into())
    );
    // A reserve időpontja nem változik: a kvótaelszámolás a kísérlet kezdete.
    assert_eq!(
        row.try_get::<chrono::DateTime<Utc>, _>("attempted_at")
            .unwrap(),
        now
    );

    // Hiányzó mezők None-ra véglegesíthetők (pl. fetch-hiba esetén).
    repo::update_ledger_outcome(
        &pool,
        id,
        FetchOutcome::Timeout,
        None,
        None,
        Some(30_000),
        None,
    )
    .await
    .unwrap();
    let row =
        sqlx::query("SELECT outcome, http_status, artifact_hash FROM ops.fetch_ledger WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row.try_get::<String, _>("outcome").unwrap(), "timeout");
    assert!(
        row.try_get::<Option<i16>, _>("http_status")
            .unwrap()
            .is_none()
    );
    assert!(
        row.try_get::<Option<String>, _>("artifact_hash")
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn record_auth_failure_writes_breaker_and_audit_together() {
    let pool = test_pool().await;
    seed_policy(&pool, "demo-synthetic").await;
    let now = Utc::now();
    let open = repo::BreakerRow {
        state: BreakerState::Open,
        failure_count: 1,
        opened_at: Some(now),
        next_probe_at: Some(now + Duration::seconds(60)),
    };
    let detail = serde_json::json!({"slug": "demo-synthetic", "status": "auth_failure"});
    repo::record_auth_failure(&pool, "demo-synthetic", &open, &detail)
        .await
        .unwrap();

    let row = repo::get_breaker(&pool, "demo-synthetic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.state, BreakerState::Open);
    assert_eq!(row.failure_count, 1);
    assert_eq!(row.next_probe_at, Some(now + Duration::seconds(60)));

    let (actor, action, subject_kind, slug, status): (String, String, String, String, String) =
        sqlx::query_as(
            "SELECT actor, action, subject_kind, detail->>'slug', detail->>'status'
             FROM ops.audit_event",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(actor, "worker");
    assert_eq!(action, "policy_review");
    assert_eq!(subject_kind, "source");
    assert_eq!(slug, "demo-synthetic");
    assert_eq!(status, "auth_failure");
}

/// Minimális, FK-érvényes meccs-seed a duplikátum-számláló teszthez.
async fn seed_core_match(
    pool: &sqlx::PgPool,
    competition_id: Uuid,
    season_id: Uuid,
    home_id: Uuid,
    away_id: Uuid,
    kickoff: chrono::DateTime<Utc>,
    round_label: Option<&str>,
) -> Uuid {
    let match_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, home_team_id, away_team_id,
          kickoff_at, competition_timezone, status, round_label)
         VALUES ($1,$2,$3,$4,$5,$6,'UTC','scheduled',$7)",
    )
    .bind(match_id)
    .bind(competition_id)
    .bind(season_id)
    .bind(home_id)
    .bind(away_id)
    .bind(kickoff)
    .bind(round_label)
    .execute(pool)
    .await
    .unwrap();
    match_id
}

#[tokio::test]
async fn count_duplicate_matches_uses_natural_key() {
    let pool = test_pool().await;
    let now = Utc::now();
    let country_id = Uuid::now_v7();
    let competition_id = Uuid::now_v7();
    let season_id = Uuid::now_v7();
    let home_id = Uuid::now_v7();
    let away_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'XD','Duplikátum Ország')")
        .bind(country_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','Dup Liga')",
    )
    .bind(competition_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name, tier, priority)
         VALUES ($1,$1,$2,'Dup Liga',1,10)",
    )
    .bind(competition_id)
    .bind(country_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season_id)
        .bind(competition_id)
        .execute(&pool)
        .await
        .unwrap();
    for (team_id, name) in [(home_id, "Hazai FC"), (away_id, "Vendég FC")] {
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(team_id)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO core.team (id, entity_id, country_id, name, gender)
             VALUES ($1,$1,$2,$3,'unknown')",
        )
        .bind(team_id)
        .bind(country_id)
        .bind(name)
        .execute(&pool)
        .await
        .unwrap();
    }

    // Nem-NULL round_label esetén az uq_match_natural_key unique index nem
    // enged duplikátumot: a számláló 0. A NULL round_label-es soroknál a
    // unique index NULL-egyenlősége miatt lehet azonos természetes kulcs.
    let base = seed_core_match(
        &pool,
        competition_id,
        season_id,
        home_id,
        away_id,
        now,
        Some("1. forduló"),
    )
    .await;
    // Eltérő kickoff: nem duplikátum.
    seed_core_match(
        &pool,
        competition_id,
        season_id,
        home_id,
        away_id,
        now + Duration::hours(3),
        Some("1. forduló"),
    )
    .await;
    // NULL round_label mindkét oldalon: IS NOT DISTINCT FROM egyezés.
    let null_base = seed_core_match(
        &pool,
        competition_id,
        season_id,
        away_id,
        home_id,
        now,
        None,
    )
    .await;
    let null_dup = seed_core_match(
        &pool,
        competition_id,
        season_id,
        away_id,
        home_id,
        now,
        None,
    )
    .await;

    assert_eq!(
        repo::count_duplicate_matches(&pool, base, home_id, away_id, now)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        repo::count_duplicate_matches(&pool, null_base, away_id, home_id, now)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        repo::count_duplicate_matches(&pool, null_dup, away_id, home_id, now)
            .await
            .unwrap(),
        1
    );
}
