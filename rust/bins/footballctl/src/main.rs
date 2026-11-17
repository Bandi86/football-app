use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use clap::{Parser, Subcommand};
#[cfg(feature = "obscura")]
use football_domain::policy::Transport;
use football_domain::trust::{ODDS_TOLERANCE_DEFAULT, OddsTrustState};
use football_ingest::adapter::SourceAdapter;
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::demo::DemoAdapter;
use football_ingest::fetch::{Fetchers, FixtureFetcher};
use football_ingest::fixture::{FixtureAdapter, FixtureSource};
use football_ingest::footballdata::FootballdataAdapter;
use football_ingest::fotmob::FotMobAdapter;
use football_ingest::http::RustHttpFetcher;
use football_ingest::openfoot::OpenFootAdapter;
use football_ingest::soccerway::SoccerwayAdapter;
use football_storage::object_store::ObjectStore;
use football_storage::repositories::day::{DayFilter, fetch_day};
use football_storage::repositories::identity as identity_repo;
use football_storage::repositories::identity_merge;
use football_storage::repositories::trust;
use football_storage::repositories::worker as worker_repo;
use football_verification::engine::{VerificationContext, VerificationEngine};
use football_verification::gate::evaluate_and_persist_gate;
use football_verification::odds::evaluate_odds_trust;
use football_verification::persist::persist_report;
use football_verification::rules::default_rules;
use football_worker::{scheduler, tick};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

/// S2b-2: a TRUST_LEVEL + odds trust szabálykészlet verziója (a tickével azonos).
const RULE_SET_VERSION: &str = "verification-1.1";

#[derive(Parser)]
#[command(name = "footballctl")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum DedupeAction {
    /// Duplikált kanonikus meccsek merge-e egy napon (dry-run alapból).
    Matches {
        #[arg(long)]
        date: NaiveDate,
        /// Tényleges végrehajtás (alapból csak terv).
        #[arg(long)]
        apply: bool,
        /// Elavult névváltozatok `|`-vel elválasztva: a túlélő az a sor,
        /// amelyik NEM ezek közül való (egyébként a legkorábbi sor).
        #[arg(long)]
        deny: Option<String>,
    },
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
    Asset {
        #[command(subcommand)]
        action: AssetAction,
    },
    Dedupe {
        #[command(subcommand)]
        action: DedupeAction,
    },
    /// Liga-szezon lefedettségi riport (S23).
    Coverage {
        /// Csak ettől a szezon-kezdőévtől (pl. 2015).
        #[arg(long)]
        from: Option<i32>,
        /// Csak eddig a szezon-kezdőévig.
        #[arg(long)]
        to: Option<i32>,
    },
    /// Történelmi backfill jobok tervezése (S23).
    Backfill {
        #[command(subcommand)]
        action: BackfillAction,
    },
}

