use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use clap::{Parser, Subcommand};
use football_ingest::adapter::SourceAdapter;
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::demo::DemoAdapter;
use football_ingest::fetch::FixtureFetcher;
use football_ingest::fixture::{FixtureAdapter, FixtureSource};
use football_storage::object_store::ObjectStore;
use football_storage::repositories::day::{DayFilter, fetch_day};
use football_storage::repositories::identity as identity_repo;
use football_storage::repositories::worker as worker_repo;
use football_verification::engine::{VerificationContext, VerificationEngine};
use football_verification::gate::evaluate_and_persist_gate;
use football_verification::persist::persist_report;
use football_verification::rules::default_rules;
use football_worker::{scheduler, tick};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

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
    Worker {
        #[command(subcommand)]
        action: WorkerAction,
    },
    Identity {
        #[command(subcommand)]
        action: IdentityAction,
    },
}

#[derive(Subcommand)]
enum WorkerAction {
    Status,
    Tick,
}

#[derive(Subcommand)]
enum IdentityAction {
    Review {
        #[arg(long)]
        status: Option<String>,
    },
    Approve {
        id: Uuid,
        #[arg(long)]
        entity: Uuid,
        #[arg(long)]
        note: Option<String>,
    },
    Reject {
        id: Uuid,
        #[arg(long)]
        note: String,
    },
    ListAliases {
        #[arg(long)]
        entity: Option<Uuid>,
    },
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
                let source_values = football_verification::conflict::load_material_values(
                    &pool,
                    materialized.match_id,
                )
                .await?;
                let context = VerificationContext {
                    snapshot: &materialized.snapshot,
                    home_identity_exact: true,
                    away_identity_exact: true,
                    duplicate_match_count: duplicate_count(&pool, &materialized).await?,
                    source_values: &source_values,
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
            let jobs_pending: i64 =
                sqlx::query_scalar("SELECT count(*) FROM ops.job WHERE status='pending'")
                    .fetch_one(&pool)
                    .await?;
            let breakers_open: i64 =
                sqlx::query_scalar("SELECT count(*) FROM ops.breaker_state WHERE state='open'")
                    .fetch_one(&pool)
                    .await?;
            let quarantined: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM ops.coverage_slot WHERE state='quarantined'",
            )
            .fetch_one(&pool)
            .await?;
            println!(
                "db={ok} gates={gates} jobs_pending={jobs_pending} breakers_open={breakers_open} quarantined={quarantined}"
            );
        }
        Command::Worker { action } => match action {
            WorkerAction::Status => worker_status(&pool).await?,
            WorkerAction::Tick => worker_tick(&pool, &store, &data_dir).await?,
        },
        Command::Identity { action } => identity_command(&pool, action).await?,
    }

    Ok(())
}

/// Worker állapot: policy-k (slug/mód/engedélyezve), mai ledger-szám a napi
/// kerethez, breaker-állapotok, függő jobok és karanténos coverage-slotok.
async fn worker_status(pool: &sqlx::PgPool) -> Result<()> {
    let policies: Vec<(String, String, bool, i32)> = sqlx::query_as(
        "SELECT source_slug, mode, enabled, daily_budget
         FROM ops.source_policy ORDER BY source_slug",
    )
    .fetch_all(pool)
    .await?;
    let today = Utc::now().date_naive();
    println!("forráspolicy-k:");
    if policies.is_empty() {
        println!("  (nincs policy)");
    }
    for (slug, mode, enabled, budget) in &policies {
        let attempts = worker_repo::count_fetch_attempts_today(pool, slug, today).await?;
        let breaker = worker_repo::get_breaker(pool, slug)
            .await?
            .map(|row| row.state.as_db_str())
            .unwrap_or("closed");
        let enabled_text = if *enabled { "igen" } else { "nem" };
        println!(
            "  {slug} mód={mode} engedélyezve={enabled_text} ma={attempts}/{budget} breaker={breaker}"
        );
    }
    let jobs_pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ops.job WHERE status='pending'")
            .fetch_one(pool)
            .await?;
    let jobs_leased: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.job WHERE status='leased'")
        .fetch_one(pool)
        .await?;
    let quarantined: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ops.coverage_slot WHERE state='quarantined'")
            .fetch_one(pool)
            .await?;
    println!("függő jobok: {jobs_pending} (leased: {jobs_leased})");
    println!("karanténban: {quarantined}");
    Ok(())
}

