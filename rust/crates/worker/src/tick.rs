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
//! Controller-döntések: kvóta/delay blokknál a job azonnal pending-re zárul
//! (nem marad leased); a complete_job fencing nélkül, saját worker_id-val zár
//! (egy-workeres S2a); a breaker auth-ág egy tranzakcióban auditál.

use std::collections::HashMap;
use std::time::{Duration as StdDuration, Instant};

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use football_domain::error::FetchError;
use football_domain::identity::{IdentityContext, Resolution};
use football_domain::policy::{
    CoverageState, FetchJob, FetchOutcome, JobStatus, SourceMode, SourcePolicy, Transport,
};
use football_domain::trust::{ODDS_TOLERANCE_DEFAULT, OddsTrustState};
use football_identity::linker;
use football_ingest::adapter::{ParseCtx, SourceAdapter};
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::fetch::{FetchLimits, FetchRequest, Fetcher, Fetchers};
use football_ingest::observe::insert_observation;
use football_storage::object_store::ObjectStore;
use football_storage::repositories::trust;
use football_storage::repositories::worker::{self as repo, LedgerEntry, RawCapture};
use football_verification::conflict::load_material_values;
use football_verification::engine::{VerificationContext, VerificationEngine};
use football_verification::gate::{evaluate_and_persist_gate, local_publish_date};
use football_verification::odds::evaluate_odds_trust;
use football_verification::persist::persist_report;
use football_verification::rules::default_rules;
use rand::Rng;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{breaker, coverage, quota, robots, scheduler};

const LEASE_SECS: i64 = 60;
/// Backoff-alap (első próba): 60 s; a sapka 1 óra.
const BACKOFF_BASE_SECS: i64 = 60;
const BACKOFF_CAP_SECS: i64 = 3600;
const CLAIM_LIMIT: i64 = 10;
const FETCH_TIMEOUT_SECS: u64 = 30;
const RULE_SET_VERSION: &str = "verification-1.1";
const PUBLISH_TIMEZONE: &str = "Europe/Budapest";