#[derive(Subcommand)]
enum BackfillAction {
    /// Liga-szezon backfill jobok sorba állítása (alapból dry-run).
    Plan {
        #[arg(long, default_value = "footballdata")]
        source: String,
        /// Kezdő szezon kezdőéve (pl. 2015).
        #[arg(long)]
        from: u16,
        /// Utolsó szezon kezdőéve (pl. 2026).
        #[arg(long)]
        to: u16,
        /// Liga-kódok vesszővel (pl. e0,e1), vagy "all".
        #[arg(long, default_value = "all")]
        leagues: String,
        #[arg(long)]
        apply: bool,
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
    /// Új kanonikus entitás létrehozása a review-sorból (élő forrás bootstrap).
    ApproveCreate {
        id: Uuid,
        #[arg(long)]
        note: Option<String>,
    },
    /// Minden pending review-sor batch jóváhagyása új entitással (bootstrap).
    ApproveCreateAll {
        /// Ország nélküli kontextusú csapatot NULL országgal hoz létre
        /// (őszinte hiány; alapból az ilyen sorok pending maradnak).
        #[arg(long)]
        allow_unknown_country: bool,
    },
    Reject {
        id: Uuid,
        #[arg(long)]
        note: String,
    },
    /// Pending review-ök összesítése (nem/korosztály/ország/verseny).
    ReviewSummary,
    /// Minden pending review-sor elutasítása kötelező indoklással
    /// (pl. lokalizált, elavult kör tisztítása).
    RejectAllPending {
        #[arg(long)]
        note: String,
    },
    ListAliases {
        #[arg(long)]
        entity: Option<Uuid>,
    },
    /// v1 crosswalk-seed: csapat- és alias-nevek importja a v1 adatbázisból
    /// (csak olvasás). Alapból dry-run; `--apply` reviewed aliasokat ír.
    SeedV1 {
        #[arg(long, default_value = "football_app")]
        v1_db: String,
        #[arg(long)]
        apply: bool,
        /// Csak az első N v1 csapatot dolgozza fel (minta-ellenőrzéshez).
        #[arg(long)]
        limit: Option<i64>,
    },
    /// Duplikált csapat-entitások (azonos név + kontextus) listája.
    Duplicates,
    /// Duplikált csapat-entitás összevonása a túlélőbe (alapból dry-run).
    MergeTeam {
        /// Az összevonandó (áldozat) entitás.
        #[arg(long)]
        from: Uuid,
        /// A túlélő entitás.
        #[arg(long)]
        to: Uuid,
        #[arg(long, default_value = "S19 entitás-merge")]
        note: String,
        #[arg(long)]
        apply: bool,
    },
    CrosswalkCheck,
}

#[derive(Subcommand)]
enum AssetAction {
    /// Asset-JSON importálása (tömb: entity_kind, entity_id?, kind, url,
    /// source, licence?, licence_status, attribution?, captured_at?).
    Import {
        #[arg(long)]
        file: PathBuf,
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
            let normalized = football_ingest::normalized::NormalizedDay::from(&fixture);
            let day = Canonicalizer::materialize(&pool, &normalized).await?;
            let rules = default_rules();

            // S2b-2: trust sorok egyszer, az ÖSSZES hozzájáruló forrásra
            // (a meccsek source_values forráskódjai + a fixture forrása).
            let mut material_values = Vec::with_capacity(day.matches.len());
            let mut source_codes = vec![fixture.source_code.clone()];
            for materialized in &day.matches {
                let values = football_verification::conflict::load_material_values(
                    &pool,
                    materialized.match_id,
                )
                .await?;
                for value in &values {
                    if !source_codes.contains(&value.source_code) {
                        source_codes.push(value.source_code.clone());
                    }
                }
                material_values.push(values);
            }
            let source_trust = trust::trust_for_sources(&pool, &source_codes).await?;

            for (materialized, source_values) in day.matches.iter().zip(&material_values) {
                let home_identity_exact = materialized.identity_exact;
                let away_identity_exact = materialized.identity_exact;
                let odds_values = trust::load_match_odds(&pool, materialized.match_id).await?;
                let context = VerificationContext {
                    snapshot: &materialized.snapshot,
                    home_identity_exact,
                    away_identity_exact,
                    duplicate_match_count: duplicate_count(&pool, materialized).await?,
                    source_values,
                    source_trust: &source_trust,
                    odds_values: &odds_values,
                    odds_tolerance: ODDS_TOLERANCE_DEFAULT,
                };
                let report = VerificationEngine::run(&rules, &context);
                let run_id =
                    persist_report(&pool, materialized.match_id, &report, RULE_SET_VERSION).await?;
                let date = football_verification::gate::local_publish_date(
                    materialized.snapshot.kickoff_at,
                    "Europe/Budapest",
                )?;
                let state = evaluate_and_persist_gate(
                    &pool,
                    materialized.match_id,
                    date,
                    &report,
                    RULE_SET_VERSION,
                )
                .await?;
                // Odds trust_state a gate után; üres odds listán nincs frissítés.
                let odds_state = evaluate_odds_trust(&odds_values, ODDS_TOLERANCE_DEFAULT);
                if odds_state != OddsTrustState::NoOdds {
                    trust::update_odds_trust_state(&pool, materialized.match_id, odds_state)
                        .await?;
                }
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
                    competition_id: None,
                    window_days: 0,
                    team_id: None,
                },
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&rows)?);
        }
        Command::Doctor => {
            let counts = worker_repo::doctor_counts(&pool).await?;
            println!(
                "db={} gates={} jobs_pending={} breakers_open={} quarantined={}",
                counts.reachable,
                counts.publish_gates,
                counts.jobs_pending,
                counts.breakers_open,
                counts.quarantined
            );
        }
        Command::Worker { action } => match action {
            WorkerAction::Status => worker_status(&pool).await?,
            WorkerAction::Tick => worker_tick(&pool, &store, &data_dir).await?,
        },
        Command::Identity { action } => identity_command(&pool, action).await?,
        Command::Dedupe { action } => match action {
            DedupeAction::Matches { date, apply, deny } => {
                let deny_list: Vec<String> = deny
                    .as_deref()
                    .map(|d| d.split('|').map(|x| x.trim().to_string()).collect())
                    .unwrap_or_default();
                let groups =
                    football_storage::repositories::dedupe::duplicate_groups(&pool, date).await?;
                if groups.is_empty() {
                    println!("nincs duplikált meccs ezen a napon: {date}");
                } else {
                    let mut merges = 0usize;
                    let mut skipped = 0usize;
                    for group in &groups {
                        // Túlélő: nem deny-olt név; ha mindkettő/egyik sem,
                        // a legkorábbi sor.
                        let survivor = group
                            .matches
                            .iter()
                            .filter(|m| !deny_list.iter().any(|d| d == &m.competition_name))
                            .min_by_key(|m| m.created_at)
                            .or_else(|| group.matches.iter().min_by_key(|m| m.created_at))
                            .expect("csoport nem üres");
                        println!(
                            "csoport {} — túlélő: {} [{}]",
                            group.key, survivor.match_id, survivor.competition_name
                        );
                        for m in group
                            .matches
                            .iter()
                            .filter(|m| m.match_id != survivor.match_id)
                        {
                            println!(
                                "  áldozat: {} [{}] (prov={}, result={}, odds={}, gate={:?})",
                                m.match_id,
                                m.competition_name,
                                m.provenance_count,
                                m.has_result,
                                m.has_odds,
                                m.gate_state
                            );
                            if apply {
                                // Egy hibás/kockázatos pár ne állítsa meg a
                                // teljes napi takarítást: kihagyjuk, jelezzük.
                                let outcome: Result<()> = async {
                                    let same =
                                        football_storage::repositories::dedupe::is_same_match(
                                            &pool,
                                            survivor.match_id,
                                            m.match_id,
                                        )
                                        .await?;
                                    anyhow::ensure!(
                                        same,
                                        "a pár nem ugyanaz a meccs: {} / {}",
                                        survivor.match_id,
                                        m.match_id
                                    );
                                    football_storage::repositories::dedupe::merge_duplicate(
                                        &pool,
                                        survivor.match_id,
                                        m.match_id,
                                        "S16 dedupe (duplikált kanonikus meccs)",
                                    )
                                    .await?;
                                    Ok(())
                                }
                                .await;
                                match outcome {
                                    Ok(()) => merges += 1,
                                    Err(err) => {
                                        skipped += 1;
                                        println!("  kihagyva: {err:#}");
                                    }
                                }
                            }
                        }
                    }
                    println!(
                        "duplikált csoportok: {}, merge-ek: {}, kihagyva: {}{}",
                        groups.len(),
                        merges,
                        skipped,
                        if apply {
                            ""
                        } else {
                            " (dry-run — --apply a végrehajtáshoz)"
                        }
                    );
                }
            }
        },
        Command::Coverage { from, to } => {
            let rows: Vec<(String, String, i64, i64, i64)> = sqlx::query_as(
                "SELECT c.name, s.label, count(*)::bigint,
                        count(r.match_id)::bigint,
                        count(*) FILTER (WHERE EXISTS (
                            SELECT 1 FROM fact.odds_snapshot o WHERE o.match_id = m.id))::bigint
                 FROM core.match m
                 JOIN core.competition c ON c.id = m.competition_id
                 JOIN core.season s ON s.id = m.season_id
                 LEFT JOIN core.match_result r ON r.match_id = m.id
                 WHERE ($1::int IS NULL OR substring(s.label from 1 for 4)::int >= $1)
                   AND ($2::int IS NULL OR substring(s.label from 1 for 4)::int <= $2)
                 GROUP BY 1, 2
                 ORDER BY 3 DESC
                 LIMIT 300",
            )
            .bind(from)
            .bind(to)
            .fetch_all(&pool)
            .await?;
            println!("verseny | szezon | meccs | eredmény % | odds %");
            let (mut total, mut res, mut odds) = (0i64, 0i64, 0i64);
            for (name, season, matches, with_result, with_odds) in &rows {
                total += matches;
                res += with_result;
                odds += with_odds;
                println!(
                    "{} | {} | {} | {:.1}% | {:.1}%",
                    name,
                    season,
                    matches,
                    *with_result as f64 / *matches as f64 * 100.0,
                    *with_odds as f64 / *matches as f64 * 100.0
                );
            }
            if total > 0 {
                println!(
                    "összesen: {} meccs | eredmény {:.1}% | odds {:.1}%",
                    total,
                    res as f64 / total as f64 * 100.0,
                    odds as f64 / total as f64 * 100.0
                );
            } else {
                println!("(nincs adat a szűrésre)");
            }
        }
        Command::Backfill { action } => match action {
            BackfillAction::Plan {
                source,
                from,
                to,
                leagues,
                apply,
            } => {
                if source != "footballdata" {
                    anyhow::bail!(
                        "a backfill-tervezés egyelőre csak a footballdata forrást támogatja"
                    );
                }
                let codes: Vec<String> = if leagues == "all" {
                    football_ingest::footballdata::league_codes()
                        .into_iter()
                        .map(str::to_string)
                        .collect()
                } else {
                    leagues
                        .split(',')
                        .map(|c| c.trim().to_ascii_lowercase())
                        .filter(|c| !c.is_empty())
                        .collect()
                };
                let mut planned = 0usize;
                let mut skipped = 0usize;
                for code in &codes {
                    let first = football_ingest::footballdata::first_season(code)
                        .ok_or_else(|| anyhow::anyhow!("ismeretlen liga-kód: {code}"))?;
                    for start in from..=to {
                        if start < first {
                            skipped += 1;
                            continue;
                        }
                        let season = format!("{:02}{:02}", start % 100, (start + 1) % 100);
                        let label = football_ingest::footballdata::season_label_from_code(&season)
                            .expect("érvényes szezonkód");
                        if !apply {
                            if planned < 6 {
                                println!("  terv: {code} {label} ({season})");
                            }
                            planned += 1;
                            continue;
                        }
                        let inserted = worker_repo::insert_job_counted(
                            &pool,
                            &worker_repo::NewJob {
                                source_slug: "footballdata".into(),
                                domain: "backfill".into(),
                                entity_key: code.clone(),
                                event_type: "backfill".into(),
                                time_window: season,
                                due_at: Utc::now(),
                                priority: 200,
                            },
                        )
                        .await?;
                        planned += inserted;
                    }
                }
                println!(
                    "backfill terv: {} job{}, {} kihagyott (a liga még nem létezett){}",
                    planned,
                    if apply {
                        " sorba állítva"
                    } else {
                        " (dry-run)"
                    },
                    skipped,
                    if apply {
                        ""
                    } else {
                        " — --apply a sorba állításhoz"
                    }
                );
            }
        },
        Command::Asset { action } => match action {
            AssetAction::Import { file } => {
                let rows: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(&file)?)?;
                let now = Utc::now();
                let mut written = 0usize;
                for row in &rows {
                    let field = |k: &str| row.get(k).and_then(|v| v.as_str());
                    // Helyi fájl (v1-audit logó) → tartalom-címzett blob a
                    // nyers objektumtárba; a storage_key kerül az asset-sorba.
                    let storage_key = match field("file") {
                        Some(file) => {
                            let bytes = std::fs::read(file).map_err(|e| {
                                anyhow::anyhow!("asset fájl olvasása ({file}): {e}")
                            })?;
                            let ext = std::path::Path::new(file)
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("bin");
                            let stored = store.put_blob(&bytes, ext)?;
                            if let Some(expected) = field("sha256") {
                                anyhow::ensure!(
                                    expected.eq_ignore_ascii_case(&stored.sha256),
                                    "sha256 eltérés: {file} (várt {expected}, kapott {})",
                                    stored.sha256
                                );
                            }
                            Some(stored.storage_key)
                        }
                        None => field("storage_key").map(str::to_string),
                    };
                    let input = worker_repo::AssetRowInput {
                        entity_kind: field("entity_kind")
                            .ok_or_else(|| anyhow::anyhow!("hiányzó entity_kind"))?,
                        entity_id: field("entity_id").and_then(|v| v.parse::<Uuid>().ok()),
                        kind: field("kind").ok_or_else(|| anyhow::anyhow!("hiányzó kind"))?,
                        url: field("url").ok_or_else(|| anyhow::anyhow!("hiányzó url"))?,
                        storage_key: storage_key.as_deref(),
                        source: field("source").ok_or_else(|| anyhow::anyhow!("hiányzó source"))?,
                        licence: field("licence"),
                        licence_status: field("licence_status").unwrap_or("unknown"),
                        attribution: field("attribution"),
                        captured_at: field("captured_at")
                            .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
                            .map(|d| d.with_timezone(&Utc))
                            .unwrap_or(now),
                    };
                    worker_repo::upsert_asset(&pool, &input).await?;
                    written += 1;
                }
                println!("asset import: {written} sor feldolgozva");
            }
        },
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
    let counts = worker_repo::worker_status_counts(pool).await?;
    println!(
        "függő jobok: {} (leased: {})",
        counts.jobs_pending, counts.jobs_leased
    );
    println!("karanténban: {}", counts.quarantined);
    println!("függő identitás-review: {}", counts.reviews_pending);
    Ok(())
}

