//! Üzemeltetési állapot (S8): worker-számlálók, függő review-k és
//! forrásonkénti policy/breaker/kvóta — read-only, ops-token.

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use chrono::Utc;
use serde::Serialize;

use crate::AppState;
use crate::auth::{Scope, authorize};
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub struct OpsStatus {
    pub pending_jobs: i64,
    pub leased_jobs: i64,
    pub quarantined: i64,
    pub pending_reviews: i64,
    pub sources: Vec<OpsSource>,
}

#[derive(Debug, Serialize)]
pub struct OpsSource {
    pub source_slug: String,
    pub mode: String,
    pub enabled: bool,
    pub attempts_today: i64,
    pub daily_budget: i32,
    pub breaker_state: String,
}

pub async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<OpsStatus>, AppError> {
    authorize(&headers, Scope::Ops, &state.read_token, &state.ops_token)?;

    let counts = football_storage::repositories::worker::worker_status_counts(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!(error=%e, "ops status számlálók");
            AppError::internal("Az üzemeltetési állapot lekérdezése sikertelen.")
        })?;
    let policies = football_storage::repositories::worker::list_policies(&state.pool)
        .await
        .map_err(|e| {
            tracing::error!(error=%e, "ops status policy-k");
            AppError::internal("Az üzemeltetési állapot lekérdezése sikertelen.")
        })?;

    let today = Utc::now().date_naive();
    let mut sources = Vec::with_capacity(policies.len());
    for policy in policies {
        let attempts = football_storage::repositories::worker::count_fetch_attempts_today(
            &state.pool,
            &policy.source_slug,
            today,
        )
        .await
        .unwrap_or(0);
        let breaker =
            football_storage::repositories::worker::get_breaker(&state.pool, &policy.source_slug)
                .await
                .ok()
                .flatten()
                .map(|row| row.state.as_db_str().to_string())
                .unwrap_or_else(|| "closed".to_string());
        sources.push(OpsSource {
            source_slug: policy.source_slug,
            mode: policy.mode.as_db_str().to_string(),
            enabled: policy.enabled,
            attempts_today: attempts,
            daily_budget: policy.daily_budget,
            breaker_state: breaker,
        });
    }

    Ok(Json(OpsStatus {
        pending_jobs: counts.jobs_pending,
        leased_jobs: counts.jobs_leased,
        quarantined: counts.quarantined,
        pending_reviews: counts.reviews_pending,
        sources,
    }))
}
