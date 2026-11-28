//! Atom fetch reservation: a kvóta, min-delay és ledger-foglalás
//! egy per-source tranzakcióban dönt.

use anyhow::Result;
use football_domain::policy::JobLease;
use football_storage::repositories::worker::{FetchReservation, reserve_fetch};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaDecision {
    Allowed(Uuid),
    QuotaBlocked,
    DelayBlocked,
    LeaseLost,
}

pub async fn reserve(
    pool: &PgPool,
    source_slug: &str,
    request_key: &str,
    lease: &JobLease,
) -> Result<QuotaDecision> {
    match reserve_fetch(pool, source_slug, request_key, lease.job_id, lease.token).await? {
        FetchReservation::Reserved(id) => Ok(QuotaDecision::Allowed(id)),
        FetchReservation::QuotaBlocked => Ok(QuotaDecision::QuotaBlocked),
        FetchReservation::DelayBlocked => Ok(QuotaDecision::DelayBlocked),
        FetchReservation::LeaseLost => Ok(QuotaDecision::LeaseLost),
    }
}
