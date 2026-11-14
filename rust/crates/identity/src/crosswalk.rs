//! Crosswalk-egyértelműség ellenőrzés az S3-import előkészítéséhez.
//!
//! Egy forrás-entitás (left_source + left_external_id) csak egyetlen aktív
//! cél-kötéssel rendelkezhet; több aktív, eltérő célú kötés ambiguous és a
//! fail-closed linker szempontjából nem elfogadható.

use anyhow::{Context, Result};
use sqlx::{PgPool, Row};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrosswalkConflict {
    pub left_source: String,
    pub left_external_id: String,
    pub conflicting_right_external_ids: Vec<String>,
}

/// Az aktív crosswalk-kötések ambiguous sorai (egy forrás → több cél).
pub async fn crosswalk_conflicts(pool: &PgPool) -> Result<Vec<CrosswalkConflict>> {
    let rows = sqlx::query(
        "SELECT left_source, left_external_id,
                array_agg(DISTINCT right_external_id) AS rights
         FROM identity.crosswalk
         WHERE status = 'active'
         GROUP BY left_source, left_external_id
         HAVING count(DISTINCT right_external_id) > 1",
    )
    .fetch_all(pool)
    .await
    .context("crosswalk konfliktus lekérés")?;
    rows.iter()
        .map(|row| {
            Ok(CrosswalkConflict {
                left_source: row.try_get("left_source").context("left_source")?,
                left_external_id: row
                    .try_get("left_external_id")
                    .context("left_external_id")?,
                conflicting_right_external_ids: row.try_get("rights").context("rights")?,
            })
        })
        .collect()
}
