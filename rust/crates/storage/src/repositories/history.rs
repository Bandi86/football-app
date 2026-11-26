//! Egy meccs mező-szintű változástörténete: az audit-események (`before`/`after`
//! a detail jsonb-ben) és a kiválasztott provenance-mezők egységes időrendje.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct HistoryEntry {
    pub at: DateTime<Utc>,
    pub origin: String,
    pub actor: String,
    pub action: String,
    pub field_key: Option<String>,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub source: Option<String>,
    pub detail: Value,
}

#[derive(Debug, FromRow)]
struct AuditRow {
    at: DateTime<Utc>,
    actor: String,
    action: String,
    detail: Value,
}

#[derive(Debug, FromRow)]
struct ProvenanceRow {
    at: DateTime<Utc>,
    field_key: String,
    source: String,
}

/// A legfrissebb események elöl; a két forrás együtt, időrendben.
pub async fn list_for_match(
    pool: &PgPool,
    match_id: Uuid,
    limit: i64,
) -> Result<Vec<HistoryEntry>, sqlx::Error> {
    let audit = sqlx::query_as::<_, AuditRow>(
        "SELECT created_at AS at, actor, action, detail
           FROM ops.audit_event
          WHERE subject_kind = 'match' AND subject_id = $1
          ORDER BY created_at DESC LIMIT $2",
    )
    .bind(match_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let provenance = sqlx::query_as::<_, ProvenanceRow>(
        "SELECT p.valid_from AS at, p.field_key, s.code AS source
           FROM fact.provenance p
           JOIN stage.observation o ON o.id = p.observation_id
           JOIN ops.source s ON s.id = o.source_id
          WHERE p.subject_kind = 'match' AND p.subject_id = $1 AND p.is_selected
          ORDER BY p.valid_from DESC LIMIT $2",
    )
    .bind(match_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut entries: Vec<HistoryEntry> = audit
        .into_iter()
        .map(|row| HistoryEntry {
            at: row.at,
            origin: "audit".to_owned(),
            actor: row.actor,
            action: row.action,
            field_key: None,
            before: row.detail.get("before").filter(|v| !v.is_null()).cloned(),
            after: row.detail.get("after").filter(|v| !v.is_null()).cloned(),
            source: None,
            detail: row.detail,
        })
        .chain(provenance.into_iter().map(|row| HistoryEntry {
            at: row.at,
            origin: "provenance".to_owned(),
            actor: "forrás".to_owned(),
            action: "mező kiválasztás".to_owned(),
            field_key: Some(row.field_key.clone()),
            before: None,
            after: None,
            source: Some(row.source.clone()),
            detail: json!({ "field_key": row.field_key }),
        }))
        .collect();

    entries.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| b.origin.cmp(&a.origin)));
    entries.truncate(limit.max(0) as usize);
    Ok(entries)
}
