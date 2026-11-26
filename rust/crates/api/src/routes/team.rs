//! Csapat-lista és csapat-adatlap (S12).

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use chrono::{NaiveDate, Utc};
use football_storage::repositories::day::{DayFilter, fetch_day};
use football_storage::repositories::match_detail::{
    FormEntry, NextMatchEntry, fetch_team_form, fetch_team_next,
};
use football_storage::repositories::team::{
    TeamDetailRow, TeamListRow, TeamPlayerRow, fetch_team_detail, fetch_team_players, fetch_teams,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{Scope, authorize};
use crate::error::{ApiError, AppError};
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

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct TeamDetailResponse {
    pub detail: TeamDetailRow,
    pub matches: Vec<DayMatch>,
    /// Az utolsó öt lejátszott meccs (a mai nap előtt), W/D/L sorrendben.
    #[serde(default)]
    pub form: Vec<FormEntry>,
    /// A következő legfeljebb három kijelölt meccs.
    #[serde(default)]
    pub next: Vec<NextMatchEntry>,
    /// A legfrissebb felállásokból származó as-of keret (hiány esetén []).
    #[serde(default)]
    pub players: Vec<TeamPlayerRow>,
}

#[utoipa::path(
    get,
    path = "/v1/teams",
    tag = "csapat",
    summary = "Csapatkereső (név-részletre), utolsó 30 nap mérlegével",
    params(
        ("query" = Option<String>, Query, description = "Név-részlet, kis/nagybetűtől független"),
        ("date" = Option<String>, Query, description = "Viszonyítási nap, YYYY-MM-DD (alap: ma)")
    ),
    responses(
        (status = 200, body = Vec<TeamListRow>),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
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

#[utoipa::path(
    get,
    path = "/v1/team/{id}",
    tag = "csapat",
    summary = "Csapat-adatlap: mérleg, Elo és a környékbeli meccsek",
    params(
        ("id" = String, Path, description = "Csapat UUID"),
        ("date" = Option<String>, Query, description = "Viszonyítási nap, YYYY-MM-DD (alap: ma)"),
        ("window" = Option<i64>, Query, description = "Ablak napokban (0–90, alap 30)")
    ),
    responses(
        (status = 200, body = TeamDetailResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
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

    // A forma és a következő meccsek nem kritikusak: ha bármelyik lekérdezés
    // elhasal, a csapat-adatlap a többivel együtt akkor is megjelenik.
    let now = Utc::now();
    let (form, next) = tokio::join!(
        fetch_team_form(&state.pool, team_id, now, 5),
        fetch_team_next(&state.pool, team_id, now, 3),
    );
    let form = form.unwrap_or_else(|e| {
        tracing::warn!(error=%e, "csapat forma lekérdezés");
        Vec::new()
    });
    let next = next.unwrap_or_else(|e| {
        tracing::warn!(error=%e, "csapat következő meccsek lekérdezés");
        Vec::new()
    });

    let players = fetch_team_players(&state.pool, team_id)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error=%e, "csapat keret lekérdezés");
            Vec::new()
        });

    Ok(Json(TeamDetailResponse {
        detail,
        matches: matches.into_iter().map(map_match).collect(),
        form,
        next,
        players,
    }))
}
