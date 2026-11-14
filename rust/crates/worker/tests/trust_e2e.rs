//! S2b-2 Task 5 — bizalmi gate E2E tesztek.
//!
//! Öt eset a tick live/shadow ágán (nincs élő hálózat, FixtureFetcher):
//! 1. két medium forrás (NULL provider) → 2/2 verified_publish + verified odds,
//! 2. egyetlen medium forrás → provisional_publish + TRUST_LEVEL warning
//!    (a warning az ops.validation_finding sorokban él; a gate blockers
//!    jsonb-je definíció szerint csak blocker súlyú leletet tartalmaz),
//! 3. high forrás egymagában → verified_publish,
//! 4. két forrás eltérő oddsokkal → a meccs verified_publish, az odds
//!    trust_state provisional (az odds-eltérés NEM Finding),
//! 5. shadow ág → nincs kanonikus meccs, nincs odds trust_state frissítés.
//!
//! A golden worker fixture dátumát a helper a mai UTC napra írja át (az S2a
//! tick_e2e mintáján), és odds mezőkkel egészíti ki. A második forrás a
//! FixtureAdapter + Canonicalizer úton érkezik (ugyanazok a kanonikus
//! azonosítók és anyagi mezők, eltérő forráskód).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use football_domain::policy::{EventTemplate, SourceMode, SourcePolicy, Transport};
use football_domain::trust::TrustClass;
use football_ingest::adapter::SourceAdapter;
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::demo::DemoAdapter;
use football_ingest::fetch::{Fetchers, FixtureFetcher};
use football_ingest::fixture::{FixtureAdapter, FixtureOdds, FixtureSource};
use football_ingest::http::RustHttpFetcher;
use football_storage::object_store::ObjectStore;
use football_storage::repositories::trust as trust_repo;
use football_storage::repositories::worker as repo;
use football_storage::test_support::test_pool;
use football_worker::{scheduler, tick};
use sqlx::PgPool;
use uuid::Uuid;

/// A tesztek fixture policy-i a fixture fetcheren mennek (nincs hálózat).
fn as_fetchers(fixture: FixtureFetcher) -> Fetchers {
    Fetchers::new(
        RustHttpFetcher::new("football-app-test").expect("rust_http fetcher"),
        fixture,
    )
}

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

fn adapters() -> HashMap<String, Box<dyn SourceAdapter>> {
    HashMap::from([(
        "demo-synthetic".to_string(),
        Box::new(DemoAdapter) as Box<dyn SourceAdapter>,
    )])
}

/// Egyszerű 1X2/BTTS/O-U odds készlet a tesztekhez.
fn demo_odds(home: f64) -> FixtureOdds {
    FixtureOdds {
        home,
        draw: 3.2,
        away: 3.5,
        btts_yes: 1.8,
        btts_no: 1.95,
        over25: 1.9,
        under25: 1.85,
    }
}

/// A golden worker fixture a mai UTC dátummal (DST-álló kickoffokkal),
/// mindkét meccsre azonos odds mezőkkel kiegészítve.
fn base_fixture(now: DateTime<Utc>) -> FixtureSource {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/worker/demo_day.json");
    let text = std::fs::read_to_string(&source).expect("golden fixture olvasása");
    let today = now.date_naive().to_string();
    let rewritten = text
        .replace("2026-11-12", &today)
        .replace("T22:30:00Z", "T16:00:00Z");
    let mut fixture: FixtureSource =
        serde_json::from_str(&rewritten).expect("golden fixture értelmezése");
    for m in &mut fixture.matches {
        m.odds = Some(demo_odds(2.0));
        m.market_count = Some(7);
    }
    fixture
}

/// Megerősítő második forrás fixture-e: ugyanazok a kanonikus azonosítók és
/// anyagi mezők, eltérő forráskód (a TRUST_LEVEL két független mediumot lát).
fn second_source_fixture(base: &FixtureSource) -> FixtureSource {
    let mut fixture = base.clone();
    fixture.source_code = "demo-corroborating".into();
    fixture.source_display_name = "Demó megerősítő forrás".into();
    fixture.request_url = format!(
        "fixture://demo-corroborating/{}",
        base.captured_at.date_naive()
    );
    fixture
}

fn write_fixture(dir: &Path, name: &str, fixture: &FixtureSource) -> PathBuf {
    let path = dir.join(name);
    let bytes = serde_json::to_vec_pretty(fixture).expect("fixture sorosítás");
    std::fs::write(&path, bytes).expect("fixture másolat írása");
    path
}

