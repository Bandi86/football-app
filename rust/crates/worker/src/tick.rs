//! Tick orchestráció (Task 10): a spec §3.1 szerinti 10 lépés egy körben.
//!
//! Sorrend: lease-visszasorolás + claim → policy/robots/kvóta/breaker kapuk →
//! reserve (számoló ledger-placeholder) → fetch → L0 nyers megőrzés → parse →
//! observationök → shadow ágban megáll, live/canary ágban kanonikus írás +
//! verification + publish gate → ledger-véglegesítés, coverage, breaker, complete.
//!
//! A robots/kvóta/breaker blokkok saját, NEM számláló ledger-sort kapnak a
//! reserve előtt. A reserve pillanatától a kísérlet számlál: ha a folyamat
//! megszakad, a placeholder (transport_error) jogosan marad a kvótában.
//! A sikeres fetch utáni váratlan infrastruktúra-hiba a coverage-slotot
//! Failed-re állítja a job lezárása előtt. A nyers megőrzés forrásazonosítója
//! a job source_slugja; a payload forráskódja NEM azonosít forrást.
//!
//! Controller-döntések: a completion, requeue és coverage minden írása a saját,
//! aktív lease tokennel történik. A robots/kvóta/breaker blokkok nem fogyasztják
//! a kvótát; a reservation pillanatától a kísérlet crash esetén is számlál.

use std::collections::HashMap;
use std::time::{Duration as StdDuration, Instant};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use football_domain::error::FetchError;
use football_domain::identity::{IdentityContext, Resolution};
use football_domain::policy::{
    CoverageState, FetchJob, FetchOutcome, JobStatus, LeasedJob, SourceMode, SourcePolicy,
    Transport,
};
use football_domain::trust::{ODDS_TOLERANCE_DEFAULT, OddsTrustState};
use football_identity::linker;
use football_ingest::adapter::{Observation, ParseCtx, SourceAdapter};
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::fetch::{FetchLimits, FetchRequest, Fetcher, Fetchers};
use football_ingest::observe::insert_observation;
use football_storage::object_store::ObjectStore;
use football_storage::repositories::dedupe;
use football_storage::repositories::trust;
use football_storage::repositories::worker::{self as repo, LedgerEntry, RawCapture};
use football_verification::conflict::{load_material_values, load_resolved_fields};
use football_verification::engine::{VerificationContext, VerificationEngine};
use football_verification::gate::{evaluate_and_persist_gate, local_publish_date};
use football_verification::odds::evaluate_odds_trust;
use football_verification::persist::persist_report;
use football_verification::rules::default_rules;
use rand::Rng;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{breaker, coverage, lease, quota, robots, scheduler};

const LEASE_SECS: i64 = 60;
const HEARTBEAT_INTERVAL: StdDuration = StdDuration::from_secs(15);
/// Backoff-alap (első próba): 60 s; a sapka 1 óra.
const BACKOFF_BASE_SECS: i64 = 60;
const BACKOFF_CAP_SECS: i64 = 3600;
const CLAIM_LIMIT: i64 = 10;
// Browser-render (Obscura) 10–45 s között fut; a HTTP-források ennél jóval
// gyorsabbak. A hosszabb plafon a renderelt forrásokat nem vágja le.
const FETCH_TIMEOUT_SECS: u64 = 60;
const RULE_SET_VERSION: &str = "verification-1.3";
const PUBLISH_TIMEZONE: &str = "Europe/Budapest";
/// A job-utáni dedupe csak a frissen feldolgozott napot takarítja. A review
/// jóváhagyás, a kézi import és a különböző források viszont később is
/// létrehozhatnak névvariáns-duplikátumokat — ezért időzített, korábbi napokra
/// is kiterjedő sweep kell (S79). 30 perces throttle: a tick 60 s-es, a
/// párosítás O(n²) ezért nem fut minden körben.
const DEDUPE_SWEEP_INTERVAL: chrono::Duration = chrono::Duration::minutes(30);
static LAST_DEDUPE_SWEEP: std::sync::Mutex<Option<DateTime<Utc>>> = std::sync::Mutex::new(None);

/// Egy tick kör összesítése: feldolgozott (done), blokkolt (kapuk/cancelled)
/// és sikertelen (failed) jobok száma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickOutcome {
    pub processed: usize,
    pub blocked: usize,
    pub failed: usize,
    /// S139: forrásfrissesség miatt elhalasztott catch-up jobok száma.
    pub deferred: usize,
}

/// Tranziens fetch-hiba maximális újrapróbálkozás-száma. Efölött a job
/// `failed`-re zárul, hogy egy tartósan hibás forrás ne maradjon örökre a
/// sorban (a korábbi viselkedés akár több száz kísérletet is engedett).
const MAX_TRANSIENT_ATTEMPTS: i64 = 8;

/// A napi kvóta UTC-éjfélkor áll vissza; a kvóta miatt blokkolt jobot a
/// következő UTC-éjfél utáni pár másodpercre soroljuk vissza. Enélkül a job
/// `pending` állapotban, változatlan `due_at`-tel percenként újra claimelődne
/// (hot loop), miközben a kísérletek számlálója folyamatosan nőne.
fn next_quota_reset(now: DateTime<Utc>) -> DateTime<Utc> {
    let next_day = now
        .date_naive()
        .succ_opt()
        .unwrap_or_else(|| now.date_naive());
    match next_day.and_hms_opt(0, 0, 5) {
        Some(midnight) => midnight.and_utc(),
        // Elméleti túlcsordulás: egy óra múlva próbáljuk újra.
        None => now + Duration::hours(1),
    }
}

/// Exponenciális backoff + jitter: min(60s·2^(n-1), 1h) + [0, jitter_ms).
fn backoff_delay(attempts: u32, jitter_ms: i64, rng: &mut impl FnMut() -> u32) -> Duration {
    let exponent = attempts.saturating_sub(1).min(6);
    let base = BACKOFF_BASE_SECS << exponent;
    let capped = base.min(BACKOFF_CAP_SECS);
    let jitter = if jitter_ms <= 0 {
        0
    } else {
        i64::from(rng() % jitter_ms as u32)
    };
    Duration::milliseconds(capped * 1000 + jitter)
}

enum JobResult {
    Processed,
    Blocked,
    Failed,
    /// A forrás még nem frissült — a job várakozik (S139).
    Deferred,
}

