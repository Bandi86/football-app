//! `GET /v1/issues` — tartós hiányjelek szűrhető listája idővonallal.
//! Read tokennel olvasható; a jeleket a detektor (későbbi szelet) írja.

use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use chrono::NaiveDate;
use football_domain::ops::{IssueCode, IssueSeverity};
use football_storage::repositories::issue::{IssueFilter, IssueRow, IssueScope};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    AppState,
    auth::{Scope, authorize},
    error::{ApiError, AppError},
};

#[derive(Debug, Deserialize)]
pub struct IssuesQuery {
    /// `day` (meccsnap szerint, Europe/Budapest) vagy `all`.
    #[serde(default = "default_scope")]
    pub scope: String,
    pub date: Option<String>,
    pub code: Option<String>,
    pub severity: Option<String>,
    #[serde(default)]
    pub open_only: bool,
}

fn default_scope() -> String {
    "all".to_owned()
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct IssuesResponse {
    pub issues: Vec<IssueRow>,
    pub count: usize,
}

#[utoipa::path(
    get,
    path = "/v1/issues",
    tag = "hiány",
    summary = "Hiányjelek listája első/utolsó láttatási idővel",
    params(
        ("scope" = Option<String>, Query, description = "day vagy all (alap: all)"),
        ("date" = Option<String>, Query, description = "YYYY-MM-DD, scope=day esetén kötelező"),
        ("code" = Option<String>, Query, description = "Hiány-kód szűrő"),
        ("severity" = Option<String>, Query, description = "info|warning|hard"),
        ("open_only" = Option<bool>, Query, description = "Csak megoldatlan jelek")
    ),
    responses(
        (status = 200, body = IssuesResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn issues(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<IssuesQuery>,
) -> Result<Json<IssuesResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;

    let scope = match query.scope.as_str() {
        "day" => {
            let raw = query.date.as_deref().ok_or_else(|| {
                AppError::bad_request("A scope=day használathoz date (YYYY-MM-DD) szükséges.")
            })?;
            let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                .map_err(|_| AppError::bad_request("A dátum formátuma YYYY-MM-DD legyen."))?;
            IssueScope::Day(date)
        }
        "all" => IssueScope::All,
        other => {
            return Err(AppError::bad_request(format!(
                "A scope csak 'day' vagy 'all' lehet: {other}."
            )));
        }
    };

    let code = query
        .code
        .as_deref()
        .map(|raw| {
            IssueCode::parse(raw)
                .ok_or_else(|| AppError::bad_request(format!("Ismeretlen hiány-kód: {raw}.")))
        })
        .transpose()?;
    let severity = query
        .severity
        .as_deref()
        .map(|raw| {
            IssueSeverity::parse(raw)
                .ok_or_else(|| AppError::bad_request(format!("Ismeretlen súlyosság: {raw}.")))
        })
        .transpose()?;

    let filter = IssueFilter {
        scope,
        timezone: "Europe/Budapest".to_owned(),
        code,
        severity,
        open_only: query.open_only,
    };
    let rows = football_storage::repositories::issue::list(&state.pool, &filter)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "hiányjelek lekérdezése");
            AppError::internal("A hiányjelek lekérdezése sikertelen.")
        })?;
    let count = rows.len();
    Ok(Json(IssuesResponse {
        issues: rows,
        count,
    }))
}
