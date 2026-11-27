//! A `/v1/match/{id}/markets` végpont auth-mátrixa, 404-e és piac-tartalma.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use football_api::{AppState, router};
use football_storage::test_support::test_pool;
use http_body_util::BodyExt;
use tower::ServiceExt;
use uuid::Uuid;

async fn seed_match(pool: &sqlx::PgPool) -> Uuid {
    let country: Uuid = sqlx::query_scalar(
        "INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Magyarorszag')
         ON CONFLICT (code) DO UPDATE SET name=EXCLUDED.name RETURNING id",
    )
    .bind(Uuid::now_v7())
    .fetch_one(pool)
    .await
    .unwrap();
    let home = Uuid::now_v7();
    let away = Uuid::now_v7();
    for (entity_id, team_id, name) in [
        (Uuid::now_v7(), home, "Home"),
        (Uuid::now_v7(), away, "Away"),
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
        .bind(country)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    }
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
         VALUES ($1,$2,$3,$4,$5,now(),'UTC','scheduled')",
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

fn app(pool: sqlx::PgPool) -> axum::Router {
    router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    })
}

#[tokio::test]
async fn markets_endpoint_has_auth_matrix_empty_list_and_404() {
    let pool = test_pool().await;
    let match_id = seed_match(&pool).await;
    let app = app(pool);
    let uri = format!("/v1/match/{match_id}/markets");

    let (status, _) = get(&app, &uri, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, body) = get(&app, &uri, Some("read-test")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, r#"{"quotes":[],"count":0}"#);
    let (status, _) = get(&app, &uri, Some("ops-test")).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = get(
        &app,
        &format!("/v1/match/{}/markets", Uuid::now_v7()),
        Some("read-test"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(&app, "/v1/match/nem-uuid/markets", Some("read-test")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn markets_endpoint_returns_quotes_with_source_and_latest_capture() {
    let pool = test_pool().await;
    let match_id = seed_match(&pool).await;
    let source_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ops.source (id, code, display_name) VALUES ($1,'tipplap','Tipplap')
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .fetch_one(&pool)
    .await
    .unwrap();
    let older = chrono::Utc::now() - chrono::Duration::hours(1);
    sqlx::query(
        "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count)
         VALUES ($1,$2,$3,2)",
    )
    .bind(match_id)
    .bind(source_id)
    .bind(older)
    .execute(&pool)
    .await
    .unwrap();
    for (market, outcome, price) in [("btts", "yes", 1.53), ("btts", "no", 2.19)] {
        sqlx::query(
            "INSERT INTO fact.odds_quote
             (match_id, source_id, bookmaker, market, outcome, line, phase, price)
             VALUES ($1,$2,'Tippmix',$3,$4,0,'opening',$5)",
        )
        .bind(match_id)
        .bind(source_id)
        .bind(market)
        .bind(outcome)
        .bind(price)
        .execute(&pool)
        .await
        .unwrap();
    }
    let app = app(pool);
    let (status, body) = get(
        &app,
        &format!("/v1/match/{match_id}/markets"),
        Some("read-test"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let value: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(value["count"], 2);
    assert_eq!(value["quotes"][0]["market"], "btts");
    assert_eq!(value["quotes"][0]["outcome"], "no");
    assert_eq!(value["quotes"][1]["outcome"], "yes");
    assert_eq!(value["quotes"][1]["source_code"], "tipplap");
    assert_eq!(value["quotes"][1]["price"], 1.53);
}
