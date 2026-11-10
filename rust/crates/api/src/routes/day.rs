use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use chrono::NaiveDate;
use football_storage::repositories::day::{DayFilter, fetch_day};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::{Scope, authorize};
use crate::error::AppError;

#[derive(Debug, Deserialize)]
pub struct DayQuery {
    pub include: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DayResponse {
    pub date: NaiveDate,
    pub timezone: &'static str,
    pub matches: Vec<DayMatch>,
}

#[derive(Debug, Serialize)]
pub struct DayMatch {
    pub id: uuid::Uuid,
    pub kickoff_at: chrono::DateTime<chrono::Utc>,
    pub status: String,
    pub home: String,
    pub away: String,
    pub home_score: Option<i16>,
    pub away_score: Option<i16>,
    pub publish_state: String,
    pub blockers: serde_json::Value,
}

pub async fn day(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(date): Path<String>,
    Query(query): Query<DayQuery>,
) -> Result<Json<DayResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|_| AppError::bad_request("A dátum formátuma YYYY-MM-DD legyen."))?;

    let rows = fetch_day(
        &state.pool,
        &DayFilter {
            date,
            timezone: "Europe/Budapest".into(),
            include_all: query.include.as_deref() == Some("all"),
        },
    )
    .await
    .map_err(|e| {
        tracing::error!(error=%e, "napi lista lekérdezés");
        AppError::internal("A napi lista lekérdezése sikertelen.")
    })?;

    Ok(Json(DayResponse {
        date,
        timezone: "Europe/Budapest",
        matches: rows
            .into_iter()
            .map(|row| DayMatch {
                id: row.match_id,
                kickoff_at: row.kickoff_at,
                status: row.status,
                home: row.home_name,
                away: row.away_name,
                home_score: row.home_score,
                away_score: row.away_score,
                publish_state: row.publish_state,
                blockers: row.blockers,
            })
            .collect(),
    }))
}
