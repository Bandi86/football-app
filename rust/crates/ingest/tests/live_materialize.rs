use chrono::{TimeZone, Utc};
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::normalized::{
    NormalizedCompetition, NormalizedDay, NormalizedMatch, NormalizedSeason, NormalizedTeam,
};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_source(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,'live-src','Élő')")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn seed_country(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Hungary')")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    id
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

fn live_day(teams: &[(&str, &str)], home: &str, away: &str) -> NormalizedDay {
    let kickoff = Utc.with_ymd_and_hms(2026, 11, 21, 15, 0, 0).unwrap();
    NormalizedDay {
        source_code: "live-src".into(),
        captured_at: kickoff,
        competition: NormalizedCompetition {
            id: None,
            name: "Premier League".into(),
            country_code: "HU".into(),
            country_name: "Hungary".into(),
            tier: 1,
            priority: 6,
        },
        season: NormalizedSeason {
            id: None,
            label: "2026/27".into(),
        },
        teams: teams
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
            home_source_entity_id: home.into(),
            away_source_entity_id: away.into(),
            round_label: None,
            kickoff_at: kickoff,
            competition_timezone: "Europe/Budapest".into(),
            status: "scheduled".into(),
            result: None,
            odds: None,
        }],
        standings: None,
    }
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn live_day_materializes_and_is_idempotent() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    seed_linked_team(&pool, source_id, country_id, "ujpest", "Újpest").await;
    seed_linked_team(&pool, source_id, country_id, "gyor", "Győr").await;

    let day = live_day(&[("ujpest", "Újpest"), ("gyor", "Győr")], "ujpest", "gyor");
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(materialized.matches.len(), 1, "egy meccs materializálva");
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.competition").await,
        1
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM core.season").await, 1);
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 1);
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM identity.source_entity_ref WHERE method='fixture_seed'"
        )
        .await,
        0,
        "élő úton nincs fixture_seed ref"
    );

    // Újrafuttatás: natural-key upsert, nincs duplikátum.
    Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.competition").await,
        1
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM core.season").await, 1);
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 1);
}

#[tokio::test]
async fn unresolved_team_skips_the_match() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    seed_linked_team(&pool, source_id, country_id, "ujpest", "Újpest").await;
    // A vendég nincs kötve: a linker review-sort nyit, a meccs kimarad.

    let day = live_day(
        &[("ujpest", "Újpest"), ("ismeretlen", "Ismeretlen SE")],
        "ujpest",
        "ismeretlen",
    );
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(
        materialized.matches.len(),
        0,
        "feloldatlan csapatnál nincs meccs"
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 0);
    assert!(
        count(&pool, "SELECT count(*) FROM identity.review_queue").await >= 1,
        "a linker review-sort nyitott"
    );
}
