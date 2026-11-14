use football_storage::object_store::ObjectStore;
use football_storage::repositories::worker::{self as repo, RawCapture};
use football_storage::test_support::{seed_min_match, test_pool};

#[tokio::test]
async fn persist_raw_uses_payload_media_type() {
    let pool = test_pool().await;
    let (_match_id, _source_id) = seed_min_match(&pool).await;
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();
    let artifact = store.put_json(b"<html></html>").unwrap();
    let now = chrono::Utc::now();
    let capture = RawCapture {
        endpoint_key: "match",
        request_url: "https://example.test/page",
        request_key: "test:page",
        requested_at: now,
        received_at: now,
        http_status: 200,
        adapter_version: "1.0",
        parser_version: "1.0",
    };
    repo::persist_raw(&pool, "seed", "Seed", &artifact, &capture, "text/html")
        .await
        .unwrap();
    let media_type: String =
        sqlx::query_scalar("SELECT media_type FROM raw.artifact WHERE content_sha256=$1")
            .bind(&artifact.sha256)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(media_type, "text/html");
}
