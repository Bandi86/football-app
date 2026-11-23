//! `GET /v1/player/{id}` — játékosadatlap a már betöltött kanonikus adatokból.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use football_storage::repositories::player::{
    PlayerAppearanceRow, PlayerDetailRow, PlayerIncidentRow, PlayerSpellRow, PlayerStatRow,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{Scope, authorize},
    error::{ApiError, AppError},
};

#[derive(Debug, Deserialize)]
pub struct PlayerQuery {
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PlayerResponse {
    pub player: PlayerDetailRow,
    /// Időben érvényes csapattagságok (a 2026/11-es ablakból).
    pub spells: Vec<PlayerSpellRow>,
    pub appearances: Vec<PlayerAppearanceRow>,
    pub stats: Vec<PlayerStatRow>,
    /// Események stabil player-ID alapján kötve (név nélkül is megtalálhatók).
    pub incidents: Vec<PlayerIncidentRow>,
}

#[utoipa::path(
    get,
    path = "/v1/player/{id}",
    tag = "játékos",
    params(
        ("id" = Uuid, Path, description = "Játékos-entitás azonosítója"),
        ("limit" = Option<i64>, Query, description = "Listák hossza (1..200, alap 50)")
    ),
    responses(
        (status = 200, description = "Játékosadatlap forrással és rögzítési idővel", body = PlayerResponse),
        (status = 400, description = "Érvénytelen azonosító vagy limit", body = ApiError),
        (status = 401, description = "Hiányzó token", body = ApiError),
        (status = 404, description = "Nincs ilyen játékos", body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn player_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<PlayerQuery>,
) -> Result<Json<PlayerResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    if !(1..=200).contains(&query.limit) {
        return Err(AppError::bad_request("A limit 1 és 200 között lehet."));
    }
    let pool = &state.pool;
    let detail = football_storage::repositories::player::fetch_player(pool, id)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "játékos lekérése");
            AppError::internal("A játékos lekérése nem sikerült.")
        })?
        .ok_or_else(|| AppError::not_found("Nincs ilyen játékos."))?;
    let spells = football_storage::repositories::player::fetch_spells(pool, id, query.limit)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "játékos tagságok");
            AppError::internal("A csapattagságok lekérése nem sikerült.")
        })?;
    let appearances =
        football_storage::repositories::player::fetch_appearances(pool, id, query.limit)
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "játékos fellépések");
                AppError::internal("A fellépések lekérése nem sikerült.")
            })?;
    let stats = football_storage::repositories::player::fetch_stats(pool, id, query.limit)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "játékos statisztika");
            AppError::internal("A statisztika lekérése nem sikerült.")
        })?;
    let incidents = football_storage::repositories::player::fetch_incidents(pool, id, query.limit)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "játékos események");
            AppError::internal("Az események lekérése nem sikerült.")
        })?;
    Ok(Json(PlayerResponse {
        player: detail,
        spells,
        appearances,
        stats,
        incidents,
    }))
}
