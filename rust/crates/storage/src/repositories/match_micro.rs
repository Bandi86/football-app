//! Meccs-szintű mikro-adatok (S182/S3): felállás, játékosstatisztika, lövések.
//!
//! A `0009_s2d2_match_micro.sql` tábláiból olvas; minden sor viszi a forrást
//! és a rögzítési időt. Hiány esetén üres lista, nem kitalált érték. A
//! `source_player_id` és a feloldott `player_entity_id` egyaránt megjelenik:
//! feloldatlan név esetén a kliens nem talál ki entitást.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

/// Olcsó létezés-ellenőrzés: az üres adat és a nem létező meccs nem keveredik.
pub async fn exists(pool: &PgPool, match_id: Uuid) -> Result<bool> {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM core.match WHERE id = $1)")
        .bind(match_id)
        .fetch_one(pool)
        .await
        .context("meccs létezésének ellenőrzése")
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct LineupRow {
    pub side: String,
    pub role: String,
    pub source_player_id: String,
    pub player_entity_id: Option<Uuid>,
    pub player_name: Option<String>,
    pub position: Option<String>,
    pub shirt_number: Option<i32>,
    pub rating: Option<f64>,
    pub captured_at: DateTime<Utc>,
}

/// A felállás research-első, majd legfrissebb rögzítés szerint; egy (side,
/// source_player_id) párhoz csak a legjobb forrás sora kerül be.
pub async fn fetch_lineups(pool: &PgPool, match_id: Uuid) -> Result<Vec<LineupRow>> {
    sqlx::query_as::<_, LineupRow>(
        "SELECT DISTINCT ON (side, source_player_id)
                l.side, l.role, l.source_player_id, l.player_entity_id,
                e.canonical_name AS player_name, l.position, l.shirt_number,
                l.rating, l.captured_at
           FROM fact.match_lineups l
           LEFT JOIN core.entity e ON e.id = l.player_entity_id
          WHERE l.match_id = $1
          ORDER BY side, source_player_id,
                   (SELECT code = 'research' FROM ops.source s WHERE s.id = l.source_id) DESC,
                   l.captured_at DESC",
    )
    .bind(match_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct PlayerStatRow {
    pub side: String,
    pub source_player_id: String,
    pub player_entity_id: Option<Uuid>,
    pub player_name: Option<String>,
    pub stat_key: String,
    pub value: Option<f64>,
    pub captured_at: DateTime<Utc>,
}

/// Játékosszintű statisztika: research-első, majd legfrissebb; egy
/// (source_player_id, stat_key) párhoz egy érték.
pub async fn fetch_player_stats(pool: &PgPool, match_id: Uuid) -> Result<Vec<PlayerStatRow>> {
    sqlx::query_as::<_, PlayerStatRow>(
        "SELECT DISTINCT ON (source_player_id, stat_key)
                p.side, p.source_player_id, p.player_entity_id,
                e.canonical_name AS player_name, p.stat_key, p.value, p.captured_at
           FROM fact.match_player_stats p
           LEFT JOIN core.entity e ON e.id = p.player_entity_id
          WHERE p.match_id = $1
          ORDER BY source_player_id, stat_key,
                   (SELECT code = 'research' FROM ops.source s WHERE s.id = p.source_id) DESC,
                   p.captured_at DESC, side",
    )
    .bind(match_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct ShotRow {
    pub shot_no: i32,
    pub side: String,
    pub source_player_id: Option<String>,
    pub player_entity_id: Option<Uuid>,
    pub player_name: Option<String>,
    pub minute: Option<i32>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub xg: Option<f64>,
    pub xgot: Option<f64>,
    pub shot_type: Option<String>,
    pub captured_at: DateTime<Utc>,
}

/// Lövések: research-első, majd legfrissebb; shot_no szerint rendezve.
pub async fn fetch_shots(pool: &PgPool, match_id: Uuid) -> Result<Vec<ShotRow>> {
    sqlx::query_as::<_, ShotRow>(
        "SELECT DISTINCT ON (shot_no)
                s.shot_no, s.side, s.source_player_id, s.player_entity_id,
                e.canonical_name AS player_name, s.minute, s.x, s.y, s.xg,
                s.xgot, s.shot_type, s.captured_at
           FROM fact.match_shots s
           LEFT JOIN core.entity e ON e.id = s.player_entity_id
          WHERE s.match_id = $1
          ORDER BY shot_no,
                   (SELECT code = 'research' FROM ops.source src WHERE src.id = s.source_id) DESC,
                   s.captured_at DESC",
    )
    .bind(match_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}
