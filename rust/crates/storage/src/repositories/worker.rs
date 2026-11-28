use crate::object_store::StoredArtifact;
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use football_domain::policy::{
    BreakerState, CoverageState, FetchJob, FetchOutcome, JobLease, JobStatus, LeasedJob,
    SourcePolicy,
};
use sqlx::{PgPool, Row, postgres::PgRow};
use uuid::Uuid;

/// A fetch_ledger egy append-only bejegyzése; a job_id szándékosan FK nélküli,
/// mert a ledger túléli a jobokat.
#[derive(Debug, Clone)]
pub struct LedgerEntry {
    pub source_slug: String,
    pub request_key: String,
    pub attempted_at: DateTime<Utc>,
    pub outcome: FetchOutcome,
    pub http_status: Option<i16>,
    pub byte_size: Option<i64>,
    pub latency_ms: Option<i64>,
    pub artifact_hash: Option<String>,
    pub job_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchReservation {
    Reserved(Uuid),
    QuotaBlocked,
    DelayBlocked,
    LeaseLost,
}

#[derive(Debug, Clone)]
pub struct BreakerRow {
    pub state: BreakerState,
    pub failure_count: i32,
    pub opened_at: Option<DateTime<Utc>>,
    pub next_probe_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub enum BreakerWrite {
    Set(BreakerRow),
    Keep,
}

#[derive(Debug, Clone)]
pub struct NewJob {
    pub source_slug: String,
    pub domain: String,
    pub entity_key: String,
    pub event_type: String,
    pub time_window: String,
    pub due_at: DateTime<Utc>,
    pub priority: i32,
}

#[derive(Debug, Clone)]
pub struct RobotsCacheRow {
    pub host: String,
    pub fetched_at: DateTime<Utc>,
    pub verdict: String,
    pub rules_text: String,
    pub expires_at: DateTime<Utc>,
}

pub async fn upsert_policy(pool: &PgPool, policy: &SourcePolicy) -> Result<()> {
    let domains =
        serde_json::to_value(&policy.domains).context("policy domains jsonb sorosítás")?;
    let events = serde_json::to_value(&policy.events).context("policy events jsonb sorosítás")?;
    sqlx::query(
        "INSERT INTO ops.source_policy
         (source_slug, transport, mode, daily_budget, min_delay_ms, jitter_ms,
          breaker_threshold, breaker_cooldown_secs, adapter_version, parser_version,
          domains, events, enabled, max_bytes)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
         ON CONFLICT (source_slug) DO UPDATE SET
          transport = EXCLUDED.transport, mode = EXCLUDED.mode,
          daily_budget = EXCLUDED.daily_budget, min_delay_ms = EXCLUDED.min_delay_ms,
          jitter_ms = EXCLUDED.jitter_ms, breaker_threshold = EXCLUDED.breaker_threshold,
          breaker_cooldown_secs = EXCLUDED.breaker_cooldown_secs,
          adapter_version = EXCLUDED.adapter_version, parser_version = EXCLUDED.parser_version,
          domains = EXCLUDED.domains, events = EXCLUDED.events,
          enabled = EXCLUDED.enabled, max_bytes = EXCLUDED.max_bytes,
          updated_at = now()",
    )
    .bind(&policy.source_slug)
    .bind(policy.transport.as_db_str())
    .bind(policy.mode.as_db_str())
    .bind(policy.daily_budget)
    .bind(policy.min_delay_ms)
    .bind(policy.jitter_ms)
    .bind(policy.breaker_threshold)
    .bind(policy.breaker_cooldown_secs)
    .bind(&policy.adapter_version)
    .bind(&policy.parser_version)
    .bind(domains)
    .bind(events)
    .bind(policy.enabled)
    .bind(policy.max_bytes)
    .execute(pool)
    .await
    .context("policy upsert")?;
    Ok(())
}

fn policy_from_row(row: &PgRow) -> Result<SourcePolicy> {
    let transport: String = row
        .try_get("transport")
        .context("policy transport oszlop")?;
    let mode: String = row.try_get("mode").context("policy mode oszlop")?;
    let domains: serde_json::Value = row.try_get("domains").context("policy domains oszlop")?;
    let events: serde_json::Value = row.try_get("events").context("policy events oszlop")?;
    Ok(SourcePolicy {
        source_slug: row
            .try_get("source_slug")
            .context("policy source_slug oszlop")?,
        transport: football_domain::policy::Transport::from_db_str(&transport)
            .context("policy transport érték")?,
        mode: football_domain::policy::SourceMode::from_db_str(&mode)
            .context("policy mode érték")?,
        daily_budget: row
            .try_get("daily_budget")
            .context("policy daily_budget oszlop")?,
        min_delay_ms: row
            .try_get("min_delay_ms")
            .context("policy min_delay_ms oszlop")?,
        jitter_ms: row
            .try_get("jitter_ms")
            .context("policy jitter_ms oszlop")?,
        breaker_threshold: row
            .try_get("breaker_threshold")
            .context("policy breaker_threshold oszlop")?,
        breaker_cooldown_secs: row
            .try_get("breaker_cooldown_secs")
            .context("policy breaker_cooldown_secs oszlop")?,
        adapter_version: row
            .try_get("adapter_version")
            .context("policy adapter_version oszlop")?,
        parser_version: row
            .try_get("parser_version")
            .context("policy parser_version oszlop")?,
        domains: serde_json::from_value(domains).context("policy domains jsonb visszafejtés")?,
        events: serde_json::from_value(events).context("policy events jsonb visszafejtés")?,
        enabled: row.try_get("enabled").context("policy enabled oszlop")?,
        max_bytes: row
            .try_get("max_bytes")
            .context("policy max_bytes oszlop")?,
    })
}

const POLICY_COLUMNS: &str = "source_slug, transport, mode, daily_budget, min_delay_ms, jitter_ms, \
     breaker_threshold, breaker_cooldown_secs, adapter_version, parser_version, \
     domains, events, enabled, max_bytes";

pub async fn get_policy(pool: &PgPool, slug: &str) -> Result<Option<SourcePolicy>> {
    let sql = format!("SELECT {POLICY_COLUMNS} FROM ops.source_policy WHERE source_slug = $1");
    let row = sqlx::query(&sql)
        .bind(slug)
        .fetch_optional(pool)
        .await
        .context("policy lekérés")?;
    row.as_ref().map(policy_from_row).transpose()
}

/// Egy job fetch-kísérletei (a nem-számláló blocked sorok nélkül).
pub async fn count_job_fetch_attempts(pool: &PgPool, job_id: Uuid) -> Result<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.fetch_ledger
         WHERE job_id = $1
           AND outcome IN ('success','http_error','timeout','oversize','transport_error','parse_error','reserved')",
    )
    .bind(job_id)
    .fetch_one(pool)
    .await
    .context("job fetch-kísérletek száma")?;
    Ok(count)
}

