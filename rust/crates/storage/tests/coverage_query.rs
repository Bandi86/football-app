use chrono::{NaiveDate, TimeZone, Utc};
use football_storage::repositories::coverage::fetch_daily_coverage;
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

struct Base {
    competition_id: Uuid,
    season_id: Uuid,
    home_team_id: Uuid,
    away_team_id: Uuid,
}

#[tokio::test]
async fn daily_coverage_counts_results_odds_and_logos() {
    let pool = test_pool().await;
    let base = seed(&pool).await;

    let with_all = seed_match(&pool, &base, "verified_publish", 20, 18, true).await;
    let bare = seed_match(&pool, &base, "withheld", 21, 0, false).await;
    let _ = bare;

    let rows = fetch_daily_coverage(
        &pool,
        NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
        NaiveDate::from_ymd_opt(2026, 11, 21).unwrap(),
        "Europe/Budapest",
    )
    .await
    .unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].day, NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
    assert_eq!(rows[0].matches, 1);
    assert_eq!(rows[0].with_result, 1);
    assert_eq!(rows[0].with_odds, 1);
    assert_eq!(rows[0].with_both_logos, 1);
    assert_eq!(rows[0].withheld, 0);
    assert_eq!(rows[0].verified, 1);

    assert_eq!(rows[1].day, NaiveDate::from_ymd_opt(2026, 11, 21).unwrap());
    assert_eq!(rows[1].matches, 1);
    assert_eq!(rows[1].with_result, 0);
    assert_eq!(rows[1].with_odds, 0);
    assert_eq!(rows[1].withheld, 1);
    let _ = with_all;
}

async fn seed(pool: &PgPool) -> Base {
    let country_id = Uuid::now_v7();
    let competition_entity_id = Uuid::now_v7();
    let competition_id = Uuid::now_v7();
    let season_id = Uuid::now_v7();
    let source_id = Uuid::now_v7();

    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Magyarország')")
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
    sqlx::query(
        "INSERT INTO ops.source (id, code, display_name) VALUES ($1,'coverage-test','Teszt')",
    )
    .bind(source_id)
    .execute(pool)
    .await
    .unwrap();

    let mut ids = Vec::new();
    for name in ["Újpest", "Győr"] {
        let entity_id = Uuid::now_v7();
        let team_id = Uuid::now_v7();
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(entity_id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch)
             VALUES ($1,$2,$3,$4,'male','senior','senior')",
        )
        .bind(team_id)
        .bind(entity_id)
        .bind(country_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO ops.asset
             (id, entity_kind, entity_id, kind, url, storage_key, source, captured_at)
             VALUES ($1,'team',$2,'logo',$3,$4,'coverage-test',now())",
        )
        .bind(Uuid::now_v7())
        .bind(team_id)
        .bind(format!("https://example.invalid/{team_id}.png"))
        .bind(format!("assets/aa/bb/{team_id}.png"))
        .execute(pool)
        .await
        .unwrap();
        ids.push(team_id);
    }

    Base {
        competition_id,
        season_id,
        home_team_id: ids[0],
        away_team_id: ids[1],
    }
}

async fn seed_match(
    pool: &PgPool,
    base: &Base,
    state: &str,
    day: u32,
    market_count: i32,
    with_result: bool,
) -> Uuid {
    let match_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, round_label, home_team_id, away_team_id,
          kickoff_at, competition_timezone, status)
         VALUES ($1,$2,$3,'12',$4,$5,$6,'Europe/Budapest',$7)",
    )
    .bind(match_id)
    .bind(base.competition_id)
    .bind(base.season_id)
    .bind(base.home_team_id)
    .bind(base.away_team_id)
    .bind(Utc.with_ymd_and_hms(2026, 11, day, 18, 0, 0).unwrap())
    .bind(if with_result { "finished" } else { "scheduled" })
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO ops.publish_gate
         (id, match_id, publish_date, state, rule_set_version, blockers, evaluated_at)
         VALUES ($1,$2,'2026-11-20',$3,'verification-1.3','[]',now())",
    )
    .bind(Uuid::now_v7())
    .bind(match_id)
    .bind(state)
    .execute(pool)
    .await
    .unwrap();

    if with_result {
        sqlx::query(
            "INSERT INTO core.match_result
             (match_id, home_ft, away_ft, winner_method, validity, decided_at)
             VALUES ($1,2,1,'regular','valid',now())",
        )
        .bind(match_id)
        .execute(pool)
        .await
        .unwrap();
    }
    if market_count > 0 {
        let source_id: Uuid =
            sqlx::query_scalar("SELECT id FROM ops.source WHERE code='coverage-test'")
                .fetch_one(pool)
                .await
                .unwrap();
        sqlx::query(
            "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count)
             VALUES ($1,$2,now(),$3)",
        )
        .bind(match_id)
        .bind(source_id)
        .bind(market_count)
        .execute(pool)
        .await
        .unwrap();
    }
    match_id
}
