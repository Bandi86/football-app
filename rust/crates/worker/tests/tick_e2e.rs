//! Tick E2E tesztek (Task 10): verified publish, shadow, decoy karantén.
//!
//! Nincs élő hálózat: minden fetch a FixtureFetcher-en megy. A Task 6 golden
//! fixture rögzített 2026-11-12-i dátumát a helper a mai UTC napra írja át,
//! hogy a DemoAdapter dátum-guardja zöld legyen. A második meccs 22:30Z
//! kickoffját 16:00Z-ra visszük: nyári időszámításnál (UTC+2) a 22:30Z
//! Budapesten már a következő nap 00:30-a lenne, és a dátum-guard bukna.
//! A tick `now` paramétere a mai nap 12:00 UTC-je: determinisztikus, és a
//! Budapestre átszámított publish-dátum is a mai nap marad. A verified_publish
//! E2E ághoz megerősítő második forrás is kell (TRUST_LEVEL szabály).

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Duration, TimeZone, Utc};
use football_domain::policy::{EventTemplate, FetchOutcome, SourceMode, SourcePolicy, Transport};
use football_ingest::adapter::SourceAdapter;
use football_ingest::demo::DemoAdapter;
use football_ingest::fetch::FixtureFetcher;
use football_ingest::fixture::FixtureSource;
use football_ingest::observe::insert_observation;
use football_storage::object_store::ObjectStore;
use football_storage::repositories::day::{DayFilter, fetch_day};
use football_storage::repositories::worker as repo;
use football_storage::test_support::test_pool;
use football_worker::{quota, scheduler, tick};
use sqlx::PgPool;
use uuid::Uuid;

/// Mai nap 12:00 UTC — a fixture dátum-guard és a publish-dátum is erre a napra esik.
fn today_noon() -> DateTime<Utc> {
    let day = Utc::now().date_naive();
    Utc.from_utc_datetime(&day.and_hms_opt(12, 0, 0).expect("12:00 érvényes idő"))
}

