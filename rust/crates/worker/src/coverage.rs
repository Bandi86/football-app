//! Coverage-slot wrapper: a tick a job domainjét entity_type-ként,
//! az eseménytípusát data_kind-ként rögzíti az adott időablakra.

use anyhow::Result;
use football_domain::policy::{CoverageState, LeasedJob};
use football_storage::repositories::worker as repo;
use sqlx::PgPool;

pub async fn set(pool: &PgPool, leased: &LeasedJob, state: CoverageState) -> Result<bool> {
    repo::upsert_job_coverage(
        pool,
        &leased.lease,
        &leased.job.domain,
        &leased.job.event_type,
        &leased.job.time_window,
        state,
    )
    .await
}
