//! S2b-1 Task 6 — identitás-integráció E2E tesztek.
//!
//! A tick live ágának `home/away_identity_exact` flagjei a fail-closed
//! linker feloldásából jönnek (nem hardcode). Négy eset:
//! 1. egzakt fixture → verified_publish (fixture_seed ref → Exact),
//! 2. bizonytalan identitás: withheld → pending review → approve → Exact
//!    (a controller-döntés szerint resolve_team_identity szinten bizonyítva),
//! 3. shadow ág érintetlen: nincs kanonikus írás és nincs linkelés,
//! 4. DB-hiba (lezárt pool) → fail-closed false, pánik nélkül.
//!
//! Nincs élő hálózat: minden fetch a FixtureFetcher-en megy; a golden
//! fixture dátumát a helper a mai UTC napra írja át (az S2a tick_e2e
//! mintáján). A verified_publish-hez megerősítő második forrás is kell
//! (TRUST_LEVEL szabály).

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, TimeZone, Utc};
use football_domain::policy::{EventTemplate, SourceMode, SourcePolicy, Transport};
use football_identity::review;
use football_ingest::adapter::SourceAdapter;
use football_ingest::demo::DemoAdapter;
use football_ingest::fetch::{Fetchers, FixtureFetcher};
use football_ingest::fixture::FixtureSource;
use football_ingest::http::RustHttpFetcher;
use football_ingest::observe::insert_observation;
use football_storage::object_store::ObjectStore;
use football_storage::repositories::identity as identity_repo;
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

/// FixtureFetcher a DemoAdapter plan-kulcsához ("demo-synthetic:match:<mai nap>").
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

/// Megerősítő második forrás a TRUST_LEVEL szabályhoz (az S2a tick_e2e
/// mintáján): az observationök anyagi mezői egyeznek a DemoAdapter
/// kimenetével, így a MATERIAL_SOURCE_CONFLICT sem talál eltérést.
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
        "application/json",
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

/// Közös előkészítés: policy + job + fixture másolat + megerősítő forrás.
/// A TempDir guardokat visszaadjuk: a FixtureFetcher fetch-időben olvas,
/// ezért a könyvtáraknak a tickig élniük kell.
async fn setup_live(
    pool: &PgPool,
    now: DateTime<Utc>,
) -> (
    ObjectStore,
    FixtureFetcher,
    FixtureSource,
    tempfile::TempDir,
    tempfile::TempDir,
) {
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    let fetcher = fixture_fetcher("demo_day.json", now, fetch_dir.path());
    seed_and_generate(pool, SourceMode::Live, now).await;
    let rewritten = std::fs::read_to_string(fetch_dir.path().join("demo_day.json")).unwrap();
    let fixture: FixtureSource = serde_json::from_str(&rewritten).unwrap();
    seed_corroborating_source(pool, &store, &fixture, now).await;
    (store, fetcher, fixture, store_dir, fetch_dir)
}

#[tokio::test]
async fn exact_fixture_stays_verified() {
    let pool = test_pool().await;
    let now = today_noon();
    let (store, fetcher, _fixture, _store_dir, _fetch_dir) = setup_live(&pool, now).await;
    let fetchers = as_fetchers(fetcher);

    let outcome = tick::tick(&pool, &store, &fetchers, &adapters(), Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(outcome.processed, 1);
    assert_eq!(outcome.blocked, 0);
    assert_eq!(outcome.failed, 0);

    // Mindkét meccs verified: a fixture_seed ref egzakt feloldást ad.
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM ops.publish_gate WHERE state='verified_publish'"
        )
        .await,
        2
    );
    // Nincs bizonytalan identitás: review-sor nem nyílt.
    assert_eq!(
        count(&pool, "SELECT count(*) FROM identity.review_queue").await,
        0
    );
    // A két csapat refje aktív fixture_seed.
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM identity.source_entity_ref
             WHERE method='fixture_seed' AND status='active'"
        )
        .await,
        2
    );
    // Magyar szövegek visszaolvasása: forrás-név és csapatnevek épségben.
    let display_name: String =
        sqlx::query_scalar("SELECT display_name FROM ops.source WHERE code='demo-synthetic'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(display_name, "Demó szintetikus forrás");
    let team_names: Vec<String> = sqlx::query_scalar(
        "SELECT canonical_name FROM core.entity WHERE kind='team' ORDER BY canonical_name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        team_names,
        vec!["Demó FC".to_string(), "Vendég SC".to_string()]
    );
}