fn policy(mode: SourceMode) -> SourcePolicy {
    SourcePolicy {
        source_slug: "demo-synthetic".into(),
        transport: Transport::Fixture,
        mode,
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

/// A golden fixture másolata a mai UTC dátummal (DST-álló kickoffokkal);
/// visszaadja az átírt fájl útvonalát.
fn write_rewritten_fixture(
    fixture_name: &str,
    now: DateTime<Utc>,
    dir: &std::path::Path,
) -> PathBuf {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/worker")
        .join(fixture_name);
    let text = std::fs::read_to_string(&source).expect("golden fixture olvasása");
    let today = now.date_naive().to_string();
    let rewritten = text
        .replace("2026-11-12", &today)
        .replace("T22:30:00Z", "T16:00:00Z");
    let path = dir.join(fixture_name);
    std::fs::write(&path, rewritten).expect("fixture másolat írása");
    path
}

/// A golden fixture másolata a mai UTC dátummal; visszaadja a FixtureFetcher
/// mapet a DemoAdapter plan-kulcsához ("demo-synthetic:match:<mai nap>").
fn fixture_fetcher(
    fixture_name: &str,
    now: DateTime<Utc>,
    dir: &std::path::Path,
) -> FixtureFetcher {
    let path = write_rewritten_fixture(fixture_name, now, dir);
    let request_key = format!("demo-synthetic:match:{}", now.date_naive());
    FixtureFetcher::new(HashMap::from([(request_key, path)]))
}

fn adapters() -> HashMap<String, Box<dyn SourceAdapter>> {
    HashMap::from([(
        "demo-synthetic".to_string(),
        Box::new(DemoAdapter) as Box<dyn SourceAdapter>,
    )])
}

async fn seed_and_generate(pool: &PgPool, mode: SourceMode, now: DateTime<Utc>) {
    let policy = policy(mode);
    repo::upsert_policy(pool, &policy).await.unwrap();
    let inserted = scheduler::generate_jobs(pool, &[policy], now)
        .await
        .unwrap();
    assert_eq!(inserted, 1);
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

/// Megerősítő második forrás: a TRUST_LEVEL szabály miatt egyetlen forrásból
/// csak provisional_publish születik; a verified_publish-hez egyező második
/// forrás-megfigyelés kell (a dev demó pipeline is két fixture forrással fut).
/// Az observationök anyagi mezői megegyeznek a DemoAdapter kimenetével, így
/// a MATERIAL_SOURCE_CONFLICT szabály sem talál eltérést.
async fn seed_corroborating_source(
    pool: &PgPool,
    store: &ObjectStore,
    fixture: &FixtureSource,
    now: DateTime<Utc>,
) {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "source": "demo-corroborating", "purpose": "megerősítő forrás"
    }))
    .unwrap();
    let artifact = store.put_json(&bytes).unwrap();
    let (source_id, capture_id) = repo::persist_raw(
        pool,
        "demo-corroborating",
        "Demó megerősítő forrás",
        &artifact,
        &repo::RawCapture {
            endpoint_key: "match",
            request_url: "fixture://demo-corroborating/meccs",
            request_key: "demo-corroborating:match:meccs",
            requested_at: now,
            received_at: now,
            http_status: 200,
            adapter_version: "1.0",
            parser_version: "1.0",
        },
    )
    .await
    .unwrap();
    for m in &fixture.matches {
        let home = fixture
            .teams
            .iter()
            .find(|t| t.source_entity_id == m.home_source_entity_id)
            .unwrap();
        let away = fixture
            .teams
            .iter()
            .find(|t| t.source_entity_id == m.away_source_entity_id)
            .unwrap();
        let payload = serde_json::json!({
            "canonical_match_id": m.id, "home_team_id": home.id, "away_team_id": away.id,
            "source_home_id": m.home_source_entity_id, "source_away_id": m.away_source_entity_id,
            "kickoff_at": m.kickoff_at, "status": m.status, "result": m.result,
            "round_label": m.round_label, "competition_timezone": m.competition_timezone,
        });
        insert_observation(
            pool,
            source_id,
            capture_id,
            "match",
            "match_core",
            &m.id.to_string(),
            &m.source_match_id,
            fixture.captured_at,
            payload,
        )
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn e2e_replay_publishes_verified_match() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    let fetcher = fixture_fetcher("demo_day.json", now, fetch_dir.path());
    seed_and_generate(&pool, SourceMode::Live, now).await;
    // A verified_publish-hez kell a megerősítő második forrás is.
    let rewritten = std::fs::read_to_string(fetch_dir.path().join("demo_day.json")).unwrap();
    let fixture: FixtureSource = serde_json::from_str(&rewritten).unwrap();
    seed_corroborating_source(&pool, &store, &fixture, now).await;

    let outcome = tick::tick(&pool, &store, &fetcher, &adapters(), Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(outcome.processed, 1);
    assert_eq!(outcome.blocked, 0);
    assert_eq!(outcome.failed, 0);

    let gates = count(
        &pool,
        "SELECT count(*) FROM ops.publish_gate WHERE state='verified_publish'",
    )
    .await;
    assert!(gates >= 1);

    // /v1/day szintű ellenőrzés: fetch_day a mai napra ad sort.
    let rows = fetch_day(
        &pool,
        &DayFilter {
            date: now.date_naive(),
            timezone: "Europe/Budapest".into(),
            include_all: false,
        },
    )
    .await
    .unwrap();
    assert!(!rows.is_empty());

    // Reserve → run → log: a ledger-placeholder véglegesítve, hash-szel.
    let (ledger_outcome, artifact_hash): (String, Option<String>) =
        sqlx::query_as("SELECT outcome, artifact_hash FROM ops.fetch_ledger")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(ledger_outcome, "success");
    assert!(artifact_hash.is_some());

    let job_status: String = sqlx::query_scalar("SELECT status FROM ops.job")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(job_status, "done");
    let coverage: String = sqlx::query_scalar("SELECT state FROM ops.coverage_slot")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(coverage, "done");
    // 2 artifact (demo + megerősítő), 6 observation (2 csapat + 2 meccs a
    // demo forrásból, 2 meccs a megerősítőből).
    assert_eq!(count(&pool, "SELECT count(*) FROM raw.artifact").await, 2);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM stage.observation").await,
        6
    );
}

#[tokio::test]
async fn shadow_mode_writes_no_canonical() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    let fetcher = fixture_fetcher("demo_day.json", now, fetch_dir.path());
    seed_and_generate(&pool, SourceMode::Shadow, now).await;

    let outcome = tick::tick(&pool, &store, &fetcher, &adapters(), Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(outcome.processed, 1);

    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 0);
    assert!(count(&pool, "SELECT count(*) FROM stage.observation").await > 0);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM ops.publish_gate").await,
        0
    );
    let coverage: String = sqlx::query_scalar("SELECT state FROM ops.coverage_slot")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(coverage, "done");
    let job_status: String = sqlx::query_scalar("SELECT status FROM ops.job")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(job_status, "done");
    let ledger_outcome: String = sqlx::query_scalar("SELECT outcome FROM ops.fetch_ledger")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ledger_outcome, "success");
}