/// A job létrehozásának időpontja (S139 frissesség-kapuhoz).
pub async fn job_created_at(pool: &PgPool, job_id: Uuid) -> Result<DateTime<Utc>> {
    sqlx::query_scalar("SELECT created_at FROM ops.job WHERE id = $1")
        .bind(job_id)
        .fetch_one(pool)
        .await
        .context("job created_at lekérés")
}

/// Worker-állapot számlálók (CLI `worker status`).
pub struct WorkerStatusCounts {
    pub jobs_pending: i64,
    pub jobs_leased: i64,
    pub quarantined: i64,
    /// Nyitott identitás-review sorok (operatív figyelmeztetés).
    pub reviews_pending: i64,
}

pub async fn worker_status_counts(pool: &PgPool) -> Result<WorkerStatusCounts> {
    let jobs_pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ops.job WHERE status='pending'")
            .fetch_one(pool)
            .await
            .context("függő jobok száma")?;
    let jobs_leased: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.job WHERE status='leased'")
        .fetch_one(pool)
        .await
        .context("lease-elt jobok száma")?;
    let quarantined: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ops.coverage_slot WHERE state='quarantined'")
            .fetch_one(pool)
            .await
            .context("karanténos slotok száma")?;
    let reviews_pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM identity.review_queue WHERE status='pending'")
            .fetch_one(pool)
            .await
            .context("pending review-szám")?;
    Ok(WorkerStatusCounts {
        jobs_pending,
        jobs_leased,
        quarantined,
        reviews_pending,
    })
}

/// Doctor-számlálók (CLI `doctor`).
pub struct DoctorCounts {
    pub reachable: bool,
    pub publish_gates: i64,
    pub jobs_pending: i64,
    pub breakers_open: i64,
    pub quarantined: i64,
}

pub async fn doctor_counts(pool: &PgPool) -> Result<DoctorCounts> {
    let reachable: bool = sqlx::query_scalar("SELECT true")
        .fetch_one(pool)
        .await
        .map_err(|_| anyhow::anyhow!("az adatbázis nem válaszol"))?;
    let publish_gates: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.publish_gate")
        .fetch_one(pool)
        .await
        .context("gate-szám")?;
    let jobs_pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ops.job WHERE status='pending'")
            .fetch_one(pool)
            .await
            .context("függő jobok száma")?;
    let breakers_open: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ops.breaker_state WHERE state='open'")
            .fetch_one(pool)
            .await
            .context("nyitott breakerek száma")?;
    let quarantined: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ops.coverage_slot WHERE state='quarantined'")
            .fetch_one(pool)
            .await
            .context("karanténos slotok száma")?;
    Ok(DoctorCounts {
        reachable,
        publish_gates,
        jobs_pending,
        breakers_open,
        quarantined,
    })
}

/// Van-e engedélyezett policy az adott transzporttal? A footballd ezzel
/// dönti el, hogy lazy elindítsa-e az Obscura szervert (S2c).
pub async fn has_enabled_transport(pool: &PgPool, transport: &str) -> Result<bool> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM ops.source_policy WHERE enabled AND transport = $1)",
    )
    .bind(transport)
    .fetch_one(pool)
    .await
    .context("source_policy transport ellenőrzése")?;
    Ok(exists)
}

