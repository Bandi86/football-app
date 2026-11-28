//! Breaker állapotgép: tiszta `next_state` + async repo-wrapperek.
//! Szemantika: nincs sor = closed; open + next_probe_at <= now → half_open
//! átmenet, egyetlen probe; half_open alatt további fetch tiltott.
//! Auth-hiba azonnali open + `policy_review` audit-esemény.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use football_domain::policy::{BreakerState, SourcePolicy};
use football_storage::repositories::worker::{self as repo, BreakerRow, BreakerWrite};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerDecision {
    Allowed(Option<Uuid>),
    Blocked,
}

fn closed_row() -> BreakerRow {
    BreakerRow {
        state: BreakerState::Closed,
        failure_count: 0,
        opened_at: None,
        next_probe_at: None,
        probe_token: None,
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
            probe_token: None,
        }
    } else {
        BreakerRow {
            state: BreakerState::Closed,
            failure_count,
            opened_at: None,
            next_probe_at: None,
            probe_token: None,
        }
    }
}

pub async fn allow(
    pool: &PgPool,
    policy: &SourcePolicy,
    now: DateTime<Utc>,
) -> Result<BreakerDecision> {
    repo::update_breaker_locked(pool, &policy.source_slug, |current| {
        let Some(row) = current else {
            return (BreakerDecision::Allowed(None), BreakerWrite::Keep);
        };
        match row.state {
            BreakerState::Closed => (BreakerDecision::Allowed(None), BreakerWrite::Keep),
            BreakerState::HalfOpen => {
                if row.probe_token.is_none() && row.next_probe_at.is_some_and(|probe| probe <= now)
                {
                    let token = Uuid::now_v7();
                    (
                        BreakerDecision::Allowed(Some(token)),
                        BreakerWrite::Set(BreakerRow {
                            state: BreakerState::HalfOpen,
                            probe_token: Some(token),
                            ..row.clone()
                        }),
                    )
                } else {
                    (BreakerDecision::Blocked, BreakerWrite::Keep)
                }
            }
            BreakerState::Open => {
                if row.next_probe_at.is_some_and(|probe| probe <= now) {
                    let token = Uuid::now_v7();
                    (
                        BreakerDecision::Allowed(Some(token)),
                        BreakerWrite::Set(BreakerRow {
                            state: BreakerState::HalfOpen,
                            probe_token: Some(token),
                            ..row.clone()
                        }),
                    )
                } else {
                    (BreakerDecision::Blocked, BreakerWrite::Keep)
                }
            }
        }
    })
    .await
}

/// Csak a saját, még aktív half_open generációt adja vissza open állapotba.
pub async fn release_probe(pool: &PgPool, slug: &str, token: Uuid) -> Result<()> {
    repo::update_breaker_locked(pool, slug, |current| {
        let Some(row) = current else {
            return ((), BreakerWrite::Keep);
        };
        if row.state == BreakerState::HalfOpen && row.probe_token == Some(token) {
            (
                (),
                BreakerWrite::Set(BreakerRow {
                    state: BreakerState::Open,
                    probe_token: None,
                    ..row.clone()
                }),
            )
        } else {
            ((), BreakerWrite::Keep)
        }
    })
    .await
}

fn owns_current_probe(row: &BreakerRow, token: Option<Uuid>) -> bool {
    match row.state {
        BreakerState::HalfOpen => row.probe_token.is_some() && row.probe_token == token,
        _ => token.is_none(),
    }
}

pub async fn on_success(pool: &PgPool, slug: &str, token: Option<Uuid>) -> Result<()> {
    repo::update_breaker_locked(pool, slug, |current| {
        let Some(row) = current else {
            return ((), BreakerWrite::Keep);
        };
        if owns_current_probe(row, token) {
            ((), BreakerWrite::Set(closed_row()))
        } else {
            ((), BreakerWrite::Keep)
        }
    })
    .await
}

pub async fn on_failure(
    pool: &PgPool,
    policy: &SourcePolicy,
    now: DateTime<Utc>,
    token: Option<Uuid>,
) -> Result<()> {
    repo::update_breaker_locked(pool, &policy.source_slug, |current| {
        let current = current.cloned().unwrap_or_else(closed_row);
        if !owns_current_probe(&current, token) {
            return ((), BreakerWrite::Keep);
        }
        (
            (),
            BreakerWrite::Set(next_state(&current, false, policy, now)),
        )
    })
    .await
}

/// A probe tulajdonosa a fetch előtti összes exit és cancellation ágán.
/// A Drop csak akkor indít cleanup-t, ha a probe még nem véglegesített vagy
/// Explicit felszabadításra került.
pub struct ProbeGuard {
    pool: PgPool,
    source_slug: String,
    token: Option<Uuid>,
}

impl ProbeGuard {
    pub fn new(pool: &PgPool, source_slug: &str, decision: BreakerDecision) -> Self {
        let token = match decision {
            BreakerDecision::Allowed(token) => token,
            BreakerDecision::Blocked => None,
        };
        Self {
            pool: pool.clone(),
            source_slug: source_slug.to_owned(),
            token,
        }
    }

    pub fn token(&self) -> Option<Uuid> {
        self.token
    }

    pub async fn release(&mut self) -> Result<()> {
        if let Some(token) = self.token {
            release_probe(&self.pool, &self.source_slug, token).await?;
            self.token = None;
        }
        Ok(())
    }

    pub fn complete(&mut self) {
        self.token = None;
    }
}

impl Drop for ProbeGuard {
    fn drop(&mut self) {
        let Some(token) = self.token.take() else {
            return;
        };
        let pool = self.pool.clone();
        let source_slug = self.source_slug.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if let Err(err) = release_probe(&pool, &source_slug, token).await {
                    tracing::error!(
                        source = %source_slug,
                        error = %err,
                        "felhasználatlan breaker probe felszabadítása"
                    );
                }
            });
        } else {
            tracing::error!(source = %source_slug, "breaker probe cleanup runtime nélkül");
        }
    }
}

/// Auth-hiba (401/403, login-redirect): azonnali open, küszöbtől függetlenül,
/// plusz `policy_review` audit-esemény a kézi áttekintéshez.
pub async fn on_auth_failure(
    pool: &PgPool,
    policy: &SourcePolicy,
    now: DateTime<Utc>,
    token: Option<Uuid>,
) -> Result<()> {
    let detail = serde_json::json!({
        "slug": policy.source_slug,
        "status": "auth_failure",
    });
    repo::record_auth_failure(pool, policy, now, token, &detail).await
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
            probe_token: None,
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
            probe_token: Some(uuid::Uuid::now_v7()),
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
            probe_token: Some(uuid::Uuid::now_v7()),
        };
        let next = next_state(&row, false, &policy(), now());
        assert_eq!(next.state, BreakerState::Open);
        assert_eq!(next.failure_count, 1);
        assert_eq!(next.opened_at, Some(now()));
        assert_eq!(next.next_probe_at, Some(now() + Duration::seconds(60)));
    }
}