#[tokio::test]
async fn decoy_payload_quarantined_raw_kept() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    let fetcher = fixture_fetcher("demo_day_decoy.json", now, fetch_dir.path());
    seed_and_generate(&pool, SourceMode::Live, now).await;

    let outcome = tick::tick(&pool, &store, &fetcher, &adapters(), Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(outcome.failed, 1);
    assert_eq!(outcome.processed, 0);

    let coverage: String = sqlx::query_scalar("SELECT state FROM ops.coverage_slot")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(coverage, "quarantined");
    // A nyers megmarad: artifact + capture, observation nélkül.
    assert_eq!(count(&pool, "SELECT count(*) FROM raw.artifact").await, 1);
    assert_eq!(count(&pool, "SELECT count(*) FROM raw.capture").await, 1);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM stage.observation").await,
        0
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 0);
    let ledger_outcome: String = sqlx::query_scalar("SELECT outcome FROM ops.fetch_ledger")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ledger_outcome, "parse_error");
    let job_status: String = sqlx::query_scalar("SELECT status FROM ops.job")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(job_status, "failed");
}

// --- Task 10 review-javítások regressziós tesztek ---

/// DelayBlocked livelock regresszió: a quota_blocked ledger-sorok nem
/// tolhatják ki a min_delay óráját. Valódi kísérlet T-ben, blokk-sorok
/// T+10/20/30 mp-ben (tick-intervallus < min_delay): az óra T-nél marad,
/// és T+min_delay+1 ms-nél a guard Allowed.
#[tokio::test]
async fn delay_blocked_rows_do_not_reset_min_delay_clock() {
    let pool = test_pool().await;
    let mut policy = policy(SourceMode::Shadow);
    policy.min_delay_ms = 60_000;
    repo::upsert_policy(&pool, &policy).await.unwrap();
    let t = today_noon();

    let entry = |at: DateTime<Utc>, outcome: FetchOutcome| repo::LedgerEntry {
        source_slug: "demo-synthetic".into(),
        request_key: "k".into(),
        attempted_at: at,
        outcome,
        http_status: None,
        byte_size: None,
        latency_ms: None,
        artifact_hash: None,
        job_id: None,
    };
    repo::insert_ledger(&pool, &entry(t, FetchOutcome::Success))
        .await
        .unwrap();
    for secs in [10, 20, 30] {
        repo::insert_ledger(
            &pool,
            &entry(t + Duration::seconds(secs), FetchOutcome::QuotaBlocked),
        )
        .await
        .unwrap();
    }

    // Az utolsó kísérlet időpontja a valódi fetch marad, nem a blokk-soroké.
    let last = repo::last_attempt_at(&pool, "demo-synthetic")
        .await
        .unwrap();
    assert_eq!(last, Some(t));

    // T+30 mp: a min_delay (60 mp) még nem telt le → DelayBlocked.
    let decision = quota::guard(&pool, &policy, t + Duration::seconds(30))
        .await
        .unwrap();
    assert_eq!(decision, quota::QuotaDecision::DelayBlocked);
    // T+60 mp + 1 ms: Allowed — a blokk-sorok nem resetelték az órát.
    let decision = quota::guard(
        &pool,
        &policy,
        t + Duration::seconds(60) + Duration::milliseconds(1),
    )
    .await
    .unwrap();
    assert_eq!(decision, quota::QuotaDecision::Allowed);
}