/// Minden policy (engedélyezett és tiltott) — az üzemeltetési nézethez.
pub async fn list_policies(pool: &PgPool) -> Result<Vec<SourcePolicy>> {
    let sql = format!("SELECT {POLICY_COLUMNS} FROM ops.source_policy ORDER BY source_slug");
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .context("policy-k listája")?;
    rows.iter().map(policy_from_row).collect()
}

pub async fn list_enabled_policies(pool: &PgPool) -> Result<Vec<SourcePolicy>> {
    let sql = format!(
        "SELECT {POLICY_COLUMNS} FROM ops.source_policy WHERE enabled ORDER BY source_slug"
    );
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .context("engedélyezett policy-k listája")?;
    rows.iter().map(policy_from_row).collect()
}

pub async fn insert_ledger(pool: &PgPool, entry: &LedgerEntry) -> Result<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ops.fetch_ledger
         (id, source_slug, request_key, attempted_at, outcome,
          http_status, byte_size, latency_ms, artifact_hash, job_id)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
    )
    .bind(id)
    .bind(&entry.source_slug)
    .bind(&entry.request_key)
    .bind(entry.attempted_at)
    .bind(entry.outcome.as_db_str())
    .bind(entry.http_status)
    .bind(entry.byte_size)
    .bind(entry.latency_ms)
    .bind(&entry.artifact_hash)
    .bind(entry.job_id)
    .execute(pool)
    .await
    .context("ledger bejegyzés")?;
    Ok(id)
}

/// A reserve→run→log minta záró lépése: a fetch előtt beszúrt placeholder
/// ledger-sor véglegesítése a tényleges eredménnyel. A sor már a reserve
/// pillanatában kísérletnek számít; ez a frissítés nem változtat a számláláson.
pub async fn update_ledger_outcome(
    pool: &PgPool,
    id: Uuid,
    outcome: FetchOutcome,
    http_status: Option<i16>,
    byte_size: Option<i64>,
    latency_ms: Option<i64>,
    artifact_hash: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE ops.fetch_ledger
         SET outcome = $2, http_status = $3, byte_size = $4, latency_ms = $5, artifact_hash = $6
         WHERE id = $1",
    )
    .bind(id)
    .bind(outcome.as_db_str())
    .bind(http_status)
    .bind(byte_size)
    .bind(latency_ms)
    .bind(artifact_hash)
    .execute(pool)
    .await
    .context("ledger véglegesítés")?;
    Ok(())
}

/// Csak a tényleges fetch-kísérletek számítanak a napi kvótába;
/// a robots_blocked/quota_blocked/breaker_open naplózott, de nem kísérlet.
pub async fn count_fetch_attempts_today(pool: &PgPool, slug: &str, day: NaiveDate) -> Result<i64> {
    sqlx::query_scalar(
        "SELECT count(*) FROM ops.fetch_ledger
         WHERE source_slug = $1 AND (attempted_at AT TIME ZONE 'UTC')::date = $2
           AND outcome IN ('success','http_error','timeout','oversize','transport_error','parse_error','reserved')
           AND request_key NOT LIKE '%:robots:%'",
    )
    .bind(slug)
    .bind(day)
    .fetch_one(pool)
    .await
    .context("napi fetch-számláló")
}

/// Az utolsó VALÓDI fetch-kísérlet időpontja: ugyanazokat az outcome-okat
/// nézzük, mint a napi kvóta számlálója (success/http_error/timeout/oversize/
/// transport_error/parse_error). A robots_blocked/quota_blocked/breaker_open
/// sorok megfigyelhetőségi naplók, nem kísérletek: NEM tolhatják ki a
/// min_delay óráját, különben tick-intervallus < min_delay esetén a job
/// sosem szabadulna a DelayBlocked ágból (livelock).
pub async fn last_attempt_at(pool: &PgPool, slug: &str) -> Result<Option<DateTime<Utc>>> {
    sqlx::query_scalar(
        "SELECT max(attempted_at) FROM ops.fetch_ledger
         WHERE source_slug = $1
           AND outcome IN ('success','http_error','timeout','oversize','transport_error','parse_error','reserved')
           AND request_key NOT LIKE '%:robots:%'",
    )
    .bind(slug)
    .fetch_one(pool)
    .await
    .context("utolsó kísérlet időpontja")
}

