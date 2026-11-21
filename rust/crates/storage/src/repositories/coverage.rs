//! Napi lefedettségi riport (S64): naponta hány meccs van, mennyihez van
//! eredmény, odds, mindkét csapatlogó, kezdésidő, illetve mennyi van
//! visszatartva / verified állapotban. A napi „Hiányzó adatok" panel
//! aggregált párja, hogy a teljes időszakot egy képben lehessen látni.

use anyhow::Result;
use chrono::NaiveDate;
use sqlx::PgPool;

/// Egy nap lefedettségi sorai.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayCoverage {
    pub day: NaiveDate,
    pub matches: i64,
    pub finished: i64,
    pub with_result: i64,
    pub with_odds: i64,
    pub with_both_logos: i64,
    pub missing_kickoff: i64,
    pub withheld: i64,
    pub verified: i64,
}

pub async fn fetch_daily_coverage(
    pool: &PgPool,
    from: NaiveDate,
    to: NaiveDate,
    timezone: &str,
) -> Result<Vec<DayCoverage>> {
    type CoverageTuple = (NaiveDate, i64, i64, i64, i64, i64, i64, i64, i64);
    let rows: Vec<CoverageTuple> = sqlx::query_as(
        r#"
        WITH days AS (
            SELECT generate_series($1::date, $2::date, interval '1 day')::date AS day
        ),
        day_matches AS (
            SELECT d.day,
                   m.id,
                   m.status,
                   m.kickoff_precision,
                   g.state,
                   (r.match_id IS NOT NULL) AS has_result,
                   EXISTS (
                       SELECT 1 FROM fact.odds_snapshot o WHERE o.match_id = m.id
                   ) AS has_odds,
                   EXISTS (
                       SELECT 1 FROM ops.asset a
                       WHERE a.entity_kind = 'team' AND a.kind = 'logo'
                         AND a.entity_id = m.home_team_id
                   ) AND EXISTS (
                       SELECT 1 FROM ops.asset a
                       WHERE a.entity_kind = 'team' AND a.kind = 'logo'
                         AND a.entity_id = m.away_team_id
                   ) AS both_logos
            FROM days d
            LEFT JOIN core.match m ON timezone($3, m.kickoff_at)::date = d.day
            LEFT JOIN core.match_result r ON r.match_id = m.id
            LEFT JOIN ops.publish_gate g ON g.match_id = m.id
        )
        SELECT day,
               count(id)::bigint,
               count(*) FILTER (WHERE status IN ('finished','awarded'))::bigint,
               count(*) FILTER (WHERE has_result)::bigint,
               count(*) FILTER (WHERE has_odds)::bigint,
               count(*) FILTER (WHERE both_logos)::bigint,
               count(*) FILTER (WHERE kickoff_precision = 'date_only')::bigint,
               count(*) FILTER (WHERE state = 'withheld' OR state = 'quarantined')::bigint,
               count(*) FILTER (WHERE state = 'verified_publish')::bigint
        FROM day_matches
        GROUP BY day
        ORDER BY day
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(timezone)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(
                day,
                matches,
                finished,
                with_result,
                with_odds,
                both,
                kickoff,
                withheld,
                verified,
            )| {
                DayCoverage {
                    day,
                    matches,
                    finished,
                    with_result,
                    with_odds,
                    with_both_logos: both,
                    missing_kickoff: kickoff,
                    withheld,
                    verified,
                }
            },
        )
        .collect())
}
