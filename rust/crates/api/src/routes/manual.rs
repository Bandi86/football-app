//! Kézi meccsszerkesztés a kliensből (S80): csak ops tokennel, auditáltan.
//!
//! A handler nem tartalmaz domain-szabályt: a kérést a domain
//! `validate_manual_edit`-je validálja és normalizálja, a tartósítást a
//! storage végzi egy tranzakcióban, a handler csak státuszkódot képez.

use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use chrono::{DateTime, Utc};
use football_domain::manual_edit::{
    DecidedBy, KickoffPrecision, ManualMatchEdit, ManualResultEdit, validate_manual_edit,
};
use football_domain::match_state::MatchStatus;
use football_storage::object_store::ObjectStore;
use football_storage::repositories::manual::{
    ManualEditError, ManualMatchEditInput, apply_manual_match_edit,
};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{Scope, authorize};
use crate::error::{ApiError, AppError};

/// Jelenlét-érzékelő a test-mezőkre: a `null` és a hiányzó mező nem ugyanaz.
/// `Present(None)` = a kulcs jelen van `null` értékkel, a külső `None` = a kulcs
/// hiányzik. Az eredmény-mezőknél a puszta jelenlét is azt jelenti, hogy a
/// teljes `core.match_result` sor cserélődik (a hiányzó pontszám NULL lesz).
#[derive(Debug, Clone, Copy, Default)]
pub struct Present<T>(pub Option<T>);

impl<'de, T> Deserialize<'de> for Present<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(Option::<T>::deserialize(deserializer)?))
    }
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ManualResultRequest {
    /// Kezdés UTC-ben (RFC 3339). Hiányzó mezőnél a meccs kezdése nem változik.
    #[schema(value_type = Option<String>, format = "date-time")]
    pub kickoff_at: Option<DateTime<Utc>>,
    /// `exact` vagy `date_only`; utóbbinál a kezdés 12:00:00Z helyőrző.
    pub kickoff_precision: Option<String>,
    /// `scheduled|confirmed|live|halftime|finished|postponed|canceled|abandoned|awarded`.
    pub status: Option<String>,
    /// Végeredmény hazai góljai; hiányzó kulcs -> NULL a cserélt sorban.
    #[schema(value_type = Option<i16>)]
    pub home_ft: Option<Present<i16>>,
    #[schema(value_type = Option<i16>)]
    pub away_ft: Option<Present<i16>>,
    #[schema(value_type = Option<i16>)]
    pub home_ht: Option<Present<i16>>,
    #[schema(value_type = Option<i16>)]
    pub away_ht: Option<Present<i16>>,
    #[schema(value_type = Option<i16>)]
    pub home_et: Option<Present<i16>>,
    #[schema(value_type = Option<i16>)]
    pub away_et: Option<Present<i16>>,
    #[schema(value_type = Option<i16>)]
    pub home_pens: Option<Present<i16>>,
    #[schema(value_type = Option<i16>)]
    pub away_pens: Option<Present<i16>>,
    /// `ft|et|pens`; a jelenlét is eredmény-cserét jelent.
    #[schema(value_type = Option<String>)]
    pub decided_by: Option<Present<String>>,
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
    summary = "Meccs kézi szerkesztése (kezdés, státusz, teljes eredmény, auditált)",
    params(("id" = String, Path, description = "Meccs UUID")),
    request_body = ManualResultRequest,
    responses(
        (status = 200, body = ManualResultResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 409, body = ApiError),
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

    let kickoff_precision = body
        .kickoff_precision
        .as_deref()
        .map(|raw| {
            KickoffPrecision::parse(raw).ok_or_else(|| {
                AppError::bad_request(format!("Ismeretlen kickoff-pontosság: {raw}."))
            })
        })
        .transpose()?;
    let status = body
        .status
        .as_deref()
        .map(|raw| {
            MatchStatus::parse(raw)
                .ok_or_else(|| AppError::bad_request(format!("Ismeretlen státusz: {raw}.")))
        })
        .transpose()?;

    let has_result_payload = body.home_ft.is_some()
        || body.away_ft.is_some()
        || body.home_ht.is_some()
        || body.away_ht.is_some()
        || body.home_et.is_some()
        || body.away_et.is_some()
        || body.home_pens.is_some()
        || body.away_pens.is_some()
        || body.decided_by.is_some();
    let result = if has_result_payload {
        let decided_by = body
            .decided_by
            .and_then(|present| present.0)
            .map(|raw| {
                DecidedBy::parse(&raw)
                    .ok_or_else(|| AppError::bad_request(format!("Ismeretlen decided_by: {raw}.")))
            })
            .transpose()?;
        Some(ManualResultEdit {
            home_ft: body.home_ft.and_then(|present| present.0),
            away_ft: body.away_ft.and_then(|present| present.0),
            home_ht: body.home_ht.and_then(|present| present.0),
            away_ht: body.away_ht.and_then(|present| present.0),
            home_et: body.home_et.and_then(|present| present.0),
            away_et: body.away_et.and_then(|present| present.0),
            home_pens: body.home_pens.and_then(|present| present.0),
            away_pens: body.away_pens.and_then(|present| present.0),
            decided_by,
        })
    } else {
        None
    };

    let edit = ManualMatchEdit {
        kickoff_at: body.kickoff_at,
        kickoff_precision,
        status,
        result,
        note: body.note,
    };
    let edit =
        validate_manual_edit(&edit).map_err(|error| AppError::bad_request(error.to_string()))?;

    let store = ObjectStore::new(&state.data_dir)
        .map_err(|e| AppError::internal(format!("Objektumtár hiba: {e}")))?;
    let outcome = apply_manual_match_edit(
        &state.pool,
        &store,
        ManualMatchEditInput {
            match_id,
            actor: "ui",
            edit: &edit,
        },
    )
    .await
    .map_err(|error| match error {
        ManualEditError::MatchNotFound(_) => AppError::not_found("A meccs nem található."),
        ManualEditError::NaturalKeyConflict { existing_match_id } => {
            AppError::conflict(existing_match_id)
        }
        ManualEditError::Other(error) => {
            tracing::error!(error=%error, %match_id, "kézi meccsszerkesztés írása sikertelen");
            AppError::internal("A kézi szerkesztés rögzítése sikertelen.")
        }
    })?;

    // A gate-et a meglévő újraellenőrzés számolja ki — a kézi út nem kerüli
    // meg a verification láncot, csak új bizonyítékot ad hozzá.
    if let Err(err) = football_worker::tick::reverify_match(&state.pool, match_id).await {
        tracing::error!(error=%err, %match_id, "kézi szerkesztés utáni reverify sikertelen");
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
