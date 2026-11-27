//! S211: a forduló-pótlás csak NULL mezőt ír, és idempotens.

use football_storage::repositories::round_labels;
use football_storage::test_support::{seed_min_match, test_pool};
use serde_json::json;
use uuid::Uuid;

async fn seed_research_observation(pool: &sqlx::PgPool, match_id: Uuid, round: &str) {
    let source_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ops.source (id, code, display_name) VALUES ($1,'research','Research')
         ON CONFLICT (code) DO UPDATE SET display_name=EXCLUDED.display_name RETURNING id",
    )
    .bind(Uuid::now_v7())
    .fetch_one(pool)
    .await
    .unwrap();

    let artifact_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO raw.artifact
         (id, content_sha256, compression, byte_size, storage_key, media_type)
         VALUES ($1,$2,'none',0,$3,'application/json')",
    )
    .bind(artifact_id)
    .bind(format!("sha-{artifact_id}"))
    .bind(format!("key-{artifact_id}"))
    .execute(pool)
    .await
    .unwrap();
    let capture_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO raw.capture
         (id, source_id, artifact_id, endpoint_key, request_url, requested_at,
          received_at, http_status, adapter_version, parser_version, request_key)
         VALUES ($1,$2,$3,'day','https://example.test/day',now(),now(),200,'t','p',$4)",
    )
    .bind(capture_id)
    .bind(source_id)
    .bind(artifact_id)
    .bind(format!("req-{capture_id}"))
    .execute(pool)
    .await
    .unwrap();

    let natural_key = format!("fm:test-{match_id}");
    let payload = json!({ "round_label": round, "status": "finished" });
    sqlx::query(
        "INSERT INTO stage.observation
         (id, source_id, capture_id, entity_kind, aspect, natural_key,
          source_record_key, observed_at, normalized_payload,
          schema_version, quality_flags, content_hash)
         VALUES ($1,$2,$3,'match','match_core',$4,$4,now(),$5,'1.0','{}',$6)",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(capture_id)
    .bind(&natural_key)
    .bind(&payload)
    .bind(format!("hash-{match_id}"))
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO identity.source_entity_ref
         (id, source_id, entity_kind, source_entity_id, entity_id, method)
         VALUES ($1,$2,'match',$3,$4,'fixture_seed')",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(&natural_key)
    .bind(match_id)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn round_backfill_fills_only_null_and_is_idempotent() {
    let pool = test_pool().await;
    let (match_id, _source_id) = seed_min_match(&pool).await;
    seed_research_observation(&pool, match_id, "13").await;

    assert_eq!(
        round_labels::count_missing_round_labels(&pool)
            .await
            .unwrap(),
        1
    );
    let first = round_labels::backfill_round_labels(&pool).await.unwrap();
    assert_eq!(first.candidates, 1);
    assert_eq!(first.updated, 1);

    let stored: String = sqlx::query_scalar("SELECT round_label FROM core.match WHERE id = $1")
        .bind(match_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, "13");

    let second = round_labels::backfill_round_labels(&pool).await.unwrap();
    assert_eq!(second.candidates, 0);
    assert_eq!(second.updated, 0);
}
