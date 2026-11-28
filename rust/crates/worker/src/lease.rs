use std::time::Duration;

use football_domain::policy::JobLease;
use football_storage::repositories::worker as repo;
use sqlx::PgPool;
use tokio::sync::watch;

pub async fn heartbeat_loop(
    pool: PgPool,
    lease: JobLease,
    interval: Duration,
    lease_secs: i64,
    mut shutdown: watch::Receiver<bool>,
) -> bool {
    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return true;
                }
            }
            _ = tokio::time::sleep(interval) => {
                if *shutdown.borrow() {
                    return true;
                }
                match repo::heartbeat_job(&pool, &lease, lease_secs).await {
                    Ok(true) => {}
                    Ok(false) => {
                        tracing::warn!(job_id = %lease.job_id, "worker lease elveszett");
                        return false;
                    }
                    Err(err) => {
                        tracing::error!(job_id = %lease.job_id, error = %err, "worker heartbeat hiba");
                        return false;
                    }
                }
            }
        }
    }
}
