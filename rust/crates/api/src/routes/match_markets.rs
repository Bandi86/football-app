//! Egy meccs részletes odds-piacai (S194) — read-only, read scope.
//!
//! A fő `/v1/match/{id}` válasz változatlan; a sok szorzót külön végpont
//! hordozza, hogy a kliens a részletezőben csoportosítva, korlátozott
//! darabszámmal jelenítheti meg őket.

use axum::{Json, extract::Path, extract::State, http::HeaderMap};
use football_storage::repositories::{
    match_markets::{MarketQuoteRow, fetch_quotes},
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
pub struct MarketsResponse {
    pub quotes: Vec<MarketQuoteRow>,
    pub count: usize,
}

#[utoipa::path(
    get,
    path = "/v1/match/{id}/markets",
    tag = "meccs",
    summary = "Meccs részletes odds-piacai (minden piac, forrással)",
    params(("id" = String, Path, description = "Meccs UUID")),
    responses(
        (status = 200, body = MarketsResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn match_markets(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<MarketsResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let match_id =
        Uuid::parse_str(&id).map_err(|_| AppError::bad_request("Érvénytelen meccs-azonosító."))?;
    if !exists(&state.pool, match_id).await.map_err(|err| {
        tracing::error!(error=%err, %match_id, "meccs ellenőrzése");
        AppError::internal("A meccs ellenőrzése sikertelen.")
    })? {
        return Err(AppError::not_found("A meccs nem található."));
    }
    let quotes = fetch_quotes(&state.pool, match_id).await.map_err(|err| {
        tracing::error!(error=%err, %match_id, "odds-piacok lekérése");
        AppError::internal("Az odds-piacok lekérése sikertelen.")
    })?;
    let count = quotes.len();
    Ok(Json(MarketsResponse { quotes, count }))
}