/// S139: a catch-up akkor dolgozható fel, ha a forrásfájl a job létrehozása
/// után frissült. Hiányzó fájl vagy 6 óránál régebbi job esetén a döntés:
/// előbbi várakozás, utóbbi mégis feldolgozás (a forrás tartósan nem frissül).
fn catchup_source_stale(
    modified: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> bool {
    if now.signed_duration_since(created_at) >= Duration::hours(6) {
        return false;
    }
    modified.map(|m| m <= created_at).unwrap_or(true)
}

/// S163: a research refresh cél-napja. A `catchup-<dátum>` job a cél napot az
/// `entity_key`-ben hordozza (a `time_window` ekkor a mai nap), a `slot-NN`
/// job viszont a `time_window`-ban. Minden más jobra a frissesség-kapu nem
/// vonatkozik → `None`. A hibásan formázott dátum szintén `None`: a catchup-ág
/// nem esik vissza a `time_window`-ra.
fn research_refresh_target(job: &FetchJob) -> Option<NaiveDate> {
    if job.source_slug != "research" || job.event_type != "refresh" {
        return None;
    }
    if let Some(value) = job.entity_key.strip_prefix("catchup-") {
        return NaiveDate::parse_from_str(value, "%Y-%m-%d").ok();
    }
    if job.entity_key.starts_with("slot-") {
        return NaiveDate::parse_from_str(&job.time_window, "%Y-%m-%d").ok();
    }
    None
}

/// S179: a `slot-NN` job a `time_window` napját kérdei a forrástól. Hálózati
/// forrásnál a site csak az aktuális napot szolgálja, így a tegnapi slot már
/// soha nem teljesíthető. A helyi fájl-forrásnál (research) a régi nap is
/// olvasható, ezért ott nem érvénytelenítünk.
fn stale_slot_job(job: &FetchJob, transport: Transport, now: DateTime<Utc>) -> bool {
    if !job.entity_key.starts_with("slot-") || transport == Transport::LocalFile {
        return false;
    }
    NaiveDate::parse_from_str(&job.time_window, "%Y-%m-%d")
        .is_ok_and(|date| date < now.date_naive())
}

pub async fn tick(
    pool: &PgPool,
    store: &ObjectStore,
    fetchers: &Fetchers,
    adapters: &HashMap<String, Box<dyn SourceAdapter>>,
    worker_id: Uuid,
    now: DateTime<Utc>,
) -> Result<TickOutcome> {
    // 1. Lejárt lease-ek visszasorolása, majd esedékes jobok igénylése.
    scheduler::requeue_expired(pool, now).await?;
    let jobs = scheduler::claim(pool, worker_id, now, LEASE_SECS, CLAIM_LIMIT).await?;

    let mut outcome = TickOutcome {
        processed: 0,
        blocked: 0,
        failed: 0,
        deferred: 0,
    };
    for leased in &jobs {
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        let heartbeat = tokio::spawn(lease::heartbeat_loop(
            pool.clone(),
            leased.lease,
            HEARTBEAT_INTERVAL,
            LEASE_SECS,
            stop_rx,
        ));
        let mut result = process_job(pool, store, fetchers, adapters, leased, now).await;
        let _ = stop_tx.send(true);
        let heartbeat_ok = heartbeat.await.unwrap_or(false);
        if !heartbeat_ok {
            result = Ok(JobResult::Blocked);
        }
        match result {
            Ok(JobResult::Processed) => outcome.processed += 1,
            Ok(JobResult::Blocked) => outcome.blocked += 1,
            Ok(JobResult::Failed) => outcome.failed += 1,
            Ok(JobResult::Deferred) => outcome.deferred += 1,
            Err(err) => {
                tracing::error!(job_id = %leased.job.id, error = %err, "tick: a job feldolgozása megszakadt");
                match finish(pool, leased, JobStatus::Failed, Some(&err.to_string())).await {
                    Ok(JobResult::Failed) => outcome.failed += 1,
                    Ok(_) => outcome.blocked += 1,
                    Err(complete_err) => {
                        tracing::error!(job_id = %leased.job.id, error = %complete_err,
                            "tick: a hibás job lezárása sem sikerült");
                        outcome.blocked += 1;
                    }
                }
            }
        }
    }

    // Időzített dedupe sweep az előző és a mai napra: a research job-utáni
    // pass nem fedi le a később keletkezett duplikátumokat (S79).
    let sweep_due = match LAST_DEDUPE_SWEEP.lock() {
        Ok(mut guard) => {
            let due = guard
                .map(|last| now.signed_duration_since(last) >= DEDUPE_SWEEP_INTERVAL)
                .unwrap_or(true);
            if due {
                *guard = Some(now);
            }
            due
        }
        Err(_) => false,
    };
    if sweep_due {
        for days_ago in 0..=1i64 {
            let date = (now - chrono::Duration::days(days_ago)).date_naive();
            match auto_dedupe_day(pool, date).await {
                Ok((0, _)) => {}
                Ok((merged, skipped)) => {
                    tracing::info!(%date, merged, skipped, "időzített dedupe pass");
                }
                Err(err) => {
                    tracing::warn!(%date, error = %err, "időzített dedupe pass hiba");
                }
            }
        }
    }

    Ok(outcome)
}

async fn process_job(
    pool: &PgPool,
    store: &ObjectStore,
    fetchers: &Fetchers,
    adapters: &HashMap<String, Box<dyn SourceAdapter>>,
    leased: &LeasedJob,
    now: DateTime<Utc>,
) -> Result<JobResult> {
    let job = &leased.job;
    let lease = &leased.lease;
    // S139/S163: a research refresh csak akkor olvashat, ha a CÉL napi fájl a
    // job létrehozása UTÁN frissült (catchup: entity_key, slot: time_window).
    // A daemon 07:31-kor indult, a research fájl 07:38-kor frissül — régen a
    // job elavult `scheduled` státuszokat olvasott be, és az egyszeri
    // idempotencia miatt nem javult többé.
    // 6 óra után mégis feldolgozzuk (a forrás tartósan nem frissül).
    if let Some(target) = research_refresh_target(job) {
        let created_at = repo::job_created_at(pool, job.id).await?;
        let rel = football_ingest::research::ResearchAdapter::daily_rel(target);
        let modified = fetchers
            .local_file
            .as_ref()
            .and_then(|fetcher| fetcher.source_path(&rel).ok())
            .and_then(|path| {
                std::fs::metadata(path)
                    .and_then(|meta| meta.modified())
                    .ok()
            })
            .map(DateTime::<Utc>::from);
        if catchup_source_stale(modified, created_at, now) {
            tracing::info!(%target, job_id = %job.id, "catch-up várakozik a research frissítésre");
            if !repo::requeue_leased(pool, lease, now + Duration::minutes(15)).await? {
                return Ok(JobResult::Blocked);
            }
            return Ok(JobResult::Deferred);
        }
    }

    // 2. Policy-kapu: hiányzó vagy tiltott policy → cancelled.
    let Some(policy) = repo::get_policy(pool, &job.source_slug).await? else {
        return finish(
            pool,
            leased,
            JobStatus::Cancelled,
            Some("hiányzó source policy"),
        )
        .await;
    };
    if !policy.enabled {
        return finish(
            pool,
            leased,
            JobStatus::Cancelled,
            Some("tiltott source policy"),
        )
        .await;
    }
    let Some(adapter) = adapters.get(&job.source_slug) else {
        return finish(
            pool,
            leased,
            JobStatus::Failed,
            Some("nincs regisztrált adapter a forráshoz"),
        )
        .await;
    };

    // S179: a napon belüli slot jobok a `time_window` napját kérdik a
    // forrástól. Ha a nap átfordult, a régi slot már nem teljesíthető (a site
    // mindig az aktuális napot szolgálja) — dátum-guard hiba helyett
    // érvénytelenítjük, hogy ne pazarolja a kvótát és ne szennyezze a jelzéseket.
    if stale_slot_job(job, policy.transport, now) {
        return finish(
            pool,
            leased,
            JobStatus::Cancelled,
            Some("elavult slot: a forrás csak az aktuális napot szolgálja"),
        )
        .await;
    }

    // S9: a job a forrás egy napi körét képviseli; a terv MINDEN kérését
    // feldolgozzuk (a korábbi kód csak az elsőt használta).
    let plan = adapter.plan(job);
    if plan.is_empty() {
        return finish(pool, leased, JobStatus::Failed, Some("üres fetch-terv")).await;
    }

    // A tartalom-fetch fetcherét a policy transzportja választja (S2c).
    let fetcher = match fetchers.for_transport(&policy.transport) {
        Ok(fetcher) => fetcher,
        Err(err) => {
            return finish(pool, leased, JobStatus::Failed, Some(&err.to_string())).await;
        }
    };

    // Robots: fixture forrásnál a fixture fetcheren (nincs hálózat), valódi
    // forrásnál plain HTTP-vel. Helyi fájl-forrásnál (S38) nincs robots-sem
    // kvóta, nincs késleltetés: nem hálózati forgalom.
    let local_file = policy.transport == Transport::LocalFile;
    let robots_fetcher: &dyn Fetcher =
        if matches!(policy.transport, Transport::Fixture | Transport::LocalFile) {
            fetcher
        } else {
            fetchers.rust_http()
        };

    let mut observations: Vec<Observation> = Vec::new();
    let result = async {
        for req in &plan {
            // Robots-kapu kérésenként (fail-closed); helyi fájl-forrásnál nincs.
            if !local_file
                && robots::check(pool, robots_fetcher, &job.source_slug, &req.url).await?
                    == robots::RobotsVerdict::Blocked
            {
                block_ledger(pool, job, req, now, FetchOutcome::RobotsBlocked).await?;
                if !coverage::set(pool, leased, CoverageState::Blocked).await? {
                    return Ok(JobResult::Blocked);
                }
                return finish(pool, leased, JobStatus::Failed, Some("robots tiltás")).await;
            }

            // Breaker-kapu: nyitva → nem számláló ledger + failed.
            if !breaker::allow(pool, &policy, now).await? {
                block_ledger(pool, job, req, now, FetchOutcome::BreakerOpen).await?;
                return finish(pool, leased, JobStatus::Failed, Some("breaker nyitva")).await;
            }

            // Helyi fájl-nál nincs kvóta/delay, de a megszokott időben kiírt
            // placeholder megmarad. Hálózati kérésnél a reservation dönti el
            // egyszerre a kvótát, a min-delayt és a ledger-foglalást.
            let ledger_id = if local_file {
                repo::insert_ledger(
                    pool,
                    &LedgerEntry {
                        source_slug: job.source_slug.clone(),
                        request_key: req.request_key.clone(),
                        attempted_at: now,
                        outcome: FetchOutcome::TransportError,
                        http_status: None,
                        byte_size: None,
                        latency_ms: None,
                        artifact_hash: None,
                        job_id: Some(job.id),
                    },
                )
                .await?
            } else {
                match quota::reserve(pool, &job.source_slug, &req.request_key, lease, now).await {
                    Ok(quota::QuotaDecision::Allowed(id)) => id,
                    Ok(quota::QuotaDecision::QuotaBlocked) => {
                        breaker::release_probe(pool, &job.source_slug).await?;
                        block_ledger(pool, job, req, now, FetchOutcome::QuotaBlocked).await?;
                        let due_at = next_quota_reset(now);
                        tracing::debug!(
                            job_id = %job.id,
                            source = %job.source_slug,
                            %due_at,
                            "job visszasorolva: napi kvóta kimerült"
                        );
                        return requeue_for_gate(pool, leased, due_at).await;
                    }
                    Ok(quota::QuotaDecision::DelayBlocked) => {
                        block_ledger(pool, job, req, now, FetchOutcome::QuotaBlocked).await?;
                        match wait_for_delay(pool, &policy, leased, &req.request_key).await {
                            Ok(DelayDecision::Reserved(id)) => id,
                            Ok(DelayDecision::LeaseLost) => {
                                breaker::release_probe(pool, &job.source_slug).await?;
                                return Ok(JobResult::Blocked);
                            }
                            Ok(DelayDecision::QuotaBlocked) => {
                                breaker::release_probe(pool, &job.source_slug).await?;
                                let due_at = next_quota_reset(now);
                                tracing::debug!(
                                    job_id = %job.id,
                                    source = %job.source_slug,
                                    %due_at,
                                    "job visszasorolva: napi kvóta kimerült"
                                );
                                return requeue_for_gate(pool, leased, due_at).await;
                            }
                            Ok(DelayDecision::RetryAt(due_at)) => {
                                breaker::release_probe(pool, &job.source_slug).await?;
                                tracing::debug!(
                                    job_id = %job.id,
                                    source = %job.source_slug,
                                    %due_at,
                                    "job visszasorolva: a min_delay még nem telt le"
                                );
                                return requeue_for_gate(pool, leased, due_at).await;
                            }
                            Err(err) => {
                                breaker::release_probe(pool, &job.source_slug).await?;
                                return Err(err);
                            }
                        }
                    }
                    Ok(quota::QuotaDecision::LeaseLost) => {
                        breaker::release_probe(pool, &job.source_slug).await?;
                        return Ok(JobResult::Blocked);
                    }
                    Err(err) => {
                        breaker::release_probe(pool, &job.source_slug).await?;
                        return Err(err);
                    }
                }
            };

            // Fetch a policy limitjeivel.
            let limits = FetchLimits {
                max_bytes: u64::try_from(policy.max_bytes)
                    .context("policy max_bytes nem ábrázolható")?,
                timeout: StdDuration::from_secs(FETCH_TIMEOUT_SECS),
            };
            let started = Instant::now();
            let fetched = fetcher.fetch(req, &limits).await;
            let latency_ms = i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX);
            let raw = match fetched {
                Ok(raw) => raw,
                Err(err) => {
                    return fetch_failure(pool, &policy, leased, ledger_id, now, latency_ms, &err)
                        .await;
                }
            };
            // A breaker a fetch-egészséget tükrözi (S3a).
            breaker::on_success(pool, &job.source_slug).await?;

            let http_status =
                i16::try_from(raw.http_status).context("HTTP-státusz i16 konverzió")?;
            let byte_size = i64::try_from(raw.bytes.len()).ok();

            // L0: objektumtár + forrás/artifact/capture megőrzés (parse előtt).
            let artifact = store
                .put_json(&raw.bytes)
                .context("nyers payload objektumtárba írása")?;
            let display_name = raw_display_name(&raw.bytes, &job.source_slug);
            let (source_id, capture_id) = repo::persist_raw(
                pool,
                &job.source_slug,
                &display_name,
                &artifact,
                &RawCapture {
                    endpoint_key: &req.endpoint_key,
                    request_url: &req.url,
                    request_key: &req.request_key,
                    requested_at: now,
                    received_at: raw.fetched_at,
                    http_status,
                    adapter_version: &policy.adapter_version,
                    parser_version: &policy.parser_version,
                },
                &raw.content_type,
            )
            .await?;

            // Parse: hiba → karantén (coverage quarantined, ledger parse_error).
            // Napi jobok: a time_window dátum. Backfill joboknál a
            // time_window szezonkód; a szezon-hatókört a request_key hordozza,
            // a requested_date ilyenkor csak helyőrző.
            let requested_date = match NaiveDate::parse_from_str(&job.time_window, "%Y-%m-%d") {
                Ok(date) => date,
                Err(_) if job.domain == "backfill" => Utc::now().date_naive(),
                Err(err) => {
                    return Err(err).with_context(|| {
                        format!("a job időablaka nem napi dátum: {}", job.time_window)
                    });
                }
            };
            let ctx = ParseCtx {
                requested_date,
                request_key: req.request_key.clone(),
            };
            let parsed = match adapter.parse(&raw, &ctx) {
                Ok(parsed) => parsed,
                Err(err) => {
                    repo::update_ledger_outcome(
                        pool,
                        ledger_id,
                        FetchOutcome::ParseError,
                        Some(http_status),
                        byte_size,
                        Some(latency_ms),
                        Some(&artifact.sha256),
                    )
                    .await?;
                    if !coverage::set(pool, leased, CoverageState::Quarantined).await? {
                        return Ok(JobResult::Blocked);
                    }
                    return finish(pool, leased, JobStatus::Failed, Some(&err.to_string())).await;
                }
            };

            // Observationök a stage rétegbe.
            for obs in &parsed {
                insert_observation(
                    pool,
                    source_id,
                    capture_id,
                    &obs.entity_kind,
                    &obs.aspect,
                    &obs.natural_key,
                    &obs.source_record_key,
                    obs.observed_at,
                    obs.payload.clone(),
                )
                .await?;
            }
            observations.extend(parsed.clone());

            // S192: a markets domain nem megy át a napi verifikációs úton —
            // a forrás sorát a már linkelt kanonikus meccshez rendeljük, és
            // az egész odds-blokkot (1X2 + minden piacot) együtt írjuk.
            if job.domain == "markets" && policy.mode != SourceMode::Shadow {
                let mut written = 0usize;
                for obs in &parsed {
                    if obs.aspect != "match_odds" {
                        continue;
                    }
                    let Some(match_id) =
                        repo::active_match_ref(pool, source_id, &obs.source_record_key).await?
                    else {
                        tracing::warn!(
                            source = %job.source_slug,
                            key = %obs.source_record_key,
                            "markets odds: nincs aktív kanonikus link, kihagyva"
                        );
                        continue;
                    };
                    let mut odds = football_ingest::normalized::NormalizedOdds {
                        market_count: obs
                            .payload
                            .get("market_count")
                            .and_then(|v| v.as_i64())
                            .map(|v| v as i32),
                        quotes: Vec::new(),
                    };
                    if let Some(rows) = obs.payload.get("quotes").and_then(|v| v.as_array()) {
                        for row in rows {
                            let get_str = |key: &str| {
                                row.get(key)
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_owned()
                            };
                            odds.quotes
                                .push(football_ingest::normalized::NormalizedQuote {
                                    bookmaker: get_str("bookmaker"),
                                    market: get_str("market"),
                                    outcome: get_str("outcome"),
                                    line: row.get("line").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                    phase: get_str("phase"),
                                    price: row.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0),
                                });
                        }
                    }
                    football_ingest::canonicalize::write_market_odds(
                        pool,
                        match_id,
                        source_id,
                        obs.observed_at,
                        &odds,
                    )
                    .await?;
                    written += 1;
                }
                tracing::info!(written, "tipplap markets odds materializálva");
            }

            // A fetch sikeres: a ledger-sor véglegesítése (a kanonikus szakasz
            // hibája már nem a fetch eredménye).
            repo::update_ledger_outcome(
                pool,
                ledger_id,
                FetchOutcome::Success,
                Some(http_status),
                byte_size,
                Some(latency_ms),
                Some(&artifact.sha256),
            )
            .await?;
        }

        // 8–9. Shadow ágban nincs kanonikus írás; live/canary ágban a teljes
        // verification + gate lánc fut, VERSENYENKÉNT (S9). Üres observation-
        // lista legitím nulla-nap → no_data, nem hiba.
        if policy.mode != SourceMode::Shadow && !observations.is_empty() && job.domain != "markets"
        {
            if observations.iter().any(|o| o.aspect == "club_elo") {
                let elo = football_ingest::normalized::ClubEloDay::from_observations(&observations)
                    .context("observationökből club_elo nap")?;
                let written = Canonicalizer::materialize_elo(pool, &elo).await?;
                tracing::info!(written, "club_elo pillanatkép materializálva");
            } else {
                let days = football_ingest::normalized::NormalizedDay::grouped_from_observations(
                    &observations,
                )
                .context("observationökből normalizált napok")?;
                for day in &days {
                    verify_and_publish(pool, day).await?;
                }
                tracing::info!(competitions = days.len(), "napi versenyek materializálva");
                // S37e: helyi forrás (research) után automatikus napi dedupe-pass,
                // hogy a forrás verseny-név-variánsai ne hagyjanak duplikátumot
                // a napi feedben. A túlélő a legrégebben létrehozott sor.
                if policy.transport == Transport::LocalFile
                    && let Ok(date) =
                        chrono::NaiveDate::parse_from_str(&job.time_window, "%Y-%m-%d")
                {
                    match auto_dedupe_day(pool, date).await {
                        Ok((merged, skipped)) => {
                            tracing::info!(%date, merged, skipped, "napi dedupe pass (research)");
                        }
                        Err(err) => {
                            tracing::warn!(%date, error = %err, "a napi dedupe pass nem futott le");
                        }
                    }
                }
            }
        }

        // 10. Tick-zárás: coverage done (0 observation → no_data), job done.
        let coverage_state = if observations.is_empty() {
            CoverageState::NoData
        } else {
            CoverageState::Done
        };
        if !coverage::set(pool, leased, coverage_state).await? {
            return Ok(JobResult::Blocked);
        }
        finish(pool, leased, JobStatus::Done, None).await
    }
    .await;
    if result.is_err() {
        // Best-effort coverage-failed: ha ez sem megy, a hiba naplózódik.
        if let Err(cov_err) = coverage::set(pool, leased, CoverageState::Failed).await {
            tracing::error!(job_id = %job.id, error = %cov_err,
                "tick: a coverage failed állapot rögzítése sem sikerült");
        }
    }
    result
}

