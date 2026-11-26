//! Meccsjegyzetek: olvasás read tokennel, írás és szerkesztés ops tokennel.
//! A handler nem ír kanonikus adatot; minden írás `ops.audit_event`-be kerül.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use football_domain::ops::{NoteError, NoteKind, validate_note_author, validate_note_body};
use football_storage::repositories::note::{MatchNoteRow, NotePatch};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{Scope, authorize},
    error::{ApiError, AppError},
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct NoteCreateRequest {
    /// `ui`, `agent:<név>` vagy `import:<forrás>`.
    pub author: String,
    pub body: String,
    /// `info|problem|fix|question|resolved`.
    pub kind: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NotePatchRequest {
    pub body: Option<String>,
    pub kind: Option<String>,
    /// `true` = lezárás, `false` = újranyitás.
    pub resolved: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotesResponse {
    pub notes: Vec<MatchNoteRow>,
}

async fn match_exists(pool: &sqlx::PgPool, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM core.match WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
}

async fn audit(
    pool: &sqlx::PgPool,
    actor: &str,
    action: &str,
    match_id: Uuid,
    detail: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO ops.audit_event (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1, $2, $3, 'match', $4, $5)",
    )
    .bind(Uuid::now_v7())
    .bind(actor)
    .bind(action)
    .bind(match_id)
    .bind(detail)
    .execute(pool)
    .await
    .map(|_| ())
}

#[utoipa::path(
    get,
    path = "/v1/match/{id}/notes",
    tag = "jegyzet",
    summary = "Meccsjegyzetek időrendben",
    params(("id" = String, Path, description = "Meccs UUID")),
    responses(
        (status = 200, body = NotesResponse),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn list_notes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<NotesResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let match_id =
        Uuid::parse_str(&id).map_err(|_| AppError::bad_request("Érvénytelen meccs-azonosító."))?;
    if !match_exists(&state.pool, match_id).await.map_err(|err| {
        tracing::error!(error = %err, "jegyzet meccs-ellenőrzés");
        AppError::internal("A meccs ellenőrzése sikertelen.")
    })? {
        return Err(AppError::not_found("A meccs nem található."));
    }
    let notes = football_storage::repositories::note::list_for_match(&state.pool, match_id)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "jegyzetek lekérdezése");
            AppError::internal("A jegyzetek lekérdezése sikertelen.")
        })?;
    Ok(Json(NotesResponse { notes }))
}

#[utoipa::path(
    post,
    path = "/v1/match/{id}/notes",
    tag = "jegyzet",
    summary = "Új jegyzet rögzítése (ops token, auditált)",
    params(("id" = String, Path, description = "Meccs UUID")),
    request_body = NoteCreateRequest,
    responses(
        (status = 201, body = MatchNoteRow),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn create_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<NoteCreateRequest>,
) -> Result<(StatusCode, Json<MatchNoteRow>), AppError> {
    authorize(&headers, Scope::Ops, &state.read_token, &state.ops_token)?;
    let match_id =
        Uuid::parse_str(&id).map_err(|_| AppError::bad_request("Érvénytelen meccs-azonosító."))?;

    let author = validate_note_author(&body.author)
        .map_err(|error: NoteError| AppError::bad_request(error.to_string()))?;
    let text = validate_note_body(&body.body)
        .map_err(|error: NoteError| AppError::bad_request(error.to_string()))?;
    let kind = NoteKind::parse(&body.kind).ok_or_else(|| {
        AppError::bad_request(format!("Ismeretlen jegyzet-típus: {}.", body.kind))
    })?;

    if !match_exists(&state.pool, match_id).await.map_err(|err| {
        tracing::error!(error = %err, "jegyzet meccs-ellenőrzés");
        AppError::internal("A meccs ellenőrzése sikertelen.")
    })? {
        return Err(AppError::not_found("A meccs nem található."));
    }

    let note =
        football_storage::repositories::note::insert(&state.pool, match_id, author, text, kind)
            .await
            .map_err(|err| {
                tracing::error!(error = %err, %match_id, "jegyzet írása sikertelen");
                AppError::internal("A jegyzet rögzítése sikertelen.")
            })?;

    audit(
        &state.pool,
        author,
        "note_added",
        match_id,
        json!({ "note_id": note.id, "kind": kind.as_db_str() }),
    )
    .await
    .map_err(|err| {
        tracing::error!(error = %err, "jegyzet audit írása sikertelen");
        AppError::internal("A jegyzet rögzítése sikertelen.")
    })?;

    Ok((StatusCode::CREATED, Json(note)))
}

#[utoipa::path(
    patch,
    path = "/v1/note/{id}",
    tag = "jegyzet",
    summary = "Jegyzet szerkesztése vagy lezárása (ops token, auditált)",
    params(("id" = String, Path, description = "Jegyzet UUID")),
    request_body = NotePatchRequest,
    responses(
        (status = 200, body = MatchNoteRow),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn edit_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<NotePatchRequest>,
) -> Result<Json<MatchNoteRow>, AppError> {
    authorize(&headers, Scope::Ops, &state.read_token, &state.ops_token)?;
    let note_id = Uuid::parse_str(&id)
        .map_err(|_| AppError::bad_request("Érvénytelen jegyzet-azonosító."))?;

    if let Some(text) = body.body.as_deref() {
        validate_note_body(text)
            .map_err(|error: NoteError| AppError::bad_request(error.to_string()))?;
    }
    let kind = body
        .kind
        .as_deref()
        .map(|raw| {
            NoteKind::parse(raw)
                .ok_or_else(|| AppError::bad_request(format!("Ismeretlen jegyzet-típus: {raw}.")))
        })
        .transpose()?;

    let patch = NotePatch {
        body: body.body,
        kind,
        resolved: body.resolved,
    };
    let note = football_storage::repositories::note::update(&state.pool, note_id, &patch)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, %note_id, "jegyzet szerkesztése sikertelen");
            AppError::internal("A jegyzet szerkesztése sikertelen.")
        })?
        .ok_or_else(|| AppError::not_found("A jegyzet nem található."))?;

    audit(
        &state.pool,
        &note.author,
        "note_edited",
        note.match_id,
        json!({
            "note_id": note.id,
            "body_changed": patch.body.is_some(),
            "kind": patch.kind.map(|value| value.as_db_str()),
            "resolved": patch.resolved
        }),
    )
    .await
    .map_err(|err| {
        tracing::error!(error = %err, "jegyzet audit írása sikertelen");
        AppError::internal("A jegyzet szerkesztése sikertelen.")
    })?;

    Ok(Json(note))
}
