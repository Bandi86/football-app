//! Verseny-lista és verseny-adatlap (S11): a nap/környék ellenőrzött
//! meccseiből; a meccsek a napi API azonos alakjában mennek vissza.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use chrono::NaiveDate;
use football_storage::repositories::competition::{
    CompetitionDetailRow, CompetitionListRow, CompetitionSeasonRow, CompetitionStandingRow,
    CompetitionTeamRow, fetch_competition_detail, fetch_competition_seasons,
    fetch_competition_standings, fetch_competition_teams, fetch_competitions,
};
use football_storage::repositories::day::{DayFilter, fetch_day};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::{Scope, authorize};
use crate::error::{ApiError, AppError};
use crate::routes::day::{DayMatch, map_match};

const TIMEZONE: &str = "Europe/Budapest";
const MAX_WINDOW: i64 = 30;

#[derive(Debug, Deserialize)]
pub struct DateQuery {
    pub date: NaiveDate,
}

#[derive(Debug, Deserialize)]
pub struct DetailQuery {
    pub date: NaiveDate,
    pub window: Option<i64>,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct CompetitionDetailResponse {
    pub detail: CompetitionDetailRow,
    pub matches: Vec<DayMatch>,
    pub teams: Vec<CompetitionTeamRow>,
    pub standings: Vec<CompetitionStandingRow>,
}

#[utoipa::path(
    get,
    path = "/v1/competitions",
    tag = "verseny",
    summary = "Az adott nap versenyei meccsszámlálókkal",
    params(("date" = String, Query, description = "Naptári nap, YYYY-MM-DD")),
    responses(
        (status = 200, body = Vec<CompetitionListRow>),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn competitions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<DateQuery>,
) -> Result<Json<Vec<CompetitionListRow>>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let rows = fetch_competitions(&state.pool, query.date, TIMEZONE)
        .await
        .map_err(|e| {
            tracing::error!(error=%e, "verseny-lista lekérdezés");
            AppError::internal("A verseny-lista lekérdezése sikertelen.")
        })?;
    Ok(Json(rows))
}

#[utoipa::path(
    get,
    path = "/v1/competition/{id}",
    tag = "verseny",
    summary = "Verseny-adatlap: összesítők, környékbeli meccsek, csapatmérlegek",
    params(
        ("id" = String, Path, description = "Verseny UUID"),
        ("date" = String, Query, description = "Naptári nap, YYYY-MM-DD"),
        ("window" = Option<i64>, Query, description = "Ablak napokban (0–30, alap 7)")
    ),
    responses(
        (status = 200, body = CompetitionDetailResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn competition_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<DetailQuery>,
) -> Result<Json<CompetitionDetailResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let competition_id = Uuid::parse_str(&id)
        .map_err(|_| AppError::bad_request("Érvénytelen verseny-azonosító."))?;
    let window = query.window.unwrap_or(7).clamp(0, MAX_WINDOW);

    let detail =
        fetch_competition_detail(&state.pool, competition_id, query.date, TIMEZONE, window)
            .await
            .map_err(|e| {
                tracing::error!(error=%e, "verseny-adatlap lekérdezés");
                AppError::internal("A verseny adatlapjának lekérdezése sikertelen.")
            })?
            .ok_or_else(|| AppError::not_found("A verseny nem található."))?;
    let matches = fetch_day(
        &state.pool,
        &DayFilter {
            date: query.date,
            timezone: TIMEZONE.into(),
            include_all: false,
            competition_id: Some(competition_id),
            window_days: window,
            team_id: None,
        },
    )
    .await
    .map_err(|e| {
        tracing::error!(error=%e, "verseny-meccsek lekérdezés");
        AppError::internal("A verseny meccseinek lekérdezése sikertelen.")
    })?;
    let standings = fetch_competition_standings(&state.pool, competition_id)
        .await
        .map_err(|e| {
            tracing::error!(error=%e, "verseny-tabella lekérdezés");
            AppError::internal("A verseny tabellájának lekérdezése sikertelen.")
        })?;
    let teams = fetch_competition_teams(&state.pool, competition_id, query.date, TIMEZONE, window)
        .await
        .map_err(|e| {
            tracing::error!(error=%e, "verseny-csapatok lekérdezés");
            AppError::internal("A verseny csapatainak lekérdezése sikertelen.")
        })?;

    Ok(Json(CompetitionDetailResponse {
        detail,
        matches: matches.into_iter().map(map_match).collect(),
        teams,
        standings,
    }))
}

/// A verseny szezon-listája (meccs/eredmény/odds darabszámokkal).
#[utoipa::path(
    get,
    path = "/v1/competition/{id}/seasons",
    tag = "verseny",
    summary = "A verseny szezonjai meccs/eredmény/odds darabszámmal",
    params(("id" = String, Path, description = "Verseny UUID")),
    responses(
        (status = 200, body = Vec<CompetitionSeasonRow>),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn competition_seasons(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Vec<CompetitionSeasonRow>>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let competition_id = Uuid::parse_str(&id)
        .map_err(|_| AppError::bad_request("Érvénytelen verseny-azonosító."))?;
    let rows = fetch_competition_seasons(&state.pool, competition_id)
        .await
        .map_err(|e| {
            tracing::error!(error=%e, "verseny-szezonok lekérdezés");
            AppError::internal("A verseny szezonjainak lekérdezése sikertelen.")
        })?;
    Ok(Json(rows))
}
