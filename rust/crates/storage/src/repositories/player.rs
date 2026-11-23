//! Játékos-read model a már betöltött kanonikus játékosadatokhoz.
//!
//! Csak olvasás: a név, ország, tagság, fellépések, statisztikák és események
//! mind a `core.entity` / `fact.*` táblákból jönnek, forrás- és rögzítési
//! idővel. A hiányzó adat `None`, nem kitalált érték.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PlayerDetailRow {
    pub id: Uuid,
    pub name: String,
    pub country_code: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PlayerSpellRow {
    pub team_name: String,
    pub valid_from: chrono::NaiveDate,
    pub valid_to: Option<chrono::NaiveDate>,
    pub loan: bool,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PlayerAppearanceRow {
    pub match_id: Uuid,
    pub kickoff_at: DateTime<Utc>,
    pub status: String,
    pub competition_name: Option<String>,
    pub side: Option<String>,
    pub role: String,
    pub rating: Option<f64>,
    pub captured_at: DateTime<Utc>,
    pub source_code: String,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PlayerStatRow {
    pub match_id: Uuid,
    pub stat_key: String,
    pub value: Option<f64>,
    pub captured_at: DateTime<Utc>,
    pub source_code: String,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct PlayerIncidentRow {
    pub match_id: Uuid,
    pub incident_type: String,
    pub minute: Option<i32>,
    pub player_name: Option<String>,
    pub related_player_name: Option<String>,
    pub captured_at: DateTime<Utc>,
}

pub async fn fetch_player(pool: &PgPool, id: Uuid) -> Result<Option<PlayerDetailRow>> {
    sqlx::query_as::<_, PlayerDetailRow>(
        "SELECT e.id, e.canonical_name AS name, c.code AS country_code
           FROM core.entity e
           JOIN core.player p ON p.entity_id = e.id
           LEFT JOIN core.country c ON c.id = p.country_id
          WHERE e.id = $1 AND e.kind = 'player' AND e.merged_into_id IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

pub async fn fetch_spells(pool: &PgPool, id: Uuid, limit: i64) -> Result<Vec<PlayerSpellRow>> {
    sqlx::query_as::<_, PlayerSpellRow>(
        "SELECT t.canonical_name AS team_name, s.valid_from, s.valid_to, s.loan
           FROM core.player_team_spell s
           JOIN core.entity t ON t.id = s.team_entity_id
          WHERE s.player_entity_id = $1
          ORDER BY s.valid_from DESC, t.canonical_name
          LIMIT $2",
    )
    .bind(id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn fetch_appearances(
    pool: &PgPool,
    id: Uuid,
    limit: i64,
) -> Result<Vec<PlayerAppearanceRow>> {
    sqlx::query_as::<_, PlayerAppearanceRow>(
        "SELECT l.match_id, m.kickoff_at, m.status, comp.name AS competition_name,
                l.side, l.role, l.rating::double precision AS rating,
                l.captured_at, src.code AS source_code
           FROM fact.match_lineups l
           JOIN core.match m ON m.id = l.match_id
           JOIN ops.source src ON src.id = l.source_id
           LEFT JOIN core.competition comp ON comp.id = m.competition_id
          WHERE l.player_entity_id = $1
          ORDER BY m.kickoff_at DESC
          LIMIT $2",
    )
    .bind(id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

pub async fn fetch_stats(pool: &PgPool, id: Uuid, limit: i64) -> Result<Vec<PlayerStatRow>> {
    sqlx::query_as::<_, PlayerStatRow>(
        "SELECT s.match_id, s.stat_key, s.value::double precision AS value,
                s.captured_at, src.code AS source_code
           FROM fact.match_player_stats s
           JOIN ops.source src ON src.id = s.source_id
          WHERE s.player_entity_id = $1
          ORDER BY s.captured_at DESC, s.match_id, s.stat_key
          LIMIT $2",
    )
    .bind(id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

/// Az események a stabil forrás-ID-n (`fact.match_incidents.player_source_id`
/// ↔ `identity.source_entity_ref.source_entity_id = 'fm:' || id`) kötődnek
/// a játékosentitáshoz — név-alapú találgatás nélkül.
pub async fn fetch_incidents(
    pool: &PgPool,
    id: Uuid,
    limit: i64,
) -> Result<Vec<PlayerIncidentRow>> {
    sqlx::query_as::<_, PlayerIncidentRow>(
        "SELECT i.match_id, i.incident_type, i.minute, i.player_name,
                i.related_player_name, i.captured_at
           FROM fact.match_incidents i
           JOIN identity.source_entity_ref r
             ON r.source_id = i.source_id
            AND r.entity_kind = 'player'
            AND r.entity_id = $1
            AND r.source_entity_id = 'fm:' || i.player_source_id
          WHERE r.status = 'active'
          ORDER BY i.captured_at DESC
          LIMIT $2",
    )
    .bind(id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}
