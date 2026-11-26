//! S173/S4: egységes meccs-szerkesztés (`PUT /v1/match/{id}/edit`).
//!
//! A régi `/manual-result` út változatlanul él (kompatibilitás). Ez az út
//! ugyanazt a domain-validálást és storage-tranzakciót használja, de a
//! whitelistból származó stabil `request_key`-vel idempotens, és a frissített
//! provenance-sor policy verziója `manual-full-1.0`.

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
use hex::encode as hex_encode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::AppState;
use crate::auth::{Scope, authorize};
use crate::error::{ApiError, AppError};

/// Az új út provenance-policy verziója; a régi út `manual-2.0` marad.
const POLICY_VERSION: &str = "manual-full-1.0";

/// A küldhető mezők whitelistja. Minden más mező figyelmen kívül marad.
#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct MatchEditRequest {
    /// Kezdés UTC-ben (RFC 3339). Hiányzó/`null` mezőnél a kezdés nem változik.
    #[schema(value_type = Option<String>, format = "date-time")]
    pub kickoff_at: Option<DateTime<Utc>>,
    /// `exact` vagy `date_only`; utóbbinál a kezdés 12:00:00Z helyőrző.
    pub kickoff_precision: Option<String>,
    /// `scheduled|confirmed|live|halftime|finished|postponed|canceled|abandoned|awarded`.
    pub status: Option<String>,
    #[schema(value_type = Option<i16>)]
    pub home_ft: Option<i16>,
    #[schema(value_type = Option<i16>)]
    pub away_ft: Option<i16>,
    #[schema(value_type = Option<i16>)]
    pub home_ht: Option<i16>,
    #[schema(value_type = Option<i16>)]
    pub away_ht: Option<i16>,
    #[schema(value_type = Option<i16>)]
    pub home_et: Option<i16>,
    #[schema(value_type = Option<i16>)]
    pub away_et: Option<i16>,
    #[schema(value_type = Option<i16>)]
    pub home_pens: Option<i16>,
    #[schema(value_type = Option<i16>)]
    pub away_pens: Option<i16>,
    /// `ft|et|pens`; hiányában a domain az eredmény alakjából származtatja.
    #[schema(value_type = Option<String>)]
    pub decided_by: Option<String>,
    /// Kötelező indoklás (auditba kerül).
    pub note: String,
    /// Stabil kéréskulcs. Hiányában a whitelist kanonikus lenyomata a kulcs.
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MatchEditResponse {
    pub match_id: Uuid,
    pub observation_id: Uuid,
    pub audit_id: Uuid,
    /// A publish gate aktuális állapota a szerkesztés után.
    pub state: String,
    /// Igaz, ha a kérés egy korábban rögzített szerkesztés ismétlése volt.
    pub reused: bool,
}

/// A whitelist kanonikus lenyomata: a struct deklarált mezősorrendjében
/// szerializált JSON SHA-256 lenyomata. Ugyanaz a tartalom mindig ugyanazt
/// a kulcsot adja, függetlenül a kérésben szereplő kulcsok sorrendjétől.
fn canonical_key(body: &MatchEditRequest) -> String {
    let canonical = serde_json::to_vec(body).expect("a szerkesztési kérés szerializálható");
    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    format!("canonical-{}", hex_encode(hasher.finalize()))
}

#[utoipa::path(
    put,
    path = "/v1/match/{id}/edit",
    tag = "meccs",
    summary = "Meccs egységes szerkesztése (kezdés, státusz, eredmény, auditált, idempotens)",
    params(("id" = String, Path, description = "Meccs UUID")),
    request_body = MatchEditRequest,
    responses(
        (status = 200, body = MatchEditResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 409, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn match_edit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<MatchEditRequest>,
) -> Result<Json<MatchEditResponse>, AppError> {
    authorize(&headers, Scope::Ops, &state.read_token, &state.ops_token)?;
    let match_id =
        Uuid::parse_str(&id).map_err(|_| AppError::bad_request("Érvénytelen meccs-azonosító."))?;

    // A stabil kulcsot még a mezők átvétele előtt számoljuk: a canonical hash
    // a teljes whitelist lenyomata, az explicit kulcs pedig változatlan marad.
    let request_key = body
        .idempotency_key
        .clone()
        .unwrap_or_else(|| canonical_key(&body));

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
            .as_deref()
            .map(|raw| {
                DecidedBy::parse(raw)
                    .ok_or_else(|| AppError::bad_request(format!("Ismeretlen decided_by: {raw}.")))
            })
            .transpose()?;
        Some(ManualResultEdit {
            home_ft: body.home_ft,
            away_ft: body.away_ft,
            home_ht: body.home_ht,
            away_ht: body.away_ht,
            home_et: body.home_et,
            away_et: body.away_et,
            home_pens: body.home_pens,
            away_pens: body.away_pens,
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
            idempotency_key: Some(&request_key),
            policy_version: POLICY_VERSION,
        },
    )
    .await
    .map_err(|error| match error {
        ManualEditError::MatchNotFound(_) => AppError::not_found("A meccs nem található."),
        ManualEditError::NaturalKeyConflict { existing_match_id } => {
            AppError::conflict(existing_match_id)
        }
        ManualEditError::Other(error) => {
            tracing::error!(error=%error, %match_id, "egységes meccsszerkesztés írása sikertelen");
            AppError::internal("A szerkesztés rögzítése sikertelen.")
        }
    })?;

    // Ismétlésnél nem írtunk új bizonyítékot, ezért a gate-et sem kell újra
    // számolni; a lekérdezett state így is a current publish gate állapota.
    if !outcome.reused
        && let Err(err) = football_worker::tick::reverify_match(&state.pool, match_id).await
    {
        tracing::error!(error=%err, %match_id, "szerkesztés utáni reverify sikertelen");
    }
    let state_text: String = sqlx::query_scalar(
        "SELECT state FROM ops.publish_gate WHERE match_id=$1 ORDER BY evaluated_at DESC LIMIT 1",
    )
    .bind(match_id)
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or_else(|| "unknown".to_string());

    Ok(Json(MatchEditResponse {
        match_id,
        observation_id: outcome.observation_id,
        audit_id: outcome.audit_id,
        state: state_text,
        reused: outcome.reused,
    }))
}
