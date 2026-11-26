//! Meccs-szintű mikro-adatok read-only végpontjai (S182/S3).
//!
//! Mindhárom végpont read scope-ú, és 404-et ad ismeretlen meccsre —
//! az üres adat (`[]`) és a nem létező entitás nem keveredik össze.

use axum::{Json, extract::Path, extract::State, http::HeaderMap};
use football_storage::repositories::match_micro::{LineupRow, PlayerStatRow, ShotRow, exists};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{Scope, authorize},
    error::{ApiError, AppError},
};

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LineupsResponse {
    pub lineups: Vec<LineupRow>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PlayerStatsResponse {
    pub player_stats: Vec<PlayerStatRow>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ShotsResponse {
    pub shots: Vec<ShotRow>,
}

async fn ensure_match(state: &AppState, id: &str) -> Result<Uuid, AppError> {
    let match_id =
        Uuid::parse_str(id).map_err(|_| AppError::bad_request("Érvénytelen meccs-azonosító."))?;
    if !exists(&state.pool, match_id).await.map_err(|err| {
        tracing::error!(error=%err, %match_id, "meccs ellenőrzése");
        AppError::internal("A meccs ellenőrzése sikertelen.")
    })? {
        return Err(AppError::not_found("A meccs nem található."));
    }
    Ok(match_id)
}

#[utoipa::path(
    get,
    path = "/v1/match/{id}/lineups",
    tag = "meccs",
    summary = "Meccs felállása (kezdők, cserék, értékelés)",
    params(("id" = String, Path, description = "Meccs UUID")),
    responses(
        (status = 200, body = LineupsResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn match_lineups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<LineupsResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let match_id = ensure_match(&state, &id).await?;
    let lineups = football_storage::repositories::match_micro::fetch_lineups(&state.pool, match_id)
        .await
        .map_err(|err| {
            tracing::error!(error=%err, %match_id, "felállás lekérése");
            AppError::internal("A felállás lekérése sikertelen.")
        })?;
    Ok(Json(LineupsResponse { lineups }))
}

#[utoipa::path(
    get,
    path = "/v1/match/{id}/player-stats",
    tag = "meccs",
    summary = "Meccs játékosszintű statisztikája",
    params(("id" = String, Path, description = "Meccs UUID")),
    responses(
        (status = 200, body = PlayerStatsResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn match_player_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<PlayerStatsResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let match_id = ensure_match(&state, &id).await?;
    let player_stats =
        football_storage::repositories::match_micro::fetch_player_stats(&state.pool, match_id)
            .await
            .map_err(|err| {
                tracing::error!(error=%err, %match_id, "játékosstatisztika lekérése");
                AppError::internal("A játékosstatisztika lekérése sikertelen.")
            })?;
    Ok(Json(PlayerStatsResponse { player_stats }))
}

#[utoipa::path(
    get,
    path = "/v1/match/{id}/shots",
    tag = "meccs",
    summary = "Meccs lövései (x, y, xG, xGoT)",
    params(("id" = String, Path, description = "Meccs UUID")),
    responses(
        (status = 200, body = ShotsResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn match_shots(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ShotsResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let match_id = ensure_match(&state, &id).await?;
    let shots = football_storage::repositories::match_micro::fetch_shots(&state.pool, match_id)
        .await
        .map_err(|err| {
            tracing::error!(error=%err, %match_id, "lövések lekérése");
            AppError::internal("A lövések lekérése sikertelen.")
        })?;
    Ok(Json(ShotsResponse { shots }))
}
