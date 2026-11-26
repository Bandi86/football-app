pub mod auth;
pub mod error;
pub mod openapi;
pub mod routes;

use std::path::PathBuf;

use axum::{
    Router,
    routing::{get, patch, post, put},
};
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
        .route("/v1/weather", get(routes::weather::weather))
        .route("/openapi.json", get(openapi::document))
        .route("/v1/day/{date}", get(routes::day::day))
        .route("/v1/search", get(routes::search::search))
        .route("/v1/asset/{*key}", get(routes::asset::asset))
        .route("/v1/asset/upload", post(routes::asset_upload::upload_asset))
        .route("/v1/match/{id}", get(routes::match_detail::match_detail))
        .route(
            "/v1/match/{id}/lineups",
            get(routes::match_micro::match_lineups),
        )
        .route(
            "/v1/match/{id}/player-stats",
            get(routes::match_micro::match_player_stats),
        )
        .route(
            "/v1/match/{id}/shots",
            get(routes::match_micro::match_shots),
        )
        .route(
            "/v1/match/{id}/manual-result",
            put(routes::manual::manual_result),
        )
        .route("/v1/match/{id}/edit", put(routes::edit::match_edit))
        .route(
            "/v1/match/{id}/notes",
            get(routes::notes::list_notes).post(routes::notes::create_note),
        )
        .route("/v1/note/{id}", patch(routes::notes::edit_note))
        .route("/v1/match/{id}/history", get(routes::history::history))
        .route("/v1/issues", get(routes::issues::issues))
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
        .route("/v1/player/{id}", get(routes::player::player_detail))
        .route(
            "/v1/ops/verification/gates",
            get(routes::verification::gates),
        )
        .route("/v1/ops/status", get(routes::ops::status))
        .route("/v1/ops/audit", get(routes::audit::audit))
        .route("/v1/ops/summary", get(routes::summary::summary))
        .route(
            "/v1/issue/{id}/resolve",
            post(routes::issue_action::resolve_issue),
        )
        .with_state(state)
}
