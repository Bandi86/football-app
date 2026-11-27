//! S211: a `core.match.round_label` visszajátszható pótlása research
//! obszervációkból. Csak olyan meccset ír, ahol a kanonikus érték jelenleg
//! NULL és a forrás egyértelmű, legfrissebb körét tudjuk adni.

use anyhow::{Context, Result};
use sqlx::PgPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundBackfill {
    pub candidates: i64,
    pub updated: i64,
}

const CANDIDATES: &str = "
    WITH candidates AS (
        SELECT DISTINCT ON (r.entity_id)
               r.entity_id AS match_id,
               o.normalized_payload->>'round_label' AS round
          FROM identity.source_entity_ref r
          JOIN stage.observation o
            ON o.source_id = r.source_id AND o.natural_key = r.source_entity_id
          JOIN ops.source s ON s.id = r.source_id
         WHERE r.entity_kind = 'match' AND r.status = 'active'
           AND s.code = 'research'
           AND o.entity_kind = 'match' AND o.aspect = 'match_core'
           AND o.normalized_payload->>'round_label' IS NOT NULL
           AND btrim(o.normalized_payload->>'round_label') <> ''
         ORDER BY r.entity_id, o.observed_at DESC
    )
    SELECT c.match_id, c.round FROM candidates c
      JOIN core.match m ON m.id = c.match_id
     WHERE m.round_label IS NULL
";

/// Csak számol — nem ír. A CLI ezt mutatja `--apply` nélkül.
pub async fn count_missing_round_labels(pool: &PgPool) -> Result<i64> {
    let sql = format!("SELECT count(*) FROM ({CANDIDATES}) q");
    sqlx::query_scalar::<_, i64>(&sql)
        .fetch_one(pool)
        .await
        .context("forduló-pótlás számlálása")
}

/// A hiányzó fordulók visszajátszható pótlása. Idempotens: második futás már
/// 0 sort ír. Csak akkor érnek el a mezőt, ha az épp NULL.
pub async fn backfill_round_labels(pool: &PgPool) -> Result<RoundBackfill> {
    let candidates = count_missing_round_labels(pool).await?;
    let sql = "WITH candidates AS (
            SELECT DISTINCT ON (r.entity_id)
                   r.entity_id AS match_id,
                   o.normalized_payload->>'round_label' AS round
              FROM identity.source_entity_ref r
              JOIN stage.observation o
                ON o.source_id = r.source_id AND o.natural_key = r.source_entity_id
              JOIN ops.source s ON s.id = r.source_id
             WHERE r.entity_kind = 'match' AND r.status = 'active'
               AND s.code = 'research'
               AND o.entity_kind = 'match' AND o.aspect = 'match_core'
               AND o.normalized_payload->>'round_label' IS NOT NULL
               AND btrim(o.normalized_payload->>'round_label') <> ''
             ORDER BY r.entity_id, o.observed_at DESC
         )
         UPDATE core.match m SET round_label = c.round, updated_at = now()
           FROM candidates c
          WHERE m.id = c.match_id AND m.round_label IS NULL";
    let result = sqlx::query(sql)
        .execute(pool)
        .await
        .context("forduló-pótlás végrehajtása")?;
    Ok(RoundBackfill {
        candidates,
        updated: result.rows_affected() as i64,
    })
}
