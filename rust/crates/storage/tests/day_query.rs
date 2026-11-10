use chrono::{NaiveDate, TimeZone, Utc};
use football_storage::repositories::day::{DayFilter, fetch_day};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

struct MatchBase {
    competition_id: Uuid,
    season_id: Uuid,
    home_team_id: Uuid,
    away_team_id: Uuid,
}

#[tokio::test]
async fn default_day_contains_only_verified_publish_matches() {
    let pool = test_pool().await;
    let base = seed_match_base(&pool).await;
    let match_id = seed_match(
        &pool,
        &base,
        "verified_publish",
        Utc.with_ymd_and_hms(2026, 11, 9, 18, 0, 0).unwrap(),
    )
    .await;
    let provisional_id = seed_match(
        &pool,
        &base,
        "provisional_publish",
        Utc.with_ymd_and_hms(2026, 11, 9, 19, 0, 0).unwrap(),
    )
    .await;

    let rows = fetch_day(
        &pool,
        &DayFilter {
            date: NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
            timezone: "Europe/Budapest".into(),
            include_all: false,
        },
    )
    .await
    .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].match_id, match_id);
    assert_ne!(rows[0].match_id, provisional_id);

    let all = fetch_day(
        &pool,
        &DayFilter {
            date: NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
            timezone: "Europe/Budapest".into(),
            include_all: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(all.len(), 2);
}

async fn seed_match_base(pool: &PgPool) -> MatchBase {
    let country_id = Uuid::now_v7();
    let competition_entity_id = Uuid::now_v7();
    let competition_id = Uuid::now_v7();
    let season_id = Uuid::now_v7();
    let home_entity_id = Uuid::now_v7();
    let home_team_id = Uuid::now_v7();
    let away_entity_id = Uuid::now_v7();
    let away_team_id = Uuid::now_v7();

    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Hungary')")
        .bind(country_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','NB I')",
    )
    .bind(competition_entity_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name)
         VALUES ($1,$2,$3,'NB I')",
    )
    .bind(competition_id)
    .bind(competition_entity_id)
    .bind(country_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season_id)
        .bind(competition_id)
        .execute(pool)
        .await
        .unwrap();

    for (entity_id, team_id, name) in [
        (home_entity_id, home_team_id, "Újpest"),
        (away_entity_id, away_team_id, "Győr"),
    ] {
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(entity_id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO core.team
             (id, entity_id, country_id, name, gender, age_group, branch)
             VALUES ($1,$2,$3,$4,'male','senior','senior')",
        )
        .bind(team_id)
        .bind(entity_id)
        .bind(country_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    }

    MatchBase {
        competition_id,
        season_id,
        home_team_id,
        away_team_id,
    }
}

async fn seed_match(
    pool: &PgPool,
    base: &MatchBase,
    state: &str,
    kickoff_at: chrono::DateTime<Utc>,
) -> Uuid {
    let match_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, round_label, home_team_id, away_team_id,
          kickoff_at, competition_timezone, status)
         VALUES ($1,$2,$3,'12',$4,$5,$6,'Europe/Budapest','scheduled')",
    )
    .bind(match_id)
    .bind(base.competition_id)
    .bind(base.season_id)
    .bind(base.home_team_id)
    .bind(base.away_team_id)
    .bind(kickoff_at)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO ops.publish_gate
         (id, match_id, publish_date, state, rule_set_version, blockers, evaluated_at)
         VALUES ($1,$2,'2026-11-09',$3,'verification-1.0','[]',now())",
    )
    .bind(Uuid::now_v7())
    .bind(match_id)
    .bind(state)
    .execute(pool)
    .await
    .unwrap();

    match_id
}
