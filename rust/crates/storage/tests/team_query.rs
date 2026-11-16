use chrono::{NaiveDate, TimeZone, Utc};
use football_storage::repositories::team::{fetch_team_detail, fetch_teams};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed(pool: &PgPool) -> (Uuid, Uuid) {
    let country_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Hungary')")
        .bind(country_id)
        .execute(pool)
        .await
        .unwrap();
    let competition_entity = Uuid::now_v7();
    let competition_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','NB I')",
    )
    .bind(competition_entity)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name, tier, priority)
         VALUES ($1,$2,$3,'NB I',1,8)",
    )
    .bind(competition_id)
    .bind(competition_entity)
    .bind(country_id)
    .execute(pool)
    .await
    .unwrap();
    let season_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season_id)
        .bind(competition_id)
        .execute(pool)
        .await
        .unwrap();

    let mut teams = Vec::new();
    for name in ["Újpest", "Győr", "Vasas"] {
        let entity = Uuid::now_v7();
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(entity)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch)
             VALUES ($1,$1,$2,$3,'male','senior','senior')",
        )
        .bind(entity)
        .bind(country_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
        teams.push(entity);
    }

    let m = |home: Uuid, away: Uuid, score: (i16, i16), hour: u32| {
        let pool = pool.clone();
        async move {
            let id = Uuid::now_v7();
            let kickoff = Utc.with_ymd_and_hms(2026, 11, 9, hour, 0, 0).unwrap();
            sqlx::query(
                "INSERT INTO core.match
                 (id, competition_id, season_id, round_label, home_team_id, away_team_id,
                  kickoff_at, competition_timezone, status)
                 VALUES ($1,$2,$3,'1',$4,$5,$6,'Europe/Budapest','finished')",
            )
            .bind(id)
            .bind(competition_id)
            .bind(season_id)
            .bind(home)
            .bind(away)
            .bind(kickoff)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO ops.publish_gate
                 (id, match_id, publish_date, state, rule_set_version, blockers, evaluated_at)
                 VALUES ($1,$2,'2026-11-09','verified_publish','verification-1.1','[]',now())",
            )
            .bind(Uuid::now_v7())
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO core.match_result (match_id, home_ft, away_ft, home_ht, away_ht)
                 VALUES ($1,$2,$3,0,0)",
            )
            .bind(id)
            .bind(score.0)
            .bind(score.1)
            .execute(&pool)
            .await
            .unwrap();
        }
    };
    m(teams[0], teams[1], (2, 1), 16).await;
    m(teams[2], teams[0], (1, 1), 18).await;

    (teams[0], competition_id)
}

#[tokio::test]
async fn team_list_search_and_detail_form() {
    let pool = test_pool().await;
    let (ujpest_id, _) = seed(&pool).await;
    let date = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();

    let all = fetch_teams(&pool, "", date, "Europe/Budapest")
        .await
        .unwrap();
    assert_eq!(all.len(), 3);
    let found = fetch_teams(&pool, "Újp", date, "Europe/Budapest")
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Újpest");
    assert_eq!(found[0].recent_matches, 2);

    let detail = fetch_team_detail(&pool, ujpest_id, date, "Europe/Budapest", 30)
        .await
        .unwrap()
        .expect("csapat");
    assert_eq!(
        (detail.played, detail.wins, detail.draws, detail.losses),
        (2, 1, 1, 0)
    );
    assert_eq!((detail.goals_for, detail.goals_against), (3, 2));
    assert!(detail.elo.is_none());

    let missing = fetch_team_detail(&pool, Uuid::now_v7(), date, "Europe/Budapest", 30)
        .await
        .unwrap();
    assert!(missing.is_none());
}
