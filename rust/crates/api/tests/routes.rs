use axum::body::Body;
use axum::http::{Request, StatusCode};
use football_api::{AppState, router};
use football_storage::test_support::test_pool;
use tower::ServiceExt;

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
