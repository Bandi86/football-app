# S2a — Ingest keretrendszer megvalósítási terv

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Adapter-szerződés + Fetcher réteg + worker (kvóta/breaker/ledger/robots/coverage/ütemező) megépítése a footballd-be ágyazva, élő hálózati adapter nélkül, szintetikus contract-tesztekkel bizonyítva.

**Architecture:** Új `football-worker` crate vezényli a due_at jobokat (lease, `FOR UPDATE SKIP LOCKED`); a policy-őrök (robots/kvóta/breaker/delay) a workerben, a transport a `Fetcher` trait mögött (reqwest / fixture), a tartalmi őrök a `SourceAdapter` parserében élnek. A nyers payload az objektumtárba kerül, a meglévő observation → canonicalize → verification → gate lánc változatlan.

**Tech Stack:** Rust (edition 2024, rust-version 1.85), sqlx 0.8 (PostgreSQL), tokio, reqwest 0.13.5 (rustls), robotstxt 0.3.0, wiremock 0.6.5 (dev), async-trait 0.1 (már a workspace-ben), thiserror 2, chrono, uuid v7.

**Spec:** `docs/superpowers/specs/2026-11-12-s2a-ingest-framework-design.md`

## Global Constraints

- AGENTS.md §2: **minden task végén STOP** — összefoglaló + bizonyíték, következő task csak felhasználói engedéllyel.
- AGENTS.md §9 + S1 ruling 2: **commit NEM része a task-lépéseknek** — commit csak kifejezett felhasználói kérésre; a review-csomag a munkafa-diff.
- Munkaág: `feat/s2a-ingest-framework` (a `feat/source-research`-ről indul; létrehozása a végrehajtás első lépése, felhasználói jóváhagyással). Nincs git worktree (S1 ruling 1).
- Dokumentumok, hibaüzenetek, CLI-szövegek **magyarul**; kódazonosítók angolul. Magyar szöveg Write/Edit után visszaolvasandó (ékezetek, idegen glyph).
- Kérésfeldolgozásban **nincs `unwrap()`/`expect()`/elnémítás**; tesztkódban megengedett.
- **Élő hálózati kérés 0** az S2a-ban; RustHttpFetcher csak wiremock lokális szerveren tesztelt.
- **Nyers payload a repóban 0**; a golden fixture-ök szintetikusak.
- Tesztek: `TEST_DATABASE_URL` kötelező; integrációs tesztek **egy szálon** (`--test-threads=1`); minden migráció előtt tesztDB, zöld után devDB.
- Új függőségek pontos verzióval: `reqwest = "0.13.5"` (default-features off, `rustls-tls`), `robotstxt = "0.3.0"`, `wiremock = "0.6.5"` (dev). Az `async-trait` már workspace-függőség.
- Hiányzó adat NULL/státusz, soha nem nulla; minden időpont UTC.
- Alapellenőrzés a szelet végén: `./scripts/check.sh` + `./scripts/db-reset.sh && ./scripts/db-check.sh`.

## Fájlstruktúra

| Fájl | Felelősség |
|---|---|
| `rust/crates/domain/src/policy.rs` (új) | tiszta policy/job/állapot típusok + db-string konverziók |
| `rust/crates/domain/src/error.rs` (új) | `FetchError`, `ParseError` típusos hibák |
| `db/migrations/0004_worker_framework.sql` (új) | 6 ops tábla, tranzakciós |
| `rust/crates/storage/src/repositories/worker.rs` (új) | ops táblák SQL repository-i |
| `rust/crates/storage/src/test_support.rs` (módosul) | TRUNCATE-lista bővítés |
| `rust/crates/ingest/src/fetch.rs` (új) | `Fetcher` trait, `RawPayload`, `FixtureFetcher` |
| `rust/crates/ingest/src/http.rs` (új) | `RustHttpFetcher` (reqwest) |
| `rust/crates/ingest/src/adapter.rs` (új) | `SourceAdapter` trait, `AdapterMeta`, `ParseCtx`, `Observation` |
| `rust/crates/ingest/src/demo.rs` (új) | `DemoAdapter` szintetikus referencia-adapter + őrök |
| `rust/crates/ingest/src/observe.rs` (új) | `insert_observation` kiemelve a fixture.rs-ből (megosztott) |
| `rust/crates/ingest/src/fixture.rs` (módosul) | a kiemelt `insert_observation`-t használja |
| `rust/crates/worker/` (új crate) | `robots.rs`, `quota.rs`, `breaker.rs`, `scheduler.rs`, `coverage.rs`, `tick.rs`, `runner.rs` |
| `rust/bins/footballd/src/main.rs` (módosul) | worker loop tokio-task, graceful leállítás |
| `rust/bins/footballctl/src/main.rs` (módosul) | `worker status` / `worker tick`, doctor-bővítés |
| `tests/fixtures/worker/` (új) | szintetikus golden fixture-ök |
| `docs/operations/worker.md` (új) | üzemeltetési útmutató + seed SQL |

---

### Task 1: Domain típusok (policy, job, állapotok, hibák)

**Files:**
- Create: `rust/crates/domain/src/policy.rs`, `rust/crates/domain/src/error.rs`
- Modify: `rust/crates/domain/src/lib.rs`, `rust/crates/domain/Cargo.toml` (thiserror)
- Test: `rust/crates/domain/tests/policy_types.rs`

**Interfaces:**
- Produces: `SourcePolicy`, `EventTemplate`, `FetchJob`, `Transport`, `SourceMode`, `BreakerState`, `CoverageState`, `FetchOutcome`, `JobStatus` (mind `as_db_str()`/`from_db_str()`), `FetchError`, `ParseError`. Minden későbbi task ezekre épül.

- [ ] **Step 1: Bukó teszt írása**

`rust/crates/domain/tests/policy_types.rs`:

```rust
use football_domain::policy::{
    BreakerState, CoverageState, EventTemplate, FetchOutcome, JobStatus, SourceMode,
    SourcePolicy, Transport,
};

fn demo_policy() -> SourcePolicy {
    SourcePolicy {
        source_slug: "demo-synthetic".into(),
        transport: Transport::Fixture,
        mode: SourceMode::Shadow,
        daily_budget: 10,
        min_delay_ms: 2000,
        jitter_ms: 500,
        breaker_threshold: 3,
        breaker_cooldown_secs: 300,
        adapter_version: "1.0".into(),
        parser_version: "1.0".into(),
        domains: vec!["match".into()],
        events: vec![EventTemplate {
            event_type: "discovery".into(),
            offset_minutes: None,
            interval_minutes: Some(1440),
        }],
        enabled: true,
        max_bytes: 5_242_880,
    }
}

#[test]
fn policy_serde_roundtrip() {
    let policy = demo_policy();
    let json = serde_json::to_string(&policy).unwrap();
    let back: SourcePolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(back.source_slug, "demo-synthetic");
    assert_eq!(back.events[0].interval_minutes, Some(1440));
}

#[test]
fn db_string_roundtrip() {
    assert_eq!(Transport::RustHttp.as_db_str(), "rust_http");
    assert_eq!(SourceMode::Shadow.as_db_str(), "shadow");
    assert_eq!(BreakerState::HalfOpen.as_db_str(), "half_open");
    assert_eq!(CoverageState::NoData.as_db_str(), "no_data");
    assert_eq!(FetchOutcome::RobotsBlocked.as_db_str(), "robots_blocked");
    assert_eq!(JobStatus::Leased.as_db_str(), "leased");
    assert_eq!(CoverageState::from_db_str("quarantined").unwrap(), CoverageState::Quarantined);
    assert!(CoverageState::from_db_str("ismeretlen").is_err());
}
```

- [ ] **Step 2: Futtatás — bukás ellenőrzése**

Run: `cd rust && cargo test -p football-domain --test policy_types`
Expected: FAIL — `policy` modul nem létezik (fordítási hiba).

- [ ] **Step 3: Implementáció**

`rust/crates/domain/src/policy.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport { RustHttp, Fixture }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceMode { Shadow, Canary, Live }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState { Closed, Open, HalfOpen }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageState {
    Pending, Fetching, Done, NoData, Blocked, Failed, Stale, Quarantined, Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchOutcome {
    Success, HttpError, Timeout, Oversize, RobotsBlocked, QuotaBlocked,
    BreakerOpen, TransportError, ParseError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus { Pending, Leased, Done, Failed, Cancelled }

macro_rules! db_str {
    ($t:ty, $($variant:ident => $s:literal),+ $(,)?) => {
        impl $t {
            pub fn as_db_str(self) -> &'static str {
                match self { $(Self::$variant => $s),+ }
            }
            pub fn from_db_str(s: &str) -> Result<Self, String> {
                match s { $($s => Ok(Self::$variant),)+ other => Err(format!("ismeretlen db-érték: {other}")) }
            }
        }
    };
}

db_str!(Transport, RustHttp => "rust_http", Fixture => "fixture");
db_str!(SourceMode, Shadow => "shadow", Canary => "canary", Live => "live");
db_str!(BreakerState, Closed => "closed", Open => "open", HalfOpen => "half_open");
db_str!(CoverageState,
    Pending => "pending", Fetching => "fetching", Done => "done", NoData => "no_data",
    Blocked => "blocked", Failed => "failed", Stale => "stale",
    Quarantined => "quarantined", Skipped => "skipped");
db_str!(FetchOutcome,
    Success => "success", HttpError => "http_error", Timeout => "timeout",
    Oversize => "oversize", RobotsBlocked => "robots_blocked", QuotaBlocked => "quota_blocked",
    BreakerOpen => "breaker_open", TransportError => "transport_error", ParseError => "parse_error");
db_str!(JobStatus,
    Pending => "pending", Leased => "leased", Done => "done",
    Failed => "failed", Cancelled => "cancelled");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTemplate {
    pub event_type: String,
    pub offset_minutes: Option<i64>,
    pub interval_minutes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePolicy {
    pub source_slug: String,
    pub transport: Transport,
    pub mode: SourceMode,
    pub daily_budget: i32,
    pub min_delay_ms: i64,
    pub jitter_ms: i64,
    pub breaker_threshold: i32,
    pub breaker_cooldown_secs: i64,
    pub adapter_version: String,
    pub parser_version: String,
    pub domains: Vec<String>,
    pub events: Vec<EventTemplate>,
    pub enabled: bool,
    pub max_bytes: i64,
}

#[derive(Debug, Clone)]
pub struct FetchJob {
    pub id: Uuid,
    pub source_slug: String,
    pub domain: String,
    pub entity_key: String,
    pub event_type: String,
    pub time_window: String,
    pub due_at: DateTime<Utc>,
    pub priority: i32,
}
```

