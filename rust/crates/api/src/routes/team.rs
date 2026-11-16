//! Csapat-lista és csapat-adatlap (S12).

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use chrono::{NaiveDate, Utc};
use football_storage::repositories::day::{DayFilter, fetch_day};
use football_storage::repositories::team::{
    TeamDetailRow, TeamListRow, fetch_team_detail, fetch_teams,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{Scope, authorize};
use crate::error::AppError;
use crate::routes::day::{DayMatch, map_match};

const TIMEZONE: &str = "Europe/Budapest";
const MAX_WINDOW: i64 = 90;

#[derive(Debug, Deserialize)]
pub struct TeamsQuery {
    pub query: Option<String>,
    pub date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub struct TeamDetailQuery {
    pub date: Option<NaiveDate>,
    pub window: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TeamDetailResponse {
    pub detail: TeamDetailRow,
    pub matches: Vec<DayMatch>,
}

pub async fn teams(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<TeamsQuery>,
) -> Result<Json<Vec<TeamListRow>>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let date = query.date.unwrap_or_else(|| Utc::now().date_naive());
    let rows = fetch_teams(
        &state.pool,
        query.query.as_deref().unwrap_or(""),
        date,
        TIMEZONE,
    )
    .await
    .map_err(|e| {
        tracing::error!(error=%e, "csapat-lista lekérdezés");
        AppError::internal("A csapat-lista lekérdezése sikertelen.")
    })?;
    Ok(Json(rows))
}

pub async fn team_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<TeamDetailQuery>,
) -> Result<Json<TeamDetailResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let team_id =
        Uuid::parse_str(&id).map_err(|_| AppError::bad_request("Érvénytelen csapat-azonosító."))?;
    let date = query.date.unwrap_or_else(|| Utc::now().date_naive());
    let window = query.window.unwrap_or(30).clamp(0, MAX_WINDOW);

    let detail = fetch_team_detail(&state.pool, team_id, date, TIMEZONE, window)
        .await
        .map_err(|e| {
            tracing::error!(error=%e, "csapat-adatlap lekérdezés");
            AppError::internal("A csapat adatlapjának lekérdezése sikertelen.")
        })?
        .ok_or_else(|| AppError::not_found("A csapat nem található."))?;
    let matches = fetch_day(
        &state.pool,
        &DayFilter {
            date,
            timezone: TIMEZONE.into(),
            include_all: false,
            competition_id: None,
            window_days: window,
            team_id: Some(team_id),
        },
    )
    .await
    .map_err(|e| {
        tracing::error!(error=%e, "csapat-meccsek lekérdezés");
        AppError::internal("A csapat meccseinek lekérdezése sikertelen.")
    })?;

    Ok(Json(TeamDetailResponse {
        detail,
        matches: matches.into_iter().map(map_match).collect(),
    }))
}
