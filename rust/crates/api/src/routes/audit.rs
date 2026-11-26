//! `GET /v1/ops/audit` — globális, időrendi audit-feed (Ops scope).

use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use football_storage::repositories::ops_audit::{AuditEntryRow, AuditFilter};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{Scope, authorize},
    error::{ApiError, AppError},
};

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    /// Legfrissebb események száma (1–200, alap 50).
    #[serde(default = "default_limit")]
    pub limit: i64,
    pub subject_kind: Option<String>,
    pub subject_id: Option<Uuid>,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuditResponse {
    pub entries: Vec<AuditEntryRow>,
    pub count: usize,
}

#[utoipa::path(
    get,
    path = "/v1/ops/audit",
    tag = "ops",
    summary = "Globális audit-események időrendben (ops token)",
    params(
        ("limit" = Option<i64>, Query, description = "1–200, alap 50"),
        ("subject_kind" = Option<String>, Query, description = "match|team|competition|day"),
        ("subject_id" = Option<Uuid>, Query, description = "Tárgy UUID")
    ),
    responses(
        (status = 200, body = AuditResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn audit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuditQuery>,
) -> Result<Json<AuditResponse>, AppError> {
    authorize(&headers, Scope::Ops, &state.read_token, &state.ops_token)?;
    if !(1..=200).contains(&query.limit) {
        return Err(AppError::bad_request(
            "A limit értéke 1 és 200 között lehet.",
        ));
    }
    let entries = football_storage::repositories::ops_audit::list(
        &state.pool,
        &AuditFilter {
            limit: query.limit,
            subject_kind: query.subject_kind,
            subject_id: query.subject_id,
        },
    )
    .await
    .map_err(|err| {
        tracing::error!(error = %err, "audit feed lekérdezése");
        AppError::internal("Az audit-események lekérdezése sikertelen.")
    })?;
    let count = entries.len();
    Ok(Json(AuditResponse { entries, count }))
}