`rust/crates/domain/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("transport hiba: {0}")]
    Transport(String),
    #[error("időtúllépés a forrás felé")]
    Timeout,
    #[error("méretplafon túllépve: {0} bájt")]
    Oversize(u64),
    #[error("HTTP-státusz: {0}")]
    HttpStatus(u16),
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("decoy payload: {0}")]
    DecoyDetected(String),
    #[error("dátum-guard eltérés: kért {requested}, kapott {got}")]
    DateGuardMismatch { requested: String, got: String },
    #[error("séma-drift: {0}")]
    SchemaDrift(String),
    #[error("parse hiba: {0}")]
    Parse(String),
}
```

`lib.rs` bővítés: `pub mod error; pub mod policy;` + a Cargo.toml-ba `thiserror.workspace = true` és `serde_json` (dev-dep a roundtrip teszthez: `serde_json.workspace = true`).

- [ ] **Step 4: Futtatás — zöld**

Run: `cd rust && cargo test -p football-domain --test policy_types && cargo fmt --all --check && cargo clippy -p football-domain --all-targets -- -D warnings`
Expected: 2 teszt PASS, fmt/clippy tiszta.

- [ ] **Step 5: STOP — jelentés**

Checkpoint-jegyzet: futtatott parancsok + eredmény. Következő task csak engedéllyel. Commit NEM készül.

---

### Task 2: 0004 migráció + teszt-támogatás

**Files:**
- Create: `db/migrations/0004_worker_framework.sql`
- Modify: `rust/crates/storage/src/test_support.rs` (TRUNCATE-lista)
- Test: `rust/crates/storage/tests/worker_schema.rs`

**Interfaces:**
- Consumes: Task 1 db-string értékei (a CHECK-készletek ezekkel egyeznek).
- Produces: `ops.source_policy`, `ops.fetch_ledger`, `ops.breaker_state`, `ops.job`, `ops.coverage_slot`, `ops.robots_cache` táblák; a `test_pool()` ezeket is truncatálja.

- [ ] **Step 1: Bukó séma-teszt**

`rust/crates/storage/tests/worker_schema.rs`:

```rust
use football_storage::test_support::test_pool;

#[tokio::test]
async fn worker_tables_exist_and_constraints_hold() {
    let pool = test_pool().await;

    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_tables WHERE schemaname='ops'
         AND tablename IN ('source_policy','fetch_ledger','breaker_state','job','coverage_slot','robots_cache')",
    )
    .fetch_one(&pool).await.unwrap();
    assert_eq!(tables, 6);

    // CHECK: ismeretlen transport nem írható
    let bad = sqlx::query(
        "INSERT INTO ops.source_policy (source_slug, transport, mode, daily_budget,
         min_delay_ms, jitter_ms, breaker_threshold, breaker_cooldown_secs,
         adapter_version, parser_version)
         VALUES ('x','wormhole','shadow',10,0,0,3,60,'1.0','1.0')",
    )
    .execute(&pool).await;
    assert!(bad.is_err());

    // job egyedi kulcs: duplikált generálás nem szúr be második sort
    sqlx::query(
        "INSERT INTO ops.source_policy (source_slug, transport, mode, daily_budget,
         min_delay_ms, jitter_ms, breaker_threshold, breaker_cooldown_secs,
         adapter_version, parser_version)
         VALUES ('demo-synthetic','fixture','shadow',10,0,0,3,60,'1.0','1.0')",
    )
    .execute(&pool).await.unwrap();
    for _ in 0..2 {
        sqlx::query(
            "INSERT INTO ops.job (id, source_slug, domain, entity_key, event_type, time_window, due_at)
             VALUES (gen_random_uuid(),'demo-synthetic','match','day','discovery','2026-11-12', now())
             ON CONFLICT (source_slug, domain, entity_key, event_type, time_window) DO NOTHING",
        )
        .execute(&pool).await.unwrap();
    }
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.job")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(jobs, 1);
}
```

- [ ] **Step 2: Futtatás — bukás**

Run: `cd rust && cargo test -p football-storage --test worker_schema -- --test-threads=1`
Expected: FAIL — a táblák nem léteznek.

- [ ] **Step 3: Migráció + test_support**

`db/migrations/0004_worker_framework.sql`:

```sql
-- 0004 — worker keretrendszer ops táblái (S2a).
-- Visszaállítási út: scripts/db-reset.sh (tesztDB); dev DB-n a táblák üresek,
-- eldobásuk fordított sorrendben: robots_cache, coverage_slot, job, breaker_state,
-- fetch_ledger, source_policy (DROP TABLE ... CASCADE nélkül, FK-sorrendben).

BEGIN;

CREATE TABLE ops.source_policy (
    source_slug text PRIMARY KEY,
    transport text NOT NULL CHECK (transport IN ('rust_http','fixture')),
    mode text NOT NULL CHECK (mode IN ('shadow','canary','live')),
    daily_budget integer NOT NULL CHECK (daily_budget >= 0),
    min_delay_ms bigint NOT NULL CHECK (min_delay_ms >= 0),
    jitter_ms bigint NOT NULL CHECK (jitter_ms >= 0),
    breaker_threshold integer NOT NULL CHECK (breaker_threshold >= 1),
    breaker_cooldown_secs bigint NOT NULL CHECK (breaker_cooldown_secs >= 0),
    adapter_version text NOT NULL,
    parser_version text NOT NULL,
    domains jsonb NOT NULL DEFAULT '[]',
    events jsonb NOT NULL DEFAULT '[]',
    enabled boolean NOT NULL DEFAULT false,
    max_bytes bigint NOT NULL DEFAULT 5242880 CHECK (max_bytes > 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE ops.fetch_ledger (
    id uuid PRIMARY KEY,
    source_slug text NOT NULL REFERENCES ops.source_policy(source_slug),
    request_key text NOT NULL,
    attempted_at timestamptz NOT NULL,
    outcome text NOT NULL CHECK (outcome IN
        ('success','http_error','timeout','oversize','robots_blocked',
         'quota_blocked','breaker_open','transport_error','parse_error')),
    http_status smallint,
    byte_size bigint,
    latency_ms bigint,
    artifact_hash text,
    job_id uuid,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_fetch_ledger_source_day ON ops.fetch_ledger (source_slug, attempted_at);

CREATE TABLE ops.breaker_state (
    source_slug text PRIMARY KEY REFERENCES ops.source_policy(source_slug),
    state text NOT NULL CHECK (state IN ('closed','open','half_open')),
    failure_count integer NOT NULL DEFAULT 0 CHECK (failure_count >= 0),
    opened_at timestamptz,
    next_probe_at timestamptz,
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE ops.job (
    id uuid PRIMARY KEY,
    source_slug text NOT NULL REFERENCES ops.source_policy(source_slug),
    domain text NOT NULL,
    entity_key text NOT NULL,
    event_type text NOT NULL,
    time_window text NOT NULL,
    due_at timestamptz NOT NULL,
    priority integer NOT NULL DEFAULT 100,
    status text NOT NULL CHECK (status IN ('pending','leased','done','failed','cancelled')),
    leased_until timestamptz,
    worker_id uuid,
    attempts integer NOT NULL DEFAULT 0,
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (source_slug, domain, entity_key, event_type, time_window)
);

CREATE INDEX idx_job_due ON ops.job (status, due_at, priority);

CREATE TABLE ops.coverage_slot (
    source_slug text NOT NULL REFERENCES ops.source_policy(source_slug),
    entity_type text NOT NULL,
    data_kind text NOT NULL,
    time_window text NOT NULL,
    state text NOT NULL CHECK (state IN
        ('pending','fetching','done','no_data','blocked','failed','stale','quarantined','skipped')),
    last_job_id uuid,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (source_slug, entity_type, data_kind, time_window)
);

CREATE TABLE ops.robots_cache (
    host text PRIMARY KEY,
    fetched_at timestamptz NOT NULL,
    verdict text NOT NULL CHECK (verdict IN ('allowed','no_robots','blocked')),
    rules_text text NOT NULL,
    expires_at timestamptz NOT NULL
);

COMMIT;
```

`test_support.rs` TRUNCATE-listájához hozzáadandó (a `ops.audit_event` elé):
`ops.robots_cache, ops.coverage_slot, ops.job, ops.breaker_state, ops.fetch_ledger, ops.source_policy,`

- [ ] **Step 4: Futtatás — zöld + migrációs fegyelem**

Run: `cd rust && cargo test -p football-storage --test worker_schema -- --test-threads=1`
Expected: PASS.
Run: `./scripts/db-reset.sh && ./scripts/db-check.sh`
Expected: a táblaszám 6-tal nő a korábbi állapothoz képest; tesztDB 0 üzleti sor. A dev DB-re a migráció csak a teszt zöld után kerül (a check.sh a tesztDB-t használja; dev: `footballctl migrate` külön engedéllyel a Task 12-ben).

- [ ] **Step 5: STOP — jelentés**

Checkpoint: migráció + teszteredmény + db-check számok. Commit NEM készül.

---

### Task 3: Storage repository-k a worker táblákhoz

**Files:**
- Create: `rust/crates/storage/src/repositories/worker.rs`
- Modify: `rust/crates/storage/src/repositories/mod.rs` (`pub mod worker;`)
- Test: `rust/crates/storage/tests/worker_repos.rs`