#[tokio::test]
async fn uncertain_identity_cycle() {
    let pool = test_pool().await;
    let now = today_noon();
    let (store, fetcher, fixture, _store_dir, _fetch_dir) = setup_live(&pool, now).await;
    let fetchers = as_fetchers(fetcher);

    // a) Tick → verified (a fixture_seed ref aktív, egzakt feloldás).
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

    let source_id = identity_repo::source_id_by_code(&pool, "demo-synthetic")
        .await
        .unwrap()
        .expect("a demo forrás létezik");
    let team = fixture
        .teams
        .iter()
        .find(|t| t.source_entity_id == "DEMO-FC")
        .expect("a DEMO-FC csapat a fixture-ben van");

    // b) Teszt-only SQL: a csapat fixture_seed refje rejected lesz.
    sqlx::query(
        "UPDATE identity.source_entity_ref SET status='rejected'
         WHERE source_id=$1 AND entity_kind='team' AND source_entity_id='DEMO-FC'",
    )
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();

    // Teszt-only SQL: azonos nevű és kontextusú második kanonikus entitás.
    // Erre azért van szükség, mert a linker 4. lépése (egzakt normalizált
    // név + kontextus, pontosan 1 találat) a ref nélkül is feloldaná a
    // "Demó FC" nevet — a fail-closed withheld csak akkor áll elő, ha a
    // név-lépés sem egyértelmű. A két azonos nevű entitás a linker
    // multiple_candidates ágát adja (Candidates → false + pending queue).
    let twin_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team','Demó FC')")
        .bind(twin_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch)
         SELECT $1, $1, country_id, name, gender, age_group, branch FROM core.team WHERE id=$2",
    )
    .bind(twin_id)
    .bind(team.id)
    .execute(&pool)
    .await
    .unwrap();

    // c) Közvetlen feloldás → false (withheld flag), pending review-sor nyílik.
    // A resolve_team_identity szándékosan pub: a teljes tick újrafutása a
    // canonicalize lépésben újraélesztené a fixture_seed refet (ON CONFLICT
    // DO UPDATE status='active'), ezért a withheld → approve → verified
    // ciklus resolve-szinten bizonyított; a tick-szintű withheld állapot
    // nem-fixture forrásoknál jelentkezik (S3).
    let resolved = tick::resolve_team_identity(
        &pool,
        source_id,
        "demo-synthetic",
        &football_ingest::normalized::NormalizedDay::from(&fixture),
        team.id,
    )
    .await;
    assert!(
        !resolved,
        "bizonytalan identitásnál a flag fail-closed false"
    );

    let (queue_id, reason, candidates): (Uuid, String, serde_json::Value) = sqlx::query_as(
        "SELECT id, reason, candidate_entity_ids FROM identity.review_queue
         WHERE status='pending' AND source_entity_id='DEMO-FC'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(reason, "multiple_candidates");
    let candidate_ids: Vec<Uuid> = serde_json::from_value(candidates).unwrap();
    assert!(candidate_ids.contains(&team.id));
    assert!(candidate_ids.contains(&twin_id));

    // d) Kézi jóváhagyás → a kézi ref aktív, a feloldás újra egzakt.
    review::approve(&pool, queue_id, team.id, "test", None)
        .await
        .unwrap();
    let resolved = tick::resolve_team_identity(
        &pool,
        source_id,
        "demo-synthetic",
        &football_ingest::normalized::NormalizedDay::from(&fixture),
        team.id,
    )
    .await;
    assert!(
        resolved,
        "jóváhagyás után a ManualReview ref egzakt feloldást ad"
    );

    let (method, status, entity_id): (String, String, Uuid) = sqlx::query_as(
        "SELECT method, status, entity_id FROM identity.source_entity_ref
         WHERE source_id=$1 AND entity_kind='team' AND source_entity_id='DEMO-FC'",
    )
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(method, "manual_review");
    assert_eq!(status, "active");
    assert_eq!(entity_id, team.id);

    let (queue_status, decided_by): (String, String) =
        sqlx::query_as("SELECT status, decided_by FROM identity.review_queue WHERE id=$1")
            .bind(queue_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(queue_status, "approved");
    assert_eq!(decided_by, "test");
    // Az audit append-only: a jóváhagyás nyoma megmaradt.
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM ops.audit_event WHERE action='identity_approve'"
        )
        .await,
        1
    );
}

#[tokio::test]
async fn shadow_mode_unchanged() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    let fetchers = as_fetchers(fixture_fetcher("demo_day.json", now, fetch_dir.path()));
    seed_and_generate(&pool, SourceMode::Shadow, now).await;

    let outcome = tick::tick(&pool, &store, &fetchers, &adapters(), Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(outcome.processed, 1);

    // Shadow ágban nincs kanonikus írás és nincs linkelés.
    assert!(count(&pool, "SELECT count(*) FROM stage.observation").await > 0);
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 0);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM identity.review_queue").await,
        0
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM identity.source_entity_ref").await,
        0
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM ops.publish_gate").await,
        0
    );
}

#[tokio::test]
async fn db_error_is_fail_closed() {
    let pool = test_pool().await;
    // Golden fixture közvetlenül (dátum-átírás nélkül): a feloldás csak a
    // csapatnév/kontextus mezőket használja, nem a kickoff dátumát.
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/worker/demo_day.json");
    let fixture: FixtureSource =
        serde_json::from_str(&std::fs::read_to_string(&fixture_path).unwrap()).unwrap();
    let team = &fixture.teams[0];

    // A lezárt pool minden lekérdezése PoolClosed hibát ad → a linker
    // IdentityError::Db-t kap, a tick-ági hívó naplóz és false-t ad.
    pool.close().await;
    let resolved = tick::resolve_team_identity(
        &pool,
        Uuid::now_v7(),
        "demo-synthetic",
        &football_ingest::normalized::NormalizedDay::from(&fixture),
        team.id,
    )
    .await;
    assert!(
        !resolved,
        "DB-hibánál a flag fail-closed false, pánik nélkül"
    );
}
