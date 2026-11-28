//! Kvóta- és delay-kapu: a reserve atomi ledger-foglalást készít,
//! a guard a még használatban lévő régi tick olvasási kapuja.

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use football_domain::policy::{JobLease, SourcePolicy};
use football_storage::repositories::worker::{
    FetchReservation, count_fetch_attempts_today, last_attempt_at, reserve_fetch,
};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaDecision {
    Allowed(Uuid),
    QuotaBlocked,
    DelayBlocked,
    LeaseLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardDecision {
    Allowed,
    QuotaBlocked,
    DelayBlocked,
}

pub async fn reserve(
    pool: &PgPool,
    source_slug: &str,
    request_key: &str,
    lease: &JobLease,
    now: DateTime<Utc>,
) -> Result<QuotaDecision> {
    match reserve_fetch(
        pool,
        source_slug,
        request_key,
        lease.job_id,
        lease.token,
        now,
    )
    .await?
    {
        FetchReservation::Reserved(id) => Ok(QuotaDecision::Allowed(id)),
        FetchReservation::QuotaBlocked => Ok(QuotaDecision::QuotaBlocked),
        FetchReservation::DelayBlocked => Ok(QuotaDecision::DelayBlocked),
        FetchReservation::LeaseLost => Ok(QuotaDecision::LeaseLost),
    }
}

pub async fn guard(
    pool: &PgPool,
    policy: &SourcePolicy,
    now: DateTime<Utc>,
) -> Result<GuardDecision> {
    let attempts = count_fetch_attempts_today(pool, &policy.source_slug, now.date_naive()).await?;
    if attempts >= i64::from(policy.daily_budget) {
        return Ok(GuardDecision::QuotaBlocked);
    }
    // A last_attempt_at csak a számláló (tényleges fetch) outcome-okat adja
    // vissza (Task 10 ruling): a delay az utolsó valódi kérés óta telik, a
    // blokkolt ledger-sorok nem resetelik az órát (nincs DelayBlocked-livelock).
    if let Some(last) = last_attempt_at(pool, &policy.source_slug).await? {
        let delay = Duration::try_milliseconds(policy.min_delay_ms)
            .context("quota: min_delay_ms nem ábrázolható időtartam")?;
        match last.checked_add_signed(delay) {
            Some(earliest) if earliest > now => return Ok(GuardDecision::DelayBlocked),
            // időtúlcsordulás: fail-closed, nem engedélyezünk vakon
            None => return Ok(GuardDecision::DelayBlocked),
            _ => {}
        }
    }
    Ok(GuardDecision::Allowed)
}
