//! Kvóta- és delay-kapu: a fetch előtti döntés, ledgerbe NEM ír
//! (az elszámolás a tickben történik). A jitter a scheduler next due
//! számításához tartozik, nem a guardhoz.

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use football_domain::policy::SourcePolicy;
use football_storage::repositories::worker::{count_fetch_attempts_today, last_attempt_at};
use sqlx::PgPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaDecision {
    Allowed,
    QuotaBlocked,
    DelayBlocked,
}

pub async fn guard(
    pool: &PgPool,
    policy: &SourcePolicy,
    now: DateTime<Utc>,
) -> Result<QuotaDecision> {
    let attempts = count_fetch_attempts_today(pool, &policy.source_slug, now.date_naive()).await?;
    if attempts >= i64::from(policy.daily_budget) {
        return Ok(QuotaDecision::QuotaBlocked);
    }
    // A last_attempt_at csak a számláló (tényleges fetch) outcome-okat adja
    // vissza (Task 10 ruling): a delay az utolsó valódi kérés óta telik, a
    // blokkolt ledger-sorok nem resetelik az órát (nincs DelayBlocked-livelock).
    if let Some(last) = last_attempt_at(pool, &policy.source_slug).await? {
        let delay = Duration::try_milliseconds(policy.min_delay_ms)
            .context("quota: min_delay_ms nem ábrázolható időtartam")?;
        match last.checked_add_signed(delay) {
            Some(earliest) if earliest > now => return Ok(QuotaDecision::DelayBlocked),
            // időtúlcsordulás: fail-closed, nem engedélyezünk vakon
            None => return Ok(QuotaDecision::DelayBlocked),
            _ => {}
        }
    }
    Ok(QuotaDecision::Allowed)
}
