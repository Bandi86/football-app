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
            g.blockers
         FROM core.match m
         JOIN core.team home ON home.id = m.home_team_id
         JOIN core.team away ON away.id = m.away_team_id
         LEFT JOIN core.match_result r ON r.match_id = m.id
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