/// FixtureFetcher a DemoAdapter plan-kulcsához ("demo-synthetic:match:<mai nap>").
fn fetcher_for(path: PathBuf, now: DateTime<Utc>) -> FixtureFetcher {
    let request_key = format!("demo-synthetic:match:{}", now.date_naive());
    FixtureFetcher::new(HashMap::from([(request_key, path)]))
}

/// Forrás upsert (a trust seedhez a forrásnak már a tick előtt léteznie kell).
async fn seed_source(pool: &PgPool, code: &str, display_name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO ops.source (id, code, display_name)
         VALUES ($1,$2,$3)
         ON CONFLICT (code) DO UPDATE SET display_name = EXCLUDED.display_name
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(code)
    .bind(display_name)
    .fetch_one(pool)
    .await
    .expect("forrás seed")
}

/// Trust seed mindhárom demó domainre (fixture/kickoff, result, odds).
async fn seed_trust(pool: &PgPool, source_id: Uuid, class: TrustClass, provider: Option<&str>) {
    for domain in ["fixture/kickoff", "result", "odds"] {
        trust_repo::upsert_trust(pool, source_id, domain, class, provider)
            .await
            .expect("trust seed");
    }
}

/// A második forrás teljes ingest + kanonikus materializálás útja
/// (a footballctl IngestFixture mintáján): observationök + odds sorok.
async fn ingest_second_source(
    pool: &PgPool,
    store: &ObjectStore,
    dir: &Path,
    fixture: &FixtureSource,
) {
    let path = write_fixture(dir, "demo_corroborating.json", fixture);
    FixtureAdapter::ingest(pool, store, &path)
        .await
        .expect("második forrás ingest");
    Canonicalizer::materialize(
        pool,
        &football_ingest::normalized::NormalizedDay::from(fixture),
    )
    .await
    .expect("második forrás materializálás");
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

#[tokio::test]
async fn demo_two_medium_sources_verified_with_verified_odds() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    let fixture = base_fixture(now);
    let path = write_fixture(fetch_dir.path(), "demo_day.json", &fixture);
    let fetchers = as_fetchers(fetcher_for(path, now));
    seed_and_generate(&pool, SourceMode::Live, now).await;

    // Két medium forrás, NULL provider → 2 független csoport (spec §3.3).
    let src_a = seed_source(&pool, "demo-synthetic", "Demó szintetikus forrás").await;
    seed_trust(&pool, src_a, TrustClass::Medium, None).await;
    let second = second_source_fixture(&fixture);
    ingest_second_source(&pool, &store, fetch_dir.path(), &second).await;
    let src_b = seed_source(&pool, "demo-corroborating", "Demó megerősítő forrás").await;
    seed_trust(&pool, src_b, TrustClass::Medium, None).await;

    let outcome = tick::tick(&pool, &store, &fetchers, &adapters(), Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(outcome.processed, 1);
    assert_eq!(outcome.failed, 0);

    // 2/2 verified_publish, a frissített rule_set_versionnel.
    assert_eq!(
        count(&pool, "SELECT count(*) FROM ops.publish_gate").await,
        2
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM ops.publish_gate WHERE state='verified_publish'"
        )
        .await,
        2
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM ops.publish_gate WHERE rule_set_version='verification-1.1'"
        )
        .await,
        2
    );

    // Mindkét meccs mindkét forrású odds sora verified (2 meccs × 2 forrás).
    assert_eq!(
        count(&pool, "SELECT count(*) FROM fact.odds_snapshot").await,
        4
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM fact.odds_snapshot WHERE trust_state='verified'"
        )
        .await,
        4
    );
}

#[tokio::test]
async fn single_medium_source_provisional() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    let fixture = base_fixture(now);
    let path = write_fixture(fetch_dir.path(), "demo_day.json", &fixture);
    let fetchers = as_fetchers(fetcher_for(path, now));
    seed_and_generate(&pool, SourceMode::Live, now).await;
    let src = seed_source(&pool, "demo-synthetic", "Demó szintetikus forrás").await;
    seed_trust(&pool, src, TrustClass::Medium, None).await;

    let outcome = tick::tick(&pool, &store, &fetchers, &adapters(), Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(outcome.processed, 1);

    // Egyetlen medium csoport → warning → provisional_publish mindkét meccsre.
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM ops.publish_gate WHERE state='provisional_publish'"
        )
        .await,
        2
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM ops.publish_gate WHERE state='verified_publish'"
        )
        .await,
        0
    );

    // TRUST_LEVEL warning mindkét anyagi domainre, meccsenként (2×2 = 4 sor).
    // A warning a validation_findingben él; a gate blockers jsonb csak blockert tart.
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM ops.validation_finding
             WHERE rule_key='TRUST_LEVEL' AND severity='warning'"
        )
        .await,
        4
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(DISTINCT field_key) FROM ops.validation_finding
             WHERE rule_key='TRUST_LEVEL'"
        )
        .await,
        2
    );

    // Egy forrású odds → provisional (nincs független megerősítés).
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM fact.odds_snapshot WHERE trust_state='provisional'"
        )
        .await,
        2
    );
}