**Interfaces:**
- Consumes: Task 1 típusok, Task 2 táblák.
- Produces (mind `anyhow::Result`, `pool: &PgPool`):
  - `upsert_policy(pool, &SourcePolicy)`, `get_policy(pool, slug) -> Option<SourcePolicy>`, `list_enabled_policies(pool) -> Vec<SourcePolicy>`
  - `insert_ledger(pool, &LedgerEntry) -> Uuid`, `count_fetch_attempts_today(pool, slug, day: NaiveDate) -> i64`, `last_attempt_at(pool, slug) -> Option<DateTime<Utc>>`
  - `get_breaker(pool, slug) -> Option<BreakerRow>`, `set_breaker(pool, slug, &BreakerRow)`
  - `insert_job(pool, &NewJob)`, `claim_due_jobs(pool, worker_id: Uuid, now, lease_secs: i64, limit: i64) -> Vec<FetchJob>`, `requeue_expired_leases(pool, now) -> u64`, `complete_job(pool, job_id, JobStatus, last_error: Option<&str>)`, `release_worker_leases(pool, worker_id)`
  - `upsert_coverage(pool, slug, entity_type, data_kind, time_window, CoverageState, job_id: Option<Uuid>)`
  - `get_robots_cache(pool, host) -> Option<RobotsCacheRow>`, `put_robots_cache(pool, &RobotsCacheRow)`
  - `LedgerEntry { source_slug, request_key, attempted_at, outcome: FetchOutcome, http_status: Option<i16>, byte_size: Option<i64>, latency_ms: Option<i64>, artifact_hash: Option<String>, job_id: Option<Uuid> }`
  - `BreakerRow { state: BreakerState, failure_count: i32, opened_at: Option<DateTime<Utc>>, next_probe_at: Option<DateTime<Utc>> }`
  - `NewJob { source_slug, domain, entity_key, event_type, time_window, due_at, priority }`
  - `RobotsCacheRow { host, fetched_at, verdict: String, rules_text, expires_at }`

- [ ] **Step 1: Bukó integrációs teszt**

`rust/crates/storage/tests/worker_repos.rs` — a lényegi esetek (a teljes fájl a repository függvényeivel együtt írandó):

```rust
use chrono::{Duration, Utc};
use football_domain::policy::*;
use football_storage::repositories::worker as repo;
use football_storage::test_support::test_pool;
use uuid::Uuid;

async fn seed_policy(pool: &sqlx::PgPool, slug: &str) {
    let policy = SourcePolicy {
        source_slug: slug.into(), transport: Transport::Fixture, mode: SourceMode::Shadow,
        daily_budget: 2, min_delay_ms: 0, jitter_ms: 0, breaker_threshold: 3,
        breaker_cooldown_secs: 60, adapter_version: "1.0".into(), parser_version: "1.0".into(),
        domains: vec!["match".into()], events: vec![], enabled: true, max_bytes: 5_242_880,
    };
    repo::upsert_policy(pool, &policy).await.unwrap();
}

#[tokio::test]
async fn ledger_counts_only_fetch_attempts() {
    let pool = test_pool().await;
    seed_policy(&pool, "demo-synthetic").await;
    let now = Utc::now();
    for outcome in [FetchOutcome::Success, FetchOutcome::Timeout, FetchOutcome::QuotaBlocked] {
        repo::insert_ledger(&pool, &repo::LedgerEntry {
            source_slug: "demo-synthetic".into(), request_key: "k".into(),
            attempted_at: now, outcome, http_status: None, byte_size: None,
            latency_ms: None, artifact_hash: None, job_id: None,
        }).await.unwrap();
    }
    // a quota_blocked nem fetch-kísérlet: nem számít a kvótába
    let n = repo::count_fetch_attempts_today(&pool, "demo-synthetic", now.date_naive()).await.unwrap();
    assert_eq!(n, 2);
}

#[tokio::test]
async fn claim_lease_and_requeue() {
    let pool = test_pool().await;
    seed_policy(&pool, "demo-synthetic").await;
    let now = Utc::now();
    repo::insert_job(&pool, &repo::NewJob {
        source_slug: "demo-synthetic".into(), domain: "match".into(), entity_key: "day".into(),
        event_type: "discovery".into(), time_window: now.date_naive().to_string(),
        due_at: now - Duration::minutes(1), priority: 10,
    }).await.unwrap();

    let worker = Uuid::now_v7();
    let claimed = repo::claim_due_jobs(&pool, worker, now, 60, 10).await.unwrap();
    assert_eq!(claimed.len(), 1);
    // második claim ugyanarra a jobra nem ad vissza semmit (leased)
    assert!(repo::claim_due_jobs(&pool, worker, now, 60, 10).await.unwrap().is_empty());
    // lejárt lease visszasorolása
    let later = now + Duration::seconds(120);
    assert_eq!(repo::requeue_expired_leases(&pool, later).await.unwrap(), 1);
    let reclaimed = repo::claim_due_jobs(&pool, Uuid::now_v7(), later, 60, 10).await.unwrap();
    assert_eq!(reclaimed.len(), 1);
    assert_eq!(reclaimed[0].id, claimed[0].id);
}
```

- [ ] **Step 2: Futtatás — bukás**

Run: `cd rust && cargo test -p football-storage --test worker_repos -- --test-threads=1`
Expected: FAIL — `worker` repository nem létezik.

- [ ] **Step 3: Implementáció**

`rust/crates/storage/src/repositories/worker.rs` — a fenti interfészek SQLx-szel. Kulcsdarabok (a teljes fájl ezekből áll):

```rust
pub async fn count_fetch_attempts_today(pool: &PgPool, slug: &str, day: NaiveDate) -> Result<i64> {
    sqlx::query_scalar(
        "SELECT count(*) FROM ops.fetch_ledger
         WHERE source_slug = $1 AND attempted_at::date = $2
           AND outcome IN ('success','http_error','timeout','oversize','transport_error','parse_error')",
    )
    .bind(slug).bind(day).fetch_one(pool).await.context("napi fetch-számláló")
}

pub async fn claim_due_jobs(pool: &PgPool, worker_id: Uuid, now: DateTime<Utc>,
                            lease_secs: i64, limit: i64) -> Result<Vec<FetchJob>> {
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
    .bind(now).bind(worker_id).bind(lease_secs).bind(limit)
    .fetch_all(pool).await.context("job claim")?;
    rows.iter().map(|r| Ok(FetchJob {
        id: r.try_get("id")?, source_slug: r.try_get("source_slug")?,
        domain: r.try_get("domain")?, entity_key: r.try_get("entity_key")?,
        event_type: r.try_get("event_type")?, time_window: r.try_get("time_window")?,
        due_at: r.try_get("due_at")?, priority: r.try_get("priority")?,
    })).collect()
}

pub async fn requeue_expired_leases(pool: &PgPool, now: DateTime<Utc>) -> Result<u64> {
    let res = sqlx::query(
        "UPDATE ops.job SET status='pending', worker_id=NULL, leased_until=NULL
         WHERE status='leased' AND leased_until < $1",
    ).bind(now).execute(pool).await.context("lease visszasorolás")?;
    Ok(res.rows_affected())
}
```

A többi függvény ugyanilyen mintán: `upsert_policy` `ON CONFLICT (source_slug) DO UPDATE` (minden mező + `updated_at = now()`), jsonb domainek/events `serde_json::to_value`-vel; `insert_job` `ON CONFLICT ... DO NOTHING`; `upsert_coverage` `ON CONFLICT (source_slug, entity_type, data_kind, time_window) DO UPDATE SET state=EXCLUDED.state, last_job_id=EXCLUDED.last_job_id, updated_at=now()`; enumok `as_db_str()`/`from_db_str()` konverzióval.

- [ ] **Step 4: Futtatás — zöld**

Run: `cd rust && cargo test -p football-storage --test worker_repos -- --test-threads=1 && cargo clippy -p football-storage --all-targets -- -D warnings`
Expected: minden teszt PASS.

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 4: Fetcher trait + FixtureFetcher + observe-kiemelés

**Files:**
- Create: `rust/crates/ingest/src/fetch.rs`, `rust/crates/ingest/src/observe.rs`
- Modify: `rust/crates/ingest/src/lib.rs`, `rust/crates/ingest/src/fixture.rs` (a privát `insert_observation` törlődik, a megosztott használata), `rust/crates/ingest/Cargo.toml` (async-trait)
- Test: `rust/crates/ingest/tests/fetcher.rs`

**Interfaces:**
- Consumes: Task 1 `FetchError`.
- Produces:
  - `FetchRequest { url: String, request_key: String, endpoint_key: String }`
  - `RawPayload { bytes: Vec<u8>, content_type: String, http_status: u16, fetched_at: DateTime<Utc>, sha256: String }`
  - `FetchLimits { max_bytes: u64, timeout: std::time::Duration }`
  - `#[async_trait] trait Fetcher: Send + Sync { async fn fetch(&self, req: &FetchRequest, limits: &FetchLimits) -> Result<RawPayload, FetchError>; }`
  - `FixtureFetcher::new(HashMap<String, PathBuf>)`, `FixtureFetcher::from_dir(dir: &Path) -> Result<Self>` (request-key = fájlnév törzse, tartalom = JSON)
  - `observe::insert_observation(pool, source_id, capture_id, entity_kind, aspect, natural_key, source_record_key, observed_at, normalized_payload) -> Result<Uuid>` (a fixture.rs-ből kiemelve, változatlan SQL-lel)

- [ ] **Step 1: Bukó teszt**

`rust/crates/ingest/tests/fetcher.rs`:

```rust
use std::collections::HashMap;
use std::time::Duration;
use football_domain::error::FetchError;
use football_ingest::fetch::{FetchLimits, FetchRequest, FixtureFetcher};
use football_ingest::fetch::Fetcher;

fn limits() -> FetchLimits { FetchLimits { max_bytes: 1024, timeout: Duration::from_secs(5) } }
fn req(key: &str) -> FetchRequest {
    FetchRequest { url: format!("fixture://{key}"), request_key: key.into(), endpoint_key: "match".into() }
}

#[tokio::test]
async fn fixture_fetch_returns_payload_with_hash() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("day.json"), br#"{"ok":true}"#).unwrap();
    let fetcher = FixtureFetcher::from_dir(dir.path()).unwrap();
    let raw = fetcher.fetch(&req("day"), &limits()).await.unwrap();
    assert_eq!(raw.http_status, 200);
    assert_eq!(raw.bytes, br#"{"ok":true}"#);
    assert_eq!(raw.sha256.len(), 64);
}

#[tokio::test]
async fn missing_key_is_transport_error() {
    let fetcher = FixtureFetcher::new(HashMap::new());
    let err = fetcher.fetch(&req("nincs"), &limits()).await.unwrap_err();
    assert!(matches!(err, FetchError::Transport(_)));
}

#[tokio::test]
async fn oversize_payload_rejected() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("big.json"), vec![b'x'; 2048]).unwrap();
    let fetcher = FixtureFetcher::from_dir(dir.path()).unwrap();
    let err = fetcher.fetch(&req("big"), &limits()).await.unwrap_err();
    assert!(matches!(err, FetchError::Oversize(2048)));
}
```

- [ ] **Step 2: Futtatás — bukás**

Run: `cd rust && cargo test -p football-ingest --test fetcher`
Expected: FAIL — `fetch` modul nincs.

- [ ] **Step 3: Implementáció**

`fetch.rs`: a fenti típusok + trait; `FixtureFetcher::fetch`: fájlnév-feloldás a mapből, `std::fs::read`, méretplafon-ellenőrzés (`bytes.len() as u64 > limits.max_bytes` → `Oversize`), sha256 (`sha2::Sha256::digest` + hex), `content_type: "application/json"`, `http_status: 200`, `fetched_at: Utc::now()`. `from_dir`: `*.json` fájlok beolvasása, key = `file_stem`.

`observe.rs`: a fixture.rs jelenlegi privát `insert_observation` függvénye **változatlan SQL-lel és szignatúrával** publikusként; a fixture.rs `use crate::observe::insert_observation;`-t használ.

- [ ] **Step 4: Futtatás — zöld + regresszió**

Run: `cd rust && cargo test -p football-ingest -- --test-threads=1`
Expected: az új 3 teszt PASS ÉS a meglévő fixture/canonicalize/display tesztek változatlanul zöldek (a kiemelés viselkedése azonos).

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 5: RustHttpFetcher (reqwest + wiremock)

**Files:**
- Create: `rust/crates/ingest/src/http.rs`
- Modify: `rust/crates/ingest/src/lib.rs`, `rust/Cargo.toml` (workspace deps: reqwest, wiremock), `rust/crates/ingest/Cargo.toml`
- Test: `rust/crates/ingest/tests/http_fetcher.rs`

**Interfaces:**
- Consumes: Task 4 `Fetcher`/`FetchRequest`/`RawPayload`/`FetchLimits`.
- Produces: `RustHttpFetcher::new(user_agent: &str) -> Result<Self, FetchError>`; `Fetcher` impl: GET, UA header, per-request timeout, nem-2xx → `HttpStatus(u16)`, timeout → `Timeout`, méretplafon → `Oversize`, hálózat → `Transport`.

- [ ] **Step 1: Workspace-függőségek**

`rust/Cargo.toml` `[workspace.dependencies]`:
```toml
reqwest = { version = "0.13.5", default-features = false, features = ["rustls-tls"] }
wiremock = "0.6.5"
```
`ingest/Cargo.toml`: `reqwest.workspace = true`; `[dev-dependencies]` `wiremock.workspace = true`, `tempfile.workspace = true`.

- [ ] **Step 2: Bukó teszt**

`rust/crates/ingest/tests/http_fetcher.rs`:

```rust
use std::time::Duration;
use football_domain::error::FetchError;
use football_ingest::fetch::{FetchLimits, FetchRequest, Fetcher};
use football_ingest::http::RustHttpFetcher;
use wiremock::matchers::{method, path, header};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn limits() -> FetchLimits { FetchLimits { max_bytes: 1024, timeout: Duration::from_secs(2) } }

#[tokio::test]
async fn fetch_200_with_honest_ua() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/data.json"))
        .and(header("user-agent", "football-app-v2/0.1"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"ok":true}"#))
        .mount(&server).await;
    let fetcher = RustHttpFetcher::new("football-app-v2/0.1").unwrap();
    let req = FetchRequest { url: format!("{}/data.json", server.uri()),
        request_key: "k".into(), endpoint_key: "match".into() };
    let raw = fetcher.fetch(&req, &limits()).await.unwrap();
    assert_eq!(raw.http_status, 200);
    assert_eq!(raw.sha256.len(), 64);
}

#[tokio::test]
async fn http_error_status_is_typed() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(ResponseTemplate::new(403)).mount(&server).await;
    let fetcher = RustHttpFetcher::new("football-app-v2/0.1").unwrap();
    let req = FetchRequest { url: server.uri(), request_key: "k".into(), endpoint_key: "m".into() };
    let err = fetcher.fetch(&req, &limits()).await.unwrap_err();
    assert!(matches!(err, FetchError::HttpStatus(403)));
}

#[tokio::test]
async fn oversize_body_rejected() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("x".repeat(2048)))
        .mount(&server).await;
    let fetcher = RustHttpFetcher::new("football-app-v2/0.1").unwrap();
    let req = FetchRequest { url: server.uri(), request_key: "k".into(), endpoint_key: "m".into() };
    let err = fetcher.fetch(&req, &limits()).await.unwrap_err();
    assert!(matches!(err, FetchError::Oversize(_)));
}

#[tokio::test]
async fn timeout_is_typed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
        .mount(&server).await;
    let fetcher = RustHttpFetcher::new("football-app-v2/0.1").unwrap();
    let req = FetchRequest { url: server.uri(), request_key: "k".into(), endpoint_key: "m".into() };
    let err = fetcher.fetch(&req, &limits()).await.unwrap_err();
    assert!(matches!(err, FetchError::Timeout));
}
```

- [ ] **Step 3: Futtatás — bukás**

Run: `cd rust && cargo test -p football-ingest --test http_fetcher`
Expected: FAIL — `http` modul nincs.

- [ ] **Step 4: Implementáció**

`rust/crates/ingest/src/http.rs`:

```rust
use async_trait::async_trait;
use chrono::Utc;
use football_domain::error::FetchError;
use sha2::{Digest, Sha256};

use crate::fetch::{FetchLimits, FetchRequest, Fetcher, RawPayload};

pub struct RustHttpFetcher {
    client: reqwest::Client,
    user_agent: String,
}

impl RustHttpFetcher {
    pub fn new(user_agent: &str) -> Result<Self, FetchError> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| FetchError::Transport(e.to_string()))?;
        Ok(Self { client, user_agent: user_agent.to_string() })
    }
}

#[async_trait]
impl Fetcher for RustHttpFetcher {
    async fn fetch(&self, req: &FetchRequest, limits: &FetchLimits) -> Result<RawPayload, FetchError> {
        let response = self.client.get(&req.url)
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .timeout(limits.timeout)
            .send().await
            .map_err(|e| if e.is_timeout() { FetchError::Timeout } else { FetchError::Transport(e.to_string()) })?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(FetchError::HttpStatus(status));
        }
        let content_type = response.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = response.bytes().await
            .map_err(|e| if e.is_timeout() { FetchError::Timeout } else { FetchError::Transport(e.to_string()) })?;
        if bytes.len() as u64 > limits.max_bytes {
            return Err(FetchError::Oversize(bytes.len() as u64));
        }
        Ok(RawPayload {
            sha256: hex::encode(Sha256::digest(&bytes)),
            bytes: bytes.to_vec(),
            content_type,
            http_status: status,
            fetched_at: Utc::now(),
        })
    }
}
```

- [ ] **Step 5: Futtatás — zöld**

Run: `cd rust && cargo test -p football-ingest --test http_fetcher -- --test-threads=1 && cargo clippy -p football-ingest --all-targets -- -D warnings`
Expected: 4 teszt PASS. Élő hálózati kérés nem történt (wiremock lokális).

- [ ] **Step 6: STOP — jelentés.** Commit NEM készül.

---

### Task 6: SourceAdapter trait + DemoAdapter + golden fixture-ök

**Files:**
- Create: `rust/crates/ingest/src/adapter.rs`, `rust/crates/ingest/src/demo.rs`, `tests/fixtures/worker/demo_day.json`, `tests/fixtures/worker/demo_day_decoy.json`, `tests/fixtures/worker/demo_day_wrong_date.json`, `tests/fixtures/worker/demo_day_aet.json`, `tests/fixtures/worker/demo_day_drift.json`
- Modify: `rust/crates/ingest/src/lib.rs`
- Test: `rust/crates/ingest/tests/demo_adapter.rs`

**Interfaces:**
- Consumes: Task 1 `FetchJob`/`ParseError`, Task 4 `RawPayload`.
- Produces:
  - `AdapterMeta { source_slug, adapter_version, parser_version, domains: Vec<String>, transport: Transport }`
  - `ParseCtx { requested_date: chrono::NaiveDate, request_key: String }`
  - `Observation { entity_kind: String, aspect: String, natural_key: String, source_record_key: String, observed_at: DateTime<Utc>, payload: serde_json::Value }`
  - `trait SourceAdapter: Send + Sync { fn meta(&self) -> AdapterMeta; fn plan(&self, job: &FetchJob) -> Vec<FetchRequest>; fn parse(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError>; }`
  - `DemoAdapter` — a payload a meglévő `FixtureSource` JSON-alakját követi (szintetikus); őrök: decoy (üres matches), dátum-guard (kickoff napja a verseny TZ-jában == requested_date), AET/pens fail-closed karantén, séma-drift (ismeretlen top-level mezők aránya > 0.2).

- [ ] **Step 1: Golden fixture-ök (szintetikusak)**

