use crate::object_store::StoredArtifact;
use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use football_domain::policy::{
    BreakerState, CoverageState, FetchJob, FetchOutcome, JobStatus, SourcePolicy,
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

#[derive(Debug, Clone)]
pub struct BreakerRow {
    pub state: BreakerState,
    pub failure_count: i32,
    pub opened_at: Option<DateTime<Utc>>,
    pub next_probe_at: Option<DateTime<Utc>>,
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
           AND outcome IN ('success','http_error','timeout','oversize','transport_error','parse_error')",
    )
    .bind(job_id)
    .fetch_one(pool)
    .await
    .context("job fetch-kísérletek száma")?;
    Ok(count)
}

/// A job visszasorolása pending állapotba késleltetett due_at-tel.
pub async fn requeue_with_delay(pool: &PgPool, job_id: Uuid, due_at: DateTime<Utc>) -> Result<()> {
    sqlx::query(
        "UPDATE ops.job SET status='pending', due_at=$2, worker_id=NULL, leased_until=NULL
         WHERE id=$1",
    )
    .bind(job_id)
    .bind(due_at)
    .execute(pool)
    .await
    .context("job késleltetett visszasorolása")?;
    Ok(())
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
           AND outcome IN ('success','http_error','timeout','oversize','transport_error','parse_error')
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
           AND outcome IN ('success','http_error','timeout','oversize','transport_error','parse_error')
           AND request_key NOT LIKE '%:robots:%'",
    )
    .bind(slug)
    .fetch_one(pool)
    .await
    .context("utolsó kísérlet időpontja")
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
    let Some(row) = row else { return Ok(None) };
    let state: String = row.try_get("state").context("breaker state oszlop")?;
    Ok(Some(BreakerRow {
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
    }))
}

pub async fn set_breaker(pool: &PgPool, slug: &str, breaker: &BreakerRow) -> Result<()> {
    write_breaker(pool, slug, breaker).await
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

/// Auth-hiba rögzítése egyetlen tranzakcióban: a breaker azonnali open-ra
/// nyitása és a `policy_review` audit-esemény együtt commitsolódik, így a
/// kézi áttekintésre jelzett forrás sosem marad audit nélkül (Task 8 deferred).
pub async fn record_auth_failure(
    pool: &PgPool,
    slug: &str,
    breaker: &BreakerRow,
    detail: &serde_json::Value,
) -> Result<()> {
    let mut tx = pool
        .begin()
        .await
        .context("auth-hiba tranzakció indítása")?;
    write_breaker(&mut *tx, slug, breaker).await?;
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
) -> Result<Vec<FetchJob>> {
    let rows = sqlx::query(
        "UPDATE ops.job SET status='leased', worker_id=$2,
             leased_until=$1 + make_interval(secs => $3), attempts = attempts + 1
         WHERE id IN (
             SELECT id FROM ops.job
             WHERE status='pending' AND due_at <= $1
             ORDER BY priority, due_at
             FOR UPDATE SKIP LOCKED LIMIT $4)
         RETURNING id, source_slug, domain, entity_key, event_type, time_window, due_at, priority",
    )
    .bind(now)
    .bind(worker_id)
    .bind(lease_secs)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("job claim")?;
    let mut jobs: Vec<FetchJob> = rows
        .iter()
        .map(|r| {
            Ok(FetchJob {
                id: r.try_get("id").context("job id oszlop")?,
                source_slug: r.try_get("source_slug").context("job source_slug oszlop")?,
                domain: r.try_get("domain").context("job domain oszlop")?,
                entity_key: r.try_get("entity_key").context("job entity_key oszlop")?,
                event_type: r.try_get("event_type").context("job event_type oszlop")?,
                time_window: r.try_get("time_window").context("job time_window oszlop")?,
                due_at: r.try_get("due_at").context("job due_at oszlop")?,
                priority: r.try_get("priority").context("job priority oszlop")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    jobs.sort_by_key(|job| (job.priority, job.due_at));
    Ok(jobs)
}

pub async fn requeue_expired_leases(pool: &PgPool, now: DateTime<Utc>) -> Result<u64> {
    let res = sqlx::query(
        "UPDATE ops.job SET status='pending', worker_id=NULL, leased_until=NULL
         WHERE status='leased' AND leased_until < $1",
    )
    .bind(now)
    .execute(pool)
    .await
    .context("lease visszasorolás")?;
    Ok(res.rows_affected())
}

pub async fn complete_job(
    pool: &PgPool,
    job_id: Uuid,
    status: JobStatus,
    last_error: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE ops.job SET status=$2, last_error=$3, worker_id=NULL, leased_until=NULL
         WHERE id=$1",
    )
    .bind(job_id)
    .bind(status.as_db_str())
    .bind(last_error)
    .execute(pool)
    .await
    .context("job befejezés")?;
    Ok(())
}

pub async fn release_worker_leases(pool: &PgPool, worker_id: Uuid) -> Result<u64> {
    let res = sqlx::query(
        "UPDATE ops.job SET status='pending', worker_id=NULL, leased_until=NULL
         WHERE status='leased' AND worker_id=$1",
    )
    .bind(worker_id)
    .execute(pool)
    .await
    .context("worker lease-ek elengedése")?;
    Ok(res.rows_affected())
}

pub async fn upsert_coverage(
    pool: &PgPool,
    slug: &str,
    entity_type: &str,
    data_kind: &str,
    time_window: &str,
    state: CoverageState,
    job_id: Option<Uuid>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO ops.coverage_slot
         (source_slug, entity_type, data_kind, time_window, state, last_job_id)
         VALUES ($1,$2,$3,$4,$5,$6)
         ON CONFLICT (source_slug, entity_type, data_kind, time_window) DO UPDATE SET
          state = EXCLUDED.state, last_job_id = EXCLUDED.last_job_id, updated_at = now()",
    )
    .bind(slug)
    .bind(entity_type)
    .bind(data_kind)
    .bind(time_window)
    .bind(state.as_db_str())
    .bind(job_id)
    .execute(pool)
    .await
    .context("coverage upsert")?;
    Ok(())
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
/// upsert (content_sha256 unique) + raw.capture insert (source_id, request_key
/// unique). Azonos tartalom és azonos request_key esetén idempotens: a meglévő
/// artifact/capture azonosítóját adja vissza. Visszatérés: (source_id, capture_id).
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
         VALUES ($1, $2, $3, $4, $5, 'application/json')
         ON CONFLICT (content_sha256) DO UPDATE SET content_sha256 = EXCLUDED.content_sha256
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(&artifact.sha256)
    .bind(artifact.compression)
    .bind(artifact.byte_size as i64)
    .bind(&artifact.storage_key)
    .fetch_one(&mut *tx)
    .await
    .context("artifact upsert")?;

    sqlx::query(
        "INSERT INTO raw.capture
         (id, source_id, artifact_id, endpoint_key, request_url, requested_at,
          received_at, http_status, adapter_version, parser_version, request_key)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
         ON CONFLICT (source_id, request_key) DO NOTHING",
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
    .execute(&mut *tx)
    .await
    .context("capture beszúrás")?;

    let capture_id: Uuid =
        sqlx::query_scalar("SELECT id FROM raw.capture WHERE source_id = $1 AND request_key = $2")
            .bind(source_id)
            .bind(capture.request_key)
            .fetch_one(&mut *tx)
            .await
            .context("capture azonosító")?;

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
           AND round_label IS NOT DISTINCT FROM (SELECT round_label FROM core.match WHERE id=$1)
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
