//! Globális `ops.audit_event` feed (S3). A meccs-szintű változat a `history`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, query_builder::QueryBuilder};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct AuditEntryRow {
    pub id: Uuid,
    pub actor: String,
    pub action: String,
    pub subject_kind: String,
    pub subject_id: Option<Uuid>,
    pub detail: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct AuditFilter {
    pub limit: i64,
    pub subject_kind: Option<String>,
    pub subject_id: Option<Uuid>,
}

pub async fn list(pool: &PgPool, filter: &AuditFilter) -> Result<Vec<AuditEntryRow>, sqlx::Error> {
    let limit = filter.limit.clamp(1, 200);
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT id, actor, action, subject_kind, subject_id, detail, created_at
           FROM ops.audit_event WHERE true",
    );
    if let Some(kind) = &filter.subject_kind {
        query.push(" AND subject_kind = ").push_bind(kind);
    }
    if let Some(id) = filter.subject_id {
        query.push(" AND subject_id = ").push_bind(id);
    }
    query
        .push(" ORDER BY created_at DESC, id LIMIT ")
        .push_bind(limit);
    query
        .build_query_as::<AuditEntryRow>()
        .fetch_all(pool)
        .await
}
