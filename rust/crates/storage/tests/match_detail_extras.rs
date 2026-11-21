//! A részletező nézet forma- és egymás-elleni kiegészítései (S53): a meccs
//! kezdete előtti lejátszott meccsekből számolnak, jövőbeli adat nélkül.

use chrono::{TimeZone, Utc};
use football_storage::repositories::match_detail::{
    fetch_head_to_head, fetch_team_form, fetch_team_next,
};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

struct Base {
    competition_id: Uuid,
    season_id: Uuid,
    a: Uuid,
    b: Uuid,
    c: Uuid,
}

async fn seed(pool: &PgPool) -> Base {
    let country = Uuid::now_v7();
    let comp_entity = Uuid::now_v7();
    let competition = Uuid::now_v7();
    let season = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Magyarország')")
        .bind(country)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','NB I')",
    )
    .bind(comp_entity)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name) VALUES ($1,$2,$3,'NB I')",
    )
    .bind(competition)
    .bind(comp_entity)
    .bind(country)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season)
        .bind(competition)
        .execute(pool)
        .await
        .unwrap();
    let mut teams = Vec::new();
    for name in ["Alfa", "Beta", "Gamma"] {
        let entity = Uuid::now_v7();
        let team = Uuid::now_v7();
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(entity)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch)
             VALUES ($1,$2,$3,$4,'male','senior','senior')",
        )
        .bind(team)
        .bind(entity)
        .bind(country)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
        teams.push(team);
    }
    Base {
        competition_id: competition,
        season_id: season,
        a: teams[0],
        b: teams[1],
        c: teams[2],
    }
}

async fn add_match(
    pool: &PgPool,
    base: &Base,
    home: Uuid,
    away: Uuid,
    kickoff: chrono::DateTime<Utc>,
    score: (i16, i16),
    status: &str,
) {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, home_team_id, away_team_id, kickoff_at,
          competition_timezone, status)
         VALUES ($1,$2,$3,$4,$5,$6,'UTC',$7)",
    )
    .bind(id)
    .bind(base.competition_id)
    .bind(base.season_id)
    .bind(home)
    .bind(away)
    .bind(kickoff)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
    if status != "finished" {
        return;
    }
    sqlx::query(
        "INSERT INTO core.match_result
         (match_id, home_ft, away_ft, winner_method, validity, decided_by)
         VALUES ($1,$2,$3,'regular','valid','ft')",
    )
    .bind(id)
    .bind(score.0)
    .bind(score.1)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn form_and_head_to_head_use_only_past_matches() {
    let pool = test_pool().await;
    let base = seed(&pool).await;
    let now = Utc.with_ymd_and_hms(2026, 11, 21, 12, 0, 0).unwrap();

    add_match(
        &pool,
        &base,
        base.a,
        base.b,
        now - chrono::Duration::days(10),
        (2, 1),
        "finished",
    )
    .await;
    add_match(
        &pool,
        &base,
        base.c,
        base.a,
        now - chrono::Duration::days(7),
        (0, 0),
        "finished",
    )
    .await;
    add_match(
        &pool,
        &base,
        base.a,
        base.b,
        now - chrono::Duration::days(3),
        (1, 3),
        "finished",
    )
    .await;
    // Jövőbeli meccs: nem szivároghat vissza.
    add_match(
        &pool,
        &base,
        base.a,
        base.b,
        now + chrono::Duration::days(2),
        (0, 0),
        "scheduled",
    )
    .await;

    let form = fetch_team_form(&pool, base.a, now, 5).await.unwrap();
    assert_eq!(form.len(), 3);
    assert_eq!(form[0].outcome, "L");
    assert_eq!(form[0].team_goals, 1);
    assert_eq!(form[0].opponent_goals, 3);
    assert_eq!(form[1].outcome, "D");
    assert_eq!(form[2].outcome, "W");
    assert_eq!(form[2].opponent_name, "Beta");

    let h2h = fetch_head_to_head(&pool, base.a, base.b, now, 5)
        .await
        .unwrap();
    assert_eq!(h2h.len(), 2, "a jövőbeli meccs nem szerepelhet");
    assert_eq!(h2h[0].home_name, "Alfa");
    assert_eq!(h2h[0].away_goals, Some(3));

    // Következő meccsek: csak a kezdés utáni, még le nem játszott sorok.
    add_match(
        &pool,
        &base,
        base.c,
        base.a,
        now + chrono::Duration::days(4),
        (0, 0),
        "scheduled",
    )
    .await;
    let next = fetch_team_next(&pool, base.a, now, 3).await.unwrap();
    assert_eq!(next.len(), 2);
    assert_eq!(next[0].opponent_name, "Beta");
    assert!(next[0].is_home);
    assert_eq!(next[1].opponent_name, "Gamma");
    assert!(!next[1].is_home);
}
