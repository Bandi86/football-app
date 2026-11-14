mod runtime_file;

use std::collections::HashMap;

use anyhow::{Context, Result};
use football_api::{AppState, router};
#[cfg(feature = "obscura")]
use football_domain::policy::Transport;
use football_ingest::adapter::SourceAdapter;
use football_ingest::demo::DemoAdapter;
use football_ingest::fetch::{Fetchers, FixtureFetcher};
use football_ingest::http::RustHttpFetcher;
use football_storage::object_store::ObjectStore;
#[cfg(feature = "obscura")]
use football_storage::repositories::worker as worker_repo;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let database_url = std::env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("../../../db/migrations").run(&pool).await?;

    // Worker spawn a serve előtt: WORKER_ENABLED=true esetén a daemon saját
    // tokio taskban futtatja a worker loopot, graceful shutdown watch-csatornával.
    let worker_enabled = std::env::var("WORKER_ENABLED")
        .map(|v| v == "true")
        .unwrap_or(false);
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    #[cfg(feature = "obscura")]
    let mut obscura_server: Option<football_ingest::obscura::server::ObscuraServer> = None;
    let worker_handle = if worker_enabled {
        let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".into());
        let store = ObjectStore::new(&data_dir)?;
        let fixture_dir = std::path::Path::new(&data_dir).join("worker-fixtures");
        let rust_http = RustHttpFetcher::new("football-app-v2")?;
        let fixture = FixtureFetcher::from_dir(&fixture_dir)?;
        let fetchers = Fetchers::new(rust_http, fixture);
        // Lazy Obscura-szerver: csak engedélyezett obscura policy esetén,
        // és csak ha az obscura feature be van kapcsolva ebben a buildben.
        #[cfg(feature = "obscura")]
        let fetchers = {
            let mut fetchers = fetchers;
            if worker_repo::has_enabled_transport(&pool, Transport::Obscura.as_db_str()).await? {
                let cfg = football_ingest::obscura::ObscuraConfig::from_env();
                let server = football_ingest::obscura::server::ObscuraServer::start(&cfg).await?;
                let fetcher = football_ingest::obscura::ObscuraFetcher::connect(
                    server.cdp_url(),
                    std::time::Duration::from_millis(cfg.timeout_ms),
                )
                .await?;
                obscura_server = Some(server);
                fetchers = fetchers.with_obscura(fetcher);
                tracing::info!("obscura fetcher aktiv (lazy szerverinditas)");
            }
            fetchers
        };
        let mut adapters: HashMap<String, Box<dyn SourceAdapter>> = HashMap::new();
        adapters.insert("demo-synthetic".into(), Box::new(DemoAdapter));
        let tick_secs = worker_tick_secs(std::env::var("WORKER_TICK_SECS").ok().as_deref());
        let worker_id = uuid::Uuid::now_v7();
        tracing::info!(worker_id = %worker_id, tick_secs, "worker elindult");
        Some(tokio::spawn(football_worker::runner::run_loop(
            pool.clone(),
            store,
            fetchers,
            adapters,
            worker_id,
            std::time::Duration::from_secs(tick_secs),
            shutdown_rx,
        )))
    } else {
        None
    };

    // A bind/serve belső blokkban fut, hogy hiba esetén is lefusson a worker
    // graceful shutdownja (különben a lease-ek a lejáratukig ragadnának).
    let serve_result: Result<()> = async {
        let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:0".into());
        let listener = TcpListener::bind(bind_addr).await?;
        let port = listener.local_addr()?.port();
        let runtime = runtime_file::write_runtime_file(port)?;

        let app = router(AppState {
            pool,
            read_token: runtime.read_token,
            ops_token: runtime.ops_token,
        });

        tracing::info!(port, "footballd elindult");
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await?;
        Ok(())
    }
    .await;

    // Graceful shutdown: a serve sikerétől függetlenül jelzünk a workernek, és
    // megvárjuk, hogy a lease-ek elengedése után lépjen ki.
    let _ = shutdown_tx.send(true);
    let worker_result: Result<()> = match worker_handle {
        Some(handle) => handle
            .await
            .context("worker task csatlakoztatása")
            .and_then(|r| r),
        None => Ok(()),
    };
    #[cfg(feature = "obscura")]
    if let Some(mut server) = obscura_server {
        server.stop();
        tracing::info!("obscura szerver leállítva");
    }
    tracing::info!("footballd leállt");
    serve_result?;
    worker_result
}

/// A WORKER_TICK_SECS feldolgozása: hiányzó vagy érvénytelen érték esetén 60
/// másodperces alapérték, 0 esetén minimum 1 másodperc (a tokio interval
/// Duration::ZERO-ra pánikolna), mindkét esetről figyelmeztető naplóval.
fn worker_tick_secs(raw: Option<&str>) -> u64 {
    match raw {
        None => 60,
        Some(value) => match value.parse::<u64>() {
            Ok(secs) => {
                if secs == 0 {
                    tracing::warn!("WORKER_TICK_SECS=0, minimum 1 másodperc");
                }
                secs.max(1)
            }
            Err(_) => {
                tracing::warn!(
                    value,
                    "érvénytelen WORKER_TICK_SECS, 60 másodperces alapérték"
                );
                60
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::worker_tick_secs;

    #[test]
    fn tick_secs_missing_defaults_to_60() {
        assert_eq!(worker_tick_secs(None), 60);
    }

    #[test]
    fn tick_secs_invalid_defaults_to_60() {
        assert_eq!(worker_tick_secs(Some("abc")), 60);
        assert_eq!(worker_tick_secs(Some("-5")), 60);
        assert_eq!(worker_tick_secs(Some("")), 60);
    }

    #[test]
    fn tick_secs_zero_clamped_to_1() {
        assert_eq!(worker_tick_secs(Some("0")), 1);
    }

    #[test]
    fn tick_secs_valid_value_kept() {
        assert_eq!(worker_tick_secs(Some("5")), 5);
        assert_eq!(worker_tick_secs(Some("60")), 60);
    }
}