enum DelayDecision {
    Reserved(Uuid),
    QuotaBlocked,
    LeaseLost,
    RetryAt(DateTime<Utc>),
}

/// A policy min_delay-ét kivárja a jobon belüli következő kéréshez
/// (legfeljebb 15 s); a várakozás alatt is a saját lease-re reserve-ol.
async fn wait_for_delay(
    pool: &PgPool,
    policy: &SourcePolicy,
    leased: &LeasedJob,
    request_key: &str,
) -> Result<DelayDecision> {
    let deadline = tokio::time::Instant::now() + StdDuration::from_secs(15);
    loop {
        let now = Utc::now();
        match quota::reserve(
            pool,
            &leased.job.source_slug,
            request_key,
            &leased.lease,
            now,
        )
        .await?
        {
            quota::QuotaDecision::Allowed(id) => return Ok(DelayDecision::Reserved(id)),
            quota::QuotaDecision::QuotaBlocked => return Ok(DelayDecision::QuotaBlocked),
            quota::QuotaDecision::LeaseLost => return Ok(DelayDecision::LeaseLost),
            quota::QuotaDecision::DelayBlocked => {}
        }
        if tokio::time::Instant::now() >= deadline {
            let delay_ms = policy.min_delay_ms.max(1_000);
            let jitter = if policy.jitter_ms > 0 {
                rand::rng().random_range(0..policy.jitter_ms)
            } else {
                0
            };
            return Ok(DelayDecision::RetryAt(
                now + Duration::milliseconds(delay_ms + jitter),
            ));
        }
        tokio::time::sleep(StdDuration::from_millis(250)).await;
    }
}

