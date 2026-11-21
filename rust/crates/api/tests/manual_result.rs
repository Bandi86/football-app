//! S70: a kézi eredménybevitel API-szerződése és audit-nyoma.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use football_api::{AppState, router};
use football_storage::test_support::test_pool;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

fn app_state(pool: sqlx::PgPool) -> AppState {
    AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    }
}

async fn seed_match(pool: &sqlx::PgPool) -> (Uuid, Uuid, Uuid) {
    let competition_id = Uuid::now_v7();
    let season_id = Uuid::now_v7();
    let home_id = Uuid::now_v7();
    let away_id = Uuid::now_v7();
    let match_id = Uuid::now_v7();

    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'XX','Ismeretlen') ON CONFLICT DO NOTHING")
        .bind(Uuid::now_v7())
        .execute(pool)
        .await
        .unwrap();
    for (id, name) in [(home_id, "Home FC"), (away_id, "Away FC")] {
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO core.team (id, entity_id, name, gender, age_group, branch) VALUES ($1,$1,$2,'male','senior','senior')")
            .bind(id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
    }
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','Test Cup')",
    )
    .bind(competition_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.competition (id, entity_id, name, tier, priority) VALUES ($1,$1,'Test Cup',0,30)")
        .bind(competition_id)
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
        "INSERT INTO core.match
         (id, competition_id, season_id, home_team_id, away_team_id, kickoff_at,
          competition_timezone, status)
         VALUES ($1,$2,$3,$4,$5,now(),'UTC','scheduled')",
    )
    .bind(match_id)
    .bind(competition_id)
    .bind(season_id)
    .bind(home_id)
    .bind(away_id)
    .execute(pool)
    .await
    .unwrap();
    (match_id, home_id, away_id)
}

async fn put(app: &axum::Router, id: Uuid, token: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/match/{id}/manual-result"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let payload: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, payload)
}

#[tokio::test]
async fn manual_result_requires_ops_token_writes_provenance_and_audit() {
    let pool = test_pool().await;
    let (match_id, home_id, away_id) = seed_match(&pool).await;
    let app = router(app_state(pool.clone()));
    let body = json!({"home_ft": 2, "away_ft": 1, "note": "kézi javítás a tesztben"});

    let (unauth, _) = put(&app, match_id, "read-test", body.clone()).await;
    assert_eq!(unauth, StatusCode::UNAUTHORIZED, "read tokennel nem írhat");

    let (status, response) = put(&app, match_id, "ops-test", body).await;
    assert_eq!(status, StatusCode::OK, "ops tokennel írhat: {response}");
    assert_eq!(response["match_id"], match_id.to_string());
    assert!(response["observation_id"].is_string());
    assert!(response["audit_id"].is_string());

    let (home_ft, away_ft, reason): (i16, i16, Option<String>) =
        sqlx::query_as("SELECT home_ft, away_ft, reason FROM core.match_result WHERE match_id=$1")
            .bind(match_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((home_ft, away_ft), (2, 1));
    assert_eq!(reason.as_deref(), Some("kézi javítás a tesztben"));

    let status_db: String = sqlx::query_scalar("SELECT status FROM core.match WHERE id=$1")
        .bind(match_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status_db, "finished");

    let provenance: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fact.provenance
         WHERE subject_kind='match' AND subject_id=$1 AND is_selected
           AND field_key IN ('result','status')",
    )
    .bind(match_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        provenance, 2,
        "result és status is selected provenance-t kap"
    );

    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.audit_event
         WHERE subject_kind='match' AND subject_id=$1 AND action='manual_result_entry'",
    )
    .bind(match_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1);

    let _ = (home_id, away_id);
}

#[tokio::test]
async fn manual_result_validates_input_and_missing_match() {
    let pool = test_pool().await;
    let (match_id, _, _) = seed_match(&pool).await;
    let app = router(app_state(pool));

    let (bad_score, _) = put(
        &app,
        match_id,
        "ops-test",
        json!({"home_ft": -1, "away_ft": 0, "note": "negatív"}),
    )
    .await;
    assert_eq!(bad_score, StatusCode::BAD_REQUEST);

    let (empty_note, _) = put(
        &app,
        match_id,
        "ops-test",
        json!({"home_ft": 1, "away_ft": 0, "note": "  "}),
    )
    .await;
    assert_eq!(empty_note, StatusCode::BAD_REQUEST);

    let (missing, _) = put(
        &app,
        Uuid::now_v7(),
        "ops-test",
        json!({"home_ft": 1, "away_ft": 0, "note": "nincs ilyen meccs"}),
    )
    .await;
    assert_eq!(missing, StatusCode::NOT_FOUND);
}