#[tokio::test]
async fn high_source_alone_verified() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    let fixture = base_fixture(now);
    let path = write_fixture(fetch_dir.path(), "demo_day.json", &fixture);
    let fetchers = as_fetchers(fetcher_for(path, now));
    seed_and_generate(&pool, SourceMode::Live, now).await;
    // High trust a result + fixture/kickoff domaineken → egymagában verified.
    let src = seed_source(&pool, "demo-synthetic", "Demó szintetikus forrás").await;
    seed_trust(&pool, src, TrustClass::High, None).await;

    let outcome = tick::tick(&pool, &store, &fetchers, &adapters(), Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(outcome.processed, 1);

    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM ops.publish_gate WHERE state='verified_publish'"
        )
        .await,
        2
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM ops.validation_finding WHERE rule_key='TRUST_LEVEL'"
        )
        .await,
        0
    );
    // Az odds egymagában továbbra is provisional (1 provider-csoport).
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM fact.odds_snapshot WHERE trust_state='provisional'"
        )
        .await,
        2
    );
}

#[tokio::test]
async fn differing_odds_provisional_but_match_verified() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    let fixture = base_fixture(now);
    let path = write_fixture(fetch_dir.path(), "demo_day.json", &fixture);
    let fetchers = as_fetchers(fetcher_for(path, now));
    seed_and_generate(&pool, SourceMode::Live, now).await;

    let src_a = seed_source(&pool, "demo-synthetic", "Demó szintetikus forrás").await;
    seed_trust(&pool, src_a, TrustClass::Medium, None).await;
    // A második forrás anyagi mezői egyeznek, de az odds hazai árazása eltér.
    let mut second = second_source_fixture(&fixture);
    for m in &mut second.matches {
        m.odds = Some(demo_odds(2.6));
    }
    ingest_second_source(&pool, &store, fetch_dir.path(), &second).await;
    let src_b = seed_source(&pool, "demo-corroborating", "Demó megerősítő forrás").await;
    seed_trust(&pool, src_b, TrustClass::Medium, None).await;
    // Mesterségesen verified odds sorok: a gate utáni kiértékelésnek az eltérő
    // értékek miatt provisional-re kell visszaállítania őket (nem maradhatnak).
    sqlx::query("UPDATE fact.odds_snapshot SET trust_state='verified'")
        .execute(&pool)
        .await
        .unwrap();

    let outcome = tick::tick(&pool, &store, &fetchers, &adapters(), Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(outcome.processed, 1);

    // A meccs verified_publish: az odds-eltérés NEM Finding, a gate-et nem érinti.
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM ops.publish_gate WHERE state='verified_publish'"
        )
        .await,
        2
    );
    // Az odds viszont provisional minden soron (eltérő független értékek).
    assert_eq!(
        count(&pool, "SELECT count(*) FROM fact.odds_snapshot").await,
        4
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM fact.odds_snapshot WHERE trust_state='provisional'"
        )
        .await,
        4
    );
}

#[tokio::test]
async fn shadow_unchanged() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    let fixture = base_fixture(now);
    let path = write_fixture(fetch_dir.path(), "demo_day.json", &fixture);
    let fetchers = as_fetchers(fetcher_for(path, now));
    seed_and_generate(&pool, SourceMode::Shadow, now).await;
    let src = seed_source(&pool, "demo-synthetic", "Demó szintetikus forrás").await;
    seed_trust(&pool, src, TrustClass::Medium, None).await;

    let outcome = tick::tick(&pool, &store, &fetchers, &adapters(), Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(outcome.processed, 1);

    // Shadow: nincs kanonikus meccs, nincs odds sor, nincs gate és trust frissítés.
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 0);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM fact.odds_snapshot").await,
        0
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM ops.publish_gate").await,
        0
    );
    assert!(count(&pool, "SELECT count(*) FROM stage.observation").await > 0);
}
