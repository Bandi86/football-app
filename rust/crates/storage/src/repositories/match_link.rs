//! A research forráslinkek tárolása és olvasása (S181/S2).
//!
//! Csak a forrásból származó http(s) URL-ek kerülnek be; a hiány üres lista,
//! nem kitalált link. Minden sor viszi a forrást és a rögzítési időt.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Executor, PgPool, Postgres};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct MatchLinkRow {
    pub kind: String,
    pub label: String,
    pub url: String,
    pub captured_at: DateTime<Utc>,
}

/// Idempotens upsert egyetlen lekérdezéssel: az azonos (match, kind, url,
/// source) sor nem duplikálódik. A `jsonb_array_elements` miatt mind a pool,
/// mind a tranzakció executorral hívható, és a bejövő sorok száma a eredmény.
pub async fn upsert<'e, E>(
    executor: E,
    match_id: Uuid,
    source_id: Uuid,
    captured_at: DateTime<Utc>,
    links: &serde_json::Value,
) -> Result<usize>
where
    E: Executor<'e, Database = Postgres>,
{
    let Some(items) = links.as_array() else {
        return Ok(0);
    };
    let valid: Vec<&serde_json::Value> = items
        .iter()
        .filter(|item| {
            item.get("url")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|url| url.starts_with("https://") || url.starts_with("http://"))
        })
        .collect();
    if valid.is_empty() {
        return Ok(0);
    }
    let payload = serde_json::Value::Array(valid.into_iter().cloned().collect());
    let result = sqlx::query(
        "INSERT INTO fact.match_link (id, match_id, kind, label, url, source_id, captured_at)
         SELECT gen_random_uuid(), $1,
                item->>'kind', item->>'label', item->>'url', $2, $3
           FROM jsonb_array_elements($4::jsonb) AS item
          WHERE item->>'kind' IN ('source','reference','standing','weather','other')
         ON CONFLICT (match_id, kind, url, source_id) DO UPDATE SET
           label=EXCLUDED.label, captured_at=EXCLUDED.captured_at",
    )
    .bind(match_id)
    .bind(source_id)
    .bind(captured_at)
    .bind(payload)
    .execute(executor)
    .await?;
    Ok(result.rows_affected() as usize)
}

/// A meccs forráslinkjei kind szerint, majd rögzítési idő szerint rendezve.
pub async fn fetch(pool: &PgPool, match_id: Uuid) -> Result<Vec<MatchLinkRow>> {
    sqlx::query_as::<_, MatchLinkRow>(
        "SELECT kind, label, url, captured_at
           FROM fact.match_link
          WHERE match_id = $1
          ORDER BY kind, captured_at DESC, url",
    )
    .bind(match_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}
