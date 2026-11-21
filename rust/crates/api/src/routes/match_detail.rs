//! Meccs-részletező (S10): egy kanonikus meccs teljes, ellenőrzött képe.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use football_storage::repositories::match_detail::{
    MatchDetailRow, fetch_match_detail, fetch_match_extras,
};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{Scope, authorize};
use crate::error::{ApiError, AppError};

#[utoipa::path(
    get,
    path = "/v1/match/{id}",
    tag = "meccs",
    summary = "Egy kanonikus meccs teljes, ellenőrzött képe",
    params(("id" = String, Path, description = "Meccs UUID")),
    responses(
        (status = 200, body = MatchDetailRow),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn match_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<MatchDetailRow>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let match_id =
        Uuid::parse_str(&id).map_err(|_| AppError::bad_request("Érvénytelen meccs-azonosító."))?;
    let detail = fetch_match_detail(&state.pool, match_id)
        .await
        .map_err(|e| {
            tracing::error!(error=%e, %match_id, "meccs-részletek lekérdezés");
            AppError::internal("A meccs részleteinek lekérdezése sikertelen.")
        })?
        .ok_or_else(|| AppError::not_found("A meccs nem található."))?;
    // A forma és az egymás elleni lista a meccs kezdete előtti állapotra szűr,
    // hogy jövőbeli információ ne szivárogjon vissza. Kiegészítés hiányában a
    // mezők üres listák maradnak, a részletező így is működik.
    let mut detail = detail;
    if let Some(extras) = fetch_match_extras(&state.pool, match_id, detail.kickoff_at)
        .await
        .map_err(|e| {
            tracing::error!(error=%e, %match_id, "meccs-kiegészítések lekérdezése");
            AppError::internal("A meccs kiegészítéseinek lekérdezése sikertelen.")
        })?
    {
        detail.home_form = extras.home_form;
        detail.away_form = extras.away_form;
        detail.head_to_head = extras.head_to_head;
        detail.home_next = extras.home_next;
        detail.away_next = extras.away_next;
        detail.home_position = extras.home_position;
        detail.away_position = extras.away_position;
    }
    Ok(Json(detail))
}