async fn finish(
    pool: &PgPool,
    leased: &LeasedJob,
    status: JobStatus,
    last_error: Option<&str>,
) -> Result<JobResult> {
    if !scheduler::complete(pool, &leased.lease, status, last_error).await? {
        return Ok(JobResult::Blocked);
    }
    Ok(match status {
        JobStatus::Done => JobResult::Processed,
        JobStatus::Failed => JobResult::Failed,
        _ => JobResult::Blocked,
    })
}

async fn requeue_for_gate(
    pool: &PgPool,
    leased: &LeasedJob,
    due_at: DateTime<Utc>,
) -> Result<JobResult> {
    repo::requeue_leased(pool, &leased.lease, due_at).await?;
    Ok(JobResult::Blocked)
}

/// Nem számláló ledger-sor a kapu-blokkokhoz (reserve előtt).
async fn block_ledger(
    pool: &PgPool,
    job: &FetchJob,
    req: &FetchRequest,
    now: DateTime<Utc>,
    outcome: FetchOutcome,
) -> Result<()> {
    repo::insert_ledger(
        pool,
        &LedgerEntry {
            source_slug: job.source_slug.clone(),
            request_key: req.request_key.clone(),
            attempted_at: now,
            outcome,
            http_status: None,
            byte_size: None,
            latency_ms: None,
            artifact_hash: None,
            job_id: Some(job.id),
        },
    )
    .await?;
    Ok(())
}