pub async fn reserve_fetch(
    pool: &PgPool,
    source_slug: &str,
    request_key: &str,
    job_id: Uuid,
    lease_token: Uuid,
    now: DateTime<Utc>,
) -> Result<FetchReservation> {
    let mut tx = pool.begin().await.context("fetch reservation tranzakció")?;
    let policy_sql =
        format!("SELECT {POLICY_COLUMNS} FROM ops.source_policy WHERE source_slug = $1 FOR UPDATE");
    let policy_row = sqlx::query(&policy_sql)
        .bind(source_slug)
        .fetch_one(&mut *tx)
        .await
        .context("zárolt source policy lekérése")?;
    let policy = policy_from_row(&policy_row)?;
    let leased_job = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM ops.job
         WHERE id = $1 AND lease_token = $2 AND status = 'leased'
           AND leased_until > clock_timestamp()
         FOR UPDATE",
    )
    .bind(job_id)
    .bind(lease_token)
    .fetch_optional(&mut *tx)
    .await
    .context("fetch reservation lease ellenőrzése")?;
    if leased_job.is_none() {
        tx.rollback().await.context("elveszett lease rollback")?;
        return Ok(FetchReservation::LeaseLost);
    }

    let (attempts, last_attempt) = sqlx::query_as::<_, (i64, Option<DateTime<Utc>>)>(
        "SELECT count(*), max(attempted_at) FROM ops.fetch_ledger
         WHERE source_slug = $1 AND (attempted_at AT TIME ZONE 'UTC')::date = $2
           AND outcome IN ('success','http_error','timeout','oversize',
                           'transport_error','parse_error','reserved')
           AND request_key NOT LIKE '%:robots:%'",
    )
    .bind(source_slug)
    .bind(now.date_naive())
    .fetch_one(&mut *tx)
    .await
    .context("zárolt napi kvóta és utolsó kísérlet")?;
    if attempts >= i64::from(policy.daily_budget) {
        tx.rollback().await.context("kvótablokk rollback")?;
        return Ok(FetchReservation::QuotaBlocked);
    }

    if let Some(last) = last_attempt {
        let delay_ok = match Duration::try_milliseconds(policy.min_delay_ms) {
            Some(delay) => last
                .checked_add_signed(delay)
                .is_some_and(|earliest| earliest <= now),
            None => false,
        };
        if !delay_ok {
            tx.rollback().await.context("delay-blokk rollback")?;
            return Ok(FetchReservation::DelayBlocked);
        }
    }

    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ops.fetch_ledger
         (id, source_slug, request_key, attempted_at, outcome, job_id)
         VALUES ($1,$2,$3,$4,'reserved',$5)",
    )
    .bind(id)
    .bind(source_slug)
    .bind(request_key)
    .bind(now)
    .bind(job_id)
    .execute(&mut *tx)
    .await
    .context("fetch reservation ledger beszúrás")?;
    tx.commit().await.context("fetch reservation commit")?;
    Ok(FetchReservation::Reserved(id))
}

fn breaker_from_row(row: &PgRow) -> Result<BreakerRow> {
    let state: String = row.try_get("state").context("breaker state oszlop")?;
    Ok(BreakerRow {
        state: BreakerState::from_db_str(&state).context("breaker state érték")?,
        failure_count: row
            .try_get("failure_count")
            .context("breaker failure_count oszlop")?,
        opened_at: row
            .try_get("opened_at")
            .context("breaker opened_at oszlop")?,
        next_probe_at: row
            .try_get("next_probe_at")
            .context("breaker next_probe_at oszlop")?,
    })
}

pub async fn get_breaker(pool: &PgPool, slug: &str) -> Result<Option<BreakerRow>> {
    let row = sqlx::query(
        "SELECT state, failure_count, opened_at, next_probe_at
         FROM ops.breaker_state WHERE source_slug = $1",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
    .context("breaker lekérés")?;
    row.as_ref().map(breaker_from_row).transpose()
}

pub async fn update_breaker_locked<T, F>(pool: &PgPool, slug: &str, transition: F) -> Result<T>
where
    F: FnOnce(Option<&BreakerRow>) -> (T, BreakerWrite),
{
    let mut tx = pool.begin().await.context("breaker tranzakció indítása")?;
    sqlx::query(
        "INSERT INTO ops.breaker_state (source_slug, state, failure_count)
         VALUES ($1, 'closed', 0) ON CONFLICT (source_slug) DO NOTHING",
    )
    .bind(slug)
    .execute(&mut *tx)
    .await
    .context("breaker alapállapot létrehozása")?;
    let row = sqlx::query(
        "SELECT state, failure_count, opened_at, next_probe_at
         FROM ops.breaker_state WHERE source_slug = $1 FOR UPDATE",
    )
    .bind(slug)
    .fetch_one(&mut *tx)
    .await
    .context("zárolt breaker lekérés")?;
    let current = breaker_from_row(&row)?;
    let (result, write) = transition(Some(&current));
    if let BreakerWrite::Set(next) = write {
        write_breaker(&mut *tx, slug, &next).await?;
    }
    tx.commit().await.context("breaker tranzakció commit")?;
    Ok(result)
}

async fn write_breaker<'e, E>(exec: E, slug: &str, breaker: &BreakerRow) -> Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    sqlx::query(
        "INSERT INTO ops.breaker_state (source_slug, state, failure_count, opened_at, next_probe_at)
         VALUES ($1,$2,$3,$4,$5)
         ON CONFLICT (source_slug) DO UPDATE SET
          state = EXCLUDED.state, failure_count = EXCLUDED.failure_count,
          opened_at = EXCLUDED.opened_at, next_probe_at = EXCLUDED.next_probe_at,
          updated_at = now()",
    )
    .bind(slug)
    .bind(breaker.state.as_db_str())
    .bind(breaker.failure_count)
    .bind(breaker.opened_at)
    .bind(breaker.next_probe_at)
    .execute(exec)
    .await
    .context("breaker upsert")?;
    Ok(())
}

