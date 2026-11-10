use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::NaiveDate;
use clap::{Parser, Subcommand};
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::fixture::{FixtureAdapter, FixtureSource};
use football_storage::object_store::ObjectStore;
use football_storage::repositories::day::{DayFilter, fetch_day};
use football_verification::engine::{VerificationContext, VerificationEngine};
use football_verification::gate::evaluate_and_persist_gate;
use football_verification::persist::persist_report;
use football_verification::rules::default_rules;
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "footballctl")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Migrate,
    IngestFixture {
        #[arg(long)]
        path: PathBuf,
    },
    Day {
        #[arg(long)]
        date: NaiveDate,
        #[arg(long, default_value_t = false)]
        include_all: bool,
    },
    Doctor,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL hiányzik")?;
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await?;
    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".into());
    let store = ObjectStore::new(&data_dir)?;

    match cli.command {
        Command::Migrate => {
            sqlx::migrate!("../../../db/migrations").run(&pool).await?;
            println!("migráció kész");
        }
        Command::IngestFixture { path } => {
            let report = FixtureAdapter::ingest(&pool, &store, &path).await?;
            let fixture: FixtureSource = serde_json::from_slice(&std::fs::read(&path)?)?;
            let day = Canonicalizer::materialize(&pool, &fixture).await?;
            let rules = default_rules();

            for materialized in day.matches {
                let context = VerificationContext {
                    snapshot: &materialized.snapshot,
                    home_identity_exact: true,
                    away_identity_exact: true,
                    duplicate_match_count: duplicate_count(&pool, &materialized).await?,
                    source_values: &[],
                };
                let report = VerificationEngine::run(&rules, &context);
                let run_id =
                    persist_report(&pool, materialized.match_id, &report, "verification-1.0")
                        .await?;
                let date = football_verification::gate::local_publish_date(
                    materialized.snapshot.kickoff_at,
                    "Europe/Budapest",
                )?;
                let state = evaluate_and_persist_gate(
                    &pool,
                    materialized.match_id,
                    date,
                    &report,
                    "verification-1.0",
                )
                .await?;
                println!(
                    "match={} run={} state={}",
                    materialized.match_id,
                    run_id,
                    state.as_db_str()
                );
            }
            println!(
                "capture={} observations={}",
                report.capture_id,
                report.observation_ids.len()
            );
        }
        Command::Day { date, include_all } => {
            let rows = fetch_day(
                &pool,
                &DayFilter {
                    date,
                    timezone: "Europe/Budapest".into(),
                    include_all,
                },
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&rows)?);
        }
        Command::Doctor => {
            let ok: i32 = sqlx::query_scalar("SELECT 1").fetch_one(&pool).await?;
            let gates: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.publish_gate")
                .fetch_one(&pool)
                .await?;
            println!("db={ok} gates={gates}");
        }
    }

    Ok(())
}

async fn duplicate_count(
    pool: &sqlx::PgPool,
    materialized: &football_ingest::canonicalize::MaterializedMatch,
) -> Result<i64> {
    sqlx::query_scalar(
        "SELECT count(*) FROM core.match
         WHERE competition_id = (SELECT competition_id FROM core.match WHERE id=$1)
           AND season_id = (SELECT season_id FROM core.match WHERE id=$1)
           AND round_label IS NOT DISTINCT FROM (SELECT round_label FROM core.match WHERE id=$1)
           AND home_team_id = $2 AND away_team_id = $3
           AND kickoff_at = $4
           AND id <> $1",
    )
    .bind(materialized.match_id)
    .bind(materialized.home_team_id)
    .bind(materialized.away_team_id)
    .bind(materialized.snapshot.kickoff_at)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}
