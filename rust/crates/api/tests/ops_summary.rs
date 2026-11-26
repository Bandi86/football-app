//! S170 — globális audit, magyar összefoglaló és jel-lezárás API-tesztei.

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use football_api::{AppState, router};
use football_domain::ops::{IssueCode, IssueSeverity, IssueSource, IssueSubjectKind};
use football_storage::repositories::issue::NewIssue;
use football_storage::test_support::{seed_min_match, test_pool};
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

async fn call(app: &Router, method: &str, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(
            builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

#[tokio::test]
async fn summary_reports_hungarian_headline_and_actions() {
    let pool = test_pool().await;
    let (match_id, _) = seed_min_match(&pool).await;
    football_storage::repositories::issue::upsert_seen(
        &pool,
        &NewIssue {
            subject_kind: IssueSubjectKind::Match,
            subject_id: match_id,
            code: IssueCode::MissingOdds,
            severity: IssueSeverity::Hard,
            source: IssueSource::Integrity,
            detail: json!({}),
        },
    )
    .await
    .unwrap();
    let app = router(app_state(pool));

    let (status, body) = call(
        &app,
        "GET",
        "/v1/ops/summary?date=2026-11-14",
        Some("read-test"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["open_total"], 1);
    assert_eq!(body["by_code"][0]["label_hu"], "Hiányzó odds");
    assert_eq!(body["by_code"][0]["hard"], 1);
    assert_eq!(body["top_actions"][0]["subject_id"], match_id.to_string());
    let headline = body["headline_hu"].as_str().unwrap();
    assert!(headline.contains("nyitott hiányjel"), "{headline}");
    assert!(headline.contains("súlyos"), "{headline}");
}

#[tokio::test]
async fn audit_feed_needs_ops_token_and_filters_by_subject() {
    let pool = test_pool().await;
    let (match_id, _) = seed_min_match(&pool).await;
    sqlx::query(
        "INSERT INTO ops.audit_event
          (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1, 'ui', 'manual_match_edit', 'match', $2, '{}'::jsonb)",
    )
    .bind(Uuid::now_v7())
    .bind(match_id)
    .execute(&pool)
    .await
    .unwrap();
    let app = router(app_state(pool));

    let (status, _) = call(&app, "GET", "/v1/ops/audit", Some("read-test")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = call(
        &app,
        "GET",
        &format!("/v1/ops/audit?subject_kind=match&subject_id={match_id}"),
        Some("ops-test"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["count"], 1);
    assert_eq!(body["entries"][0]["action"], "manual_match_edit");

    let (status, _) = call(&app, "GET", "/v1/ops/audit?limit=0", Some("ops-test")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn resolving_an_issue_is_ops_only_idempotent_and_audited() {
    let pool = test_pool().await;
    let (match_id, _) = seed_min_match(&pool).await;
    let issue_id = football_storage::repositories::issue::upsert_seen(
        &pool,
        &NewIssue {
            subject_kind: IssueSubjectKind::Match,
            subject_id: match_id,
            code: IssueCode::OpenPast,
            severity: IssueSeverity::Warning,
            source: IssueSource::Integrity,
            detail: json!({}),
        },
    )
    .await
    .unwrap();
    let app = router(app_state(pool));
    let uri = format!("/v1/issue/{issue_id}/resolve");

    let (status, _) = call(&app, "POST", &uri, Some("read-test")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = call(&app, "POST", &uri, Some("ops-test")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["resolved"], true);

    let (status, body) = call(&app, "POST", &uri, Some("ops-test")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["resolved"], true, "második hívás is idempotens");

    let (status, _) = call(
        &app,
        "POST",
        "/v1/issue/00000000-0000-0000-0000-000000000009/resolve",
        Some("ops-test"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