/// Auth-hiba rögzítése egyetlen tranzakcióban: a zárolt breaker-növelés
/// és a `policy_review` audit-esemény együtt commitsolódik.
pub async fn record_auth_failure(
    pool: &PgPool,
    policy: &SourcePolicy,
    now: DateTime<Utc>,
    detail: &serde_json::Value,
) -> Result<()> {
    let slug = &policy.source_slug;
    let mut tx = pool
        .begin()
        .await
        .context("auth-hiba tranzakció indítása")?;
    sqlx::query(
        "INSERT INTO ops.breaker_state (source_slug, state, failure_count)
         VALUES ($1, 'closed', 0) ON CONFLICT (source_slug) DO NOTHING",
    )
    .bind(slug)
    .execute(&mut *tx)
    .await
    .context("breaker alapállapot létrehozása")?;
    let row = sqlx::query(
        "SELECT state, failure_count, opened_at, next_probe_at
         FROM ops.breaker_state WHERE source_slug = $1 FOR UPDATE",
    )
    .bind(slug)
    .fetch_one(&mut *tx)
    .await
    .context("zárolt auth breaker lekérés")?;
    let current = breaker_from_row(&row)?;
    let cooldown = Duration::try_seconds(policy.breaker_cooldown_secs);
    let next = BreakerRow {
        state: BreakerState::Open,
        failure_count: current.failure_count.saturating_add(1),
        opened_at: Some(now),
        next_probe_at: cooldown.and_then(|duration| now.checked_add_signed(duration)),
    };
    write_breaker(&mut *tx, slug, &next).await?;
    sqlx::query(
        "INSERT INTO ops.audit_event (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1, 'worker', 'policy_review', 'source', NULL, $2)",
    )
    .bind(Uuid::now_v7())
    .bind(detail)
    .execute(&mut *tx)
    .await
    .context("policy_review audit-esemény")?;
    tx.commit().await.context("auth-hiba tranzakció commit")?;
    Ok(())
}

pub async fn insert_job(pool: &PgPool, job: &NewJob) -> Result<()> {
    sqlx::query(
        "INSERT INTO ops.job
         (id, source_slug, domain, entity_key, event_type, time_window, due_at, priority, status)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'pending')
         ON CONFLICT (source_slug, domain, entity_key, event_type, time_window) DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .bind(&job.source_slug)
    .bind(&job.domain)
    .bind(&job.entity_key)
    .bind(&job.event_type)
    .bind(&job.time_window)
    .bind(job.due_at)
    .bind(job.priority)
    .execute(pool)
    .await
    .context("job beszúrás")?;
    Ok(())
}

/// Beszúrás ON CONFLICT DO NOTHING-gel, a ténylegesen beszúrt sorok számával
/// (0 = már létező kulcs, 1 = új job). Az ütemező idempotens generálása használja.
pub async fn insert_job_counted(pool: &PgPool, job: &NewJob) -> Result<usize> {
    let res = sqlx::query(
        "INSERT INTO ops.job
         (id, source_slug, domain, entity_key, event_type, time_window, due_at, priority, status)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'pending')
         ON CONFLICT (source_slug, domain, entity_key, event_type, time_window) DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .bind(&job.source_slug)
    .bind(&job.domain)
    .bind(&job.entity_key)
    .bind(&job.event_type)
    .bind(&job.time_window)
    .bind(job.due_at)
    .bind(job.priority)
    .execute(pool)
    .await
    .context("job beszúrás számlálóval")?;
    Ok(res.rows_affected() as usize)
}