/// Egy tick kör összesítése: feldolgozott (done), blokkolt (kapuk/cancelled)
/// és sikertelen (failed) jobok száma.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickOutcome {
    pub processed: usize,
    pub blocked: usize,
    pub failed: usize,
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
    };
    for job in &jobs {
        match process_job(pool, store, fetchers, adapters, job, now).await {
            Ok(JobResult::Processed) => outcome.processed += 1,
            Ok(JobResult::Blocked) => outcome.blocked += 1,
            Ok(JobResult::Failed) => outcome.failed += 1,
            Err(err) => {
                // Váratlan infrastruktúra-hiba: a jobot lezárjuk failed-re,
                // a kör a többi jobbal folytatódik. A reserve óta számláló
                // ledger-placeholder transport_error-ként marad (jogos kísérlet).
                tracing::error!(job_id = %job.id, error = %err, "tick: a job feldolgozása megszakadt");
                if let Err(complete_err) =
                    scheduler::complete(pool, job.id, JobStatus::Failed, Some(&err.to_string()))
                        .await
                {
                    tracing::error!(job_id = %job.id, error = %complete_err,
                        "tick: a hibás job lezárása sem sikerült");
                }
                outcome.failed += 1;
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
    job: &FetchJob,
    now: DateTime<Utc>,
) -> Result<JobResult> {
    // 2. Policy-kapu: hiányzó vagy tiltott policy → cancelled.
    let Some(policy) = repo::get_policy(pool, &job.source_slug).await? else {
        scheduler::complete(
            pool,
            job.id,
            JobStatus::Cancelled,
            Some("hiányzó source policy"),
        )
        .await?;
        return Ok(JobResult::Blocked);
    };
    if !policy.enabled {
        scheduler::complete(
            pool,
            job.id,
            JobStatus::Cancelled,
            Some("tiltott source policy"),
        )
        .await?;
        return Ok(JobResult::Blocked);
    }
    let Some(adapter) = adapters.get(&job.source_slug) else {
        scheduler::complete(
            pool,
            job.id,
            JobStatus::Failed,
            Some("nincs regisztrált adapter a forráshoz"),
        )
        .await?;
        return Ok(JobResult::Failed);
    };

    // 4. (előrehozva) A terv első kérése határozza meg a robots- és fetch-utat.
    let plan = adapter.plan(job);
    let Some(req) = plan.first() else {
        scheduler::complete(pool, job.id, JobStatus::Failed, Some("üres fetch-terv")).await?;
        return Ok(JobResult::Failed);
    };

    // A tartalom-fetch fetcherét a policy transzportja választja (S2c).
    let fetcher = match fetchers.for_transport(&policy.transport) {
        Ok(fetcher) => fetcher,
        Err(err) => {
            scheduler::complete(pool, job.id, JobStatus::Failed, Some(&err.to_string())).await?;
            return Ok(JobResult::Failed);
        }
    };

    // 2. Robots-kapu: fail-closed, nem számláló ledger + coverage blocked.
    // Fixture forrásnál a robots.txt a fixture fetcheren megy (nincs hálózat);
    // valódi forrásnál plain HTTP-vel (a böngésző itt fölösleges és drága).
    let robots_fetcher: &dyn Fetcher = if policy.transport == Transport::Fixture {
        fetcher
    } else {
        fetchers.rust_http()
    };
    if robots::check(pool, robots_fetcher, &job.source_slug, &req.url).await?
        == robots::RobotsVerdict::Blocked
    {
        block_ledger(pool, job, req, now, FetchOutcome::RobotsBlocked).await?;
        coverage::set(
            pool,
            &job.source_slug,
            &job.domain,
            &job.event_type,
            &job.time_window,
            CoverageState::Blocked,
            job.id,
        )
        .await?;
        scheduler::complete(pool, job.id, JobStatus::Failed, Some("robots tiltás")).await?;
        return Ok(JobResult::Blocked);
    }

    // 2. Kvóta/delay-kapu: nem számláló ledger, azonnali pending visszasorolás.
    let decision = quota::guard(pool, &policy, now).await?;
    if decision != quota::QuotaDecision::Allowed {
        block_ledger(pool, job, req, now, FetchOutcome::QuotaBlocked).await?;
        let reason = if decision == quota::QuotaDecision::QuotaBlocked {
            "napi kvóta kimerült"
        } else {
            "a min_delay még nem telt le"
        };
        scheduler::complete(pool, job.id, JobStatus::Pending, Some(reason)).await?;
        return Ok(JobResult::Blocked);
    }

    // 2. Breaker-kapu: nyitva → nem számláló ledger + failed.
    if !breaker::allow(pool, &policy, now).await? {
        block_ledger(pool, job, req, now, FetchOutcome::BreakerOpen).await?;
        scheduler::complete(pool, job.id, JobStatus::Failed, Some("breaker nyitva")).await?;
        return Ok(JobResult::Blocked);
    }

    // 3. Reserve: a placeholder már a reserve pillanatában kísérletnek számít.
    let ledger_id = repo::insert_ledger(
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
    .await?;

    // 4. Fetch a policy limitjeivel.
    let limits = FetchLimits {
        max_bytes: u64::try_from(policy.max_bytes).context("policy max_bytes nem ábrázolható")?,
        timeout: StdDuration::from_secs(FETCH_TIMEOUT_SECS),
    };
    let started = Instant::now();
    let fetched = fetcher.fetch(req, &limits).await;
    let latency_ms = i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX);
    let raw = match fetched {
        Ok(raw) => raw,
        Err(err) => {
            return fetch_failure(pool, &policy, job, ledger_id, now, latency_ms, &err).await;
        }
    };
    // A breaker a fetch-egészséget tükrözi: a sikeres fetch azonnal zárja
    // (half-open probe sem ragadhat be parse-karantén/infra-hiba miatt).
    breaker::on_success(pool, &job.source_slug).await?;
    // 5–10. Sikeres fetch utáni szakasz. Váratlan infrastruktúra-hibán
    // (objektumtár-írás, nem napi időablak, részleges verify_and_publish)
    // a coverage Failed-re áll, mielőtt a tick a jobot failed-re zárja.
    // A ledger-placeholder transport_error marad (dokumentált, jogos kísérlet).
    let result = async {
        let http_status = i16::try_from(raw.http_status).context("HTTP-státusz i16 konverzió")?;
        let byte_size = i64::try_from(raw.bytes.len()).ok();

        // 5. L0: objektumtár + forrás/artifact/capture megőrzés (parse előtt,
        // hogy a karanténba került payload nyersben is megmaradjon).
        // A forrásazonosító a job hiteles slugja; a payload kódja csak a
        // megjelenítési névhez járulhat hozzá (megszemélyesítés-védelem).
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

        // 6. Parse: hiba → karantén (coverage quarantined, ledger parse_error),
        // a nyers megmarad. A breaker nem változik: a forrás válaszolt.
        let requested_date = NaiveDate::parse_from_str(&job.time_window, "%Y-%m-%d")
            .with_context(|| format!("a job időablaka nem napi dátum: {}", job.time_window))?;
        let ctx = ParseCtx {
            requested_date,
            request_key: req.request_key.clone(),
        };
        let observations = match adapter.parse(&raw, &ctx) {
            Ok(observations) => observations,
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
                coverage::set(
                    pool,
                    &job.source_slug,
                    &job.domain,
                    &job.event_type,
                    &job.time_window,
                    CoverageState::Quarantined,
                    job.id,
                )
                .await?;
                scheduler::complete(pool, job.id, JobStatus::Failed, Some(&err.to_string()))
                    .await?;
                tracing::warn!(job_id = %job.id, error = %err, "tick: parse karantén, a nyers megmaradt");
                return Ok(JobResult::Failed);
            }
        };

        // 7. Observationök a stage rétegbe.
        for obs in &observations {
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

        // 8–9. Shadow ágban nincs kanonikus írás; live/canary ágban a teljes
        // verification + gate lánc fut (a footballctl IngestFixture mintáján).
        if policy.mode != SourceMode::Shadow {
            // A kanonikus út az observationökből indul (forrás-független).
            let normalized = football_ingest::normalized::NormalizedDay::from_observations(
                &observations,
            )
            .context("observationökből normalizált nap")?;
            verify_and_publish(pool, &normalized).await?;
        }

        // 10. Tick-zárás: ledger success, coverage done (0 observation → no_data),
        // breaker closed, job done.
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
        let coverage_state = if observations.is_empty() {
            CoverageState::NoData
        } else {
            CoverageState::Done
        };
        coverage::set(
            pool,
            &job.source_slug,
            &job.domain,
            &job.event_type,
            &job.time_window,
            coverage_state,
            job.id,
        )
        .await?;
        scheduler::complete(pool, job.id, JobStatus::Done, None).await?;
        Ok(JobResult::Processed)
    }
    .await;
    if result.is_err() {
        // Best-effort coverage-failed: ha ez sem megy, a hiba naplózódik,
        // az eredeti hiba terjed tovább a tick job-záró ágába.
        if let Err(cov_err) = coverage::set(
            pool,
            &job.source_slug,
            &job.domain,
            &job.event_type,
            &job.time_window,
            CoverageState::Failed,
            job.id,
        )
        .await
        {
            tracing::error!(job_id = %job.id, error = %cov_err,
                "tick: a coverage failed állapot rögzítése sem sikerült");
        }
    }
    result
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
    job: &FetchJob,
    ledger_id: Uuid,
    now: DateTime<Utc>,
    latency_ms: i64,
    err: &FetchError,
) -> Result<JobResult> {
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
    coverage::set(
        pool,
        &job.source_slug,
        &job.domain,
        &job.event_type,
        &job.time_window,
        CoverageState::Failed,
        job.id,
    )
    .await?;
    if transient {
        let attempts = repo::count_job_fetch_attempts(pool, job.id).await?;
        let delay = retry_after
            .and_then(|d| Duration::from_std(d).ok())
            .unwrap_or_else(|| {
                let mut rng = rand::rng();
                backoff_delay(attempts.max(1) as u32, policy.jitter_ms, &mut || {
                    rng.random()
                })
            });
        repo::requeue_with_delay(pool, job.id, now + delay).await?;
        return Ok(JobResult::Blocked);
    }
    scheduler::complete(pool, job.id, JobStatus::Failed, Some(&err.to_string())).await?;
    Ok(JobResult::Failed)
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
    if field("source_code").as_deref() == Some(source_slug) {
        if let Some(name) = field("source_display_name") {
            return name;
        }
    }
    source_slug.to_string()
}

/// Kanonikus materializálás + meccsenkénti verification és publish gate.
/// Az identitás-flagjei a fail-closed linker feloldásából jönnek (S2b-1):
/// egzakt feloldás → true, minden más (Candidates/NotFound/hiba) → false,
/// így a meccs withheld marad, amíg a review-döntés meg nem születik.
///
/// S2b-2: a trust sorok egyszer töltődnek be az ÖSSZES hozzájáruló forrásra
/// (a meccsek source_values forráskódjai + a fixture forrása), az odds
/// meccsenként; a gate után az odds trust_state frissül (NoOdds esetén nem).
async fn verify_and_publish(
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
        let context = VerificationContext {
            snapshot: &materialized.snapshot,
            home_identity_exact,
            away_identity_exact,
            duplicate_match_count,
            source_values,
            source_trust: &source_trust,
            odds_values: &odds_values,
            odds_tolerance: ODDS_TOLERANCE_DEFAULT,
        };
        let report = VerificationEngine::run(&rules, &context);
        persist_report(pool, materialized.match_id, &report, RULE_SET_VERSION).await?;
        let date = local_publish_date(materialized.snapshot.kickoff_at, PUBLISH_TIMEZONE)?;
        evaluate_and_persist_gate(pool, materialized.match_id, date, &report, RULE_SET_VERSION)
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
