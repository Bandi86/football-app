//! A logófeltöltés biztonsági kapui: auth, formátum, entitás, méret.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use football_api::{AppState, router};
use football_storage::test_support::test_pool;
use http_body_util::BodyExt;
use tower::ServiceExt;
use uuid::Uuid;

async fn seed_team(pool: &sqlx::PgPool) -> Uuid {
    let country: Uuid = sqlx::query_scalar(
        "INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Magyarorszag')
         ON CONFLICT (code) DO UPDATE SET name=EXCLUDED.name RETURNING id",
    )
    .bind(Uuid::now_v7())
    .fetch_one(pool)
    .await
    .unwrap();
    let entity_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team','Upload Team')",
    )
    .bind(entity_id)
    .execute(pool)
    .await
    .unwrap();
    let team_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$2,$3,'Upload Team','male','senior','senior')",
    )
    .bind(team_id)
    .bind(entity_id)
    .bind(country)
    .execute(pool)
    .await
    .unwrap();
    team_id
}

fn multipart(team_id: Uuid, image: &[u8], filename: &str) -> (String, Vec<u8>) {
    let boundary = "----football-upload-test";
    let mut body = Vec::new();
    for (name, value) in [("entity_kind", "team"), ("entity_id", &team_id.to_string())] {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: image/png\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(image);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

async fn post(
    app: &axum::Router,
    content_type: &str,
    body: Vec<u8>,
    token: Option<&str>,
) -> (StatusCode, String) {
    let mut builder = Request::builder()
        .uri("/v1/asset/upload")
        .method("POST")
        .header("content-type", content_type);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn upload_accepts_png_and_records_audit() {
    let pool = test_pool().await;
    let team_id = seed_team(&pool).await;
    let dir = std::env::temp_dir().join(format!("fb-upload-test-{}", Uuid::now_v7()));
    let app = router(AppState {
        pool: pool.clone(),
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: dir.clone(),
    });

    let png = b"\x89PNG\r\n\x1a\nminimal";
    let (content_type, body) = multipart(team_id, png, "logo.png");
    let (status, response) = post(&app, &content_type, body, Some("ops-test")).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    let value: serde_json::Value = serde_json::from_str(&response).unwrap();
    assert_eq!(value["mime_type"], "image/png");
    assert!(
        value["url"]
            .as_str()
            .unwrap()
            .starts_with("/v1/asset/assets/")
    );

    let stored: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.asset WHERE entity_kind='team' AND origin='user_upload' AND entity_id=$1",
    )
    .bind(team_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, 1);
    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.audit_event WHERE action='asset_uploaded' AND subject_id=$1",
    )
    .bind(team_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audited, 1);
    assert!(
        dir.join("raw")
            .join(value["storage_key"].as_str().unwrap())
            .exists()
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn upload_rejects_bad_auth_format_and_unknown_entity() {
    let pool = test_pool().await;
    let team_id = seed_team(&pool).await;
    let dir = std::env::temp_dir().join(format!("fb-upload-test-{}", Uuid::now_v7()));
    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: dir.clone(),
    });

    let (content_type, body) = multipart(team_id, b"\x89PNG\r\n\x1a\nok", "logo.png");
    let (status, _) = post(&app, &content_type, body.clone(), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = post(&app, &content_type, body, Some("read-test")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "read token nem írhat");

    let (content_type, body) = multipart(team_id, b"not an image", "logo.png");
    let (status, _) = post(&app, &content_type, body, Some("ops-test")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (content_type, body) = multipart(Uuid::now_v7(), b"\x89PNG\r\n\x1a\nok", "logo.png");
    let (status, _) = post(&app, &content_type, body, Some("ops-test")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let _ = std::fs::remove_dir_all(dir);
}
