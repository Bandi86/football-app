//! Játékvezető-feloldás (S83): a linker név/fuzzy lépcsője a kért
//! entitás-fajtán dolgozik, a kontextus pedig a meccs versenyének országa.
//!
//! A tesztek a valódi materializáló úton mennek végig (Canonicalizer), mert a
//! szelet lényege a hívói kontextus átadása is, nem csak a linker szabálya.

use chrono::{TimeZone, Utc};
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::normalized::{
    NormalizedCompetition, NormalizedDay, NormalizedMatch, NormalizedSeason, NormalizedTeam,
};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

/// Idempotens seed: a megosztott teszt-DB-n más folyamat is dolgozhat, ezért
/// nem feltételezzük, hogy a sor hiányzik (a test_pool TRUNCATE-ja versenyzik).
async fn seed_source(pool: &PgPool) -> Uuid {
    sqlx::query(
        "INSERT INTO ops.source (id, code, display_name) VALUES ($1,'live-src','Élő')
         ON CONFLICT (code) DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .execute(pool)
    .await
    .unwrap();
    sqlx::query_scalar("SELECT id FROM ops.source WHERE code='live-src'")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn seed_country(pool: &PgPool) -> Uuid {
    sqlx::query(
        "INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Hungary')
         ON CONFLICT (code) DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .execute(pool)
    .await
    .unwrap();
    sqlx::query_scalar("SELECT id FROM core.country WHERE code='HU'")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn seed_country_code(pool: &PgPool, code: &str) -> Uuid {
    sqlx::query(
        "INSERT INTO core.country (id, code, name) VALUES ($1,$2,$2)
         ON CONFLICT (code) DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .bind(code)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query_scalar("SELECT id FROM core.country WHERE code=$1")
        .bind(code)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn seed_referee_with_country(pool: &PgPool, name: &str, code: &str) -> Uuid {
    let country_id = seed_country_code(pool, code).await;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name, country_id)
         VALUES ($1,'referee',$2,$3)",
    )
    .bind(id)
    .bind(name)
    .bind(country_id)
    .execute(pool)
    .await
    .unwrap();
    id
}

fn referee_day_with_country(
    competition_country: &str,
    referee: &str,
    referee_country: &str,
) -> NormalizedDay {
    let mut day = referee_day(competition_country, "Versenyország", Some(referee));
    day.matches[0].referee_country = Some(referee_country.into());
    day
}

async fn seed_linked_team(
    pool: &PgPool,
    source_id: Uuid,
    country_id: Uuid,
    ext: &str,
    name: &str,
) -> Uuid {
    let entity_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
        .bind(entity_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.team
         (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$1,$2,$3,'male','senior','senior')",
    )
    .bind(entity_id)
    .bind(country_id)
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO identity.source_entity_ref
         (id, source_id, entity_kind, source_entity_id, entity_id, method)
         VALUES ($1,$2,'team',$3,$4,'manual_review')",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(ext)
    .bind(entity_id)
    .execute(pool)
    .await
    .unwrap();
    entity_id
}

/// Játékvezető-katalógus entitás forrás-ref NÉLKÜL: így a név-lépcső dönt.
async fn seed_referee(pool: &PgPool, name: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'referee',$2)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    id
}

fn referee_day(country_code: &str, country_name: &str, referee: Option<&str>) -> NormalizedDay {
    let kickoff = Utc.with_ymd_and_hms(2026, 11, 21, 15, 0, 0).unwrap();
    NormalizedDay {
        source_code: "live-src".into(),
        captured_at: kickoff,
        competition: NormalizedCompetition {
            id: None,
            source_entity_id: None,
            // Szándékosan nincs a verseny-szótárban: a megfigyelt ország marad.
            name: "Teszt Liga".into(),
            country_code: country_code.into(),
            country_name: country_name.into(),
            tier: 1,
            priority: 6,
        },
        season: NormalizedSeason {
            id: None,
            label: "2026/27".into(),
        },
        teams: [("ujpest", "Újpest"), ("gyor", "Győr")]
            .iter()
            .map(|(ext, name)| NormalizedTeam {
                id: None,
                source_entity_id: (*ext).into(),
                name: (*name).into(),
                country_code: "HU".into(),
                gender: "male".into(),
                age_group: "senior".into(),
                branch: "senior".into(),
            })
            .collect(),
        matches: vec![NormalizedMatch {
            id: None,
            source_match_id: "m-1".into(),
            home_source_entity_id: "ujpest".into(),
            away_source_entity_id: "gyor".into(),
            round_label: None,
            kickoff_at: kickoff,
            kickoff_precision: "exact".into(),
            competition_timezone: "Europe/Budapest".into(),
            status: "scheduled".into(),
            result: None,
            odds: None,
            stats: None,
            referee: referee.map(str::to_string),
            referee_country: None,
            environment: None,
            incidents: Vec::new(),
            insight: None,
        }],
        standings: None,
    }
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

async fn match_referee(pool: &PgPool, match_id: Uuid) -> Option<Uuid> {
    sqlx::query_scalar("SELECT referee_entity_id FROM core.match WHERE id=$1")
        .bind(match_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// (reason, candidate_entity_ids, context.country_code) a referee review-sorhoz.
async fn referee_review(
    pool: &PgPool,
    source_entity_id: &str,
) -> Option<(String, serde_json::Value, Option<String>)> {
    sqlx::query_as(
        "SELECT reason, candidate_entity_ids, context->>'country_code'
           FROM identity.review_queue
          WHERE entity_kind='referee' AND source_entity_id=$1 AND status='pending'",
    )
    .bind(source_entity_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn seed_match_pool(pool: &PgPool) -> (Uuid, Uuid) {
    let source_id = seed_source(pool).await;
    let country_id = seed_country(pool).await;
    seed_linked_team(pool, source_id, country_id, "ujpest", "Újpest").await;
    seed_linked_team(pool, source_id, country_id, "gyor", "Győr").await;
    (source_id, country_id)
}

/// (a) Egzakt foldolt TELJES név (ékezet-változattal) a referee-katalóguson,
/// egyetlen jelölt, valódi verseny-ország kontextussal → link, nincs review.
#[tokio::test]
async fn referee_exact_full_name_links() {
    let pool = test_pool().await;
    let (_source_id, _country_id) = seed_match_pool(&pool).await;
    let referee = seed_referee(&pool, "Felix Zwayer").await;

    let day = referee_day("HU", "Hungary", Some("Félix Zwayer"));
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(materialized.matches.len(), 1, "a meccs materializálódik");
    assert_eq!(
        match_referee(&pool, materialized.matches[0].match_id).await,
        Some(referee),
        "a teljes nevű referee linkel"
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM identity.review_queue WHERE entity_kind='referee'"
        )
        .await,
        0,
        "sikeres linknél nincs review-sor"
    );
}

/// (b) Ugyanaz a teljes név két referee-entitáson → nincs link, review a két
/// jelölttel, a verseny országával a kontextusban.
#[tokio::test]
async fn referee_ambiguous_full_name_stays_review() {
    let pool = test_pool().await;
    let (_source_id, _country_id) = seed_match_pool(&pool).await;
    let first = seed_referee(&pool, "Felix Zwayer").await;
    let second = seed_referee(&pool, "Felix Zwayer").await;

    let day = referee_day("HU", "Hungary", Some("Felix Zwayer"));
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(materialized.matches.len(), 1);
    assert_eq!(
        match_referee(&pool, materialized.matches[0].match_id).await,
        None,
        "két jelöltnél nincs automatikus link"
    );

    let (reason, candidates, country) = referee_review(&pool, "felix-zwayer")
        .await
        .expect("review-sor nyílt");
    assert_eq!(reason, "multiple_candidates");
    let ids: std::collections::HashSet<Uuid> = serde_json::from_value(candidates).unwrap();
    assert_eq!(ids, std::collections::HashSet::from([first, second]));
    assert_eq!(
        country.as_deref(),
        Some("HU"),
        "a verseny országa a kontextus"
    );
}

/// (c) Rövid alak (vezetéknév + kezdőbetű) egzakt egyezése sem linkelhető
/// automatikusan (AGENTS.md §6): csak review-jelölt, a fajtaszűrt lépcsőn.
#[tokio::test]
async fn referee_short_form_never_links() {
    let pool = test_pool().await;
    let (_source_id, _country_id) = seed_match_pool(&pool).await;
    let referee = seed_referee(&pool, "M. Oliver").await;

    let day = referee_day("HU", "Hungary", Some("M. Oliver"));
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(materialized.matches.len(), 1);
    assert_eq!(
        match_referee(&pool, materialized.matches[0].match_id).await,
        None,
        "rövid alak nem linkel automatikusan"
    );

    let (reason, candidates, _) = referee_review(&pool, "m-oliver")
        .await
        .expect("review-sor nyílt");
    assert_eq!(reason, "fuzzy_candidate");
    let ids: Vec<Uuid> = serde_json::from_value(candidates).unwrap();
    assert_eq!(ids, vec![referee], "a rövid alak jelöltként jelenik meg");
}

/// (d) A fuzzy jelöltlista fajtaszűrt: egy ugyanilyen nevű CSAPAT nem lehet
/// játékvezető-jelölt.
#[tokio::test]
async fn fuzzy_candidate_is_kind_filtered() {
    let pool = test_pool().await;
    let (source_id, country_id) = seed_match_pool(&pool).await;
    // Csapat ugyanazzal a teljes névvel: korábban ez volt a téves jelölt.
    let _team = seed_linked_team(&pool, source_id, country_id, "zwayer-team", "Felix Zwayer").await;

    let day = referee_day("HU", "Hungary", Some("Felix Zwayer"));
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(materialized.matches.len(), 1);
    assert_eq!(
        match_referee(&pool, materialized.matches[0].match_id).await,
        None,
        "csapat nem linkelhető játékvezetőként"
    );

    let (reason, candidates, _) = referee_review(&pool, "felix-zwayer")
        .await
        .expect("review-sor nyílt");
    assert_eq!(reason, "no_exact_match");
    let ids: Vec<Uuid> = serde_json::from_value(candidates).unwrap();
    assert!(
        ids.is_empty(),
        "más fajtájú entitás nem kerülhet a jelöltlistára: {ids:?}"
    );
}

/// (e) INT (nemzetközi) verseny-ország → semleges kontextus: nincs
/// ország-szűrés, a review-sor kontextusában sincs ország.
#[tokio::test]
async fn neutral_competition_country_has_no_country_context() {
    let pool = test_pool().await;
    let (_source_id, _country_id) = seed_match_pool(&pool).await;

    let day = referee_day("INT", "International", Some("Felix Zwayer"));
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(materialized.matches.len(), 1);

    // Az INT kanonikus kódja XX (a semleges kód), és a hívó ezt None-ra oldja.
    let competition_country: Option<String> = sqlx::query_scalar(
        "SELECT c.code FROM core.competition comp
           LEFT JOIN core.country c ON c.id = comp.country_id
          WHERE comp.name = 'Teszt Liga'
          LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(competition_country.as_deref(), Some("XX"));

    let (reason, candidates, country) = referee_review(&pool, "felix-zwayer")
        .await
        .expect("review-sor nyílt");
    assert_eq!(reason, "no_exact_match");
    let ids: Vec<Uuid> = serde_json::from_value(candidates).unwrap();
    assert!(ids.is_empty());
    assert!(
        country.is_none(),
        "semleges verseny-országnál nincs ország-szűrés: {country:?}"
    );
}

/// (f) S102: a bíró SAJÁT országafelülírja a versenyországot — a verseny HU,
/// a bíró DE, és a katalógusban is DE: a teljes név linkel.
#[tokio::test]
async fn referee_own_country_wins_over_competition_country() {
    let pool = test_pool().await;
    let (_source_id, _country_id) = seed_match_pool(&pool).await;
    let referee = seed_referee_with_country(&pool, "Felix Zwayer", "DE").await;

    let day = referee_day_with_country("HU", "Felix Zwayer", "DE");
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(
        match_referee(&pool, materialized.matches[0].match_id).await,
        Some(referee),
        "a bíró saját országa szerinti egyezés linkel"
    );
    let stored_country: Option<String> = sqlx::query_scalar(
        "SELECT country_code FROM fact.match_official WHERE role='referee' AND name='Felix Zwayer'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored_country.as_deref(), Some("DE"));
}

/// (g) S102: ha a bíró saját országa és a katalógus országa eltér, nincs link —
/// fail-closed, nem találgatunk, nem fuzzy-zünk.
#[tokio::test]
async fn referee_country_mismatch_blocks_exact_link() {
    let pool = test_pool().await;
    let (_source_id, _country_id) = seed_match_pool(&pool).await;
    let _referee = seed_referee_with_country(&pool, "Felix Zwayer", "DE").await;

    let day = referee_day_with_country("DE", "Felix Zwayer", "HU");
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(
        match_referee(&pool, materialized.matches[0].match_id).await,
        None,
        "eltérő bíró-ország: nincs automatikus link"
    );
}
