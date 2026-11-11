use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DayFilter {
    pub date: NaiveDate,
    pub timezone: String,
    pub include_all: bool,
}

#[derive(Debug, Clone, FromRow, serde::Serialize)]
pub struct DayMatchRow {
    pub match_id: Uuid,
    pub kickoff_at: DateTime<Utc>,
    pub status: String,
    pub home_name: String,
    pub away_name: String,
    pub home_score: Option<i16>,
    pub away_score: Option<i16>,
    pub publish_state: String,
    pub blockers: serde_json::Value,
    pub competition_id: Uuid,
    pub competition_name: String,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub tier: i16,
    pub priority: i32,
    pub home_ht: Option<i16>,
    pub away_ht: Option<i16>,
    pub home_et: Option<i16>,
    pub away_et: Option<i16>,
    pub home_pens: Option<i16>,
    pub away_pens: Option<i16>,
    pub home_position: Option<i16>,
    pub away_position: Option<i16>,
    pub odds: Option<serde_json::Value>,
    pub market_count: Option<i32>,
    pub odds_source: Option<String>,
    pub odds_captured_at: Option<DateTime<Utc>>,
}

pub async fn fetch_day(pool: &PgPool, filter: &DayFilter) -> Result<Vec<DayMatchRow>> {
    let rows = sqlx::query_as::<_, DayMatchRow>(
        "SELECT
            m.id AS match_id,
            m.kickoff_at,
            m.status,
            home.name AS home_name,
            away.name AS away_name,
            r.home_ft AS home_score,
            r.away_ft AS away_score,
            g.state AS publish_state,
            g.blockers,
            c.id AS competition_id,
            c.name AS competition_name,
            co.code AS country_code,
            co.name AS country_name,
            c.tier,
            c.priority,
            r.home_ht, r.away_ht, r.home_et, r.away_et, r.home_pens, r.away_pens,
            sh.position AS home_position,
            sa.position AS away_position,
            o.odds,
            o.market_count,
            os.display_name AS odds_source,
            o.captured_at AS odds_captured_at
         FROM core.match m
         JOIN core.team home ON home.id = m.home_team_id
         JOIN core.team away ON away.id = m.away_team_id
         JOIN core.competition c ON c.id = m.competition_id
         LEFT JOIN core.country co ON co.id = c.country_id
         LEFT JOIN core.match_result r ON r.match_id = m.id
         LEFT JOIN core.standing sh ON sh.season_id = m.season_id AND sh.team_id = m.home_team_id
         LEFT JOIN core.standing sa ON sa.season_id = m.season_id AND sa.team_id = m.away_team_id
         LEFT JOIN fact.odds_snapshot o ON o.match_id = m.id
         LEFT JOIN ops.source os ON os.id = o.source_id
         JOIN ops.publish_gate g ON g.match_id = m.id AND g.publish_date = $1
         WHERE (m.kickoff_at AT TIME ZONE $2)::date = $1
           AND ($3::boolean OR g.state = 'verified_publish')
         ORDER BY m.kickoff_at, home.name, away.name",
    )
    .bind(filter.date)
    .bind(&filter.timezone)
    .bind(filter.include_all)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
