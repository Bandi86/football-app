//! Breaker állapotgép: tiszta `next_state` + async repo-wrapperek.
//! Szemantika: nincs sor = closed; open + next_probe_at <= now → half_open
//! átmenet, egyetlen probe; half_open alatt további fetch tiltott.
//! Auth-hiba azonnali open + `policy_review` audit-esemény.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use football_domain::policy::{BreakerState, SourcePolicy};
use football_storage::repositories::worker::{self as repo, BreakerRow, BreakerWrite};
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
    repo::update_breaker_locked(pool, &policy.source_slug, |current| {
        let Some(row) = current else {
            return (true, BreakerWrite::Keep);
        };
        match row.state {
            BreakerState::Closed => (true, BreakerWrite::Keep),
            BreakerState::HalfOpen => (false, BreakerWrite::Keep),
            BreakerState::Open => {
                if row.next_probe_at.is_some_and(|probe| probe <= now) {
                    (
                        true,
                        BreakerWrite::Set(BreakerRow {
                            state: BreakerState::HalfOpen,
                            ..row.clone()
                        }),
                    )
                } else {
                    (false, BreakerWrite::Keep)
                }
            }
        }
    })
    .await
}

/// Fetch előtti kapublokk esetén a felhasználatlan half_open probe visszaadása.
/// Csak a half_open állapotot módosítja; későbbi siker/hiba döntése így nem veszhet el.
pub async fn release_probe(pool: &PgPool, slug: &str) -> Result<()> {
    repo::update_breaker_locked(pool, slug, |current| {
        let Some(row) = current else {
            return ((), BreakerWrite::Keep);
        };
        if row.state == BreakerState::HalfOpen {
            (
                (),
                BreakerWrite::Set(BreakerRow {
                    state: BreakerState::Open,
                    ..row.clone()
                }),
            )
        } else {
            ((), BreakerWrite::Keep)
        }
    })
    .await
}

pub async fn on_success(pool: &PgPool, slug: &str) -> Result<()> {
    repo::update_breaker_locked(pool, slug, |_| ((), BreakerWrite::Set(closed_row()))).await
}

pub async fn on_failure(pool: &PgPool, policy: &SourcePolicy, now: DateTime<Utc>) -> Result<()> {
    repo::update_breaker_locked(pool, &policy.source_slug, |current| {
        let current = current.cloned().unwrap_or_else(closed_row);
        (
            (),
            BreakerWrite::Set(next_state(&current, false, policy, now)),
        )
    })
    .await
}

/// Auth-hiba (401/403, login-redirect): azonnali open, küszöbtől függetlenül,
/// plusz `policy_review` audit-esemény a kézi áttekintéshez.
pub async fn on_auth_failure(
    pool: &PgPool,
    policy: &SourcePolicy,
    now: DateTime<Utc>,
) -> Result<()> {
    let detail = serde_json::json!({
        "slug": policy.source_slug,
        "status": "auth_failure",
    });
    repo::record_auth_failure(pool, policy, now, &detail).await
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
