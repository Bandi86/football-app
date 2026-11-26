//! `POST /v1/issue/{id}/resolve` — hiányjel megoldottnak jelölése (ops token).
//! A jel megmarad az idővonalban; csak a `resolved_at` töltődik ki.

use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use football_storage::repositories::issue;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{Scope, authorize},
    error::{ApiError, AppError},
};

#[derive(Debug, Serialize, ToSchema)]
pub struct ResolveResponse {
    pub issue_id: Uuid,
    pub resolved: bool,
}

#[utoipa::path(
    post,
    path = "/v1/issue/{id}/resolve",
    tag = "hiány",
    summary = "Hiányjel lezárása (ops token, auditált, idempotens)",
    params(("id" = String, Path, description = "Hiány UUID")),
    responses(
        (status = 200, body = ResolveResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn resolve_issue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ResolveResponse>, AppError> {
    authorize(&headers, Scope::Ops, &state.read_token, &state.ops_token)?;
    let issue_id =
        Uuid::parse_str(&id).map_err(|_| AppError::bad_request("Érvénytelen jel-azonosító."))?;
    let row = issue::get(&state.pool, issue_id)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "jel lekérdezése");
            AppError::internal("A hiányjel lekérdezése sikertelen.")
        })?
        .ok_or_else(|| AppError::not_found("A hiányjel nem található."))?;

    let changed = issue::resolve(&state.pool, issue_id).await.map_err(|err| {
        tracing::error!(error = %err, "jel lezárása");
        AppError::internal("A hiányjel lezárása sikertelen.")
    })?;

    if changed {
        sqlx::query(
            "INSERT INTO ops.audit_event
              (id, actor, action, subject_kind, subject_id, detail)
             VALUES ($1, 'ui', 'issue_resolved', $2, $3, $4)",
        )
        .bind(Uuid::now_v7())
        .bind(&row.subject_kind)
        .bind(row.subject_id)
        .bind(serde_json::json!({ "issue_id": issue_id, "code": row.code }))
        .execute(&state.pool)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "jel audit írása");
            AppError::internal("A hiányjel lezárása sikertelen.")
        })?;
    }

    Ok(Json(ResolveResponse {
        issue_id,
        resolved: row.resolved_at.is_some() || changed,
    }))
}