/// Meccsek a megadott kickoff-ablakban (inkluzív határok), a verseny
/// prioritásával: (match id, kickoff_at, competition priority), kickoff szerint rendezve.
pub async fn kickoff_window(
    pool: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<(Uuid, DateTime<Utc>, i32)>> {
    sqlx::query_as(
        "SELECT m.id, m.kickoff_at, c.priority
         FROM core.match m JOIN core.competition c ON c.id = m.competition_id
         WHERE m.kickoff_at BETWEEN $1 AND $2
         ORDER BY m.kickoff_at",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await
    .context("kickoff-ablak lekérés")
}

/// Az adott napon megfigyelt, kanonikus meccshez már linkelt Tipplap event ID-k
/// (a `source_record_key` `tipplap-` prefixe nélkül). A markets jobokat ezekből
/// generáljuk — feloldatlan, review-ra váró sorok nem jelennek meg itt.
pub async fn linked_tipplap_event_ids(
    pool: &PgPool,
    source_slug: &str,
    since: chrono::NaiveDate,
) -> Result<Vec<String>> {
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT o.source_record_key
           FROM stage.observation o
           JOIN ops.source s ON s.id = o.source_id AND s.code = $1
           JOIN identity.source_entity_ref r
             ON r.source_id = o.source_id AND r.entity_kind = 'match'
            AND r.source_entity_id = o.source_record_key AND r.status = 'active'
           JOIN core.match m ON m.id = r.entity_id
          WHERE o.entity_kind = 'match' AND o.aspect = 'match_core'
            AND o.source_record_key LIKE 'tipplap-%'
            AND o.observed_at >= $2
          ORDER BY o.source_record_key",
    )
    .bind(source_slug)
    .bind(since)
    .fetch_all(pool)
    .await
    .context("linkelt tipplap event ID-k lekérése")?;
    Ok(rows
        .into_iter()
        .filter_map(|key| key.strip_prefix("tipplap-").map(str::to_owned))
        .collect())
}

/// Aktív meccs-szintű forrás-hivatkozás kanonikus `core.match.id`-je.
///
/// Az `identity::active_ref` a `core.entity` táblával joinol (csapatoknak és
/// versenyeknek), de a meccsek nem entitások — ezért külön lekérés kell.
pub async fn active_match_ref(
    pool: &PgPool,
    source_id: Uuid,
    source_entity_id: &str,
) -> Result<Option<Uuid>> {
    sqlx::query_scalar(
        "SELECT r.entity_id FROM identity.source_entity_ref r
          JOIN core.match m ON m.id = r.entity_id
          WHERE r.source_id = $1 AND r.entity_kind = 'match'
            AND r.source_entity_id = $2 AND r.status = 'active'",
    )
    .bind(source_id)
    .bind(source_entity_id)
    .fetch_optional(pool)
    .await
    .context("aktív meccs-ref lekérése")
}

/// Esedékes pending jobok claimelése lease-zel (FOR UPDATE SKIP LOCKED).
/// Sorrendi szerződés: a visszaadott jobok (priority, due_at) szerint
/// növekvően rendezettek. A rendezést Rustban garantáljuk, mert az
/// UPDATE ... RETURNING sorrendjét a PostgreSQL nem specifikálja: az
/// allekérés ORDER BY-ja a kiválasztást és a LIMIT-et vezérli, de a
/// visszaadott sorok sorrendje tetszőleges lehet.
pub async fn claim_due_jobs(
    pool: &PgPool,
    worker_id: Uuid,
    now: DateTime<Utc>,
    lease_secs: i64,
    limit: i64,
) -> Result<Vec<LeasedJob>> {
    let rows = sqlx::query(
        "UPDATE ops.job SET status='leased', worker_id=$2,
             leased_until=$1 + make_interval(secs => $3),
             lease_token=gen_random_uuid(), attempts = attempts + 1
         WHERE id IN (
             SELECT id FROM ops.job
             WHERE status='pending' AND due_at <= $1
             ORDER BY priority, due_at
             FOR UPDATE SKIP LOCKED LIMIT $4)
         RETURNING id, source_slug, domain, entity_key, event_type, time_window,
                   due_at, priority, lease_token, leased_until",
    )
    .bind(now)
    .bind(worker_id)
    .bind(lease_secs)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("job claim")?;
    let mut jobs: Vec<LeasedJob> = rows
        .iter()
        .map(|r| {
            let job = FetchJob {
                id: r.try_get("id").context("job id oszlop")?,
                source_slug: r.try_get("source_slug").context("job source_slug oszlop")?,
                domain: r.try_get("domain").context("job domain oszlop")?,
                entity_key: r.try_get("entity_key").context("job entity_key oszlop")?,
                event_type: r.try_get("event_type").context("job event_type oszlop")?,
                time_window: r.try_get("time_window").context("job time_window oszlop")?,
                due_at: r.try_get("due_at").context("job due_at oszlop")?,
                priority: r.try_get("priority").context("job priority oszlop")?,
            };
            Ok(LeasedJob {
                lease: JobLease {
                    job_id: job.id,
                    token: r.try_get("lease_token").context("job lease_token oszlop")?,
                    leased_until: r
                        .try_get("leased_until")
                        .context("job leased_until oszlop")?,
                },
                job,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    jobs.sort_by_key(|item| (item.job.priority, item.job.due_at));
    Ok(jobs)
}

pub async fn heartbeat_job(pool: &PgPool, lease: &JobLease, lease_secs: i64) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE ops.job
         SET leased_until = clock_timestamp() + make_interval(secs => $3)
         WHERE id = $1 AND lease_token = $2 AND status = 'leased'
           AND leased_until > clock_timestamp()",
    )
    .bind(lease.job_id)
    .bind(lease.token)
    .bind(lease_secs)
    .execute(pool)
    .await
    .context("job lease heartbeat")?;
    Ok(result.rows_affected() == 1)
}

pub async fn requeue_expired_leases(pool: &PgPool, now: DateTime<Utc>) -> Result<u64> {
    let res = sqlx::query(
        "UPDATE ops.job SET status='pending', worker_id=NULL, leased_until=NULL,
             lease_token=NULL
         WHERE status='leased' AND leased_until < $1",
    )
    .bind(now)
    .execute(pool)
    .await
    .context("lease visszasorolás")?;
    Ok(res.rows_affected())
}

pub async fn requeue_leased(
    pool: &PgPool,
    lease: &JobLease,
    due_at: DateTime<Utc>,
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE ops.job
         SET status='pending', due_at=$3, worker_id=NULL, leased_until=NULL,
             lease_token=NULL
         WHERE id=$1 AND lease_token=$2 AND status='leased'
           AND leased_until > clock_timestamp()",
    )
    .bind(lease.job_id)
    .bind(lease.token)
    .bind(due_at)
    .execute(pool)
    .await
    .context("fenced job visszasorolás")?;
    Ok(result.rows_affected() == 1)
}

pub async fn complete_leased(
    pool: &PgPool,
    lease: &JobLease,
    status: JobStatus,
    last_error: Option<&str>,
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE ops.job
         SET status=$3, last_error=$4, worker_id=NULL, leased_until=NULL,
             lease_token=NULL
         WHERE id=$1 AND lease_token=$2 AND status='leased'
           AND leased_until > clock_timestamp()",
    )
    .bind(lease.job_id)
    .bind(lease.token)
    .bind(status.as_db_str())
    .bind(last_error)
    .execute(pool)
    .await
    .context("fenced job befejezés")?;
    Ok(result.rows_affected() == 1)
}

pub async fn upsert_job_coverage(
    pool: &PgPool,
    lease: &JobLease,
    entity_type: &str,
    data_kind: &str,
    time_window: &str,
    state: CoverageState,
) -> Result<bool> {
    let result = sqlx::query(
        "WITH leased AS (
             SELECT id, source_slug FROM ops.job
             WHERE id = $5 AND lease_token = $6 AND status = 'leased'
               AND leased_until > clock_timestamp()
             FOR UPDATE
         )
         INSERT INTO ops.coverage_slot
             (source_slug, entity_type, data_kind, time_window, state,
              last_job_id, updated_at)
         SELECT source_slug, $1, $2, $3, $4, id, clock_timestamp()
         FROM leased
         ON CONFLICT (source_slug, entity_type, data_kind, time_window)
         DO UPDATE SET state = EXCLUDED.state,
                       last_job_id = EXCLUDED.last_job_id,
                       updated_at = EXCLUDED.updated_at",
    )
    .bind(entity_type)
    .bind(data_kind)
    .bind(time_window)
    .bind(state.as_db_str())
    .bind(lease.job_id)
    .bind(lease.token)
    .execute(pool)
    .await
    .context("fenced coverage upsert")?;
    Ok(result.rows_affected() == 1)
}

