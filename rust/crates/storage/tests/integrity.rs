//! S25 — napi integritás-riport tesztjei: a kemény hibák (duplikátum,
//! árva provenance, összevont entitásra mutató aktív ref) felismerése.

use chrono::{TimeZone, Utc};
use football_storage::repositories::integrity::{IntegrityReport, report};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_country(pool: &PgPool, code: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,$2,$2)")
        .bind(id)
        .bind(code)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn seed_match(
    pool: &PgPool,
    comp: Uuid,
    season: Uuid,
    home: Uuid,
    away: Uuid,
    kickoff: chrono::DateTime<Utc>,
) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, home_team_id, away_team_id,
          kickoff_at, competition_timezone, status, kickoff_precision)
         VALUES ($1,$2,$3,$4,$5,$6,'UTC','finished','exact')",
    )
    .bind(id)
    .bind(comp)
    .bind(season)
    .bind(home)
    .bind(away)
    .bind(kickoff)
    .execute(pool)
    .await
    .unwrap();
    id
}

#[tokio::test]
async fn report_counts_duplicate_kickoff_and_fixture_groups() {
    let pool = test_pool().await;
    let before = report(&pool, None, None).await.unwrap();
    let country = seed_country(&pool, "NO").await;
    let entity = Uuid::now_v7();
    let comp = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','Eliteserien')")
        .bind(entity)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO core.competition (id, entity_id, country_id, name) VALUES ($1,$2,$3,'Eliteserien')")
        .bind(comp)
        .bind(entity)
        .bind(country)
        .execute(&pool)
        .await
        .unwrap();
    let season = Uuid::now_v7();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2015')")
        .bind(season)
        .bind(comp)
        .execute(&pool)
        .await
        .unwrap();
    let mut teams = Vec::new();
    for name in ["Molde", "Tromsø"] {
        let team = Uuid::now_v7();
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(team)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO core.team (id, entity_id, country_id, name, gender) VALUES ($1,$1,$2,$3,'male')")
            .bind(team)
            .bind(country)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
        teams.push(team);
    }
    let k1 = Utc.with_ymd_and_hms(2015, 10, 4, 12, 0, 0).unwrap();
    let k2 = Utc.with_ymd_and_hms(2015, 10, 4, 15, 0, 0).unwrap();
    seed_match(&pool, comp, season, teams[0], teams[1], k1).await;
    seed_match(&pool, comp, season, teams[0], teams[1], k1).await; // pontos duplikátum
    seed_match(&pool, comp, season, teams[0], teams[1], k2).await; // csak nap-szintű duplikátum

    let after = report(&pool, None, None).await.unwrap();
    assert_eq!(
        after.duplicate_kickoff_groups,
        before.duplicate_kickoff_groups + 1
    );
    assert_eq!(
        after.duplicate_fixture_groups,
        before.duplicate_fixture_groups + 1
    );
    assert!(after.has_hard_failures());

    // Dátum-szűrés: a csoportok kívül esnek az ablakon.
    let outside = report(
        &pool,
        Some("2020-01-01".parse().unwrap()),
        Some("2020-12-31".parse().unwrap()),
    )
    .await
    .unwrap();
    assert_eq!(outside.duplicate_kickoff_groups, 0);
    assert_eq!(outside.duplicate_fixture_groups, 0);
}

#[test]
fn clean_report_has_no_hard_failures() {
    let r = IntegrityReport {
        duplicate_kickoff_groups: 0,
        duplicate_fixture_groups: 0,
        orphan_provenance: 0,
        active_refs_to_merged: 0,
        pending_reviews: 3,
        withheld_gates: 2,
        failed_jobs: 1,
        open_breakers: 1,
    };
    assert!(!r.has_hard_failures());
}
