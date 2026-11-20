use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
}

#[utoipa::path(
    get,
    path = "/v1/health",
    tag = "health",
    summary = "Életjel (publikus, token nélkül)",
    responses((status = 200, body = HealthResponse))
)]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "footballd",
    })
}