/// Fetch-hiba zárása: ledger-véglegesítés, breaker (401/403 → azonnali open +
/// audit, egyéb → on_failure), coverage failed, job failed.
#[allow(clippy::too_many_arguments)]
async fn fetch_failure(
    pool: &PgPool,
    policy: &SourcePolicy,
    leased: &LeasedJob,
    ledger_id: Uuid,
    now: DateTime<Utc>,
    latency_ms: i64,
    err: &FetchError,
) -> Result<JobResult> {
    let job = &leased.job;
    let retry_after = match err {
        FetchError::RateLimited(retry_after) => *retry_after,
        _ => None,
    };
    let transient = matches!(
        err,
        FetchError::Transport(_) | FetchError::Timeout | FetchError::RateLimited(_)
    ) || matches!(err, FetchError::HttpStatus(status) if *status >= 500);
    let (outcome, http_status) = match err {
        FetchError::RateLimited(_) => (FetchOutcome::HttpError, Some(429i16)),
        FetchError::HttpStatus(status) => (FetchOutcome::HttpError, i16::try_from(*status).ok()),
        FetchError::Timeout => (FetchOutcome::Timeout, None),
        FetchError::Oversize(_) => (FetchOutcome::Oversize, None),
        FetchError::Transport(_) | FetchError::NotFound(_) => (FetchOutcome::TransportError, None),
    };
    repo::update_ledger_outcome(
        pool,
        ledger_id,
        outcome,
        http_status,
        None,
        Some(latency_ms),
        None,
    )
    .await?;
    if matches!(err, FetchError::HttpStatus(401 | 403)) {
        breaker::on_auth_failure(pool, policy, now).await?;
    } else {
        breaker::on_failure(pool, policy, now).await?;
    }
    if !coverage::set(pool, leased, CoverageState::Failed).await? {
        return Ok(JobResult::Blocked);
    }
    if transient {
        let attempts = repo::count_job_fetch_attempts(pool, job.id).await?;
        // A végtelen újrapróbálkozás helyett a job a limit után failed-re
        // zárul (a ledger és a coverage így is rögzíti a kísérleteket).
        if attempts >= MAX_TRANSIENT_ATTEMPTS {
            return finish(
                pool,
                leased,
                JobStatus::Failed,
                Some(&format!(
                    "transiens újrapróbálkozás-limit elérve ({attempts} kísérlet): {err}"
                )),
            )
            .await;
        }
        let delay = retry_after
            .and_then(|d| Duration::from_std(d).ok())
            .unwrap_or_else(|| {
                let mut rng = rand::rng();
                backoff_delay(attempts.max(1) as u32, policy.jitter_ms, &mut || {
                    rng.random()
                })
            });
        return requeue_for_gate(pool, leased, now + delay).await;
    }
    finish(pool, leased, JobStatus::Failed, Some(&err.to_string())).await
}

/// Megjelenítési név a nyers payloadból — kizárólag akkor, ha a payload
/// source_code-ja megegyezik a job hiteles slugjával. A payload forráskódja
/// NEM azonosít forrást (megszemélyesítés-védelem): eltérő vagy hiányzó kód
/// esetén a slug a megjelenítési név, így a nyers megőrzés metaadat-hiányon
/// sem bukik el.
fn raw_display_name(bytes: &[u8], source_slug: &str) -> String {
    let value: Option<serde_json::Value> = serde_json::from_slice(bytes).ok();
    let field =
        |key: &str| -> Option<String> { value.as_ref()?.get(key)?.as_str().map(str::to_string) };
    if field("source_code").as_deref() == Some(source_slug)
        && let Some(name) = field("source_display_name")
    {
        return name;
    }
    source_slug.to_string()
}

/// A meccs csapat-kontextusa a verification-szabályokhoz: név és ország-kód.
#[derive(Default)]
struct TeamContextInfo {
    home_name: Option<String>,
    away_name: Option<String>,
    home_country_code: Option<String>,
    away_country_code: Option<String>,
}

/// Hazai/vendég csapatnév + ország-kód a SAME_TEAM_BOTH_SIDES és a
/// FOREIGN_TEAMS_DOMESTIC_COMPETITION szabályhoz. Hiányzó csapatsor → `None`
/// (a szabályok ilyenkor nem tüzelnek).
async fn load_team_context(
    pool: &PgPool,
    home_team_id: Uuid,
    away_team_id: Uuid,
) -> Result<TeamContextInfo> {
    let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT t.id, t.name, c.code
           FROM core.team t
           LEFT JOIN core.country c ON c.id = t.country_id
          WHERE t.id = ANY($1)",
    )
    .bind(vec![home_team_id, away_team_id])
    .fetch_all(pool)
    .await
    .context("csapat-kontextus betöltése a verification-höz")?;
    let mut info = TeamContextInfo::default();
    for (id, name, country_code) in rows {
        if id == home_team_id {
            info.home_name = Some(name);
            info.home_country_code = country_code;
        } else if id == away_team_id {
            info.away_name = Some(name);
            info.away_country_code = country_code;
        }
    }
    Ok(info)
}

/// A meccs versenyének ország-kódja (FOREIGN_TEAMS_DOMESTIC_COMPETITION).
async fn load_competition_country(pool: &PgPool, match_id: Uuid) -> Result<Option<String>> {
    let row: Option<(Option<String>, String)> = sqlx::query_as(
        "SELECT ctry.code, c.name
           FROM core.match m
           JOIN core.competition c ON c.id = m.competition_id
           LEFT JOIN core.country ctry ON ctry.id = c.country_id
          WHERE m.id = $1",
    )
    .bind(match_id)
    .fetch_optional(pool)
    .await
    .context("verseny-ország betöltése a verification-höz")?;
    let Some((code, name)) = row else {
        return Ok(None);
    };
    // A kupákban (pl. Coupe de France, Taça de Portugal) tengerentúli csapatok
    // is indulhatnak, ezért ott a FOREIGN_TEAMS_DOMESTIC_COMPETITION szabály
    // ne kapjon országot (ne tüzeljen).
    if football_domain::identity::looks_like_cup_competition(&name) {
        return Ok(None);
    }
    Ok(code)
}

