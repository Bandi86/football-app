//! `GET /v1/match/{id}/history` — mező-szintű változástörténet
//! (`ops.audit_event` + kiválasztott `fact.provenance`), read tokennel.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use football_storage::repositories::history::HistoryEntry;
use serde::Deserialize;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{Scope, authorize},
    error::{ApiError, AppError},
};

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    /// A legfrissebb események száma; a tartomány 1–200.
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HistoryResponse {
    pub entries: Vec<HistoryEntry>,
}

#[utoipa::path(
    get,
    path = "/v1/match/{id}/history",
    tag = "meccs",
    summary = "Mező-szintű változástörténet forrással és idővel",
    params(
        ("id" = String, Path, description = "Meccs UUID"),
        ("limit" = Option<i64>, Query, description = "Legfrissebb események száma (alap: 50)")
    ),
    responses(
        (status = 200, body = HistoryResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<HistoryResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    if !(1..=200).contains(&query.limit) {
        return Err(AppError::bad_request(
            "A limit értéke 1 és 200 között lehet.",
        ));
    }
    let match_id =
        Uuid::parse_str(&id).map_err(|_| AppError::bad_request("Érvénytelen meccs-azonosító."))?;
    let exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM core.match WHERE id = $1)")
            .bind(match_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "történet meccs-ellenőrzés");
                AppError::internal("A meccs ellenőrzése sikertelen.")
            })?;
    if !exists {
        return Err(AppError::not_found("A meccs nem található."));
    }
    let entries =
        football_storage::repositories::history::list_for_match(&state.pool, match_id, query.limit)
            .await
            .map_err(|err| {
                tracing::error!(error = %err, %match_id, "változástörténet lekérdezése");
                AppError::internal("A változástörténet lekérdezése sikertelen.")
            })?;
    Ok(Json(HistoryResponse { entries }))
}
