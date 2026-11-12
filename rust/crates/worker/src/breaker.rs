//! Breaker állapotgép: tiszta `next_state` + async repo-wrapperek.
//! Szemantika: nincs sor = closed; open + next_probe_at <= now → half_open
//! átmenet, egyetlen probe; half_open alatt további fetch tiltott.
//! Auth-hiba azonnali open + `policy_review` audit-esemény.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use football_domain::policy::{BreakerState, SourcePolicy};
use football_storage::repositories::worker::{self as repo, BreakerRow};
use sqlx::PgPool;

fn closed_row() -> BreakerRow {
    BreakerRow {
        state: BreakerState::Closed,
        failure_count: 0,
        opened_at: None,
        next_probe_at: None,
    }
}

/// Ha a cooldown nem ábrázolható, next_probe_at None marad: az `allow`
/// így sosem enged probe-ot (fail-closed), kézi reset kell.
fn probe_time(now: DateTime<Utc>, policy: &SourcePolicy) -> Option<DateTime<Utc>> {
    Duration::try_seconds(policy.breaker_cooldown_secs).and_then(|d| now.checked_add_signed(d))
}

/// Tiszta állapotátmenet. Sikeres fetch minden állapotból closed-ra zár
/// és nullázza a számlálót. Hiba: half_open-ból azonnal open; egyébként
/// failure_count + 1, és a küszöb elérésekor open (cooldown a policy-ból).
pub fn next_state(
    row: &BreakerRow,
    success: bool,
    policy: &SourcePolicy,
    now: DateTime<Utc>,
) -> BreakerRow {
    if success {
        return closed_row();
    }
    let failure_count = row.failure_count.saturating_add(1);
    let trips = row.state == BreakerState::HalfOpen || failure_count >= policy.breaker_threshold;
    if trips {
        BreakerRow {
            state: BreakerState::Open,
            failure_count,
            opened_at: Some(now),
            next_probe_at: probe_time(now, policy),
        }
    } else {
        BreakerRow {
            state: BreakerState::Closed,
            failure_count,
            opened_at: None,
            next_probe_at: None,
        }
    }
}

pub async fn allow(pool: &PgPool, policy: &SourcePolicy, now: DateTime<Utc>) -> Result<bool> {
    let Some(row) = repo::get_breaker(pool, &policy.source_slug).await? else {
        // nincs breaker-sor → closed-ként kezeljük
        return Ok(true);
    };
    match row.state {
        BreakerState::Closed => Ok(true),
        // egyetlen probe lehet folyamatban
        BreakerState::HalfOpen => Ok(false),
        BreakerState::Open => {
            if row.next_probe_at.is_some_and(|t| t <= now) {
                let probe = BreakerRow {
                    state: BreakerState::HalfOpen,
                    ..row
                };
                repo::set_breaker(pool, &policy.source_slug, &probe).await?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}

pub async fn on_success(pool: &PgPool, slug: &str) -> Result<()> {
    repo::set_breaker(pool, slug, &closed_row()).await
}

pub async fn on_failure(pool: &PgPool, policy: &SourcePolicy, now: DateTime<Utc>) -> Result<()> {
    let current = repo::get_breaker(pool, &policy.source_slug)
        .await?
        .unwrap_or_else(closed_row);
    let next = next_state(&current, false, policy, now);
    repo::set_breaker(pool, &policy.source_slug, &next).await
}

/// Auth-hiba (401/403, login-redirect): azonnali open, küszöbtől függetlenül,
/// plusz `policy_review` audit-esemény a kézi áttekintéshez.
pub async fn on_auth_failure(
    pool: &PgPool,
    policy: &SourcePolicy,
    now: DateTime<Utc>,
) -> Result<()> {
    let current = repo::get_breaker(pool, &policy.source_slug)
        .await?
        .unwrap_or_else(closed_row);
    let next = BreakerRow {
        state: BreakerState::Open,
        failure_count: current.failure_count.saturating_add(1),
        opened_at: Some(now),
        next_probe_at: probe_time(now, policy),
    };
    let detail = serde_json::json!({
        "slug": policy.source_slug,
        "status": "auth_failure",
    });
    // A breaker-nyitás és az audit-esemény egyetlen tranzakcióban (storage).
    repo::record_auth_failure(pool, &policy.source_slug, &next, &detail).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Duration, Utc};
    use football_domain::policy::{BreakerState, SourceMode, SourcePolicy, Transport};
    use football_storage::repositories::worker::BreakerRow;

    fn now() -> DateTime<Utc> {
        DateTime::from_timestamp(1_763_000_000, 0).unwrap()
    }

    fn policy() -> SourcePolicy {
        SourcePolicy {
            source_slug: "demo-synthetic".into(),
            transport: Transport::Fixture,
            mode: SourceMode::Shadow,
            daily_budget: 10,
            min_delay_ms: 0,
            jitter_ms: 0,
            breaker_threshold: 3,
            breaker_cooldown_secs: 60,
            adapter_version: "1.0".into(),
            parser_version: "1.0".into(),
            domains: vec![],
            events: vec![],
            enabled: true,
            max_bytes: 5_242_880,
        }
    }

    fn closed_row(failure_count: i32) -> BreakerRow {
        BreakerRow {
            state: BreakerState::Closed,
            failure_count,
            opened_at: None,
            next_probe_at: None,
        }
    }

    #[test]
    fn closed_failure_below_threshold_stays_closed() {
        let next = next_state(&closed_row(1), false, &policy(), now());
        assert_eq!(next.state, BreakerState::Closed);
        assert_eq!(next.failure_count, 2);
        assert_eq!(next.next_probe_at, None);
    }

    #[test]
    fn closed_failure_at_threshold_opens_with_cooldown() {
        let next = next_state(&closed_row(2), false, &policy(), now());
        assert_eq!(next.state, BreakerState::Open);
        assert_eq!(next.failure_count, 3);
        assert_eq!(next.opened_at, Some(now()));
        assert_eq!(next.next_probe_at, Some(now() + Duration::seconds(60)));
    }

    #[test]
    fn half_open_success_closes_and_resets() {
        let row = BreakerRow {
            state: BreakerState::HalfOpen,
            failure_count: 3,
            opened_at: Some(now()),
            next_probe_at: Some(now()),
        };
        let next = next_state(&row, true, &policy(), now());
        assert_eq!(next.state, BreakerState::Closed);
        assert_eq!(next.failure_count, 0);
        assert_eq!(next.opened_at, None);
        assert_eq!(next.next_probe_at, None);
    }

    #[test]
    fn half_open_failure_reopens_immediately() {
        // half_open-ból a hiba küszöbtől függetlenül azonnal open
        let row = BreakerRow {
            state: BreakerState::HalfOpen,
            failure_count: 0,
            opened_at: Some(now()),
            next_probe_at: Some(now()),
        };
        let next = next_state(&row, false, &policy(), now());
        assert_eq!(next.state, BreakerState::Open);
        assert_eq!(next.failure_count, 1);
        assert_eq!(next.opened_at, Some(now()));
        assert_eq!(next.next_probe_at, Some(now() + Duration::seconds(60)));
    }
}
