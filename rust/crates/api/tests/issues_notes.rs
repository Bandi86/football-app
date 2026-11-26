//! S168 — az új hiány-/jegyzet-/történet-végpontok jogosultsági és
//! szerződés-tesztei. A `test_pool()` miatt `--test-threads=1` kötelező.

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use football_api::{AppState, router};
use football_storage::repositories::issue::NewIssue;
use football_storage::test_support::{seed_min_match, test_pool};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

use football_domain::ops::{IssueCode, IssueSeverity, IssueSource, IssueSubjectKind};

fn app_state(pool: sqlx::PgPool) -> AppState {
    AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    }
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(
            builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value)
}

#[tokio::test]
async fn issues_are_readable_with_read_token_and_validated() {
    let pool = test_pool().await;
    let (match_id, _) = seed_min_match(&pool).await;
    football_storage::repositories::issue::upsert_seen(
        &pool,
        &NewIssue {
            subject_kind: IssueSubjectKind::Match,
            subject_id: match_id,
            code: IssueCode::MissingOdds,
            severity: IssueSeverity::Warning,
            source: IssueSource::Integrity,
            detail: json!({}),
        },
    )
    .await
    .unwrap();
    let app = router(app_state(pool));

    let (status, _) = call(&app, "GET", "/v1/issues", None, Value::Null).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = call(&app, "GET", "/v1/issues", Some("read-test"), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["count"], 1);
    assert_eq!(body["issues"][0]["code"], "missing_odds");
    assert_eq!(body["issues"][0]["source"], "integrity");

    let (status, body) = call(
        &app,
        "GET",
        "/v1/issues?scope=day&date=2026-11-14",
        Some("read-test"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["count"], 1);

    for uri in [
        "/v1/issues?scope=day",
        "/v1/issues?scope=day&date=2026-11-14T18:00",
        "/v1/issues?scope=evil",
        "/v1/issues?code=missing_result",
        "/v1/issues?severity=blocker",
    ] {
        let (status, body) = call(&app, "GET", uri, Some("read-test"), Value::Null).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "hibás kérés: {uri}");
        assert_eq!(body["code"], "BAD_REQUEST");
    }
}

#[tokio::test]
async fn notes_require_ops_token_and_every_write_is_audited() {
    let pool = test_pool().await;
    let (match_id, _) = seed_min_match(&pool).await;
    let app = router(app_state(pool.clone()));
    let uri = format!("/v1/match/{match_id}/notes");

    let (status, _) = call(
        &app,
        "POST",
        &uri,
        Some("read-test"),
        json!({ "author": "ui", "body": "ok", "kind": "info" }),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "read token nem írhat");

    for payload in [
        json!({ "author": "valaki", "body": "ok", "kind": "info" }),
        json!({ "author": "ui", "body": "  ", "kind": "info" }),
        json!({ "author": "ui", "body": "ok", "kind": "done" }),
    ] {
        let (status, _) = call(&app, "POST", &uri, Some("ops-test"), payload).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    let (status, created) = call(
        &app,
        "POST",
        &uri,
        Some("ops-test"),
        json!({ "author": "agent:lead", "body": "Nincs odds a forrásban.", "kind": "problem" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["author"], "agent:lead");
    let note_id = created["id"].as_str().unwrap();

    let (status, listed) = call(&app, "GET", &uri, Some("read-test"), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["notes"].as_array().unwrap().len(), 1);

    let (status, edited) = call(
        &app,
        "PATCH",
        &format!("/v1/note/{note_id}"),
        Some("ops-test"),
        json!({ "body": "Az odds megjött.", "kind": "fix", "resolved": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(edited["body"], "Az odds megjött.");
    assert_eq!(edited["kind"], "fix");
    assert!(!edited["resolved_at"].is_null());

    let (status, _) = call(
        &app,
        "PATCH",
        "/v1/note/00000000-0000-0000-0000-000000000009",
        Some("ops-test"),
        json!({ "body": "nem létezik" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM ops.audit_event
          WHERE subject_kind = 'match' AND subject_id = $1 ORDER BY created_at",
    )
    .bind(match_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(actions, vec!["note_added", "note_edited"]);
}

#[tokio::test]
async fn history_is_read_only_and_rejects_bad_input() {
    let pool = test_pool().await;
    let (match_id, _) = seed_min_match(&pool).await;
    sqlx::query(
        "INSERT INTO ops.audit_event
           (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1, 'ui', 'manual_match_edit', 'match', $2, $3)",
    )
    .bind(Uuid::now_v7())
    .bind(match_id)
    .bind(json!({ "before": { "status": "scheduled" }, "after": { "status": "finished" } }))
    .execute(&pool)
    .await
    .unwrap();
    let app = router(app_state(pool));
    let uri = format!("/v1/match/{match_id}/history");

    let (status, _) = call(&app, "GET", &uri, None, Value::Null).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = call(&app, "GET", &uri, Some("read-test"), Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["entries"].as_array().unwrap().len(), 1);
    assert_eq!(body["entries"][0]["action"], "manual_match_edit");
    assert_eq!(body["entries"][0]["before"]["status"], "scheduled");

    let (status, _) = call(
        &app,
        "GET",
        "/v1/match/00000000-0000-0000-0000-000000000009/history",
        Some("read-test"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    for uri in [format!("{uri}?limit=0"), format!("{uri}?limit=201")] {
        let (status, _) = call(&app, "GET", &uri, Some("read-test"), Value::Null).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
