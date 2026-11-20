use axum::Json;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::{Scope, authorize};
use crate::error::{ApiError, AppError};

#[derive(Debug, Deserialize)]
pub struct GatesQuery {
    pub date: NaiveDate,
}

#[derive(Debug, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct GateRow {
    pub match_id: uuid::Uuid,
    pub state: String,
    #[schema(value_type = Vec<football_domain::verification::Finding>)]
    pub blockers: serde_json::Value,
    pub evaluated_at: chrono::DateTime<chrono::Utc>,
}

#[utoipa::path(
    get,
    path = "/v1/ops/verification/gates",
    tag = "ops",
    summary = "Egy nap publikálási kapu-kiértékelései (ops-token)",
    params(("date" = String, Query, description = "Publikálási nap, YYYY-MM-DD")),
    responses(
        (status = 200, body = Vec<GateRow>),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn gates(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<GatesQuery>,
) -> Result<Json<Vec<GateRow>>, AppError> {
    authorize(&headers, Scope::Ops, &state.read_token, &state.ops_token)?;

    let rows = sqlx::query_as::<_, GateRow>(
        "SELECT match_id, state, blockers, evaluated_at
         FROM ops.publish_gate
         WHERE publish_date = $1
         ORDER BY evaluated_at DESC",
    )
    .bind(query.date)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows))
}