/// Kanonikus materializálás + meccsenkénti verification és publish gate.
/// Az identitás-flagjei a fail-closed linker feloldásából jönnek (S2b-1):
/// egzakt feloldás → true, minden más (Candidates/NotFound/hiba) → false,
/// így a meccs withheld marad, amíg a review-döntés meg nem születik.
///
/// S2b-2: a trust sorok egyszer töltődnek be az ÖSSZES hozzájáruló forrásra
/// (a meccsek source_values forráskódjai + a fixture forrása), az odds
/// meccsenként; a gate után az odds trust_state frissül (NoOdds esetén nem).
pub async fn verify_and_publish(
    pool: &PgPool,
    normalized: &football_ingest::normalized::NormalizedDay,
) -> Result<()> {
    let day = Canonicalizer::materialize(pool, normalized).await?;
    let rules = default_rules();
    // Anyagi forrásértékek előrebetöltése, hogy a trust lista a meccs-loop
    // előtt egyszer álljon elő minden hozzájáruló forráskódra.
    let mut material_values = Vec::with_capacity(day.matches.len());
    let mut source_codes = vec![normalized.source_code.clone()];
    for materialized in &day.matches {
        let values = load_material_values(pool, materialized.match_id).await?;
        for value in &values {
            if !source_codes.contains(&value.source_code) {
                source_codes.push(value.source_code.clone());
            }
        }
        material_values.push(values);
    }
    let source_trust = trust::trust_for_sources(pool, &source_codes).await?;

    for (materialized, source_values) in day.matches.iter().zip(&material_values) {
        let home_identity_exact = materialized.identity_exact;
        let away_identity_exact = materialized.identity_exact;
        let duplicate_match_count = repo::count_duplicate_matches(
            pool,
            materialized.match_id,
            materialized.home_team_id,
            materialized.away_team_id,
            materialized.snapshot.kickoff_at,
        )
        .await?;
        let odds_values = trust::load_match_odds(pool, materialized.match_id).await?;
        let resolved_fields = load_resolved_fields(pool, materialized.match_id).await?;
        let team =
            load_team_context(pool, materialized.home_team_id, materialized.away_team_id).await?;
        let competition_country = load_competition_country(pool, materialized.match_id).await?;
        let context = VerificationContext {
            snapshot: &materialized.snapshot,
            home_identity_exact,
            away_identity_exact,
            home_team_name: team.home_name,
            away_team_name: team.away_name,
            home_team_country: team.home_country_code,
            away_team_country: team.away_country_code,
            competition_country,
            duplicate_match_count,
            source_values,
            source_trust: &source_trust,
            odds_values: &odds_values,
            odds_tolerance: ODDS_TOLERANCE_DEFAULT,
            resolved_fields: &resolved_fields,
        };
        let report = VerificationEngine::run(&rules, &context);
        persist_report(pool, materialized.match_id, &report, RULE_SET_VERSION).await?;
        let date = local_publish_date(materialized.snapshot.kickoff_at, PUBLISH_TIMEZONE)?;
        evaluate_and_persist_gate(pool, materialized.match_id, date, &report, RULE_SET_VERSION)
            .await?;
        // S80: a kickoff áthelyezése után a régi helyi dátumra írt gate sor
        // elavult derivált adat (a publish_date a kickoffból számolódik),
        // ezért töröljük — ugyanaz a szabály, mint a reverify útban.
        sqlx::query("DELETE FROM ops.publish_gate WHERE match_id=$1 AND publish_date <> $2")
            .bind(materialized.match_id)
            .bind(date)
            .execute(pool)
            .await?;
        // Odds trust_state a gate után; üres odds listán nincs frissítés.
        let odds_state = evaluate_odds_trust(&odds_values, ODDS_TOLERANCE_DEFAULT);
        if odds_state != OddsTrustState::NoOdds {
            trust::update_odds_trust_state(pool, materialized.match_id, odds_state).await?;
        }
    }
    Ok(())
}

/// Egy csapat fail-closed identitás-feloldása a tick live ágában.
///
/// A linker `Exact` kimenete true; `Candidates`/`NotFound` esetén a linker
/// pending review-sort nyitott, a flag false (a meccs withheld). DB-hibán
/// a hiba naplózódik és a flag false marad (fail-closed).
///
/// Szándékosan nyilvános: az E2E tesztek a withheld → review → approve
/// ciklust közvetlenül is bizonyítják. Teljes tick újrafutásnál a
/// canonicalize lépés újraélesztené a fixture_seed refet (ON CONFLICT
/// DO UPDATE status='active'), ezért a ciklus resolve-szinten bizonyított;
/// tick-szintű withheld állapot nem-fixture forrásoknál jelentkezik (S3).
pub async fn resolve_team_identity(
    pool: &PgPool,
    source_id: Uuid,
    source_slug: &str,
    day: &football_ingest::normalized::NormalizedDay,
    team_id: Uuid,
) -> bool {
    let Some(team) = day.teams.iter().find(|t| t.id == Some(team_id)) else {
        return false;
    };
    let ctx = IdentityContext {
        country_code: Some(team.country_code.clone()),
        gender: Some(team.gender.clone()),
        age_group: Some(team.age_group.clone()),
        branch: Some(team.branch.clone()),
    };
    match linker::resolve(
        pool,
        source_id,
        source_slug,
        "team",
        &team.source_entity_id,
        &team.name,
        &ctx,
    )
    .await
    {
        Ok(Resolution::Exact { .. }) => true,
        // Candidates/NotFound: a pending review-sort a linker létrehozta.
        Ok(_) => false,
        Err(err) => {
            tracing::error!(error = %err, "identitás-feloldás hiba (fail-closed)");
            false
        }
    }
}

/// Egy meccs újraellenőrzése a tárolt megfigyelésekből (fetch nélkül).
/// A trust- és konfliktus-szabály a provenance-ból töltött forrásértékekkel
/// fut, így a korábbi, forrásérték nélkül számolt gate-állapotok javulnak.
pub async fn reverify_match(pool: &PgPool, match_id: Uuid) -> Result<()> {
    use football_domain::MatchStatus;
    use football_domain::match_state::{MatchResultSnapshot, MatchSnapshot};

    let (home, away, kickoff_at, status_text): (Uuid, Uuid, DateTime<Utc>, String) =
        sqlx::query_as(
            "SELECT home_team_id, away_team_id, kickoff_at, status FROM core.match WHERE id=$1",
        )
        .bind(match_id)
        .fetch_one(pool)
        .await
        .context("meccs betöltése az újraellenőrzéshez")?;
    let status = match status_text.as_str() {
        "scheduled" => MatchStatus::Scheduled,
        "confirmed" => MatchStatus::Confirmed,
        "live" => MatchStatus::Live,
        "halftime" => MatchStatus::Halftime,
        "finished" => MatchStatus::Finished,
        "postponed" => MatchStatus::Postponed,
        "canceled" => MatchStatus::Canceled,
        "abandoned" => MatchStatus::Abandoned,
        "awarded" => MatchStatus::Awarded,
        other => anyhow::bail!("ismeretlen meccs-státusz: {other}"),
    };
    let result: Option<MatchResultSnapshot> = sqlx::query_as::<
        _,
        (
            Option<i16>,
            Option<i16>,
            Option<i16>,
            Option<i16>,
            Option<i16>,
            Option<i16>,
            Option<i16>,
            Option<i16>,
        ),
    >(
        "SELECT home_ft, away_ft, home_ht, away_ht, home_et, away_et, home_pens, away_pens
         FROM core.match_result WHERE match_id=$1",
    )
    .bind(match_id)
    .fetch_optional(pool)
    .await?
    .map(
        |(home_ft, away_ft, home_ht, away_ht, home_et, away_et, home_pens, away_pens)| {
            MatchResultSnapshot {
                home_ft,
                away_ft,
                home_ht,
                away_ht,
                home_et,
                away_et,
                home_pens,
                away_pens,
            }
        },
    );
    let snapshot = MatchSnapshot {
        id: match_id,
        home_team_id: home,
        away_team_id: away,
        kickoff_at,
        status,
        result,
    };

    let source_values = load_material_values(pool, match_id).await?;
    let codes: Vec<String> = source_values
        .iter()
        .map(|v| v.source_code.clone())
        .collect();
    let source_trust = trust::trust_for_sources(pool, &codes).await?;
    let odds_values = trust::load_match_odds(pool, match_id).await?;
    let resolved_fields = load_resolved_fields(pool, match_id).await?;
    let duplicate_match_count =
        repo::count_duplicate_matches(pool, match_id, home, away, kickoff_at).await?;
    let team = load_team_context(pool, home, away).await?;
    let competition_country = load_competition_country(pool, match_id).await?;
    let context = VerificationContext {
        snapshot: &snapshot,
        home_identity_exact: true,
        away_identity_exact: true,
        home_team_name: team.home_name,
        away_team_name: team.away_name,
        home_team_country: team.home_country_code,
        away_team_country: team.away_country_code,
        competition_country,
        duplicate_match_count,
        source_values: &source_values,
        source_trust: &source_trust,
        odds_values: &odds_values,
        odds_tolerance: ODDS_TOLERANCE_DEFAULT,
        resolved_fields: &resolved_fields,
    };
    let report = VerificationEngine::run(&default_rules(), &context);
    persist_report(pool, match_id, &report, RULE_SET_VERSION).await?;
    let date = local_publish_date(kickoff_at, PUBLISH_TIMEZONE)?;
    evaluate_and_persist_gate(pool, match_id, date, &report, RULE_SET_VERSION).await?;
    // S80: a kickoff áthelyezése után a régi helyi dátumra írt gate sor elavult
    // derivált adat (a publish_date a kickoffból számolódik), ezért töröljük.
    // A friss sor már fentebb bekerült, így a meccsnek egyetlen aktuális
    // gate állapota marad; a történetet az audit és a verification report adja.
    sqlx::query("DELETE FROM ops.publish_gate WHERE match_id=$1 AND publish_date <> $2")
        .bind(match_id)
        .bind(date)
        .execute(pool)
        .await?;
    let odds_state = evaluate_odds_trust(&odds_values, ODDS_TOLERANCE_DEFAULT);
    if odds_state != OddsTrustState::NoOdds {
        trust::update_odds_trust_state(pool, match_id, odds_state).await?;
    }
    Ok(())
}

