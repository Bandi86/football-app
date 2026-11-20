pub mod auth;
pub mod error;
pub mod openapi;
pub mod routes;

use std::path::PathBuf;

use axum::{Router, routing::get};
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub read_token: String,
    pub ops_token: String,
    /// A nyers objektumtár gyökere (asset-blobok kiszolgálásához).
    pub data_dir: PathBuf,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(routes::health::health))
        .route("/openapi.json", get(openapi::document))
        .route("/v1/day/{date}", get(routes::day::day))
        .route("/v1/asset/{*key}", get(routes::asset::asset))
        .route("/v1/match/{id}", get(routes::match_detail::match_detail))
        .route("/v1/competitions", get(routes::competition::competitions))
        .route(
            "/v1/competition/{id}",
            get(routes::competition::competition_detail),
        )
        .route(
            "/v1/competition/{id}/seasons",
            get(routes::competition::competition_seasons),
        )
        .route("/v1/teams", get(routes::team::teams))
        .route("/v1/team/{id}", get(routes::team::team_detail))
        .route(
            "/v1/ops/verification/gates",
            get(routes::verification::gates),
        )
        .route("/v1/ops/status", get(routes::ops::status))
        .with_state(state)
}