/// Egyszeri worker kör kézi indítása: job-generálás + tick a DATA_DIR/worker-fixtures
/// fixture-eivel és a DemoAdapter registry-vel. Shadow policy esetén csak
/// observationök íródnak, kanonikus adat nem.
async fn worker_tick(pool: &sqlx::PgPool, store: &ObjectStore, data_dir: &str) -> Result<()> {
    let fixture_dir = std::path::Path::new(data_dir).join("worker-fixtures");
    let fetcher = FixtureFetcher::from_dir(&fixture_dir)?;
    let mut adapters: HashMap<String, Box<dyn SourceAdapter>> = HashMap::new();
    adapters.insert("demo-synthetic".into(), Box::new(DemoAdapter));
    let now = Utc::now();
    let policies = worker_repo::list_enabled_policies(pool).await?;
    let generated = scheduler::generate_jobs(pool, &policies, now).await?;
    let outcome = tick::tick(pool, store, &fetcher, &adapters, uuid::Uuid::now_v7(), now).await?;
    println!(
        "generált={generated} feldolgozott={} blokkolt={} sikertelen={}",
        outcome.processed, outcome.blocked, outcome.failed
    );
    Ok(())
}

/// Identitás review-életciklus: lista, jóváhagyás, elutasítás, alias-lista.
/// A döntéseket a football-identity review modulja hajtja végre tranzakcióban,
/// auditálva; a CLI döntője "cli". A hibaüzenetek magyar nyelvűek.
async fn identity_command(pool: &sqlx::PgPool, action: IdentityAction) -> Result<()> {
    match action {
        IdentityAction::Review { status } => {
            if let Some(s) = &status
                && !matches!(s.as_str(), "pending" | "approved" | "rejected")
            {
                anyhow::bail!("Érvénytelen státusz: {s} (lehetséges: pending, approved, rejected)");
            }
            let rows = identity_repo::list_reviews(pool, status.as_deref()).await?;
            let sources = identity_repo::list_sources(pool).await?;
            let source_codes: HashMap<Uuid, String> = sources.into_iter().collect();
            println!("id | forrás | external ID | név | ok | jelöltek száma | létrehozva");
            if rows.is_empty() {
                println!("(nincs felülvizsgálati sor)");
            }
            for r in rows {
                let source = r
                    .source_id
                    .and_then(|id| source_codes.get(&id).cloned())
                    .unwrap_or_else(|| "-".into());
                let candidates = r.candidate_entity_ids.as_array().map_or(0, |a| a.len());
                println!(
                    "{} | {} | {} | {} | {} | {} | {}",
                    r.id,
                    source,
                    r.source_entity_id,
                    r.proposed_name,
                    r.reason,
                    candidates,
                    r.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                );
            }
        }
        IdentityAction::Approve { id, entity, note } => {
            football_identity::review::approve(pool, id, entity, "cli", note.as_deref()).await?;
            println!("jóváhagyva: {id} → {entity}");
        }
        IdentityAction::Reject { id, note } => {
            football_identity::review::reject(pool, id, "cli", &note).await?;
            println!("elutasítva: {id}");
        }
        IdentityAction::ListAliases { entity } => {
            let rows = identity_repo::list_aliases(pool, entity).await?;
            println!("entitás | alias | nyelv | ország | státusz");
            if rows.is_empty() {
                println!("(nincs alias)");
            }
            for a in rows {
                println!(
                    "{} | {} | {} | {} | {}",
                    a.entity_id,
                    a.alias,
                    a.lang,
                    a.country_code.as_deref().unwrap_or("-"),
                    a.status
                );
            }
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