`tests/fixtures/worker/demo_day.json` — a meglévő `tests/fixtures/demo_day_a_*.json` alakját követő, de **kisebb, szintetikus** állomány: 1 verseny ("Demó Liga", country_code "XX", tier 1, priority 10), 1 szezon, 2 csapat ("Demó FC", "Vendég SC"), 2 meccs a kért napon (státusz "scheduled", ill. "finished" 2–1 eredménnyel, HT 1–0), odds nélkül. A dátumok a tesztben generálódnak: a fixture fájlban rögzített dátum **2026-11-12**, a tesztek ezt kérik (`ParseCtx.requested_date = 2026-11-12`). A decoy fájl: ugyanaz, de `"matches": []`. A wrong_date: meccs-kickoff 2026-11-13. Az aet: finished meccs `"home_et": 1, "away_et": 1, "home_pens": 4, "away_pens": 3` mezőkkel. A drift: 4 ismeretlen top-level mező (`"extra1"…"extra4"`) a ~10 ismert mellett.

- [ ] **Step 2: Bukó contract-teszt**

`rust/crates/ingest/tests/demo_adapter.rs`:

```rust
use chrono::NaiveDate;
use football_domain::error::ParseError;
use football_domain::policy::{FetchJob, Transport};
use football_ingest::adapter::{ParseCtx, SourceAdapter};
use football_ingest::demo::DemoAdapter;
use football_ingest::fetch::RawPayload;
use chrono::Utc;
use uuid::Uuid;

fn raw_from(name: &str) -> RawPayload {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/worker/")
        .join(name);
    let bytes = std::fs::read(path).unwrap();
    let sha256 = hex::encode(sha2::Sha256::digest(&bytes));
    RawPayload { bytes, content_type: "application/json".into(), http_status: 200, fetched_at: Utc::now(), sha256 }
}

fn ctx() -> ParseCtx {
    ParseCtx { requested_date: NaiveDate::from_ymd_opt(2026, 11, 12).unwrap(), request_key: "demo-synthetic:match:2026-11-12".into() }
}

fn job() -> FetchJob {
    FetchJob { id: Uuid::now_v7(), source_slug: "demo-synthetic".into(), domain: "match".into(),
        entity_key: "day".into(), event_type: "discovery".into(), time_window: "2026-11-12".into(),
        due_at: Utc::now(), priority: 10 }
}

#[test]
fn happy_path_observations() {
    let adapter = DemoAdapter;
    let obs = adapter.parse(&raw_from("demo_day.json"), &ctx()).unwrap();
    // 2 team_identity + 2 match_core
    assert_eq!(obs.iter().filter(|o| o.aspect == "team_identity").count(), 2);
    assert_eq!(obs.iter().filter(|o| o.aspect == "match_core").count(), 2);
    let meta = adapter.meta();
    assert_eq!(meta.source_slug, "demo-synthetic");
    assert_eq!(meta.transport, Transport::Fixture);
    let plan = adapter.plan(&job());
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].request_key, "demo-synthetic:match:2026-11-12");
}

#[test]
fn decoy_empty_matches_rejected() {
    let err = DemoAdapter.parse(&raw_from("demo_day_decoy.json"), &ctx()).unwrap_err();
    assert!(matches!(err, ParseError::DecoyDetected(_)));
}

#[test]
fn date_guard_mismatch_rejected() {
    let err = DemoAdapter.parse(&raw_from("demo_day_wrong_date.json"), &ctx()).unwrap_err();
    assert!(matches!(err, ParseError::DateGuardMismatch { .. }));
}

#[test]
fn aet_pens_result_quarantined() {
    let err = DemoAdapter.parse(&raw_from("demo_day_aet.json"), &ctx()).unwrap_err();
    assert!(matches!(err, ParseError::Parse(_)));
}

#[test]
fn schema_drift_rejected() {
    let err = DemoAdapter.parse(&raw_from("demo_day_drift.json"), &ctx()).unwrap_err();
    assert!(matches!(err, ParseError::SchemaDrift(_)));
}
```

- [ ] **Step 3: Futtatás — bukás**

Run: `cd rust && cargo test -p football-ingest --test demo_adapter`
Expected: FAIL — `adapter`/`demo` modul nincs.

- [ ] **Step 4: Implementáció**

`adapter.rs`: a fenti típusok + trait.

`demo.rs` — a parse lépései (teljes implementáció):

```rust
use football_domain::error::ParseError;
use football_domain::policy::{FetchJob, Transport};
use crate::adapter::{AdapterMeta, Observation, ParseCtx, SourceAdapter};
use crate::fetch::{FetchRequest, RawPayload};
use crate::fixture::FixtureSource;

pub struct DemoAdapter;

const KNOWN_FIELDS: [&str; 10] = [
    "source_code", "source_display_name", "captured_at", "endpoint_key", "request_url",
    "adapter_version", "parser_version", "competition", "season", "teams",
];
// matches/standings opcionális: a drift-arány a KNOWN_FIELDS + matches + standings halmazra számol

impl SourceAdapter for DemoAdapter {
    fn meta(&self) -> AdapterMeta {
        AdapterMeta {
            source_slug: "demo-synthetic".into(),
            adapter_version: "1.0".into(),
            parser_version: "1.0".into(),
            domains: vec!["match".into()],
            transport: Transport::Fixture,
        }
    }

    fn plan(&self, job: &FetchJob) -> Vec<FetchRequest> {
        vec![FetchRequest {
            url: format!("fixture://{}/{}", job.source_slug, job.time_window),
            request_key: format!("{}:{}:{}", job.source_slug, job.domain, job.time_window),
            endpoint_key: job.domain.clone(),
        }]
    }

    fn parse(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError> {
        let value: serde_json::Value = serde_json::from_slice(&raw.bytes)
            .map_err(|e| ParseError::Parse(e.to_string()))?;
        let object = value.as_object()
            .ok_or_else(|| ParseError::DecoyDetected("nem JSON-objektum".into()))?;

        let mut known = KNOWN_FIELDS.iter().map(|s| s.to_string()).collect::<std::collections::HashSet<_>>();
        known.insert("matches".into());
        known.insert("standings".into());
        let unknown = object.keys().filter(|k| !known.contains(*k)).count();
        if unknown as f64 / object.len().max(1) as f64 > 0.2 {
            return Err(ParseError::SchemaDrift(format!("{unknown} ismeretlen mező")));
        }

        let fixture: FixtureSource = serde_json::from_value(value)
            .map_err(|e| ParseError::Parse(e.to_string()))?;
        if fixture.matches.is_empty() {
            return Err(ParseError::DecoyDetected("üres meccslista".into()));
        }

        for m in &fixture.matches {
            let tz: chrono_tz::Tz = m.competition_timezone.parse()
                .map_err(|_| ParseError::Parse("ismeretlen meccs-időzóna".into()))?;
            let local_date = m.kickoff_at.with_timezone(&tz).date_naive();
            if local_date != ctx.requested_date {
                return Err(ParseError::DateGuardMismatch {
                    requested: ctx.requested_date.to_string(),
                    got: local_date.to_string(),
                });
            }
            if let Some(r) = &m.result {
                if r.home_et.is_some() || r.away_et.is_some() || r.home_pens.is_some() || r.away_pens.is_some() {
                    return Err(ParseError::Parse(
                        "AET/pens állás nem írható 90 perces eredményként (S2a fail-closed)".into()));
                }
            }
        }

        let mut out = Vec::new();
        for team in &fixture.teams {
            out.push(Observation {
                entity_kind: "team".into(), aspect: "team_identity".into(),
                natural_key: team.id.to_string(), source_record_key: team.source_entity_id.clone(),
                observed_at: fixture.captured_at,
                payload: serde_json::to_value(team).map_err(|e| ParseError::Parse(e.to_string()))?,
            });
        }
        for m in &fixture.matches {
            let home = fixture.teams.iter().find(|t| t.source_entity_id == m.home_source_entity_id)
                .ok_or_else(|| ParseError::Parse("ismeretlen hazai forrásentitás".into()))?;
            let away = fixture.teams.iter().find(|t| t.source_entity_id == m.away_source_entity_id)
                .ok_or_else(|| ParseError::Parse("ismeretlen vendég forrásentitás".into()))?;
            out.push(Observation {
                entity_kind: "match".into(), aspect: "match_core".into(),
                natural_key: m.id.to_string(), source_record_key: m.source_match_id.clone(),
                observed_at: fixture.captured_at,
                payload: serde_json::json!({
                    "canonical_match_id": m.id, "home_team_id": home.id, "away_team_id": away.id,
                    "source_home_id": m.home_source_entity_id, "source_away_id": m.away_source_entity_id,
                    "kickoff_at": m.kickoff_at, "status": m.status, "result": m.result,
                    "round_label": m.round_label, "competition_timezone": m.competition_timezone,
                }),
            });
        }
        Ok(out)
    }
}
```

Megjegyzés: a `FixtureSource`-ban nincs verseny-szintű TZ mező, a parse meccsenként parse-olja a `competition_timezone`-t. A `chrono-tz` már workspace-függőség; az ingest Cargo.toml-jába felveendő.

- [ ] **Step 5: Futtatás — zöld**

Run: `cd rust && cargo test -p football-ingest --test demo_adapter -- --test-threads=1 && cargo clippy -p football-ingest --all-targets -- -D warnings`
Expected: 5 contract-teszt PASS.

- [ ] **Step 6: STOP — jelentés.** Commit NEM készül.

---

### Task 7: Worker crate + robots-kapu

**Files:**
- Create: `rust/crates/worker/Cargo.toml`, `rust/crates/worker/src/lib.rs`, `rust/crates/worker/src/robots.rs`
- Modify: `rust/Cargo.toml` (robotstxt workspace-dep), `rust/crates/storage/src/repositories/worker.rs` (robots_cache már a Task 3-ban kész)
- Test: `rust/crates/worker/tests/robots_gate.rs`

**Interfaces:**
- Consumes: Task 3 `get_robots_cache`/`put_robots_cache`, Task 4 `Fetcher`.
- Produces: `robots::check(pool, fetcher: &dyn Fetcher, source_slug: &str, url: &str) -> Result<RobotsVerdict>`; `RobotsVerdict::{Allowed, NoRobots, Blocked}`; a robots-kérés request-key-e `{slug}:robots:{host}`, ledgerbe NEM ír (a tick írja a kísérletet — a robots-kapu a tick része, a ledger-elszámolás a Task 10-ben történik; a kapu csak a verdictet adja).

