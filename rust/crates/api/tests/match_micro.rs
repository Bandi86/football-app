//! A három mikro-végpont auth-mátrixa, 404-e és üres-listája.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use football_api::{AppState, router};
use football_storage::test_support::test_pool;
use http_body_util::BodyExt;
use tower::ServiceExt;
use uuid::Uuid;

async fn seed_team(pool: &sqlx::PgPool, country: Uuid, name: &str) -> Uuid {
    let entity_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
        .bind(entity_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    let team_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$2,$3,$4,'male','senior','senior')",
    )
    .bind(team_id)
    .bind(entity_id)
    .bind(country)
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
    team_id
}

async fn seed_match(pool: &sqlx::PgPool) -> Uuid {
    let country: Uuid = sqlx::query_scalar(
        "INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Magyarorszag')
         ON CONFLICT (code) DO UPDATE SET name=EXCLUDED.name RETURNING id",
    )
    .bind(Uuid::now_v7())
    .fetch_one(pool)
    .await
    .unwrap();
    let home = seed_team(pool, country, "Home").await;
    let away = seed_team(pool, country, "Away").await;
    let comp = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','Liga')",
    )
    .bind(comp)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name, tier, priority)
         VALUES ($1,$1,$2,'Liga',1,1)",
    )
    .bind(comp)
    .bind(country)
    .execute(pool)
    .await
    .unwrap();
    let season = Uuid::now_v7();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'26/27')")
        .bind(season)
        .bind(comp)
        .execute(pool)
        .await
        .unwrap();
    let match_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.match (id, competition_id, season_id, home_team_id, away_team_id,
                                 kickoff_at, competition_timezone, status)
         VALUES ($1,$2,$3,$4,$5,now(),'UTC','finished')",
    )
    .bind(match_id)
    .bind(comp)
    .bind(season)
    .bind(home)
    .bind(away)
    .execute(pool)
    .await
    .unwrap();
    match_id
}

async fn get(app: &axum::Router, uri: &str, token: Option<&str>) -> (StatusCode, String) {
    let mut builder = Request::builder().uri(uri);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn match_micro_endpoints_require_read_token_and_404_on_unknown_match() {
    let pool = test_pool().await;
    let match_id = seed_match(&pool).await;
    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    });

    for suffix in ["lineups", "player-stats", "shots"] {
        let uri = format!("/v1/match/{match_id}/{suffix}");
        let (status, _) = get(&app, &uri, None).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "{suffix}: token nélkül 401"
        );

        let (status, _) = get(&app, &uri, Some("ops-test")).await;
        assert_eq!(status, StatusCode::OK, "{suffix}: ops token olvashat");

        let unknown = format!("/v1/match/{}/{}", Uuid::now_v7(), suffix);
        let (status, _) = get(&app, &unknown, Some("read-test")).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{suffix}: ismeretlen meccs 404"
        );
    }

    let invalid = "/v1/match/nem-uuid/lineups".to_string();
    let (status, _) = get(&app, &invalid, Some("read-test")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn match_micro_endpoints_return_empty_arrays_without_micro_data() {
    let pool = test_pool().await;
    let match_id = seed_match(&pool).await;
    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    });

    for (suffix, key) in [
        ("lineups", "lineups"),
        ("player-stats", "player_stats"),
        ("shots", "shots"),
    ] {
        let (status, body) = get(
            &app,
            &format!("/v1/match/{match_id}/{suffix}"),
            Some("read-test"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            value[key].as_array().unwrap().len(),
            0,
            "{suffix}: hiány = []"
        );
    }
}