pub async fn release_worker_leases(pool: &PgPool, worker_id: Uuid) -> Result<u64> {
    let res = sqlx::query(
        "UPDATE ops.job SET status='pending', worker_id=NULL, leased_until=NULL,
             lease_token=NULL
         WHERE status='leased' AND worker_id=$1",
    )
    .bind(worker_id)
    .execute(pool)
    .await
    .context("worker lease-ek elengedése")?;
    Ok(res.rows_affected())
}

pub async fn get_robots_cache(pool: &PgPool, host: &str) -> Result<Option<RobotsCacheRow>> {
    let row = sqlx::query(
        "SELECT host, fetched_at, verdict, rules_text, expires_at
         FROM ops.robots_cache WHERE host = $1",
    )
    .bind(host)
    .fetch_optional(pool)
    .await
    .context("robots cache lekérés")?;
    let Some(row) = row else { return Ok(None) };
    Ok(Some(RobotsCacheRow {
        host: row.try_get("host").context("robots host oszlop")?,
        fetched_at: row
            .try_get("fetched_at")
            .context("robots fetched_at oszlop")?,
        verdict: row.try_get("verdict").context("robots verdict oszlop")?,
        rules_text: row
            .try_get("rules_text")
            .context("robots rules_text oszlop")?,
        expires_at: row
            .try_get("expires_at")
            .context("robots expires_at oszlop")?,
    }))
}

pub async fn put_robots_cache(pool: &PgPool, entry: &RobotsCacheRow) -> Result<()> {
    sqlx::query(
        "INSERT INTO ops.robots_cache (host, fetched_at, verdict, rules_text, expires_at)
         VALUES ($1,$2,$3,$4,$5)
         ON CONFLICT (host) DO UPDATE SET
          fetched_at = EXCLUDED.fetched_at, verdict = EXCLUDED.verdict,
          rules_text = EXCLUDED.rules_text, expires_at = EXCLUDED.expires_at",
    )
    .bind(&entry.host)
    .bind(entry.fetched_at)
    .bind(&entry.verdict)
    .bind(&entry.rules_text)
    .bind(entry.expires_at)
    .execute(pool)
    .await
    .context("robots cache upsert")?;
    Ok(())
}

/// A tick nyers megőrzésének capture-mezői (a request a fetch tényleges adatai).
#[derive(Debug, Clone)]
pub struct RawCapture<'a> {
    pub endpoint_key: &'a str,
    pub request_url: &'a str,
    pub request_key: &'a str,
    pub requested_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub http_status: i16,
    pub adapter_version: &'a str,
    pub parser_version: &'a str,
}

