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

use anyhow::Result;
use chrono::Utc;
use football_ingest::adapter::SourceAdapter;
use football_ingest::fetch::Fetcher;
use football_storage::object_store::ObjectStore;
use football_storage::repositories::worker as repo;
use sqlx::PgPool;
use tokio::sync::watch;
use uuid::Uuid;

use crate::{scheduler, tick};

/// Egy teljes worker-kör: engedélyezett policy-k, idempotens job-generálás,
/// majd a tick. A generálás a tick előtt fut, hogy a friss policy-események
/// még ebben a körben feldolgozódjanak.
async fn run_cycle(
    pool: &PgPool,
    store: &ObjectStore,
    fetcher: &dyn Fetcher,
    adapters: &HashMap<String, Box<dyn SourceAdapter>>,
    worker_id: Uuid,
) -> Result<()> {
    let now = Utc::now();
    let policies = repo::list_enabled_policies(pool).await?;
    let generated = scheduler::generate_jobs(pool, &policies, now).await?;
    let outcome = tick::tick(pool, store, fetcher, adapters, worker_id, now).await?;
    tracing::info!(
        generated,
        processed = outcome.processed,
        blocked = outcome.blocked,
        failed = outcome.failed,
        "worker tick kész"
    );
    Ok(())
}

pub async fn run_loop(
    pool: PgPool,
    store: ObjectStore,
    fetcher: Box<dyn Fetcher>,
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
                if let Err(err) = run_cycle(&pool, &store, fetcher.as_ref(), &adapters, worker_id).await {
                    tracing::error!(error = %err, "worker tick hiba, a ciklus folytatódik");
                }
            }
        }
    }

    let released = repo::release_worker_leases(&pool, worker_id).await?;
    tracing::info!(worker_id = %worker_id, released, "worker leállt, a lease-ek elengedve");
    Ok(())
}
