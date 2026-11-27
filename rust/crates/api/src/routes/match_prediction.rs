//! Egy meccs shadow predikciója (S200) — read-only, read scope.
//!
//! A modell **nem ellenőrzött**, nem ad fogadási tanácsot; a kliens ezt
//! kifejezetten jelzi. A válasz a legfrissebb modellverzió sorait hordozza,
//! forrással, rögzítési idővel és input-hash-sel.

use axum::{Json, extract::Path, extract::State, http::HeaderMap};
use football_storage::repositories::{
    analytics::{PredictionRow, latest_prediction},
    match_micro::exists,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{Scope, authorize},
    error::{ApiError, AppError},
};

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PredictionResponse {
    pub rows: Vec<PredictionRow>,
    pub count: usize,
}

#[utoipa::path(
    get,
    path = "/v1/match/{id}/prediction",
    tag = "meccs",
    summary = "Meccs shadow predikciója (valószínűség, fair odds, modellverzió)",
    params(("id" = String, Path, description = "Meccs UUID")),
    responses(
        (status = 200, body = PredictionResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn match_prediction(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<PredictionResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let match_id =
        Uuid::parse_str(&id).map_err(|_| AppError::bad_request("Érvénytelen meccs-azonosító."))?;
    if !exists(&state.pool, match_id).await.map_err(|err| {
        tracing::error!(error=%err, %match_id, "meccs ellenőrzése");
        AppError::internal("A meccs ellenőrzése sikertelen.")
    })? {
        return Err(AppError::not_found("A meccs nem található."));
    }
    let rows = latest_prediction(&state.pool, match_id)
        .await
        .map_err(|err| {
            tracing::error!(error=%err, %match_id, "predikció lekérése");
            AppError::internal("A predikció lekérése sikertelen.")
        })?;
    let count = rows.len();
    Ok(Json(PredictionResponse { rows, count }))
}