- [ ] **Step 1: Crate-váz + bukó teszt**

`rust/crates/worker/Cargo.toml`:

```toml
[package]
name = "football-worker"
edition.workspace = true
rust-version.workspace = true
version.workspace = true

[dependencies]
football-domain = { path = "../domain" }
football-storage = { path = "../storage" }
football-ingest = { path = "../ingest" }
anyhow.workspace = true
async-trait.workspace = true
chrono.workspace = true
robotstxt.workspace = true
sqlx.workspace = true
tokio.workspace = true
tracing.workspace = true
uuid.workspace = true
serde_json.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

`rust/Cargo.toml`: `robotstxt = "0.3.0"` a workspace-depsbe. `lib.rs`: `pub mod robots;`

`rust/crates/worker/tests/robots_gate.rs`:

```rust
use std::collections::HashMap;
use football_ingest::fetch::FixtureFetcher;
use football_storage::test_support::test_pool;
use football_worker::robots::{self, RobotsVerdict};

async fn seed_policy(pool: &sqlx::PgPool) {
    // ugyanaz a seed, mint a Task 3 tesztekben (source_policy insert fixture transporttal)
    football_storage::repositories::worker::upsert_policy(pool, &football_domain::policy::SourcePolicy {
        source_slug: "demo-synthetic".into(), transport: football_domain::policy::Transport::Fixture,
        mode: football_domain::policy::SourceMode::Shadow, daily_budget: 10, min_delay_ms: 0,
        jitter_ms: 0, breaker_threshold: 3, breaker_cooldown_secs: 60,
        adapter_version: "1.0".into(), parser_version: "1.0".into(),
        domains: vec![], events: vec![], enabled: true, max_bytes: 5_242_880,
    }).await.unwrap();
}

fn fetcher_with(files: &[(&str, &str)]) -> (tempfile::TempDir, FixtureFetcher) {
    let dir = tempfile::tempdir().unwrap();
    let mut map = HashMap::new();
    for (key, content) in files {
        let path = dir.path().join(format!("{key}.json"));
        // a robots-tartalom NEM json: a FixtureFetcher nyers bájtot ad, a kiterjesztés .json,
        // de a robots-kapu a nyers szöveget parse-olja
        std::fs::write(&path, content).unwrap();
        map.insert(key.to_string(), path);
    }
    (dir, FixtureFetcher::new(map))
}

#[tokio::test]
async fn allowed_path_and_cache_reuse() {
    let pool = test_pool().await;
    seed_policy(&pool).await;
    let (_dir, fetcher) = fetcher_with(&[
        ("demo-synthetic:robots:example.test", "User-agent: *\nDisallow: /private\n"),
    ]);
    let v = robots::check(&pool, &fetcher, "demo-synthetic", "https://example.test/public").await.unwrap();
    assert_eq!(v, RobotsVerdict::Allowed);
    // cache-ből: a fetcher már nem kell (üres fetcher is jó)
    let empty = FixtureFetcher::new(HashMap::new());
    let v2 = robots::check(&pool, &empty, "demo-synthetic", "https://example.test/public").await.unwrap();
    assert_eq!(v2, RobotsVerdict::Allowed);
    let denied = robots::check(&pool, &empty, "demo-synthetic", "https://example.test/private").await.unwrap();
    assert_eq!(denied, RobotsVerdict::Blocked);
}

#[tokio::test]
async fn robots_404_means_no_robots() {
    let pool = test_pool().await;
    seed_policy(&pool).await;
    // a FixtureFetcher hiányzó kulcsa Transport hibát ad — a 404-esetet a robots-kapu
    // a Fetcher HttpStatus(404) ágán kezeli; FixtureFetcher-rel a hiányzó robots-fájl
    // NoRobots verdictet ad (dokumentált viselkedés: fixture módban a nem létező robots = nincs robots)
    let empty = FixtureFetcher::new(HashMap::new());
    let v = robots::check(&pool, &empty, "demo-synthetic", "https://example.test/x").await.unwrap();
    assert_eq!(v, RobotsVerdict::NoRobots);
}

#[tokio::test]
async fn unparseable_robots_fail_closed() {
    let pool = test_pool().await;
    seed_policy(&pool).await;
    let (_dir, fetcher) = fetcher_with(&[
        ("demo-synthetic:robots:example.test", "\x00\x01\xff nem robots"),
    ]);
    let v = robots::check(&pool, &fetcher, "demo-synthetic", "https://example.test/x").await.unwrap();
    assert_eq!(v, RobotsVerdict::Blocked);
}
```

- [ ] **Step 2: Futtatás — bukás**

Run: `cd rust && cargo test -p football-worker --test robots_gate -- --test-threads=1`
Expected: FAIL — crate/modul nincs.

- [ ] **Step 3: Implementáció**

`rust/crates/worker/src/robots.rs`:

```rust
use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use football_domain::error::FetchError;
use football_ingest::fetch::{FetchLimits, FetchRequest, Fetcher};
use football_storage::repositories::worker::{get_robots_cache, put_robots_cache, RobotsCacheRow};
use sqlx::PgPool;
use std::time::Duration as StdDuration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RobotsVerdict { Allowed, NoRobots, Blocked }

const UA: &str = "football-app-v2";

pub async fn check(pool: &PgPool, fetcher: &dyn Fetcher, source_slug: &str, url: &str) -> Result<RobotsVerdict> {
    let parsed = url::Url::parse(url).context("robots: URL-parse")?;
    let host = parsed.host_str().context("robots: nincs host")?.to_string();
    let now = Utc::now();

    let rules = match get_robots_cache(pool, &host).await? {
        Some(row) if row.expires_at > now => row,
        _ => {
            let robots_url = format!("{}://{}/robots.txt", parsed.scheme(), host);
            let req = FetchRequest {
                url: robots_url,
                request_key: format!("{source_slug}:robots:{host}"),
                endpoint_key: "robots".into(),
            };
            let limits = FetchLimits { max_bytes: 5_242_880, timeout: StdDuration::from_secs(30) };
            let (verdict, rules_text) = match fetcher.fetch(&req, &limits).await {
                Ok(raw) => ("allowed".to_string(), String::from_utf8_lossy(&raw.bytes).to_string()),
                Err(FetchError::HttpStatus(404)) => ("no_robots".to_string(), String::new()),
                Err(FetchError::HttpStatus(403)) => ("blocked".to_string(), String::new()),
                Err(FetchError::Transport(_)) => ("no_robots".to_string(), String::new()),
                Err(_) => ("blocked".to_string(), String::new()),
            };
            let row = RobotsCacheRow {
                host: host.clone(), fetched_at: now, verdict, rules_text,
                expires_at: now + Duration::hours(24),
            };
            put_robots_cache(pool, &row).await?;
            row
        }
    };

    if rules.verdict == "blocked" {
        return Ok(RobotsVerdict::Blocked);
    }
    if rules.verdict == "no_robots" || rules.rules_text.is_empty() {
        return Ok(RobotsVerdict::NoRobots);
    }
    let robot = robotstxt::Robot::new(&rules.rules_text);
    if !robot.parsable() {
        return Ok(RobotsVerdict::Blocked);
    }
    if robot.matches(url, UA) { Ok(RobotsVerdict::Allowed) } else { Ok(RobotsVerdict::Blocked) }
}
```

Megjegyzések: (1) az `url` crate közvetlen használatához felvétel a workspace-depsbe: `url = "2"` (a reqwest tranzitív függősége, de a közvetlen import explicit függőséget igényel); (2) a `robotstxt::Robot` API-ja (`new`, `parsable`, `matches`) a 0.3.0-ás verzióé — implementáció előtt a crate forrása/docs ellenőrzendő; ha a `parsable` nem létezik, a parse-hiba fail-closed ágát a matcher üres/hibás bemeneti viselkedésével kell lefedni (teszt kötelező); (3) a FixtureFetcher `Transport` hibája (hiányzó fixture) a robots-kapu `no_robots` ága — ez fixture-módus specifikus, éles módban a RustHttpFetcher 404-je adja ugyanazt.

- [ ] **Step 4: Futtatás — zöld**

Run: `cd rust && cargo test -p football-worker --test robots_gate -- --test-threads=1 && cargo clippy -p football-worker --all-targets -- -D warnings`
Expected: 3 teszt PASS.

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 8: Kvóta + delay + breaker

**Files:**
- Create: `rust/crates/worker/src/quota.rs`, `rust/crates/worker/src/breaker.rs`
- Modify: `rust/crates/worker/src/lib.rs`
- Test: `rust/crates/worker/tests/quota_breaker.rs` + unit tesztek a breaker.rs-ben

**Interfaces:**
- Consumes: Task 1 `SourcePolicy`/`BreakerState`/`FetchOutcome`, Task 3 ledger/breaker repo.
- Produces:
  - `quota::guard(pool, policy, now) -> Result<QuotaDecision>`; `QuotaDecision::{Allowed, QuotaBlocked, DelayBlocked}` — a napi számláló a `count_fetch_attempts_today`, a delay a `last_attempt_at` + `min_delay_ms` (jitter a scheduler next due-ra, nem a guardra).
  - `breaker::allow(pool, policy, now) -> Result<bool>` (open + next_probe elérkezett → half_open átmenet + true; half_open → false; closed → true)
  - `breaker::on_success(pool, slug)`, `breaker::on_failure(pool, policy, now)`, `breaker::on_auth_failure(pool, policy, now)` — utóbbi azonnali open + `ops.audit_event` (`actor='worker'`, `action='policy_review'`, `subject_kind='source'`, detail: slug + státusz).

- [ ] **Step 1: Bukó tesztek**

Unit (breaker.rs `#[cfg(test)]`): a `next_state` tiszta függvény esetei — closed + failure < threshold → closed; closed + failure >= threshold → open (next_probe = now + cooldown); half_open + success → closed; half_open + failure → open.

Integrációs (`tests/quota_breaker.rs`): policy daily_budget=2; 2 `Success` ledger-sor után `quota::guard` → `QuotaBlocked`; `QuotaBlocked` outcome-ú ledger-sor NEM növeli a számlálót; auth-failure után breaker open + audit_event sor létezik (`action='policy_review'`).