/// Megszemélyesítés-védelem: a payload source_code-ja eltér a job slugjától
/// ("other-source" a "demo-synthetic" job alatt) → a capture/observationök
/// a demo-synthetic forrás alá kerülnek, other-source ops.source sor NEM
/// keletkezik, és a megjelenítési név a slug alapértelmezése.
#[tokio::test]
async fn payload_source_code_cannot_impersonate_another_source() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    let fetcher = fixture_fetcher("demo_day.json", now, fetch_dir.path());

    // A payload forráskódjának hamisítása (a FixtureFetcher fetch-időben olvas).
    let path = fetch_dir.path().join("demo_day.json");
    let text = std::fs::read_to_string(&path).unwrap();
    let tampered = text
        .replace(
            "\"source_code\": \"demo-synthetic\"",
            "\"source_code\": \"other-source\"",
        )
        .replace(
            "\"source_display_name\": \"Demó szintetikus forrás\"",
            "\"source_display_name\": \"Idegen forrás\"",
        );
    assert_ne!(tampered, text, "a fixture hamisítása nem történt meg");
    std::fs::write(&path, tampered).unwrap();
    seed_and_generate(&pool, SourceMode::Shadow, now).await;

    let outcome = tick::tick(&pool, &store, &fetcher, &adapters(), Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(outcome.processed, 1);

    // Egyetlen ops.source sor: a job hiteles slugja; other-source nem jött létre.
    assert_eq!(count(&pool, "SELECT count(*) FROM ops.source").await, 1);
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM ops.source WHERE code='other-source'"
        )
        .await,
        0
    );
    let (code, display_name): (String, String) =
        sqlx::query_as("SELECT code, display_name FROM ops.source")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(code, "demo-synthetic");
    // Eltérő payload-kód esetén a slug a megjelenítési név (nem az idegen név).
    assert_eq!(display_name, "demo-synthetic");

    // Capture + observationök a demo-synthetic forrás alatt.
    let captures: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM raw.capture c
         JOIN ops.source s ON s.id = c.source_id WHERE s.code = 'demo-synthetic'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(captures, 1);
    let observations: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM stage.observation o
         JOIN ops.source s ON s.id = o.source_id WHERE s.code = 'demo-synthetic'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(observations > 0);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM stage.observation").await,
        observations
    );
}

/// Váratlan infrastruktúra-hiba sikeres fetch után (nem napi időablak):
/// a coverage Failed-re áll, a job failed-re zárul, a ledger-placeholder
/// transport_error marad (dokumentált, jogos kísérlet), a nyers megmarad.
#[tokio::test]
async fn unexpected_infra_error_marks_coverage_failed() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    // Heti (ISO-hét) időablakú job: a fetch sikeres, de a napi dátum parse
    // a persist_raw után bukik el — váratlan hiba a fetch-kapu után.
    let path = write_rewritten_fixture("demo_day.json", now, fetch_dir.path());
    let fetcher = FixtureFetcher::new(HashMap::from([(
        "demo-synthetic:match:2026-W46".to_string(),
        path,
    )]));
    repo::upsert_policy(&pool, &policy(SourceMode::Shadow))
        .await
        .unwrap();
    repo::insert_job(
        &pool,
        &repo::NewJob {
            source_slug: "demo-synthetic".into(),
            domain: "match".into(),
            entity_key: "week".into(),
            event_type: "discovery".into(),
            time_window: "2026-W46".into(),
            due_at: now - Duration::minutes(1),
            priority: 10,
        },
    )
    .await
    .unwrap();

    let outcome = tick::tick(&pool, &store, &fetcher, &adapters(), Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(outcome.failed, 1);
    assert_eq!(outcome.processed, 0);

    let coverage: String = sqlx::query_scalar("SELECT state FROM ops.coverage_slot")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(coverage, "failed");
    let job_status: String = sqlx::query_scalar("SELECT status FROM ops.job")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(job_status, "failed");
    // A placeholder nem véglegesíthető success-re: transport_error marad.
    let ledger_outcome: String = sqlx::query_scalar("SELECT outcome FROM ops.fetch_ledger")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ledger_outcome, "transport_error");
    // A fetch és az L0 megőrzés sikeres volt: a nyers megmaradt.
    assert_eq!(count(&pool, "SELECT count(*) FROM raw.artifact").await, 1);
    assert_eq!(count(&pool, "SELECT count(*) FROM raw.capture").await, 1);
}
