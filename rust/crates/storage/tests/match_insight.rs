//! `fact.match_insight` integrációs teszt: idempotens upsert, as-of, NULL-kezelés.

use football_storage::{repositories::match_insight, test_support::test_pool};
use serde_json::json;
use uuid::Uuid;

async fn seed_team(pool: &sqlx::PgPool, country_id: Uuid, name: &str) -> Uuid {
    let entity_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
        .bind(entity_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    let team_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch) VALUES ($1,$2,$3,$4,'male','senior','senior')")
        .bind(team_id).bind(entity_id).bind(country_id).bind(name).execute(pool).await.unwrap();
    team_id
}

#[tokio::test]
async fn insight_upsert_is_idempotent_and_tracks_captured_at() {
    let pool = test_pool().await;
    let source_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ops.source (id, code, display_name) VALUES ($1,'research','Research')
         ON CONFLICT (code) DO UPDATE SET display_name=EXCLUDED.display_name RETURNING id",
    )
    .bind(Uuid::now_v7())
    .fetch_one(&pool)
    .await
    .unwrap();

    let country_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Magyarorszag')
         ON CONFLICT (code) DO NOTHING RETURNING id",
    )
    .bind(Uuid::now_v7())
    .fetch_one(&pool)
    .await
    .unwrap();
    let home = seed_team(&pool, country_id, "Home").await;
    let away = seed_team(&pool, country_id, "Away").await;
    let comp = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','Liga')",
    )
    .bind(comp)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.competition (id, entity_id, country_id, name, tier, priority) VALUES ($1,$1,$2,'Liga',1,1)")
        .bind(comp).bind(country_id).execute(&pool).await.unwrap();
    let season = Uuid::now_v7();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'26/27')")
        .bind(season)
        .bind(comp)
        .execute(&pool)
        .await
        .unwrap();
    let match_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.match (id, competition_id, season_id, home_team_id, away_team_id, kickoff_at, competition_timezone, status) VALUES ($1,$2,$3,$4,$5,now(),'UTC','finished')")
        .bind(match_id).bind(comp).bind(season).bind(home).bind(away).execute(&pool).await.unwrap();

    let first = json!({
        "captured_at": "2026-11-23T10:00:00Z",
        "preview_hu": {"generated": true, "verified": false},
        "prediction": {"model": "poisson_v1"}
    });
    match_insight::upsert(&pool, match_id, source_id, chrono::Utc::now(), &first)
        .await
        .unwrap();
    let second = json!({
        "captured_at": "2026-11-23T11:00:00Z",
        "preview_hu": {"generated": true, "verified": false, "title": "Friss"},
        "prediction": {"model": "poisson_v2"},
        "injuries": [{"player_id": 1, "name": "X"}]
    });
    match_insight::upsert(&pool, match_id, source_id, chrono::Utc::now(), &second)
        .await
        .unwrap();

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.match_insight")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "az upsert nem duplikál");
    let fetched = match_insight::fetch_json(&pool, match_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched["prediction"]["model"], "poisson_v2");
    assert_eq!(fetched["preview_hu"]["title"], "Friss");
    assert!(fetched["standings"].is_null(), "a hiány NULL marad");
    assert_eq!(fetched["source"], "research");
}
