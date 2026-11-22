//! Női csapatnevek feloldása (S96): a tipplap jelölő nélkül küldi a nevet
//! ("Granada"), a kanonikus katalógus viszont jelölővel tárolja ("Granada W",
//! "Granada (W)", "Granada Women"). A `fold_name` az elválasztókat eldobja,
//! ezért a linker 4. lépése női kontextusban marker-normalizált egzakt
//! kulcsokat is kipróbál; a találat csak egyértelmű (1 entitás) esetben linkel.
//!
//! A tesztek a valódi materializáló úton mennek végig (Canonicalizer), mert a
//! női kontextus a hívótól (NormalizedTeam gender/branch) érkezik.

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

/// Katalógus-csapat: entitás + core.team sor, opcionális forrás-ref.
/// `with_ref` nélkül a név-lépcső dönt (ezt mérjük); ref-fel az 1. lépés.
async fn seed_team(
    pool: &PgPool,
    source_id: Uuid,
    country_id: Uuid,
    ext: Option<&str>,
    name: &str,
    gender: &str,
    branch: &str,
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
         VALUES ($1,$1,$2,$3,$4,'senior',$5)",
    )
    .bind(entity_id)
    .bind(country_id)
    .bind(name)
    .bind(gender)
    .bind(branch)
    .execute(pool)
    .await
    .unwrap();
    if let Some(ext) = ext {
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
    }
    entity_id
}

