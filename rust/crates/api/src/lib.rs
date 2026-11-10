pub mod auth;
pub mod error;
pub mod routes;

use axum::{Router, routing::get};
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub read_token: String,
    pub ops_token: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(routes::health::health))
        .route("/v1/day/{date}", get(routes::day::day))
        .route(
            "/v1/ops/verification/gates",
            get(routes::verification::gates),
        )
        .with_state(state)
}