/// S37e: a nap pontos duplikátumainak automatikus merge-elése (S37c motorral).
/// A túlélő a legkorábban létrehozott meccs (a meglévő kanonikus verseny),
/// a pontos kulcs (azonos kickoff + azonos csapatpár) miatt biztonságos.
async fn auto_dedupe_day(
    pool: &sqlx::PgPool,
    date: chrono::NaiveDate,
) -> anyhow::Result<(usize, usize)> {
    let exact_groups = dedupe::duplicate_groups(pool, date).await?;
    // A források ugyanazt a fixture-t gyakran más versenynéven adják (pl.
    // „Superliga" vs „Superligaen"), ezért a napi automatikus pass a
    // cross-competition név-variánsokat is összevonja. A szigorú ág csak akkor
    // enged, ha a két sor kickoffja legfeljebb 2 órával tér el, a nevek
    // mindkét oldalon egyeznek, és a nem/korosztály/ország kontextus kompatibilis
    // (lásd `is_strict_variant_same_match`), így a felnőtt és utánpótlás sorok
    // nem olvadnak össze.
    let variant_groups = dedupe::variant_duplicate_groups_between(pool, date, date, true).await?;
    let mut merged = 0usize;
    let mut skipped = 0usize;

    for group in exact_groups {
        if group.matches.len() < 2 {
            continue;
        }
        let mut rows = group.matches;
        rows.sort_by_key(|row| (row.created_at, std::cmp::Reverse(row.provenance_count)));
        let survivor = rows[0].match_id;
        for victim in &rows[1..] {
            match auto_merge_pair(pool, survivor, victim.match_id, false).await? {
                AutoMerge::Merged => merged += 1,
                AutoMerge::Skipped => skipped += 1,
            }
        }
    }

    for group in variant_groups {
        if group.matches.len() < 2 {
            continue;
        }
        let mut rows = group.matches;
        // Név-variánsnál a több provenanciájú, eredménnyel bíró sor a túlélő,
        // hogy az eredmény ne vesszen el.
        rows.sort_by_key(|row| {
            (
                std::cmp::Reverse(row.has_result),
                std::cmp::Reverse(row.has_odds),
                std::cmp::Reverse(row.provenance_count),
                row.created_at,
            )
        });
        let survivor = rows[0].match_id;
        for victim in &rows[1..] {
            match auto_merge_pair(pool, survivor, victim.match_id, true).await? {
                AutoMerge::Merged => merged += 1,
                AutoMerge::Skipped => skipped += 1,
            }
        }
    }

    // S26/S37e: fixture-szintű párok (azonos verseny+szezon+csapat-pár+UTC-nap,
    // de eltérő kickoff) — a research forrás saját fuzzy duplikátumai ilyenek.
    let fixture_groups =
        dedupe::duplicate_fixture_groups(pool, Some(date), Some(date), None).await?;
    for group in fixture_groups {
        if group.matches.len() < 2 {
            continue;
        }
        let mut rows = group.matches;
        rows.sort_by_key(|row| {
            (
                std::cmp::Reverse(row.has_result),
                std::cmp::Reverse(row.has_odds),
                std::cmp::Reverse(row.provenance_count),
                row.created_at,
            )
        });
        let survivor = rows[0].match_id;
        for victim in &rows[1..] {
            match auto_merge_fixture(pool, survivor, victim.match_id).await? {
                AutoMerge::Merged => merged += 1,
                AutoMerge::Skipped => skipped += 1,
            }
        }
    }

    Ok((merged, skipped))
}

/// Fixture-szintű pár merge-elése (S26 szabály: azonos verseny+szezon+csapat-pár+
/// UTC-nap, azonos orientáció, eredmény csak akkor cserélhető, ha a túlélőnél
/// nincs). Fordított sorrendben is próbáljuk.
async fn auto_merge_fixture(
    pool: &sqlx::PgPool,
    survivor: sqlx::types::Uuid,
    duplicate: sqlx::types::Uuid,
) -> anyhow::Result<AutoMerge> {
    if dedupe::is_same_fixture(pool, survivor, duplicate).await? {
        dedupe::merge_duplicate(
            pool,
            survivor,
            duplicate,
            "S37e automatikus fixture-dedupe (research ingest)",
        )
        .await?;
        return Ok(AutoMerge::Merged);
    }
    if dedupe::is_same_fixture(pool, duplicate, survivor).await? {
        dedupe::merge_duplicate(
            pool,
            duplicate,
            survivor,
            "S37e automatikus fixture-dedupe (research ingest, fordított pár)",
        )
        .await?;
        return Ok(AutoMerge::Merged);
    }
    Ok(AutoMerge::Skipped)
}

enum AutoMerge {
    Merged,
    Skipped,
}

