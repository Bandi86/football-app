//! Kézi eredménybevitel a kliensből (S70): csak ops tokennel, auditáltan.

use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use football_storage::object_store::ObjectStore;
use football_storage::repositories::manual::{ManualResultInput, upsert_manual_result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{Scope, authorize};
use crate::error::{ApiError, AppError};

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct ManualResultRequest {
    /// Végeredmény hazai góljai (egész szám, >= 0).
    pub home_ft: i16,
    /// Végeredmény vendég góljai (egész szám, >= 0).
    pub away_ft: i16,
    /// Kötelező indoklás (auditba kerül).
    pub note: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ManualResultResponse {
    pub match_id: Uuid,
    pub observation_id: Uuid,
    pub audit_id: Uuid,
    pub state: String,
}

#[utoipa::path(
    put,
    path = "/v1/match/{id}/manual-result",
    tag = "meccs",
    summary = "Hiányzó eredmény kézi felvétele (auditált)",
    params(("id" = String, Path, description = "Meccs UUID")),
    request_body = ManualResultRequest,
    responses(
        (status = 200, body = ManualResultResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn manual_result(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ManualResultRequest>,
) -> Result<Json<ManualResultResponse>, AppError> {
    authorize(&headers, Scope::Ops, &state.read_token, &state.ops_token)?;
    let match_id =
        Uuid::parse_str(&id).map_err(|_| AppError::bad_request("Érvénytelen meccs-azonosító."))?;
    if body.home_ft < 0 || body.away_ft < 0 {
        return Err(AppError::bad_request("A gólszám nem lehet negatív."));
    }
    if body.note.trim().is_empty() {
        return Err(AppError::bad_request("Az indoklás kötelező."));
    }

    let store = ObjectStore::new(&state.data_dir)
        .map_err(|e| AppError::internal(format!("Objektumtár hiba: {e}")))?;
    let outcome = upsert_manual_result(
        &state.pool,
        &store,
        ManualResultInput {
            match_id,
            home_ft: body.home_ft,
            away_ft: body.away_ft,
            note: &body.note,
            actor: "ui",
        },
    )
    .await
    .map_err(|e| {
        tracing::error!(error=%e, %match_id, "kézi eredmény írása sikertelen");
        if e.to_string().contains("nincs ilyen meccs") {
            AppError::not_found("A meccs nem található.")
        } else {
            AppError::internal("A kézi eredmény rögzítése sikertelen.")
        }
    })?;

    // A gate-et a meglévő újraellenőrzés számolja ki — a kézi út nem kerüli
    // meg a verification láncot, csak új bizonyítékot ad hozzá.
    if let Err(err) = football_worker::tick::reverify_match(&state.pool, match_id).await {
        tracing::error!(error=%err, %match_id, "kézi eredmény utáni reverify sikertelen");
    }
    let state_text: String = sqlx::query_scalar(
        "SELECT state FROM ops.publish_gate WHERE match_id=$1 ORDER BY evaluated_at DESC LIMIT 1",
    )
    .bind(match_id)
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or_else(|| "unknown".to_string());

    Ok(Json(ManualResultResponse {
        match_id,
        observation_id: outcome.observation_id,
        audit_id: outcome.audit_id,
        state: state_text,
    }))
}
