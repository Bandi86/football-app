//! Egy meccshez írt, ügynök és felhasználó közös jegyzetei.
//! A jegyzet nem írja felül a kanonikus adatot, nem vált ki publish gate-et.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, query_builder::QueryBuilder};
use uuid::Uuid;

use football_domain::ops::NoteKind;

#[derive(Debug, Clone, FromRow, Serialize, utoipa::ToSchema)]
pub struct MatchNoteRow {
    pub id: Uuid,
    pub match_id: Uuid,
    pub author: String,
    pub body: String,
    pub kind: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

pub async fn list_for_match(
    pool: &PgPool,
    match_id: Uuid,
) -> Result<Vec<MatchNoteRow>, sqlx::Error> {
    sqlx::query_as::<_, MatchNoteRow>(
        "SELECT id, match_id, author, body, kind, created_at, updated_at, resolved_at
           FROM ops.match_note WHERE match_id = $1
          ORDER BY created_at DESC, id",
    )
    .bind(match_id)
    .fetch_all(pool)
    .await
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<MatchNoteRow>, sqlx::Error> {
    sqlx::query_as::<_, MatchNoteRow>(
        "SELECT id, match_id, author, body, kind, created_at, updated_at, resolved_at
           FROM ops.match_note WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn insert(
    pool: &PgPool,
    match_id: Uuid,
    author: &str,
    body: &str,
    kind: NoteKind,
) -> Result<MatchNoteRow, sqlx::Error> {
    let id = Uuid::now_v7();
    sqlx::query_as::<_, MatchNoteRow>(
        "INSERT INTO ops.match_note (id, match_id, author, body, kind)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, match_id, author, body, kind, created_at, updated_at, resolved_at",
    )
    .bind(id)
    .bind(match_id)
    .bind(author)
    .bind(body)
    .bind(kind.as_db_str())
    .fetch_one(pool)
    .await
}

/// Csak a megadott mezők változnak; a többi ÉRTÉK (például az author)
/// változatlan marad. A hiányos update nem írja vissza a régit sehova.
#[derive(Debug, Clone, Default)]
pub struct NotePatch {
    pub body: Option<String>,
    pub kind: Option<NoteKind>,
    /// `Some(true)` → lezárás, `Some(false)` → újranyitás, `None` → változatlan.
    pub resolved: Option<bool>,
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    patch: &NotePatch,
) -> Result<Option<MatchNoteRow>, sqlx::Error> {
    if patch.body.is_none() && patch.kind.is_none() && patch.resolved.is_none() {
        return get(pool, id).await;
    }
    let mut query = QueryBuilder::<Postgres>::new("UPDATE ops.match_note SET updated_at = now()");
    if let Some(body) = &patch.body {
        query.push(", body = ").push_bind(body);
    }
    if let Some(kind) = patch.kind {
        query.push(", kind = ").push_bind(kind.as_db_str());
    }
    if let Some(resolved) = patch.resolved {
        if resolved {
            query.push(", resolved_at = now()");
        } else {
            query.push(", resolved_at = NULL");
        }
    }
    query
        .push(" WHERE id = ")
        .push_bind(id)
        .push(" RETURNING id, match_id, author, body, kind, created_at, updated_at, resolved_at");
    query
        .build_query_as::<MatchNoteRow>()
        .fetch_optional(pool)
        .await
}