/// L0 nyers megőrzés egy tranzakcióban: ops.source upsert + raw.artifact
/// upsert (content_sha256 unique) + minden hívásra új raw.capture. A stabil
/// request_key nem capture-identitás. Visszatérés: (source_id, capture_id), ahol
/// a capture_id a ténylegesen beszúrt sor azonosítója.
///
/// A `source_code` a hiteles forrásazonosító: a hívó a job source_slugját
/// adja át, NEM a payloadból olvasott kódot (megszemélyesítés-védelem).
/// A `source_display_name` csak megjelenítési név; forrásazonosságot nem
/// dönthet el.
pub async fn persist_raw(
    pool: &PgPool,
    source_code: &str,
    source_display_name: &str,
    artifact: &StoredArtifact,
    capture: &RawCapture<'_>,
    media_type: &str,
) -> Result<(Uuid, Uuid)> {
    let mut tx = pool
        .begin()
        .await
        .context("nyers megőrzés tranzakció indítása")?;

    let source_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ops.source (id, code, display_name)
         VALUES ($1, $2, $3)
         ON CONFLICT (code) DO UPDATE SET display_name = EXCLUDED.display_name
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(source_code)
    .bind(source_display_name)
    .fetch_one(&mut *tx)
    .await
    .context("forrás upsert")?;

    let artifact_id: Uuid = sqlx::query_scalar(
        "INSERT INTO raw.artifact
         (id, content_sha256, compression, byte_size, storage_key, media_type)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (content_sha256) DO UPDATE SET content_sha256 = EXCLUDED.content_sha256
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(&artifact.sha256)
    .bind(artifact.compression)
    .bind(artifact.byte_size as i64)
    .bind(&artifact.storage_key)
    .bind(media_type)
    .fetch_one(&mut *tx)
    .await
    .context("artifact upsert")?;

    let capture_id: Uuid = sqlx::query_scalar(
        "INSERT INTO raw.capture
         (id, source_id, artifact_id, endpoint_key, request_url, requested_at,
          received_at, http_status, adapter_version, parser_version, request_key)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(artifact_id)
    .bind(capture.endpoint_key)
    .bind(capture.request_url)
    .bind(capture.requested_at)
    .bind(capture.received_at)
    .bind(capture.http_status)
    .bind(capture.adapter_version)
    .bind(capture.parser_version)
    .bind(capture.request_key)
    .fetch_one(&mut *tx)
    .await
    .context("capture beszúrás")?;

    tx.commit().await.context("nyers megőrzés commit")?;
    Ok((source_id, capture_id))
}

/// Duplikátum-számláló a verification DUPLICATE_NATURAL_KEY szabályához:
/// azonos verseny + szezon + forduló + csapatok + kickoff, másik meccs-id-n.
/// (A footballctl IngestFixture segédjével azonos SQL.)
pub async fn count_duplicate_matches(
    pool: &PgPool,
    match_id: Uuid,
    home_team_id: Uuid,
    away_team_id: Uuid,
    kickoff_at: DateTime<Utc>,
) -> Result<i64> {
    sqlx::query_scalar(
        "SELECT count(*) FROM core.match
         WHERE competition_id = (SELECT competition_id FROM core.match WHERE id=$1)
           AND season_id = (SELECT season_id FROM core.match WHERE id=$1)
           AND home_team_id = $2 AND away_team_id = $3
           AND kickoff_at = $4
           AND id <> $1",
    )
    .bind(match_id)
    .bind(home_team_id)
    .bind(away_team_id)
    .bind(kickoff_at)
    .fetch_one(pool)
    .await
    .context("duplikátum-számláló")
}

/// Asset upsert (ops.asset) — az egyediség (entity_kind, entity_id, kind, url).
pub struct AssetRowInput<'a> {
    pub entity_kind: &'a str,
    pub entity_id: Option<Uuid>,
    pub kind: &'a str,
    pub url: &'a str,
    pub storage_key: Option<&'a str>,
    pub source: &'a str,
    pub licence: Option<&'a str>,
    pub licence_status: &'a str,
    pub attribution: Option<&'a str>,
    pub captured_at: DateTime<Utc>,
}

pub async fn upsert_asset(pool: &PgPool, row: &AssetRowInput<'_>) -> Result<Uuid> {
    sqlx::query_scalar(
        "INSERT INTO ops.asset
         (id, entity_kind, entity_id, kind, url, storage_key, source, licence,
          licence_status, attribution, captured_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
         ON CONFLICT (entity_kind, entity_id, kind, url) DO UPDATE SET
           storage_key = COALESCE(EXCLUDED.storage_key, ops.asset.storage_key),
           source = EXCLUDED.source,
           licence = EXCLUDED.licence,
           licence_status = EXCLUDED.licence_status,
           attribution = EXCLUDED.attribution,
           captured_at = EXCLUDED.captured_at
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(row.entity_kind)
    .bind(row.entity_id)
    .bind(row.kind)
    .bind(row.url)
    .bind(row.storage_key)
    .bind(row.source)
    .bind(row.licence)
    .bind(row.licence_status)
    .bind(row.attribution)
    .bind(row.captured_at)
    .fetch_one(pool)
    .await
    .context("asset upsert")
}
