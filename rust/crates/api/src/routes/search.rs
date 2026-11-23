//! `GET /v1/search?q=…` — globális keresés a kanonikus csapatokra,
//! versenyekre és az utóbbi 7 nap meccseire.

use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use football_storage::repositories::search::SearchHit;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    AppState,
    auth::{Scope, authorize},
    error::{ApiError, AppError},
};

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    q: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SearchResponse {
    pub query: String,
    pub results: Vec<SearchHit>,
}

#[utoipa::path(
    get,
    path = "/v1/search",
    tag = "keresés",
    params(("q" = String, Query, description = "Keresőszó (min. 2 karakter)")),
    responses(
        (status = 200, description = "Találatok csapat/verseny/meccs bontásban", body = SearchResponse),
        (status = 400, description = "Túl rövid keresőszó", body = ApiError),
        (status = 401, description = "Hiányzó token", body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn search(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let term = query.q.trim();
    if term.chars().count() < 2 {
        return Err(AppError::bad_request(
            "A keresőszónak legalább 2 karakter hosszúnak kell lennie.",
        ));
    }
    // A túl hosszú lekérés nem ér el több találatot, csak terhelést okoz.
    let term: String = term.chars().take(80).collect();
    let results = football_storage::repositories::search::search(&state.pool, &term, 8)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "keresési hiba");
            AppError::internal("A keresés nem hajtható végre.")
        })?;
    Ok(Json(SearchResponse {
        query: term,
        results,
    }))
}
