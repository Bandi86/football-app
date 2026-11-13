use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{TimeZone, Utc};
use football_api::{AppState, router};
use football_storage::test_support::test_pool;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn health_is_public() {
    let pool = test_pool().await;
    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn day_requires_read_token() {
    let pool = test_pool().await;
    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
    });

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/day/2026-11-09")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let authorized = app
        .oneshot(
            Request::builder()
                .uri("/v1/day/2026-11-09")
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(authorized.status(), StatusCode::OK);
}

#[tokio::test]
async fn day_response_includes_odds_trust_state() {
    let pool = test_pool().await;
    let base = seed_day_base(&pool).await;
    let verified_match = seed_day_match(&pool, &base, 18).await;
    let no_odds_match = seed_day_match(&pool, &base, 19).await;

    let source_id = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,'demo','Demó')")
        .bind(source_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO fact.odds_snapshot
         (match_id, source_id, captured_at, market_count, odds, trust_state)
         VALUES ($1,$2,now(),18,'{\"home\":2.2,\"draw\":3.3,\"away\":3.2}'::jsonb,'verified')",
    )
    .bind(verified_match)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();

    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
    });
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/day/2026-11-09")
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let matches = body["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 2);
    // A verified odds állapota megjelenik az API válaszban.
    assert_eq!(matches[0]["id"], verified_match.to_string());
    assert_eq!(matches[0]["odds_trust_state"], "verified");
    // Odds nélküli meccsnél a mező null, nem hiányzik és nem üres string.
    // A puszta null-assert nem bizonyítaná a kulcs jelenlétét (a serde_json
    // indexelés hiányzó kulcsra is Null), ezért a contains_key is kell.
    assert_eq!(matches[1]["id"], no_odds_match.to_string());
    assert!(
        matches[1]
            .as_object()
            .unwrap()
            .contains_key("odds_trust_state")
    );
    assert!(matches[1]["odds_trust_state"].is_null());
}

struct DayBase {
    competition_id: Uuid,
    season_id: Uuid,
    home_team_id: Uuid,
    away_team_id: Uuid,
}

async fn seed_day_base(pool: &PgPool) -> DayBase {
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

    DayBase {
        competition_id,
        season_id,
        home_team_id,
        away_team_id,
    }
}

async fn seed_day_match(pool: &PgPool, base: &DayBase, hour: u32) -> Uuid {
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
    .bind(Utc.with_ymd_and_hms(2026, 11, 9, hour, 0, 0).unwrap())
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO ops.publish_gate
         (id, match_id, publish_date, state, rule_set_version, blockers, evaluated_at)
         VALUES ($1,$2,'2026-11-09','verified_publish','verification-1.1','[]',now())",
    )
    .bind(Uuid::now_v7())
    .bind(match_id)
    .execute(pool)
    .await
    .unwrap();

    match_id
}