```rust
#[tokio::test]
async fn quota_blocks_after_budget() {
    let pool = test_pool().await;
    // seed policy daily_budget=2 (Task 3 minta)
    let now = Utc::now();
    for _ in 0..2 {
        repo::insert_ledger(&pool, &repo::LedgerEntry {
            source_slug: "demo-synthetic".into(), request_key: "k".into(), attempted_at: now,
            outcome: FetchOutcome::Success, http_status: Some(200), byte_size: Some(10),
            latency_ms: Some(5), artifact_hash: None, job_id: None,
        }).await.unwrap();
    }
    let decision = quota::guard(&pool, &policy, now).await.unwrap();
    assert!(matches!(decision, quota::QuotaDecision::QuotaBlocked));
}

#[tokio::test]
async fn auth_failure_opens_breaker_and_flags_policy_review() {
    let pool = test_pool().await;
    breaker::on_auth_failure(&pool, &policy, Utc::now()).await.unwrap();
    let row = repo::get_breaker(&pool, "demo-synthetic").await.unwrap().unwrap();
    assert_eq!(row.state, BreakerState::Open);
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.audit_event WHERE action='policy_review'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(audits, 1);
}
```

- [ ] **Step 2: Futtatás — bukás.** Run: `cd rust && cargo test -p football-worker -- --test-threads=1` → FAIL.

- [ ] **Step 3: Implementáció**

`quota.rs`: `guard` = `count_fetch_attempts_today >= daily_budget` → QuotaBlocked; `last_attempt_at` + min_delay_ms > now → DelayBlocked; különben Allowed.

`breaker.rs`: tiszta `next_state(row: &BreakerRow, success: bool, policy: &SourcePolicy, now) -> BreakerRow` + async wrapper-ek a repo-ra; `allow`: get_breaker (nincs sor → closed-ként kezel), open és `next_probe_at <= now` → set half_open + true; open egyébként false; half_open false; closed true. `on_auth_failure`: state open, failure_count += 1, next_probe_at = now + cooldown, opened_at = now + audit_event insert.

- [ ] **Step 4: Futtatás — zöld.** Run: `cd rust && cargo test -p football-worker -- --test-threads=1 && cargo clippy -p football-worker --all-targets -- -D warnings` → PASS.

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 9: Scheduler (job-generálás, claim, prioritás)

**Files:**
- Create: `rust/crates/worker/src/scheduler.rs`
- Modify: `rust/crates/worker/src/lib.rs`
- Test: `rust/crates/worker/tests/scheduler.rs`

**Interfaces:**
- Consumes: Task 1 `SourcePolicy`/`EventTemplate`/`FetchJob`, Task 3 job repo.
- Produces:
  - `scheduler::generate_jobs(pool, policies: &[SourcePolicy], now) -> Result<usize>` — interval események: `time_window` = UTC dátum (napi) vagy ISO-hét (heti, `format!("{}-W{:02}", year, week)`), due_at = now; kickoff események (`offset_minutes`): a `core.match` sorokból, ahol kickoff a [now−72h, now+24h] ablakban van, due_at = kickoff + offset, entity_key = match id, time_window = kickoff UTC dátuma. Prioritás: `event_base + competition.priority + kickoff_közelség`, ahol event_base: discovery=0, refresh=10, backfill=100, canary=50, drift=60; kickoff_közelség = clamp(percek a kickoffig, 0..1440). Kisebb = előbb.
  - `scheduler::claim(pool, worker_id, now, lease_secs, limit) -> Result<Vec<FetchJob>>` (repo wrapper) + `requeue_expired` + `complete`.

- [ ] **Step 1: Bukó teszt**

```rust
#[tokio::test]
async fn generate_is_idempotent_and_prioritized() {
    let pool = test_pool().await;
    // seed policy events: discovery interval 1440 + refresh offset -60
    // + egy core.match sor a holnapi napra (competition priority 10)
    let n1 = scheduler::generate_jobs(&pool, &[policy], now).await.unwrap();
    let n2 = scheduler::generate_jobs(&pool, &[policy], now).await.unwrap();
    assert!(n1 >= 2);
    assert_eq!(n2, 0); // egyedi kulcs: nincs duplikátum
    let claimed = scheduler::claim(&pool, worker_id, now + Duration::hours(1), 60, 10).await.unwrap();
    // a discovery (base 0) a refresh (base 10) előtt jön
    assert_eq!(claimed[0].event_type, "discovery");
}
```

(A core.match seed a teszthez: minimális core.entity/team/competition/season/match insert a meglévő sémán — a Task 3 mintájára, külön `seed_match` helperrel.)

- [ ] **Step 2: Futtatás — bukás.** Run: `cd rust && cargo test -p football-worker --test scheduler -- --test-threads=1` → FAIL.

- [ ] **Step 3: Implementáció**

`scheduler.rs`: a fenti logika SQLx-szel; a kickoff-ablak lekérés:

```sql
SELECT m.id, m.kickoff_at, c.priority
FROM core.match m JOIN core.competition c ON c.id = m.competition_id
WHERE m.kickoff_at BETWEEN $1 AND $2
```

Interval time_window: napi `now.date_naive().to_string()`; heti `now.iso_week()` számítás chrono-val. A `insert_job` ON CONFLICT DO NOTHING miatt a generálás idempotens; a visszatérési érték a beszúrt sorok száma (`rows_affected`).

- [ ] **Step 4: Futtatás — zöld.** Run: `cd rust && cargo test -p football-worker --test scheduler -- --test-threads=1` → PASS.

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 10: Tick orchestráció + E2E replay + shadow

**Files:**
- Create: `rust/crates/worker/src/tick.rs`, `rust/crates/worker/src/coverage.rs`
- Modify: `rust/crates/worker/src/lib.rs`, `rust/crates/worker/Cargo.toml` (football-verification dep)
- Test: `rust/crates/worker/tests/tick_e2e.rs`

**Interfaces:**
- Consumes: minden korábbi task.
- Produces:
  - `tick::tick(pool, store: &ObjectStore, fetcher: &dyn Fetcher, adapters: &HashMap<String, Box<dyn SourceAdapter>>, worker_id: Uuid, now) -> Result<TickOutcome>`
  - `TickOutcome { processed: usize, blocked: usize, failed: usize }`
  - `coverage::set(pool, slug, entity_type, data_kind, time_window, CoverageState, job_id)` (repo wrapper)

**A tick lépései (spec §3.1 szerint, sorrendben):**
1. `requeue_expired_leases`, `claim_due_jobs` (lease 60 s, limit 10).
2. Jobonként: `get_policy` (hiányzik/disabled → complete cancelled); `robots::check` → Blocked → ledger `robots_blocked` + coverage `blocked` + complete failed; `quota::guard` → QuotaBlocked/DelayBlocked → ledger `quota_blocked` + complete pending-re (job marad leased, next tick újrapróbálja: complete nélkül lease jár le); `breaker::allow` → false → ledger `breaker_open` + complete failed.
3. **Reserve → run → log**: a fetch előtt `insert_ledger` beszúr egy sort `outcome='transport_error'` placeholderrel (ez már számít a kvótába — a reserve pillanatában elszámolt kísérlet; ha a folyamat megszakad, a félbehagyott kísérlet jogosan marad számlálva). A tick végén `update_ledger_outcome(pool, id, outcome, http_status, byte_size, latency_ms, artifact_hash)` frissíti a sort a végleges eredménnyel. Új repo-függvény: `update_ledger_outcome` — ezzel a Task 3 repo bővül, külön teszttel. A robots/kvóta/breaker blokkok saját, nem számláló ledger-sort kapnak (`robots_blocked`/`quota_blocked`/`breaker_open`), a reserve előtt.
4. `adapter.plan(job)` → első `FetchRequest`; `fetcher.fetch` limitekkel a policy-ből (`max_bytes`, 30 s).
5. L0 + tárolás: `store.put_json(&raw.bytes)` → `raw.artifact` upsert (content_sha256 unique) + `raw.capture` insert (request_key unique, forrás upsert a meglévő `upsert_source` mintán) — a capture/observation SQL a fixture.rs-ből ismert; a tick saját `persist_raw` helpere ugyanezt az SQL-t használja.
6. `adapter.parse` → ParseError → coverage `quarantined` + ledger `parse_error` + complete failed (nyers megmarad).
7. Observationök: `observe::insert_observation` minden Observation-re.
8. **Shadow ág**: `policy.mode == Shadow` → coverage `done` + ledger `success` + complete done, kanonikus írás NINCS.
9. **Live/canary ág**: `serde_json::from_slice::<FixtureSource>(&raw.bytes)` → `Canonicalizer::materialize` → meccsenként verification + gate (a footballctl IngestFixture ágával azonos hívások: `load_material_values`, `VerificationEngine::run`, `persist_report`, `local_publish_date`, `evaluate_and_persist_gate`; a `home_identity_exact`/`away_identity_exact` a DemoAdapter egzakt ID-i miatt true — S2b-ben ez a fail-closed linkerre cserélődik).
10. Tick-zárás: ledger `success` + coverage `done` (vagy `no_data`, ha a parse 0 observation-t adott — DemoAdapter-nél az üres lista decoy, így a no_data ág a szintetikus forrásnál nem áll elő, de a keret kezeli) + breaker `on_success` + complete done. HTTP 401/403 → `on_auth_failure`; egyéb fetch-hiba → `on_failure` + ledger outcome.

- [ ] **Step 1: Bukó E2E teszt**

`rust/crates/worker/tests/tick_e2e.rs`:

```rust
#[tokio::test]
async fn e2e_replay_publishes_verified_match() {
    let pool = test_pool().await;
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    // policy: mode=Live, transport=Fixture, events: discovery interval 1440
    // fixture: a Task 6 demo_day.json másolata a FixtureFetcher mapbe
    //   request_key = "demo-synthetic:match:<mai UTC dátum>" — a fixture kickoff dátumát
    //   a teszt a mai napra generálja (a Task 6 fixture rögzített 2026-11-12-i dátumát
    //   a teszt-helper átírja a mai napra, hogy a dátum-guard zöld legyen)
    // adapter registry: {"demo-synthetic": Box::new(DemoAdapter)}
    scheduler::generate_jobs(&pool, &[policy], now).await.unwrap();
    let outcome = tick::tick(&pool, &store, &fetcher, &adapters, worker_id, now).await.unwrap();
    assert_eq!(outcome.processed, 1);
    let gates: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.publish_gate WHERE state='verified_publish'")
        .fetch_one(&pool).await.unwrap();
    assert!(gates >= 1);
    // /v1/day szintű ellenőrzés: fetch_day a mai napra ad sort
    let rows = football_storage::repositories::day::fetch_day(&pool, &DayFilter {
        date: now.date_naive(), timezone: "Europe/Budapest".into(), include_all: false,
    }).await.unwrap();
    assert!(!rows.is_empty());
}

#[tokio::test]
async fn shadow_mode_writes_no_canonical() {
    // ugyanaz, de mode=Shadow
    // assert: core.match count = 0, stage.observation count > 0, coverage done
}

#[tokio::test]
async fn decoy_payload_quarantined_raw_kept() {
    // fixture: demo_day_decoy.json
    // assert: coverage_slot state='quarantined', raw.artifact count=1, core.match=0,
    //         ledger outcome='parse_error', job status='failed'
}
```

- [ ] **Step 2: Futtatás — bukás.** Run: `cd rust && cargo test -p football-worker --test tick_e2e -- --test-threads=1` → FAIL.

- [ ] **Step 3: Implementáció** — a fenti 10 lépés `tick.rs`-ben, `coverage.rs` wrapper; `update_ledger_outcome` a storage repo-ban + külön repo-teszt.

- [ ] **Step 4: Futtatás — zöld + teljes regresszió**

Run: `cd rust && cargo test --workspace -- --test-threads=1 && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: minden zöld, a meglévő 21+ teszt változatlanul PASS.

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 11: footballd worker loop + footballctl worker/doctor

**Files:**
- Create: `rust/crates/worker/src/runner.rs`
- Modify: `rust/bins/footballd/src/main.rs`, `rust/bins/footballd/Cargo.toml`, `rust/bins/footballctl/src/main.rs`, `rust/bins/footballctl/Cargo.toml`
- Test: `rust/crates/worker/tests/worker_loop.rs`

**Interfaces:**
- Consumes: Task 10 tick.
- Produces:
  - `runner::run_loop(pool, store, fetcher: Box<dyn Fetcher>, adapters, worker_id, tick_interval: Duration, shutdown: tokio::sync::watch::Receiver<bool>)` — ciklus: `tokio::select!` a shutdown ágon kilép, a tick ágon futtat; kilépéskor `release_worker_leases`. (A modul neve `runner`, mert a `loop` foglalt szó.)
  - footballd: `WORKER_ENABLED=true` esetén spawn; `WORKER_TICK_SECS` (default 60); graceful shutdown: `axum::serve(...).with_graceful_shutdown(ctrl_c)` + watch küldés.
  - footballctl: `Worker { #[command(subcommand)] action: WorkerAction }`, `WorkerAction::{Status, Tick}`; a Tick a `DATA_DIR/worker-fixtures` könyvtárból épít FixtureFetcher-t (`from_dir`), DemoAdapter registry; doctor: `db=1 gates=N jobs_pending=N breakers_open=N quarantined=N`.

- [ ] **Step 1: Bukó loop-teszt**

```rust
#[tokio::test]
async fn loop_stops_on_shutdown_and_releases_leases() {
    let pool = test_pool().await;
    // seed policy + job; a loop indul, egy tick lefut, majd shutdown
    let (tx, rx) = tokio::sync::watch::channel(false);
    let handle = tokio::spawn(run_loop(pool.clone(), store, fetcher, adapters, worker_id,
        Duration::from_millis(50), rx));
    tokio::time::sleep(Duration::from_millis(200)).await;
    tx.send(true).unwrap();
    handle.await.unwrap();
    let leased: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.job WHERE status='leased'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(leased, 0);
}
```

- [ ] **Step 2: Futtatás — bukás.** Run: `cd rust && cargo test -p football-worker --test worker_loop -- --test-threads=1` → FAIL.

- [ ] **Step 3: Implementáció** — `runner.rs` + footballd/footballctl módosítások. A footballd worker-spawn vázlata:

```rust
let worker_enabled = std::env::var("WORKER_ENABLED").map(|v| v == "true").unwrap_or(false);
let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
let worker_handle = if worker_enabled {
    let pool = pool.clone();
    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".into());
    let store = ObjectStore::new(&data_dir)?;
    let fixture_dir = std::path::Path::new(&data_dir).join("worker-fixtures");
    let fetcher = Box::new(FixtureFetcher::from_dir(&fixture_dir)?);
    let mut adapters: std::collections::HashMap<String, Box<dyn SourceAdapter>> = Default::default();
    adapters.insert("demo-synthetic".into(), Box::new(DemoAdapter));
    let tick_secs: u64 = std::env::var("WORKER_TICK_SECS").ok()
        .and_then(|v| v.parse().ok()).unwrap_or(60);
    Some(tokio::spawn(football_worker::runner::run_loop(
        pool, store, fetcher, adapters, uuid::Uuid::now_v7(),
        std::time::Duration::from_secs(tick_secs), shutdown_rx)))
} else { None };
// axum::serve(...).with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; })
// serve után: shutdown_tx.send(true); worker_handle.await;
```

- [ ] **Step 4: Futtatás — zöld + kézi ellenőrzés**

Run: `cd rust && cargo test --workspace -- --test-threads=1 && cargo clippy --workspace --all-targets -- -D warnings`
Kézi (dev DB, külön engedéllyel): `WORKER_ENABLED=true footballd` indítás → log „worker elindult”; `footballctl worker status` → policy/kvóta/breaker sorok; `footballctl worker tick` → TickOutcome; `footballctl doctor` → új mezők; Ctrl-C → lease-ek elengedve (SQL-lel ellenőrizve). A kézi kör eredménye a checkpointba kerül.

- [ ] **Step 5: STOP — jelentés.** Commit NEM készül.

---

### Task 12: Záró ellenőrzés + üzemeltetési útmutató + checkpoint

**Files:**
- Create: `docs/operations/worker.md`
- Modify: `docs/checkpoints/2026-11-12/checkpoint.md` (append-only záró szekció)

- [ ] **Step 1: Üzemeltetési útmutató**

`docs/operations/worker.md` tartalma: be/kikapcsolás (`WORKER_ENABLED`, `WORKER_TICK_SECS`), fixture-könyvtár (`DATA_DIR/worker-fixtures`, request-key = fájlnév törzse), `footballctl worker status/tick`, karantén-repars menete (új parser-verzió → job újra pending), breaker policy-review teendő, seed SQL a katalógus-forrásokhoz (**enabled=false, mode=shadow** — élesítés S3):

```sql
INSERT INTO ops.source_policy (source_slug, transport, mode, daily_budget,
  min_delay_ms, jitter_ms, breaker_threshold, breaker_cooldown_secs,
  adapter_version, parser_version, domains, events, enabled, max_bytes)
VALUES
 ('footballdata','rust_http','shadow',10,2000,500,3,300,'0.0','0.0','["match","odds","standing"]','[]',false,5242880),
 ('tipplap','rust_http','shadow',10,2000,500,3,300,'0.0','0.0','["odds"]','[]',false,5242880),
 ('fotmob','rust_http','shadow',10,2000,500,3,300,'0.0','0.0','["match"]','[]',false,5242880),
 ('openfoot','rust_http','shadow',166,2000,500,3,300,'0.0','0.0','["match"]','[]',false,5242880)
ON CONFLICT (source_slug) DO NOTHING;
```

- [ ] **Step 2: Záró ellenőrzések**

Run: `./scripts/check.sh`
Expected: reset + fmt + clippy + szekvenciális workspace-tesztek zöld.
Run: `./scripts/db-reset.sh && ./scripts/db-check.sh`
Expected: tesztDB 0 üzleti sor; az új táblaszámok rögzítve.
Run: `cd rust && cargo test --workspace -- --test-threads=1`
Expected: minden teszt PASS (a pontos szám a checkpointba).
Dev DB migráció: `footballctl migrate` — csak külön felhasználói engedéllyel; a demó pipeline ellenőrzése: `footballctl day --date 2026-11-11` → 20 meccs verified (változatlan).

- [ ] **Step 3: Magyar visszaolvasás + glyph-scan** — minden új/módosult dokumentumon (operations, checkpoint); záró újsor.

- [ ] **Step 4: Checkpoint záró szekció** — append-only: futtatott parancsok + kimenetek, ami ellenőrizetlen maradt (élő adapter nincs — S3), visszaállítási út.

- [ ] **Step 5: STOP — zárójelentés + commit-kérés**

Teljes összesített zárójelentés (AGENTS §10). Commit csak a felhasználó kifejezett kérésére; a review-csomag a munkafa-diff.

---

## Önreview-jegyzet (a terv írójának)

- **Spec-lefedettség:** §2 architektúra → Task 1/3/4/5/6/7; §3 adatfolyam → Task 9/10; §4 adatmodell → Task 2/3; §5 őrök → Task 5/6/7/8/10; §6 tesztek → minden task + Task 12; §7 DoD → Task 12; shadow/canary/live → Task 10 (canary élesítés S3, a mód mező és ág kész).
- **Típus-konzisztencia:** a `FetchJob`/`SourcePolicy` mezőnevek minden taskban azonosak; a repo-függvénynevek a Task 3 interfész-blokkjából jönnek; a Task 10 `update_ledger_outcome` bővítés külön teszttel szerepel.
- **Ismert egyszerűsítések (dokumentált):** a DemoAdapter AET/pens állást fail-closed karanténba tesz (explicit ET/pens mezőkezelés S3); a robots-kapu FixtureFetcher-módban a hiányzó robots-fájlt `no_robots`-ként kezeli; a canary mód élesítési folyamata S3.
