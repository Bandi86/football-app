use chrono::{NaiveDate, TimeZone, Utc};
use football_storage::repositories::competition::{
    fetch_competition_detail, fetch_competition_teams, fetch_competitions,
};
use football_storage::repositories::day::{DayFilter, fetch_day};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

struct Seed {
    competition_id: Uuid,
}

async fn seed(pool: &PgPool) -> Seed {
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

    let mut team_ids = Vec::new();
    for (name, code) in [("Újpest", "ujpest"), ("Győr", "gyor"), ("Vasas", "vasas")] {
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
        let _ = code;
        team_ids.push(entity);
    }

    let seed_match = |home: Uuid,
                      away: Uuid,
                      kickoff: chrono::DateTime<Utc>,
                      state: &'static str,
                      score: Option<(i16, i16)>| {
        let pool = pool.clone();
        async move {
            let id = Uuid::now_v7();
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
                 VALUES ($1,$2,$3,$4,'verification-1.1','[]',now())",
            )
            .bind(Uuid::now_v7())
            .bind(id)
            .bind((kickoff + chrono::Duration::hours(2)).date_naive())
            .bind(state)
            .execute(&pool)
            .await
            .unwrap();
            if let Some((h, a)) = score {
                sqlx::query(
                    "INSERT INTO core.match_result
                     (match_id, home_ft, away_ft, home_ht, away_ht)
                     VALUES ($1,$2,$3,0,0)",
                )
                .bind(id)
                .bind(h)
                .bind(a)
                .execute(&pool)
                .await
                .unwrap();
            }
            id
        }
    };

    // 2026-11-09: Újpest–Győr 2:1, Vasas–Újpest 1:1 (verified)
    seed_match(
        team_ids[0],
        team_ids[1],
        Utc.with_ymd_and_hms(2026, 11, 9, 16, 0, 0).unwrap(),
        "verified_publish",
        Some((2, 1)),
    )
    .await;
    seed_match(
        team_ids[2],
        team_ids[0],
        Utc.with_ymd_and_hms(2026, 11, 9, 18, 0, 0).unwrap(),
        "verified_publish",
        Some((1, 1)),
    )
    .await;
    // Ablakon kívüli, provisional: nem számít.
    seed_match(
        team_ids[1],
        team_ids[2],
        Utc.with_ymd_and_hms(2026, 11, 9, 20, 0, 0).unwrap(),
        "provisional_publish",
        Some((0, 0)),
    )
    .await;

    Seed { competition_id }
}

#[tokio::test]
async fn competition_list_counts_gated_matches() {
    let pool = test_pool().await;
    let seed = seed(&pool).await;
    let date = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
    let rows = fetch_competitions(&pool, date, "Europe/Budapest")
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].competition_id, seed.competition_id);
    assert_eq!(rows[0].matches, 2, "csak a verified meccsek");
    assert_eq!(rows[0].finished, 2);
}

#[tokio::test]
async fn competition_detail_aggregates_and_teams() {
    let pool = test_pool().await;
    let seed = seed(&pool).await;
    let date = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
    let detail = fetch_competition_detail(&pool, seed.competition_id, date, "Europe/Budapest", 7)
        .await
        .unwrap()
        .expect("verseny");
    assert_eq!(detail.matches, 2);
    assert_eq!(detail.goals, 5, "2:1 + 1:1");
    assert_eq!(detail.btts_matches, 2);
    assert_eq!(detail.home_wins, 1);
    assert_eq!(detail.draws, 1);
    assert_eq!(detail.away_wins, 0);
    assert_eq!(detail.season_label.as_deref(), Some("2026/27"));

    let teams = fetch_competition_teams(&pool, seed.competition_id, date, "Europe/Budapest", 7)
        .await
        .unwrap();
    assert_eq!(teams.len(), 3);
    let ujpest = teams.iter().find(|t| t.name == "Újpest").unwrap();
    assert_eq!(
        (ujpest.played, ujpest.wins, ujpest.draws, ujpest.losses),
        (2, 1, 1, 0)
    );
    assert_eq!((ujpest.goals_for, ujpest.goals_against), (3, 2));
    // Rendezés: legtöbb győzelem elöl (Újpest), majd gólkülönbség.
    assert_eq!(teams[0].name, "Újpest");
}

#[tokio::test]
async fn day_filter_supports_competition_window() {
    let pool = test_pool().await;
    let seed = seed(&pool).await;
    let date = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
    let base = |window: i64| DayFilter {
        date,
        timezone: "Europe/Budapest".into(),
        include_all: false,
        competition_id: Some(seed.competition_id),
        window_days: window,
    };
    let day = fetch_day(&pool, &base(0)).await.unwrap();
    assert_eq!(day.len(), 2);
    let windowed = fetch_day(&pool, &base(7)).await.unwrap();
    assert_eq!(windowed.len(), 2, "a provisional továbbra is kimarad");
}
