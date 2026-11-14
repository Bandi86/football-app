//! Worker loop tesztek (Task 11): a shutdown ágon a loop kilép, és kilépéskor
//! elengedi a saját worker lease-eit (release_worker_leases).
//!
//! Nincs élő hálózat: a fetch a FixtureFetcher-en megy, a golden fixture
//! dátumait a helper a mai UTC napra írja át (a loop belül Utc::now()-t használ).
//! A tick-intervallus szándékosan hosszú (60 mp): az első tick azonnal lefut
//! (tokio interval semantics), a következőre a teszt időablakában nem kerül sor,
//! így a kézzel lease-elt második jobot csak a shutdown-release adhatja vissza.

use std::collections::HashMap;
use std::time::Duration;

use chrono::Utc;
use football_domain::policy::{EventTemplate, SourceMode, SourcePolicy, Transport};
use football_ingest::adapter::SourceAdapter;
use football_ingest::demo::DemoAdapter;
use football_ingest::fetch::{Fetchers, FixtureFetcher};
use football_ingest::http::RustHttpFetcher;
use football_storage::object_store::ObjectStore;
use football_storage::repositories::worker as repo;
use football_storage::test_support::test_pool;
use football_worker::{runner::run_loop, scheduler};
use sqlx::PgPool;
use uuid::Uuid;

/// A tesztek fixture policy-i a fixture fetcheren mennek (nincs hálózat).
fn as_fetchers(fixture: FixtureFetcher) -> Fetchers {
    Fetchers::new(
        RustHttpFetcher::new("football-app-test").expect("rust_http fetcher"),
        fixture,
    )
}

fn policy() -> SourcePolicy {
    SourcePolicy {
        source_slug: "demo-synthetic".into(),
        transport: Transport::Fixture,
        mode: SourceMode::Shadow,
        daily_budget: 100,
        min_delay_ms: 0,
        jitter_ms: 0,
        breaker_threshold: 3,
        breaker_cooldown_secs: 60,
        adapter_version: "1.0".into(),
        parser_version: "1.0".into(),
        domains: vec!["match".into()],
        events: vec![EventTemplate {
            event_type: "discovery".into(),
            offset_minutes: None,
            interval_minutes: Some(1440),
        }],
        enabled: true,
        max_bytes: 5_242_880,
    }
}

/// A golden fixture másolata a mai UTC dátummal (DST-álló 16:00Z kickoff),
/// FixtureFetcher a DemoAdapter plan-kulcsához.
fn fixture_fetcher(dir: &std::path::Path) -> FixtureFetcher {
    let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/worker/demo_day.json");
    let text = std::fs::read_to_string(&source).expect("golden fixture olvasása");
    let today = Utc::now().date_naive().to_string();
    let rewritten = text
        .replace("2026-11-12", &today)
        .replace("T22:30:00Z", "T16:00:00Z");
    let path = dir.join("demo_day.json");
    std::fs::write(&path, rewritten).expect("fixture másolat írása");
    let request_key = format!("demo-synthetic:match:{today}");
    FixtureFetcher::new(HashMap::from([(request_key, path)]))
}

fn adapters() -> HashMap<String, Box<dyn SourceAdapter>> {
    HashMap::from([(
        "demo-synthetic".to_string(),
        Box::new(DemoAdapter) as Box<dyn SourceAdapter>,
    )])
}

async fn seed_and_generate(pool: &PgPool) {
    let policy = policy();
    repo::upsert_policy(pool, &policy).await.unwrap();
    let inserted = scheduler::generate_jobs(pool, &[policy], Utc::now())
        .await
        .unwrap();
    assert_eq!(inserted, 1);
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn loop_stops_on_shutdown_and_releases_leases() {
    let pool = test_pool().await;
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    let fetcher = fixture_fetcher(fetch_dir.path());
    seed_and_generate(&pool).await;
    let worker_id = Uuid::now_v7();

    let (tx, rx) = tokio::sync::watch::channel(false);
    let handle = tokio::spawn(run_loop(
        pool.clone(),
        store,
        as_fetchers(fetcher),
        adapters(),
        worker_id,
        Duration::from_secs(60),
        rx,
    ));

    // Az első tick azonnal lefut: polloljuk, amíg a job done (vagy failed) nem
    // lesz, legfeljebb ~5 mp-ig — fix sleep helyett, hogy terhelt DB-n se legyen
    // flaky a teszt.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let job_status: String = loop {
        let status: String = sqlx::query_scalar("SELECT status FROM ops.job")
            .fetch_one(&pool)
            .await
            .unwrap();
        if status == "done" || status == "failed" {
            break status;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("az első tick nem fejeződött be 5 mp-en belül, job státusz: {status}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert_eq!(job_status, "done");
    assert!(count(&pool, "SELECT count(*) FROM stage.observation").await > 0);

    // Második job kézi lease-elése ugyanazzal a worker_id-val, hosszú lease-szel:
    // a 60 mp-es intervallum miatt a loop ezt már nem dolgozza fel, a shutdown
    // utáni release-nek kell pending-re visszaadnia.
    repo::insert_job(
        &pool,
        &repo::NewJob {
            source_slug: "demo-synthetic".into(),
            domain: "match".into(),
            entity_key: "extra".into(),
            event_type: "discovery".into(),
            time_window: Utc::now().date_naive().to_string(),
            due_at: Utc::now(),
            priority: 10,
        },
    )
    .await
    .unwrap();
    let claimed = repo::claim_due_jobs(&pool, worker_id, Utc::now(), 3600, 10)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);

    tx.send(true).unwrap();
    // A loop a shutdown ágon azonnal kilép, nem várja meg a 60 mp-es ticket.
    let joined = tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .expect("a loop nem állt le időben")
        .unwrap();
    joined.unwrap();

    let leased = count(&pool, "SELECT count(*) FROM ops.job WHERE status='leased'").await;
    assert_eq!(leased, 0);
    let (status, holder): (String, Option<Uuid>) =
        sqlx::query_as("SELECT status, worker_id FROM ops.job WHERE entity_key='extra'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "pending");
    assert_eq!(holder, None);
}

#[tokio::test]
async fn loop_exits_without_tick_when_shutdown_preset() {
    let pool = test_pool().await;
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    let fetcher = fixture_fetcher(fetch_dir.path());
    seed_and_generate(&pool).await;

    let (tx, rx) = tokio::sync::watch::channel(false);
    tx.send(true).unwrap();
    run_loop(
        pool.clone(),
        store,
        as_fetchers(fetcher),
        adapters(),
        Uuid::now_v7(),
        Duration::from_millis(50),
        rx,
    )
    .await
    .unwrap();

    // Tick nem futott: a job pending, observation nincs.
    let job_status: String = sqlx::query_scalar("SELECT status FROM ops.job")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(job_status, "pending");
    assert_eq!(
        count(&pool, "SELECT count(*) FROM stage.observation").await,
        0
    );
}
