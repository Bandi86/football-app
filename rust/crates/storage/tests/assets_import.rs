use football_storage::repositories::worker::{self as repo, AssetRowInput};
use football_storage::test_support::test_pool;
use uuid::Uuid;

#[tokio::test]
async fn upsert_asset_is_idempotent_on_natural_key() {
    let pool = test_pool().await;
    let row = AssetRowInput {
        entity_kind: "team",
        entity_id: Some(Uuid::now_v7()),
        kind: "logo",
        url: "https://example.test/logo.png",
        source: "logos-github",
        licence: Some("CC0"),
        licence_status: "cleared",
        attribution: Some("Logos GitHub"),
        captured_at: chrono::Utc::now(),
    };
    let id1 = repo::upsert_asset(&pool, &row).await.unwrap();
    let id2 = repo::upsert_asset(&pool, &row).await.unwrap();
    assert_eq!(id1, id2, "egyediség: upsert, nem duplikál");
    let status: String = sqlx::query_scalar("SELECT licence_status FROM ops.asset WHERE id=$1")
        .bind(id1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "cleared");
}

#[tokio::test]
async fn worker_status_counts_include_pending_reviews() {
    let pool = test_pool().await;
    let counts = repo::worker_status_counts(&pool).await.unwrap();
    assert_eq!(counts.reviews_pending, 0);
    sqlx::query(
        "INSERT INTO identity.review_queue
         (id, source_id, entity_kind, source_entity_id, reason, proposed_name, context)
         VALUES ($1, NULL, 'team', 'x', 'no_exact_match', 'X', '{}'::jsonb)",
    )
    .bind(Uuid::now_v7())
    .execute(&pool)
    .await
    .unwrap();
    let counts = repo::worker_status_counts(&pool).await.unwrap();
    assert_eq!(counts.reviews_pending, 1);
}
