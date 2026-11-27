//! Meccs-szintű részletes odds-piacok (S194): a `fact.odds_quote` összes
//! piaca, forrással és rögzítési idővel. Read-only; az üres lista hiány,
//! nem kitalált érték. A kliens csak ezen a végponton keresztül látja a
//! Tipplap markets-blokkot — a fő meccs-válasz változatlan marad.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct MarketQuoteRow {
    pub market: String,
    pub outcome: String,
    pub line: f64,
    pub price: f64,
    pub bookmaker: String,
    pub phase: String,
    pub source_code: String,
    pub captured_at: DateTime<Utc>,
}

/// Egy (piac, kimenetel, vonal) pár legfrissebb rögzítése. Több forrás vagy
/// bookmaker esetén a legújabb `captured_at` nyer — a kliens nem talál ki
/// árat, és nem kever össze külön időpontokból származó szorzókat.
pub async fn fetch_quotes(pool: &PgPool, match_id: Uuid) -> Result<Vec<MarketQuoteRow>> {
    sqlx::query_as::<_, MarketQuoteRow>(
        "SELECT DISTINCT ON (q.market, q.outcome, q.line)
                q.market, q.outcome,
                q.line::float8 AS line, q.price::float8 AS price,
                q.bookmaker, q.phase,
                s.code AS source_code, o.captured_at
           FROM fact.odds_quote q
           JOIN fact.odds_snapshot o
             ON o.match_id = q.match_id AND o.source_id = q.source_id
           JOIN ops.source s ON s.id = q.source_id
          WHERE q.match_id = $1
          ORDER BY q.market, q.outcome, q.line, o.captured_at DESC",
    )
    .bind(match_id)
    .fetch_all(pool)
    .await
    .context("részletes odds-piacok lekérése")
}
