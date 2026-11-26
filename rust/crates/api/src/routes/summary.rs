//! `GET /v1/ops/summary` — napi hiány-összefoglaló magyar mondattal és
//! kiemelt akciókkal. Csak `ops.issue` adatokból; nincs benne jegyzet.

use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use chrono::{Datelike, NaiveDate};
use football_domain::ops::IssueCode;
use football_storage::repositories::issue::{IssueFilter, IssueScope};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{Scope, authorize},
    error::{ApiError, AppError},
};

#[derive(Debug, Deserialize)]
pub struct SummaryQuery {
    pub date: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IssueCodeCount {
    pub code: String,
    pub label_hu: String,
    pub count: usize,
    pub hard: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OpsAction {
    pub kind: String,
    pub label_hu: String,
    pub issue_id: Uuid,
    pub subject_kind: String,
    pub subject_id: Uuid,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OpsSummary {
    pub date: String,
    pub open_total: usize,
    pub previous_day_total: usize,
    pub by_code: Vec<IssueCodeCount>,
    pub top_actions: Vec<OpsAction>,
    pub headline_hu: String,
}

#[utoipa::path(
    get,
    path = "/v1/ops/summary",
    tag = "ops",
    summary = "Napi hiány-összefoglaló magyar mondattal és kiemelt akciókkal",
    params(("date" = Option<String>, Query, description = "YYYY-MM-DD (alap: ma)")),
    responses(
        (status = 200, body = OpsSummary),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn summary(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SummaryQuery>,
) -> Result<Json<OpsSummary>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let date = query
        .date
        .as_deref()
        .map(|raw| {
            NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                .map_err(|_| AppError::bad_request("A dátum formátuma YYYY-MM-DD legyen."))
        })
        .transpose()?
        .unwrap_or_else(|| chrono::Utc::now().date_naive());

    let filter = |day| IssueFilter {
        scope: IssueScope::Day(day),
        timezone: "Europe/Budapest".to_owned(),
        open_only: true,
        ..IssueFilter::default()
    };
    let open = football_storage::repositories::issue::list(&state.pool, &filter(date))
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "összefoglaló hiányjelek");
            AppError::internal("A hiányjelek összefoglalója sikertelen.")
        })?;
    let previous = football_storage::repositories::issue::list(
        &state.pool,
        &filter(date - chrono::Duration::days(1)),
    )
    .await
    .map_err(|err| {
        tracing::error!(error = %err, "összefoglaló előző nap");
        AppError::internal("A hiányjelek összefoglalója sikertelen.")
    })?;

    let mut by_code: Vec<IssueCodeCount> = Vec::new();
    for row in &open {
        let code = IssueCode::parse(&row.code);
        let label = code
            .map(|code| code.label_hu())
            .unwrap_or(&row.code)
            .to_owned();
        if let Some(entry) = by_code.iter_mut().find(|entry| entry.code == row.code) {
            entry.count += 1;
            if row.severity == "hard" {
                entry.hard += 1;
            }
        } else {
            by_code.push(IssueCodeCount {
                code: row.code.clone(),
                label_hu: label,
                count: 1,
                hard: usize::from(row.severity == "hard"),
            });
        }
    }
    by_code.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.code.cmp(&b.code)));

    let hard_total: usize = open.iter().filter(|row| row.severity == "hard").count();
    let top_actions: Vec<OpsAction> = open
        .iter()
        .filter(|row| row.severity != "info")
        .take(5)
        .map(|row| OpsAction {
            kind: "open_issue".to_owned(),
            label_hu: format!(
                "Jel megtekintése: {}",
                IssueCode::parse(&row.code)
                    .map(|code| code.label_hu())
                    .unwrap_or(&row.code)
            ),
            issue_id: row.id,
            subject_kind: row.subject_kind.clone(),
            subject_id: row.subject_id,
        })
        .collect();

    let headline_hu = if open.is_empty() {
        "Nincs nyitott hiányjel ezen a napon.".to_owned()
    } else {
        let top = by_code
            .first()
            .map(|entry| format!(" Leggyakoribb: {} ({}).", entry.label_hu, entry.count))
            .unwrap_or_default();
        let hard = if hard_total > 0 {
            format!(" Ebből {hard_total} súlyos.")
        } else {
            String::new()
        };
        format!(
            "{}. {:02}. {:02}.: {} nyitott hiányjel.{hard}{top}",
            date.year(),
            date.month(),
            date.day(),
            open.len()
        )
    };

    Ok(Json(OpsSummary {
        date: date.to_string(),
        open_total: open.len(),
        previous_day_total: previous.len(),
        by_code,
        top_actions,
        headline_hu,
    }))
}
