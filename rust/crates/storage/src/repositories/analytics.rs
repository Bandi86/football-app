//! Predikciós analytics (S196): as-of piaci minták és mérési snapshotok.
//!
//! A nyers odds és eredmények olvasása a kanonikus táblákból történik; a
//! `football-prediction` tiszta modulja végzi a Shin-devigelést és a metrikákat.
//! A mérési sor csak `--apply` esetén íródik (auditált, idempotens upsert).

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use utoipa::ToSchema;
use uuid::Uuid;

/// Egy lezárt meccs as-of 1X2 piaca + tényleges eredménye. A három szorzót
/// a kickoff ELŐTT rögzített snapshotokból választjuk (legfrissebb).
#[derive(Debug, Clone, FromRow)]
pub struct MarketOddsSample {
    pub match_id: Uuid,
    pub competition_id: Uuid,
    pub kickoff_at: DateTime<Utc>,
    pub home_ft: i16,
    pub away_ft: i16,
    pub home_odds: f64,
    pub draw_odds: f64,
    pub away_odds: f64,
    pub captured_at: DateTime<Utc>,
}

pub async fn market_odds_samples(
    pool: &PgPool,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<MarketOddsSample>> {
    sqlx::query_as::<_, MarketOddsSample>(
        "SELECT m.id AS match_id, m.competition_id, m.kickoff_at,
                r.home_ft, r.away_ft,
                h.price::float8 AS home_odds,
                d.price::float8 AS draw_odds,
                a.price::float8 AS away_odds,
                GREATEST(h.captured_at, d.captured_at, a.captured_at) AS captured_at
           FROM core.match m
           JOIN core.match_result r ON r.match_id = m.id
           JOIN LATERAL (
             SELECT q.price, o.captured_at
               FROM fact.odds_quote q
               JOIN fact.odds_snapshot o
                 ON o.match_id = q.match_id AND o.source_id = q.source_id
              WHERE q.match_id = m.id AND q.market = '1x2' AND q.outcome = 'home'
                AND o.captured_at < m.kickoff_at
              ORDER BY o.captured_at DESC LIMIT 1
           ) h ON true
           JOIN LATERAL (
             SELECT q.price, o.captured_at
               FROM fact.odds_quote q
               JOIN fact.odds_snapshot o
                 ON o.match_id = q.match_id AND o.source_id = q.source_id
              WHERE q.match_id = m.id AND q.market = '1x2' AND q.outcome = 'draw'
                AND o.captured_at < m.kickoff_at
              ORDER BY o.captured_at DESC LIMIT 1
           ) d ON true
           JOIN LATERAL (
             SELECT q.price, o.captured_at
               FROM fact.odds_quote q
               JOIN fact.odds_snapshot o
                 ON o.match_id = q.match_id AND o.source_id = q.source_id
              WHERE q.match_id = m.id AND q.market = '1x2' AND q.outcome = 'away'
                AND o.captured_at < m.kickoff_at
              ORDER BY o.captured_at DESC LIMIT 1
           ) a ON true
          WHERE m.status = 'finished'
            AND m.kickoff_at::date BETWEEN $1 AND $2
          ORDER BY m.kickoff_at",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await
    .context("as-of piaci minták lekérése")
}

/// Egy predikcióhoz használt, már lezárt meccs (as-of: kickoff < cutoff).
#[derive(Debug, Clone, FromRow)]
pub struct ScoredMatchRow {
    pub kickoff_at: DateTime<Utc>,
    pub home: Uuid,
    pub away: Uuid,
    pub home_goals: f64,
    pub away_goals: f64,
}

pub async fn scored_matches_before(
    pool: &PgPool,
    cutoff: DateTime<Utc>,
    days_back: i64,
) -> Result<Vec<ScoredMatchRow>> {
    sqlx::query_as::<_, ScoredMatchRow>(
        "SELECT m.kickoff_at, m.home_team_id AS home, m.away_team_id AS away,
                r.home_ft::float8 AS home_goals, r.away_ft::float8 AS away_goals
           FROM core.match m
           JOIN core.match_result r ON r.match_id = m.id
          WHERE m.status = 'finished'
            AND m.kickoff_at < $1
            AND m.kickoff_at >= $1 - ($2::int) * interval '1 day'
          ORDER BY m.kickoff_at",
    )
    .bind(cutoff)
    .bind(days_back)
    .fetch_all(pool)
    .await
    .context("as-of eredmények lekérése")
}

#[derive(Debug, Clone, FromRow)]
pub struct UpcomingMatchRow {
    pub id: Uuid,
    pub home: Uuid,
    pub away: Uuid,
    pub kickoff_at: DateTime<Utc>,
}

pub async fn upcoming_matches(
    pool: &PgPool,
    date: NaiveDate,
    after: DateTime<Utc>,
) -> Result<Vec<UpcomingMatchRow>> {
    sqlx::query_as::<_, UpcomingMatchRow>(
        "SELECT id, home_team_id AS home, away_team_id AS away, kickoff_at
           FROM core.match
          WHERE kickoff_at::date = $1 AND kickoff_at > $2
          ORDER BY kickoff_at",
    )
    .bind(date)
    .bind(after)
    .fetch_all(pool)
    .await
    .context("jövőbeli meccsek lekérése")
}

#[derive(Debug, Clone)]
pub struct NewPrediction {
    pub match_id: Uuid,
    pub model_version: String,
    pub outcomes: Vec<(&'static str, f64, f64, f64)>, // (outcome, probability, fair_odds, edge)
    pub quality: &'static str,
    pub inputs_hash: String,
    pub as_of: DateTime<Utc>,
}

pub async fn upsert_prediction(pool: &PgPool, row: &NewPrediction) -> Result<usize> {
    let mut written = 0usize;
    for (outcome, probability, fair_odds, edge) in &row.outcomes {
        let res = sqlx::query(
            "INSERT INTO analytics.prediction
             (id, match_id, model_version, market, outcome, line, probability,
              fair_odds, edge, quality, inputs_hash, as_of)
             VALUES ($1,$2,$3,'1x2',$4,0,$5,$6,$7,$8,$9,$10)
             ON CONFLICT (match_id, model_version, market, outcome, line)
             DO UPDATE SET probability=EXCLUDED.probability,
               fair_odds=EXCLUDED.fair_odds, edge=EXCLUDED.edge,
               quality=EXCLUDED.quality, inputs_hash=EXCLUDED.inputs_hash,
               as_of=EXCLUDED.as_of, created_at=now()",
        )
        .bind(Uuid::now_v7())
        .bind(row.match_id)
        .bind(row.model_version.clone())
        .bind(*outcome)
        .bind(*probability)
        .bind(*fair_odds)
        .bind(*edge)
        .bind(row.quality)
        .bind(row.inputs_hash.clone())
        .bind(row.as_of)
        .execute(pool)
        .await
        .context("predikció írása")?;
        written += res.rows_affected() as usize;
    }
    Ok(written)
}

/// Egy rögzített predikció és a tényleges eredmény a méréshez.
#[derive(Debug, Clone, FromRow)]
pub struct PredictionSampleRow {
    pub match_id: Uuid,
    pub competition_id: Uuid,
    pub home_prob: f64,
    pub draw_prob: f64,
    pub away_prob: f64,
    pub home_ft: i16,
    pub away_ft: i16,
}

pub async fn predicted_outcomes(
    pool: &PgPool,
    model_version: &str,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<PredictionSampleRow>> {
    sqlx::query_as::<_, PredictionSampleRow>(
        "SELECT p.match_id, m.competition_id,
                max(p.probability) FILTER (WHERE p.outcome = 'home')::float8 AS home_prob,
                max(p.probability) FILTER (WHERE p.outcome = 'draw')::float8 AS draw_prob,
                max(p.probability) FILTER (WHERE p.outcome = 'away')::float8 AS away_prob,
                r.home_ft, r.away_ft
           FROM analytics.prediction p
           JOIN core.match m ON m.id = p.match_id
           JOIN core.match_result r ON r.match_id = m.id
          WHERE p.model_version = $1
            AND m.kickoff_at::date BETWEEN $2 AND $3
          GROUP BY p.match_id, m.competition_id, r.home_ft, r.away_ft
          HAVING count(*) FILTER (WHERE p.outcome IN ('home','draw','away')) = 3
          ORDER BY p.match_id",
    )
    .bind(model_version)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await
    .context("predikciók és eredmények lekérése")
}

#[derive(Debug, Clone)]
pub struct NewMeasurement {
    pub model_version: String,
    pub date_from: NaiveDate,
    pub date_to: NaiveDate,
    pub market: &'static str,
    pub sample_size: i32,
    pub brier: f64,
    pub brier_baseline: f64,
    pub brier_market: f64,
    pub difference: f64,
    pub ci_low: f64,
    pub ci_high: f64,
    pub log_loss: f64,
    pub wilson_low: f64,
    pub wilson_high: f64,
    pub inputs_hash: String,
}

pub async fn upsert_measurement(pool: &PgPool, row: &NewMeasurement) -> Result<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO analytics.measurement
         (id, model_version, date_from, date_to, market, sample_size,
          brier, brier_baseline, brier_market, difference, ci_low, ci_high,
          log_loss, wilson_low, wilson_high, inputs_hash)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
         ON CONFLICT (model_version, date_from, date_to, market)
         DO UPDATE SET sample_size=EXCLUDED.sample_size, brier=EXCLUDED.brier,
           brier_baseline=EXCLUDED.brier_baseline, brier_market=EXCLUDED.brier_market,
           difference=EXCLUDED.difference, ci_low=EXCLUDED.ci_low, ci_high=EXCLUDED.ci_high,
           log_loss=EXCLUDED.log_loss, wilson_low=EXCLUDED.wilson_low,
           wilson_high=EXCLUDED.wilson_high, inputs_hash=EXCLUDED.inputs_hash,
           created_at=now()
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(row.model_version.clone())
    .bind(row.date_from)
    .bind(row.date_to)
    .bind(row.market)
    .bind(row.sample_size)
    .bind(row.brier)
    .bind(row.brier_baseline)
    .bind(row.brier_market)
    .bind(row.difference)
    .bind(row.ci_low)
    .bind(row.ci_high)
    .bind(row.log_loss)
    .bind(row.wilson_low)
    .bind(row.wilson_high)
    .bind(row.inputs_hash.clone())
    .fetch_one(pool)
    .await
    .context("mérési snapshot írása")
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, FromRow)]
pub struct MeasurementRow {
    pub model_version: String,
    pub date_from: NaiveDate,
    pub date_to: NaiveDate,
    pub market: String,
    pub sample_size: i32,
    pub brier: f64,
    pub brier_baseline: f64,
    pub difference: f64,
    pub ci_low: f64,
    pub ci_high: f64,
    pub log_loss: f64,
    pub inputs_hash: String,
    pub created_at: DateTime<Utc>,
}

pub async fn list_measurements(pool: &PgPool, limit: i64) -> Result<Vec<MeasurementRow>> {
    sqlx::query_as::<_, MeasurementRow>(
        "SELECT model_version, date_from, date_to, market, sample_size,
                brier, brier_baseline, difference, ci_low, ci_high,
                log_loss, inputs_hash, created_at
           FROM analytics.measurement
          ORDER BY created_at DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("mérési snapshotok lekérése")
}
