mod runtime_file;

use anyhow::Result;
use football_api::{AppState, router};
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
    axum::serve(listener, app).await?;
    Ok(())
}
