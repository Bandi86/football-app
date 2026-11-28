//! Worker futtató ciklus (Task 11): ütemezett tick + graceful shutdown.
//!
//! A ciklus `tokio::select!`-tel fut: a shutdown ágon (watch = true vagy a
//! feladó megszűnt) kilép, a tick ágon egy teljes kört futtat (policy-k →
//! job-generálás → tick). Az első tick azonnal lefut (tokio interval), a
//! továbbiak a tick_interval szerint. A tick közben érkező shutdown nem
//! szakítja félbe a kört: a ciklus befejezi az aktuális tick-et, és csak
//! utána lép ki — így nem marad félbehagyott DB-írás.
//!
//! Kilépéskor a saját worker_id lease-ei pending-re kerülnek
//! (release_worker_leases), hogy a következő worker azonnal folytathassa.
//! A tick-hiba nem állítja le a daemont: naplózódik, a ciklus folytatódik.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use football_ingest::adapter::SourceAdapter;
use football_ingest::fetch::Fetchers;
use football_storage::object_store::ObjectStore;
use football_storage::repositories::worker as repo;
use sqlx::postgres::{PgAdvisoryLock, PgAdvisoryLockGuard};
use sqlx::{ConnectOptions, Either, PgConnection, PgPool};
use tokio::sync::watch;
use uuid::Uuid;

use crate::{scheduler, tick};

const WORKER_CYCLE_LOCK: &str = "football-app-v2:worker-cycle:v1";
const LOCK_MONITOR_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy)]
pub struct CycleOutcome {
    pub generated: usize,
    pub processed: usize,
    pub blocked: usize,
    pub failed: usize,
    pub deferred: usize,
}

pub async fn run_cycle(
    pool: &PgPool,
    store: &ObjectStore,
    fetchers: &Fetchers,
    adapters: &HashMap<String, Box<dyn SourceAdapter>>,
    worker_id: Uuid,
) -> Result<Option<CycleOutcome>> {
    let connection = pool.connect_options().connect().await?;
    let lock = PgAdvisoryLock::new(WORKER_CYCLE_LOCK);
    let mut guard = match lock.try_acquire(connection).await? {
        Either::Right(_) => return Ok(None),
        Either::Left(guard) => guard,
    };
    let cycle = run_cycle_unlocked(pool, store, fetchers, adapters, worker_id);
    let monitor = monitor_lock_connection(&mut guard);
    let result = tokio::select! {
        biased;
        result = cycle => result,
        monitor_result = monitor => match monitor_result {
            Err(err) => Err(err),
            Ok(()) => Err(anyhow::anyhow!("worker singleton lock monitor leállt")),
        },
    };
    let release_result = guard.release_now().await;
    match result {
        Err(err) => Err(err),
        Ok(outcome) => {
            release_result.map_err(|err| {
                anyhow::Error::new(err).context("worker singleton lock feloldása")
            })?;
            Ok(Some(outcome))
        }
    }
}

async fn monitor_lock_connection(guard: &mut PgAdvisoryLockGuard<'_, PgConnection>) -> Result<()> {
    loop {
        tokio::time::sleep(LOCK_MONITOR_INTERVAL).await;
        sqlx::query("SELECT 1")
            .execute(guard.as_mut())
            .await
            .context("worker singleton lockkapcsolat monitorozása")?;
    }
}

async fn run_cycle_unlocked(
    pool: &PgPool,
    store: &ObjectStore,
    fetchers: &Fetchers,
    adapters: &HashMap<String, Box<dyn SourceAdapter>>,
    worker_id: Uuid,
) -> Result<CycleOutcome> {
    let now = Utc::now();
    let policies = repo::list_enabled_policies(pool).await?;
    let generated = scheduler::generate_jobs(pool, &policies, now).await?;
    let outcome = tick::tick(pool, store, fetchers, adapters, worker_id, now).await?;
    Ok(CycleOutcome {
        generated,
        processed: outcome.processed,
        blocked: outcome.blocked,
        failed: outcome.failed,
        deferred: outcome.deferred,
    })
}

pub async fn run_loop(
    pool: PgPool,
    store: ObjectStore,
    fetchers: Fetchers,
    adapters: HashMap<String, Box<dyn SourceAdapter>>,
    worker_id: Uuid,
    tick_interval: Duration,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let mut interval = tokio::time::interval(tick_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        if *shutdown.borrow_and_update() {
            break;
        }
        tokio::select! {
            changed = shutdown.changed() => {
                // Ha a feladó megszűnt (Err), az is leállítási jel.
                if changed.is_err() || *shutdown.borrow_and_update() {
                    break;
                }
            }
            _ = interval.tick() => {
                match run_cycle(&pool, &store, &fetchers, &adapters, worker_id).await {
                    Ok(Some(outcome)) => tracing::info!(
                        generated = outcome.generated,
                        processed = outcome.processed,
                        blocked = outcome.blocked,
                        failed = outcome.failed,
                        deferred = outcome.deferred,
                        "worker tick kész"
                    ),
                    Ok(None) => tracing::debug!("worker tick kihagyva: egy másik kör már fut"),
                    Err(err) => tracing::error!(error = %err, "worker tick hiba, a ciklus folytatódik"),
                }
            }
        }
    }

    let released = repo::release_worker_leases(&pool, worker_id).await?;
    tracing::info!(worker_id = %worker_id, released, "worker leállt, a lease-ek elengedve");
    Ok(())
}
