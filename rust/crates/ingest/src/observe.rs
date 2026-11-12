use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sha2::Digest;
use sqlx::PgPool;
use uuid::Uuid;

#[allow(clippy::too_many_arguments)]
pub async fn insert_observation(
    pool: &PgPool,
    source_id: Uuid,
    capture_id: Uuid,
    entity_kind: &str,
    aspect: &str,
    natural_key: &str,
    source_record_key: &str,
    observed_at: DateTime<Utc>,
    normalized_payload: serde_json::Value,
) -> Result<Uuid> {
    let content_bytes = serde_json::to_vec(&normalized_payload)?;
    let content_hash = hex::encode(sha2::Sha256::digest(&content_bytes));
    let id = Uuid::now_v7();

    sqlx::query(
        "INSERT INTO stage.observation
         (id, source_id, capture_id, entity_kind, aspect, natural_key,
          source_record_key, observed_at, event_time, normalized_payload,
          schema_version, quality_flags, content_hash)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$8,$9,'1.0','{}',$10)
         ON CONFLICT (source_id, entity_kind, aspect, source_record_key, content_hash)
         DO NOTHING",
    )
    .bind(id)
    .bind(source_id)
    .bind(capture_id)
    .bind(entity_kind)
    .bind(aspect)
    .bind(natural_key)
    .bind(source_record_key)
    .bind(observed_at)
    .bind(normalized_payload)
    .bind(&content_hash)
    .execute(pool)
    .await?;

    sqlx::query_scalar(
        "SELECT id FROM stage.observation
         WHERE source_id=$1 AND entity_kind=$2 AND aspect=$3
           AND source_record_key=$4 AND content_hash=$5",
    )
    .bind(source_id)
    .bind(entity_kind)
    .bind(aspect)
    .bind(source_record_key)
    .bind(content_hash)
    .fetch_one(pool)
    .await
    .context("megfigyelés azonosító")
}