/// Egyszeri worker kör kézi indítása: job-generálás + tick a DATA_DIR/worker-fixtures
/// fixture-eivel és a DemoAdapter registry-vel. Shadow policy esetén csak
/// observationök íródnak, kanonikus adat nem.
async fn worker_tick(pool: &sqlx::PgPool, store: &ObjectStore, data_dir: &str) -> Result<()> {
    let fixture_dir = std::path::Path::new(data_dir).join("worker-fixtures");
    let fixture = FixtureFetcher::from_dir(&fixture_dir)?;
    let fetchers = Fetchers::new(RustHttpFetcher::new("football-app-v2")?, fixture);
    // Canary/élő Soccerway-körhöz a CLI is indíthat Obscura-szervert
    // (csak engedélyezett obscura policy esetén; a feature mögött).
    #[cfg(feature = "obscura")]
    let fetchers = {
        let mut fetchers = fetchers;
        if worker_repo::has_enabled_transport(pool, Transport::Obscura.as_db_str()).await? {
            let cfg = football_ingest::obscura::ObscuraConfig::from_env();
            let fetcher = football_ingest::obscura::ObscuraFetcher::connect_managed(&cfg).await?;
            fetchers = fetchers.with_obscura(fetcher);
            println!("obscura fetcher aktív (CLI mód, önjavító)");
        }
        fetchers
    };
    let mut adapters: HashMap<String, Box<dyn SourceAdapter>> = HashMap::new();
    adapters.insert("demo-synthetic".into(), Box::new(DemoAdapter));
    adapters.insert("footballdata".into(), Box::new(FootballdataAdapter));
    adapters.insert("fotmob".into(), Box::new(FotMobAdapter::default()));
    adapters.insert("soccerway".into(), Box::new(SoccerwayAdapter::default()));
    adapters.insert("openfoot".into(), Box::new(OpenFootAdapter::from_env()));
    adapters.insert(
        "tipplap".into(),
        Box::new(football_ingest::tipplap::TipplapAdapter),
    );
    let now = Utc::now();
    let policies = worker_repo::list_enabled_policies(pool).await?;
    let generated = scheduler::generate_jobs(pool, &policies, now).await?;
    let outcome = tick::tick(pool, store, &fetchers, &adapters, uuid::Uuid::now_v7(), now).await?;
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
        IdentityAction::Duplicates => {
            let groups = identity_merge::duplicate_team_groups(pool).await?;
            if groups.is_empty() {
                println!("(nincs duplikált csapat-entitás)");
            }
            for group in groups {
                println!(
                    "{} | ország: {} | nem: {} | korosztály: {} | ág: {}",
                    group.name,
                    group.country_code.as_deref().unwrap_or("-"),
                    group.gender.as_deref().unwrap_or("-"),
                    group.age_group.as_deref().unwrap_or("-"),
                    group.branch.as_deref().unwrap_or("-")
                );
                for team in &group.teams {
                    println!(
                        "  entitás: {} | team: {} | meccsek: {} | létre: {}",
                        team.entity_id,
                        team.team_id,
                        team.matches,
                        team.created_at.format("%Y-%m-%d %H:%M")
                    );
                }
            }
        }
        IdentityAction::MergeTeam {
            from,
            to,
            note,
            apply,
        } => {
            let preview = identity_merge::merge_team_preview(pool, to, from).await?;
            println!(
                "merge: {} ← {} | meccsek: {} | lehetséges duplikált meccs: {} | alias: {} | ref: {} | tabella: {} | szezon-stat: {} | asset: {}",
                preview.survivor_name,
                preview.duplicate_name,
                preview.matches_moved,
                preview.potential_duplicate_matches,
                preview.aliases,
                preview.refs,
                preview.standings,
                preview.season_stats,
                preview.assets
            );
            if !apply {
                println!("dry-run: nincs írás (--apply a merge-hez)");
                return Ok(());
            }
            let report = identity_merge::merge_team(pool, to, from, &note).await?;
            println!(
                "merge kész: meccsek {} | alias {} (eldobva {}) | ref {} | tabella {} (eldobva {}) | szezon-stat {} (eldobva {}) | asset {} (eldobva {}) | merge utáni duplikált meccsek: {}",
                report.matches_moved,
                report.aliases_moved,
                report.aliases_dropped,
                report.refs_moved,
                report.standings_moved,
                report.standings_dropped,
                report.season_stats_moved,
                report.season_stats_dropped,
                report.assets_moved,
                report.assets_dropped,
                report.duplicate_matches
            );
        }
        IdentityAction::CrosswalkCheck => {
            let conflicts = football_identity::crosswalk_conflicts(pool).await?;
            if conflicts.is_empty() {
                println!("crosswalk: nincs konfliktus");
            } else {
                println!("crosswalk konfliktusok:");
                for c in &conflicts {
                    println!(
                        "  {} | {} -> {}",
                        c.left_source,
                        c.left_external_id,
                        c.conflicting_right_external_ids.join(", ")
                    );
                }
                anyhow::bail!(
                    "{} ambiguous crosswalk-kötés (import előtt javítandó)",
                    conflicts.len()
                );
            }
        }
        IdentityAction::Approve { id, entity, note } => {
            football_identity::review::approve(pool, id, entity, "cli", note.as_deref()).await?;
            println!("jóváhagyva: {id} → {entity}");
        }
        IdentityAction::ApproveCreateAll {
            allow_unknown_country,
        } => {
            let (approved, failed) = football_identity::review::approve_create_pending_with(
                pool,
                "cli",
                allow_unknown_country,
            )
            .await?;
            println!(
                "batch approve-create: {} jóváhagyva, {} hiba",
                approved.len(),
                failed.len()
            );
            for (queue_id, err) in &failed {
                println!("  hiba: {queue_id} — {err}");
            }
            if !failed.is_empty() {
                anyhow::bail!("{} sor nem lett jóváhagyva", failed.len());
            }
        }
        IdentityAction::ApproveCreate { id, note } => {
            let entity =
                football_identity::review::approve_create(pool, id, "cli", note.as_deref()).await?;
            println!("létrehozva és jóváhagyva: {id} → {entity}");
        }
        IdentityAction::Reject { id, note } => {
            football_identity::review::reject(pool, id, "cli", &note).await?;
            println!("elutasítva: {id}");
        }
        IdentityAction::RejectAllPending { note } => {
            if note.trim().is_empty() {
                anyhow::bail!("a jegyzet nem lehet üres");
            }
            let rows = identity_repo::list_reviews(pool, Some("pending")).await?;
            let mut rejected = 0usize;
            let mut failed = 0usize;
            for row in rows {
                match football_identity::review::reject(pool, row.id, "cli", &note).await {
                    Ok(()) => rejected += 1,
                    Err(err) => {
                        failed += 1;
                        println!("  hiba: {} — {err}", row.id);
                    }
                }
            }
            println!("batch reject: {rejected} elutasítva, {failed} hiba");
            if failed > 0 {
                anyhow::bail!("{failed} sor nem lett elutasítva");
            }
        }
        IdentityAction::ReviewSummary => {
            let rows = identity_repo::review_summary(pool).await?;
            if rows.is_empty() {
                println!("(nincs függő review)");
            } else {
                println!("nem | ág | verseny | db | országok | minta");
                for row in &rows {
                    println!(
                        "{} | {} | {} | {} | {} | {}",
                        row.gender.as_deref().unwrap_or("-"),
                        row.branch.as_deref().unwrap_or("-"),
                        row.competition.as_deref().unwrap_or("-"),
                        row.count,
                        row.countries.as_deref().unwrap_or("-"),
                        row.sample_names.as_deref().unwrap_or("")
                    );
                }
            }
        }
        IdentityAction::SeedV1 {
            v1_db,
            apply,
            limit,
        } => {
            seed_v1(pool, &v1_db, apply, limit).await?;
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

/// v1 crosswalk-seed: a v1 csapatnevek/aliasok → reviewed v2 aliasok.
/// A v1 adatbázis csak olvasott; ismeretlen/többértelmű csapatot nem seedel.
async fn seed_v1(pool: &sqlx::PgPool, v1_db: &str, apply: bool, limit: Option<i64>) -> Result<()> {
    let user = std::env::var("USER").unwrap_or_else(|_| "postgres".into());
    let v1_url = format!("postgres://{user}@127.0.0.1/{v1_db}");
    let v1 = PgPoolOptions::new()
        .max_connections(2)
        .connect(&v1_url)
        .await
        .with_context(|| format!("v1 adatbázis kapcsolat: {v1_db}"))?;

    let iso3_map: HashMap<String, String> = sqlx::query_as::<_, (String, String)>(
        "SELECT iso3, iso2 FROM countries WHERE iso3 IS NOT NULL AND iso2 IS NOT NULL",
    )
    .fetch_all(&v1)
    .await
    .context("v1 ország-iso3 térkép")?
    .into_iter()
    .collect();

    let team_rows: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT t.id, t.name, c.iso2, t.gender
         FROM teams t
         LEFT JOIN countries c ON c.id = t.country_id
         ORDER BY t.id
         LIMIT $1",
    )
    .bind(limit.unwrap_or(i64::MAX))
    .fetch_all(&v1)
    .await
    .context("v1 csapatok")?;

    let alias_rows: Vec<(i64, String, Option<String>)> =
        sqlx::query_as("SELECT team_id, alias, language FROM team_aliases ORDER BY id")
            .fetch_all(&v1)
            .await
            .context("v1 csapat-aliasok")?;

    let mut aliases_by_team: HashMap<i64, Vec<(String, String)>> = HashMap::new();
    for (team_id, alias, lang) in alias_rows {
        aliases_by_team
            .entry(team_id)
            .or_default()
            .push((alias, lang.unwrap_or_else(|| "en".into())));
    }

    let mut v1_teams = Vec::with_capacity(team_rows.len());
    for (id, name, iso2, gender) in team_rows {
        let mut names: Vec<(String, String)> = vec![(name, "en".into())];
        names.extend(aliases_by_team.remove(&id).unwrap_or_default());
        let mut seen = std::collections::HashSet::new();
        names.retain(|(n, _)| seen.insert(football_domain::identity::fold_name(n)));
        v1_teams.push(football_identity::v1_seed::V1Team {
            names,
            country_code: iso2,
            gender: gender.map(|g| match g.as_str() {
                "M" => "male".to_string(),
                "F" => "female".to_string(),
                other => other.to_lowercase(),
            }),
        });
    }

    // A v2 országkód néha ISO3 (HUN/GER); az egyezéshez ISO2-re vetítjük.
    let mut v2 = identity_repo::all_team_entities(pool).await?;
    for row in &mut v2 {
        if let Some(code) = &row.country_code
            && code.len() == 3
            && let Some(iso2) = iso3_map.get(code)
        {
            row.country_code = Some(iso2.clone());
        }
    }

    let (seeds, stats) = football_identity::v1_seed::plan_seeds(&v2, &v1_teams);
    println!(
        "v1 csapatok: {} | egyező: {} | többértelmű: {} | nincs találat: {} | alias-nevek: {}",
        stats.v1_teams, stats.matched, stats.ambiguous, stats.unmatched, stats.seeded_names
    );
    for seed in seeds.iter().take(8) {
        println!("  seed: {} → {}", seed.alias, seed.entity_id);
    }
    if !apply {
        println!("dry-run: nincs írás (--apply a beszúráshoz)");
        return Ok(());
    }
    let inserted = identity_repo::insert_alias_seeds(pool, &seeds).await?;
    println!("beszúrt reviewed alias: {inserted}");
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
