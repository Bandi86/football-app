//! Coverage-slot wrapper (Task 10): a tick a job domainjét entity_type-ként,
//! az eseménytípusát data_kind-ként rögzíti az adott időablakra.

use anyhow::Result;
use football_domain::policy::CoverageState;
use football_storage::repositories::worker as repo;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn set(
    pool: &PgPool,
    slug: &str,
    entity_type: &str,
    data_kind: &str,
    time_window: &str,
    state: CoverageState,
    job_id: Uuid,
) -> Result<()> {
    repo::upsert_coverage(
        pool,
        slug,
        entity_type,
        data_kind,
        time_window,
        state,
        Some(job_id),
    )
    .await
}