/// Egy meccses nap: a hazai csapat neve jelölő nélkül ("Granada"), a vendég
/// pedig forrás-refen jön (nem a név-lépcsőt méri). `women` dönti el, hogy a
/// payload női kontextust jelez-e (gender=female, branch=women).
fn team_day(women: bool) -> NormalizedDay {
    let kickoff = Utc.with_ymd_and_hms(2026, 11, 21, 15, 0, 0).unwrap();
    let (gender, branch) = if women {
        ("female", "women")
    } else {
        ("male", "senior")
    };
    NormalizedDay {
        source_code: "live-src".into(),
        captured_at: kickoff,
        competition: NormalizedCompetition {
            id: None,
            source_entity_id: None,
            name: "Női Teszt Liga".into(),
            country_code: "HU".into(),
            country_name: "Hungary".into(),
            tier: 1,
            priority: 6,
        },
        season: NormalizedSeason {
            id: None,
            label: "2026/27".into(),
        },
        teams: [("g1", "Granada"), ("s2", "Sevilla")]
            .iter()
            .map(|(ext, name)| NormalizedTeam {
                id: None,
                source_entity_id: (*ext).into(),
                name: (*name).into(),
                country_code: "HU".into(),
                gender: gender.into(),
                age_group: "senior".into(),
                branch: branch.into(),
            })
            .collect(),
        matches: vec![NormalizedMatch {
            id: None,
            source_match_id: "m-1".into(),
            home_source_entity_id: "g1".into(),
            away_source_entity_id: "s2".into(),
            round_label: None,
            kickoff_at: kickoff,
            kickoff_precision: "exact".into(),
            competition_timezone: "Europe/Budapest".into(),
            status: "scheduled".into(),
            result: None,
            odds: None,
            stats: None,
            referee: None,
            environment: None,
            incidents: Vec::new(),
        }],
        standings: None,
    }
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

async fn match_teams(pool: &PgPool, match_id: Uuid) -> (Uuid, Uuid) {
    sqlx::query_as("SELECT home_team_id, away_team_id FROM core.match WHERE id=$1")
        .bind(match_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// (reason, candidate_entity_ids, context.branch) a csapat review-sorához.
async fn team_review(
    pool: &PgPool,
    source_entity_id: &str,
) -> Option<(String, serde_json::Value, Option<String>)> {
    sqlx::query_as(
        "SELECT reason, candidate_entity_ids, context->>'branch'
           FROM identity.review_queue
          WHERE entity_kind='team' AND source_entity_id=$1 AND status='pending'",
    )
    .bind(source_entity_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

/// (a) Női nap + jelölő nélküli "Granada" → a "Granada W" entitásra linkel.
#[tokio::test]
async fn women_team_without_marker_links_to_marker_entity() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    let granada_w = seed_team(
        &pool,
        source_id,
        country_id,
        None,
        "Granada W",
        "female",
        "women",
    )
    .await;
    // A vendég forrás-refen jön, hogy a meccs létrejöhessen.
    seed_team(
        &pool,
        source_id,
        country_id,
        Some("s2"),
        "Sevilla W",
        "female",
        "women",
    )
    .await;

    let materialized = Canonicalizer::materialize(&pool, &team_day(true))
        .await
        .unwrap();
    assert_eq!(materialized.matches.len(), 1, "a meccs materializálódik");
    let (home, away) = match_teams(&pool, materialized.matches[0].match_id).await;
    assert_eq!(home, granada_w, "a Granada W entitásra linkelt");
    assert_ne!(away, home);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM identity.review_queue").await,
        0,
        "nem nyílt review-sor"
    );
}

/// (b) Ugyanaz a jelölő nélküli név FÉRFI kontextussal → nem linkel a női
/// entitásra; a mai viselkedés (review, nincs meccs) marad.
#[tokio::test]
async fn men_context_does_not_link_women_entity() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    seed_team(
        &pool,
        source_id,
        country_id,
        None,
        "Granada W",
        "female",
        "women",
    )
    .await;
    seed_team(
        &pool,
        source_id,
        country_id,
        Some("s2"),
        "Sevilla",
        "male",
        "senior",
    )
    .await;

    let materialized = Canonicalizer::materialize(&pool, &team_day(false))
        .await
        .unwrap();
    assert!(
        materialized.matches.is_empty(),
        "férfi kontextusban nincs találat a női entitásra, a meccs kimarad"
    );
    let (reason, candidates, branch) = team_review(&pool, "g1").await.expect("review-sor");
    assert_eq!(reason, "no_exact_match");
    let ids: Vec<Uuid> = serde_json::from_value(candidates).unwrap();
    assert!(ids.is_empty(), "nincs jelölt: {ids:?}");
    assert_eq!(branch.as_deref(), Some("senior"));
}

/// (c) Két női entitás ("Granada W" és "Granada (W)") → 2 találat → review a
/// jelöltekkel, nincs link.
#[tokio::test]
async fn two_women_entities_stay_review_with_candidates() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    let granada_w = seed_team(
        &pool,
        source_id,
        country_id,
        None,
        "Granada W",
        "female",
        "women",
    )
    .await;
    let granada_paren = seed_team(
        &pool,
        source_id,
        country_id,
        None,
        "Granada (W)",
        "female",
        "women",
    )
    .await;
    seed_team(
        &pool,
        source_id,
        country_id,
        Some("s2"),
        "Sevilla W",
        "female",
        "women",
    )
    .await;

    let materialized = Canonicalizer::materialize(&pool, &team_day(true))
        .await
        .unwrap();
    assert!(materialized.matches.is_empty(), "két jelöltnél nincs link");
    let (reason, candidates, branch) = team_review(&pool, "g1").await.expect("review-sor");
    assert_eq!(reason, "multiple_candidates");
    let ids: std::collections::HashSet<Uuid> = serde_json::from_value(candidates).unwrap();
    assert_eq!(
        ids,
        [granada_w, granada_paren].into_iter().collect(),
        "mindkét női entitás jelölt"
    );
    assert_eq!(branch.as_deref(), Some("women"));
}

/// (d) Regresszió: férfi csapatnév pontos foldolt egyezéssel változatlanul
/// linkel (a marker-normalizálás nem érinti a férfi ágat).
#[tokio::test]
async fn men_team_exact_name_still_links() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    let granada = seed_team(
        &pool, source_id, country_id, None, "Granada", "male", "senior",
    )
    .await;
    seed_team(
        &pool,
        source_id,
        country_id,
        Some("s2"),
        "Sevilla",
        "male",
        "senior",
    )
    .await;

    let materialized = Canonicalizer::materialize(&pool, &team_day(false))
        .await
        .unwrap();
    assert_eq!(materialized.matches.len(), 1);
    let (home, _away) = match_teams(&pool, materialized.matches[0].match_id).await;
    assert_eq!(home, granada, "a férfi csapat pontos névvel linkel");
    assert_eq!(
        count(&pool, "SELECT count(*) FROM identity.review_queue").await,
        0
    );
}