/// Egy pár automatikus merge-elése; ha az eredmény/orientáció miatt a sorrend
/// nem fordítható le, a szerepeket megcserélve próbáljuk újra.
async fn auto_merge_pair(
    pool: &sqlx::PgPool,
    survivor: sqlx::types::Uuid,
    duplicate: sqlx::types::Uuid,
    variant: bool,
) -> anyhow::Result<AutoMerge> {
    let same = if variant {
        dedupe::is_strict_variant_same_match(pool, survivor, duplicate, false).await?
    } else {
        dedupe::is_same_match(pool, survivor, duplicate).await?
    };
    if same {
        dedupe::merge_duplicate(
            pool,
            survivor,
            duplicate,
            "S37e automatikus napi dedupe (research ingest)",
        )
        .await?;
        return Ok(AutoMerge::Merged);
    }
    let flipped = if variant {
        dedupe::is_strict_variant_same_match(pool, duplicate, survivor, false).await?
    } else {
        dedupe::is_same_match(pool, duplicate, survivor).await?
    };
    if flipped {
        dedupe::merge_duplicate(
            pool,
            duplicate,
            survivor,
            "S37e automatikus napi dedupe (research ingest, fordított pár)",
        )
        .await?;
        return Ok(AutoMerge::Merged);
    }
    Ok(AutoMerge::Skipped)
}

#[cfg(test)]
mod tests {
    use super::{
        BACKOFF_CAP_SECS, backoff_delay, catchup_source_stale, next_quota_reset,
        research_refresh_target, stale_slot_job,
    };
    use chrono::{NaiveDate, TimeZone, Utc};
    use football_domain::policy::{FetchJob, Transport};
    use uuid::Uuid;

    /// S139: a 07:31-es job nem olvashatja a 07:38-as frissítés előtti fájlt;
    /// 07:46-kor viszont már igen. Hiányzó fájl és tartósan elavult forrás
    /// szintén kezelt.
    #[test]
    fn catchup_waits_until_the_source_file_is_fresh() {
        let created = Utc.with_ymd_and_hms(2026, 11, 25, 6, 31, 0).unwrap();
        let early = Utc.with_ymd_and_hms(2026, 11, 25, 6, 32, 0).unwrap();
        let late = Utc.with_ymd_and_hms(2026, 11, 25, 6, 46, 0).unwrap();
        let old_file = Utc.with_ymd_and_hms(2026, 11, 24, 18, 0, 0).unwrap();
        let fresh_file = Utc.with_ymd_and_hms(2026, 11, 25, 6, 38, 0).unwrap();

        // A daemon 07:31-kor indult, a fájl még tegnapi → várakozás.
        assert!(catchup_source_stale(Some(old_file), created, early));
        // 07:46-kor a fájl már friss → feldolgozás.
        assert!(!catchup_source_stale(Some(fresh_file), created, late));
        // Hiányzó fájl → várakozás.
        assert!(catchup_source_stale(None, created, late));
        // Tartósan nem frissülő forrás: 6 óra után mégis feldolgozás.
        let much_later = created + chrono::Duration::hours(7);
        assert!(!catchup_source_stale(Some(old_file), created, much_later));
        assert!(!catchup_source_stale(None, created, much_later));
    }

    /// S179: a tegnapi slot hálózati forrásnál érvénytelen, de a research
    /// (helyi fájl) régi napja továbbra is olvasható marad.
    #[test]
    fn stale_slot_job_only_invalidates_network_sources() {
        let now = Utc.with_ymd_and_hms(2026, 11, 26, 14, 0, 0).unwrap();
        let mut job = FetchJob {
            id: Uuid::now_v7(),
            source_slug: "tipplap".into(),
            domain: "day".into(),
            entity_key: "slot-03".into(),
            event_type: "refresh".into(),
            time_window: "2026-11-25".into(),
            due_at: now,
            priority: 0,
        };
        assert!(stale_slot_job(&job, Transport::RustHttp, now));
        assert!(!stale_slot_job(&job, Transport::LocalFile, now));

        job.time_window = "2026-11-26".into();
        assert!(!stale_slot_job(&job, Transport::RustHttp, now));

        job.entity_key = "all".into();
        job.time_window = "2026-11-25".into();
        assert!(!stale_slot_job(&job, Transport::RustHttp, now));
    }

    #[test]
    fn quota_blocked_job_waits_for_the_next_utc_midnight() {
        let now = Utc.with_ymd_and_hms(2026, 11, 22, 14, 48, 10).unwrap();
        assert_eq!(
            next_quota_reset(now),
            Utc.with_ymd_and_hms(2026, 11, 23, 0, 0, 5).unwrap()
        );
    }

    #[test]
    fn quota_reset_rolls_to_the_following_day_late_at_night() {
        let now = Utc.with_ymd_and_hms(2026, 11, 22, 23, 59, 59).unwrap();
        assert_eq!(
            next_quota_reset(now),
            Utc.with_ymd_and_hms(2026, 11, 23, 0, 0, 5).unwrap()
        );
    }

    #[test]
    fn backoff_grows_and_is_capped_without_jitter() {
        assert_eq!(backoff_delay(1, 0, &mut || 0).num_seconds(), 60);
        assert_eq!(backoff_delay(2, 0, &mut || 0).num_seconds(), 120);
        assert_eq!(backoff_delay(4, 0, &mut || 0).num_seconds(), 480);
        assert_eq!(
            backoff_delay(20, 0, &mut || 0).num_seconds(),
            BACKOFF_CAP_SECS
        );
    }

    #[test]
    fn backoff_jitter_stays_below_the_configured_window() {
        let delay = backoff_delay(10, 500, &mut || 499);
        assert_eq!(delay.num_milliseconds(), BACKOFF_CAP_SECS * 1000 + 499);
        let no_jitter = backoff_delay(10, 0, &mut || 499);
        assert_eq!(no_jitter.num_milliseconds(), BACKOFF_CAP_SECS * 1000);
    }

    /// S163 regresszió-őr: a `slot-NN` refresh a `time_window`-ból olvas, a
    /// `catchup-` az `entity_key`-ból — a kettő felcserélése elavult napot
    /// olvasna be, és a 07:30-as zárás utáni eredmények soha nem jutnának el.
    #[test]
    fn research_refresh_target_reads_slot_from_time_window_catchup_from_key() {
        let slot = FetchJob {
            id: Uuid::nil(),
            source_slug: "research".into(),
            domain: "day".into(),
            entity_key: "slot-00".into(),
            event_type: "refresh".into(),
            time_window: "2026-11-26".into(),
            due_at: Utc::now(),
            priority: 0,
        };

        // slot: a cél dátum a time_window, nem az entity_key sorszáma.
        assert_eq!(
            research_refresh_target(&slot),
            Some(NaiveDate::from_ymd_opt(2026, 11, 26).unwrap())
        );

        // catchup: a time_window a mai nap marad, a cél az entity_key-ben van.
        let catchup = FetchJob {
            entity_key: "catchup-2026-11-25".into(),
            ..slot.clone()
        };
        assert_eq!(
            research_refresh_target(&catchup),
            Some(NaiveDate::from_ymd_opt(2026, 11, 25).unwrap())
        );

        // Nem research forrás: None, még slot + refresh mellett is.
        let other_source = FetchJob {
            source_slug: "soccerway".into(),
            ..slot.clone()
        };
        assert_eq!(research_refresh_target(&other_source), None);

        // Nem refresh esemény: None.
        let other_event = FetchJob {
            event_type: "collect".into(),
            ..slot.clone()
        };
        assert_eq!(research_refresh_target(&other_event), None);

        // Ismeretlen entity_key: None (pl. az "all" interval job).
        let all = FetchJob {
            entity_key: "all".into(),
            ..slot.clone()
        };
        assert_eq!(research_refresh_target(&all), None);

        // Hibás catchup-dátum: None, nem esik vissza a time_window-ra.
        let broken = FetchJob {
            entity_key: "catchup-nem-datum".into(),
            ..slot
        };
        assert_eq!(research_refresh_target(&broken), None);
    }
}
