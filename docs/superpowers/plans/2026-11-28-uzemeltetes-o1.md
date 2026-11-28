# O1 — Worker singleton, lease fencing és atomi kvóta Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A daemon és a kézi tick egyetlen adatbázis-szintű singleton-körben fut, minden jobclaim saját fencing tokent kap, a lejárt worker nem módosíthat újabb lease-t, a kvóta/min-delay/reservation atomi, a breaker-átmenet pedig zárolt.

**Architecture:** A `storage` réteg valósítja meg a per-job lease predikátumokat, a per-source `FOR UPDATE` tranzakciót és a breaker sorzárát. A `worker` egy közös, poolon kívüli PostgreSQL kapcsolaton tartott session advisory lockkal védi a policy-listázást, job-generálást és ticket; daemon és CLI ugyanazt a belépést használja. A fetch idejére külön tokio heartbeat-task tartja életben a saját lease-t, minden véglegesítés pedig a lease tokent és a még aktív lease-t is ellenőrzi.

**Tech Stack:** Rust 2024, Tokio, PostgreSQL, sqlx 0.8.6, chrono, UUIDv7, a meglévő `football-domain`, `football-storage` és `football-worker` crate-ek; új külső függőség nélkül.

**Spec:** `docs/superpowers/specs/2026-11-27-uzemeltetes-triage-console-design.md`, különösen §8.2, §13.1, §14 és §15.

## Global Constraints

- A munka kizárólag a `feat/ops-o1-worker-fencing` branch izolált worktreeában történik; a fő checkout és a `scripts/corrections-2026-11-27.sql` nem érintett.
- Minden szöveg magyar; kódazonosítók angolok. A Write/Edit után vissza kell olvasni a módosított magyar szöveget, és CJK/replacement karakter nem maradhat.
- A `domain` tiszta marad: adatbázis- vagy HTTP-detail nem kerül bele.
- Adatbázis-hozzáférés csak `storage` rétegből történik; az API-handler és a worker nem ír közvetlen SQL-t, kivéve a már meglévő, jól körülhatárolt storage-tesztek.
- Production kérésfeldolgozásban nincs `unwrap()`, `expect()` és csendben elnyelt hiba.
- A tesztadatbázis `football_v2_test`; a `football_v2_dev` és a futó daemon nem migrálható, nem indítható újra és nem cserélhető ebben a szeletben.
- A migráció tranzakciós és nem töröl adatot. A 0033 sémája a kód visszaállítása után használatlanul maradhat.
- A valódi forrásadatok, robots szabályok, rate limit és forrásvédelem nem kerülhető meg.
- Minden időpont UTC; a lease érvényessége adatbázis `clock_timestamp()` időponthoz képest értendő.
- A workspace-tesztek `--test-threads=1`-gyel futnak, mert közös teszt-adatbázist használnak; a race-teszteken belül a tokio feladatok valóban párhuzamosak.
- A végleges kaput a kódot nem író, független ellenőr is lefuttatja; builder ≠ verifier.
- Zöld feladatonként commit készül; push, FF merge és branch-törlés csak a végső független zöld verdict után történik.

## File Structure

- `db/migrations/0033_worker_fencing.sql` — egyetlen additív migráció: job lease token, token index/constraint és `reserved` ledger outcome.
- `rust/crates/domain/src/policy.rs` — `JobLease`, `LeasedJob`, `FetchOutcome::Reserved`; a meglévő `FetchJob` és adapter API változatlan.
- `rust/crates/storage/src/repositories/worker.rs` — fenced claim/heartbeat/requeue/completion/coverage, atomi fetch reservation, zárolt breaker átmenet.
- `rust/crates/storage/tests/worker_repos.rs` — valódi PostgreSQL lease/fencing/reservation repository tesztek.
- `rust/crates/storage/tests/worker_schema.rs` — 0033 oszlop-, index- és outcome-constraint ellenőrzése.
- `rust/crates/worker/src/quota.rs` — a reservation döntés leképezése; a két külön guard-lekérdezés megszűnik.
- `rust/crates/worker/src/breaker.rs` — a tiszta állapotgép storage sorzárán belüli használata.
- `rust/crates/worker/src/coverage.rs` — coverage-írás saját lease tokennel.
- `rust/crates/worker/src/scheduler.rs` — `LeasedJob` claim és fenced completion/requeue wrapper.
- `rust/crates/worker/src/lease.rs` — új, egyetlen felelősségű heartbeat-loop; a production tick és a rövid időtartamú integrációs teszt is ezt használja.
- `rust/crates/worker/src/runner.rs` — poolon kívüli session advisory singleton és közös `run_cycle`; nincs külön singleton-modul.
- `rust/crates/worker/src/tick.rs` — lease context, heartbeat, fenced véglegesítés, reservation és breaker-kapu sorrendje.
- `rust/crates/worker/src/lib.rs` — `lease` modul regisztrálása.
- `rust/crates/worker/tests/quota_breaker.rs` — valódi párhuzamos kvóta/min-delay/breaker tesztek.
- `rust/crates/worker/tests/worker_loop.rs` — singleton busy/available viselkedés és heartbeat életciklus.
- `rust/crates/worker/tests/tick_e2e.rs` — a `LeasedJob` signaturehez igazítás és fenced viselkedés E2E ellenőrzése.
- `rust/bins/footballd/src/main.rs` — a daemon a közös lockolt `run_cycle`-t hívja.
- `rust/bins/footballctl/src/main.rs` — a manual tick ugyanazt a lockolt `run_cycle`-t hívja; busy esetén nem fut tick.
- `docs/checkpoints/2026-11-28/checkpoint.md` — O1 induló állapot, piros/zöld bizonyítékok, kapuk, review, kockázatok és visszaállás.

## Interface Map

A későbbi feladatok csak az alábbi neveket és típusokat használják:

```rust
// football_domain::policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobLease {
    pub job_id: Uuid,
    pub token: Uuid,
    pub leased_until: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LeasedJob {
    pub job: FetchJob,
    pub lease: JobLease,
}

// football_storage::repositories::worker
pub async fn claim_due_jobs(
    pool: &PgPool,
    worker_id: Uuid,
    now: DateTime<Utc>,
    lease_secs: i64,
    limit: i64,
) -> Result<Vec<LeasedJob>>;

pub async fn heartbeat_job(
    pool: &PgPool,
    lease: &JobLease,
    lease_secs: i64,
) -> Result<bool>;

pub async fn requeue_leased(
    pool: &PgPool,
    lease: &JobLease,
    due_at: DateTime<Utc>,
) -> Result<bool>;

pub async fn complete_leased(
    pool: &PgPool,
    lease: &JobLease,
    status: JobStatus,
    last_error: Option<&str>,
) -> Result<bool>;

pub async fn upsert_job_coverage(
    pool: &PgPool,
    lease: &JobLease,
    entity_type: &str,
    data_kind: &str,
    time_window: &str,
    state: CoverageState,
) -> Result<bool>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchReservation {
    Reserved(Uuid),
    QuotaBlocked,
    DelayBlocked,
    LeaseLost,
}

pub async fn reserve_fetch(
    pool: &PgPool,
    source_slug: &str,
    request_key: &str,
    job_id: Uuid,
    lease_token: Uuid,
    now: DateTime<Utc>,
) -> Result<FetchReservation>;

#[derive(Debug)]
pub enum BreakerWrite {
    Keep,
    Set(BreakerRow),
}

pub async fn update_breaker_locked<T, F>(
    pool: &PgPool,
    source_slug: &str,
    update: F,
) -> Result<T>
where
    F: FnOnce(Option<BreakerRow>) -> (T, BreakerWrite) + Send,
    T: Send;

// football_worker
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaDecision {
    Allowed(Uuid),
    QuotaBlocked,
    DelayBlocked,
    LeaseLost,
}

pub async fn reserve(
    pool: &PgPool,
    source_slug: &str,
    request_key: &str,
    lease: &JobLease,
    now: DateTime<Utc>,
) -> Result<QuotaDecision>;

pub async fn heartbeat_loop(
    pool: PgPool,
    lease: JobLease,
    interval: Duration,
    lease_secs: i64,
    mut shutdown: watch::Receiver<bool>,
) -> bool;

#[derive(Debug, Clone, Copy)]
pub struct CycleOutcome {
    pub generated: usize,
    pub processed: usize,
    pub blocked: usize,
    pub failed: usize,
    pub deferred: usize,
}

pub async fn run_cycle(
    pool: &PgPool,
    store: &ObjectStore,
    fetchers: &Fetchers,
    adapters: &HashMap<String, Box<dyn SourceAdapter>>,
    worker_id: Uuid,
) -> Result<Option<CycleOutcome>>;
```

`Option::None` a singleton foglaltsága, nem hiba. A `bool` végeredmény mindig azt jelenti, hogy a fenced módosítás ténylegesen érintett egy saját, aktív lease-t.

---

### Task 1: Lease token, fencing és coverage repository

**Files:**
- Create: `db/migrations/0033_worker_fencing.sql`
- Modify: `rust/crates/domain/src/policy.rs`
- Modify: `rust/crates/storage/src/repositories/worker.rs`
- Modify: `rust/crates/storage/tests/worker_repos.rs`
- Modify: `rust/crates/storage/tests/worker_schema.rs`
- Modify: `rust/crates/worker/src/scheduler.rs`

**Interfaces:**
- Consumes: a meglévő `FetchJob`, `JobStatus`, `CoverageState`, `BreakerRow`, policy és ledger repository API.
- Produces: `JobLease`, `LeasedJob`, fenced claim/heartbeat/requeue/completion/coverage; `FetchOutcome::Reserved` a Task 2 számára.

- [ ] **Step 1: Írd meg a fenced repository regressziókat**

Add hozzá `worker_repos.rs`-hez egy valódi PostgreSQL tesztet, amely először lejáratja a régi lease-t, új tokennel claimol, majd a régi token mindhárom írási útját kipróbálja:

```rust
#[tokio::test]
async fn stale_lease_cannot_complete_requeue_or_write_coverage() {
    let pool = test_pool().await;
    let policy = SourcePolicy {
        source_slug: "demo-synthetic".into(),
        transport: Transport::Fixture,
        mode: SourceMode::Shadow,
        daily_budget: 10,
        min_delay_ms: 0,
        jitter_ms: 0,
        breaker_threshold: 3,
        breaker_cooldown_secs: 60,
        adapter_version: "1.0".into(),
        parser_version: "1.0".into(),
        domains: vec!["match".into()],
        events: vec![],
        enabled: true,
        max_bytes: 5_242_880,
    };
    repo::upsert_policy(&pool, &policy).await.unwrap();
    repo::insert_job(&pool, &repo::NewJob {
        source_slug: policy.source_slug.clone(),
        domain: "match".into(),
        entity_key: "stale".into(),
        event_type: "discovery".into(),
        time_window: "2026-11-28".into(),
        due_at: Utc::now() - Duration::seconds(1),
        priority: 0,
    }).await.unwrap();

    let stale = repo::claim_due_jobs(&pool, Uuid::now_v7(), Utc::now(), 60, 1)
        .await.unwrap().remove(0);
    assert!(!stale.lease.token.is_nil());
    assert!(stale.lease.leased_until > Utc::now());

    sqlx::query("UPDATE ops.job SET leased_until = clock_timestamp() - interval '1 second' WHERE id = $1")
        .bind(stale.job.id).execute(&pool).await.unwrap();
    repo::requeue_expired_leases(&pool).await.unwrap();
    let current = repo::claim_due_jobs(&pool, Uuid::now_v7(), Utc::now(), 60, 1)
        .await.unwrap().remove(0);
    assert_ne!(stale.lease.token, current.lease.token);

    assert!(!repo::complete_leased(&pool, &stale.lease, JobStatus::Done, None)
        .await.unwrap());
    assert!(!repo::requeue_leased(&pool, &stale.lease, Utc::now())
        .await.unwrap());
    assert!(!repo::upsert_job_coverage(
        &pool, &stale.lease, "match", "discovery", "2026-11-28", CoverageState::Done,
    ).await.unwrap());

    let (status, token): (String, Uuid) = sqlx::query_as(
        "SELECT status, lease_token FROM ops.job WHERE id = $1",
    ).bind(current.job.id).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "leased");
    assert_eq!(token, current.lease.token);
    let coverage: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.coverage_slot WHERE last_job_id = $1",
    ).bind(current.job.id).fetch_one(&pool).await.unwrap();
    assert_eq!(coverage, 0);

    assert!(repo::complete_leased(&pool, &current.lease, JobStatus::Done, None)
        .await.unwrap());
}
```

A kipróbált production-változtatás: a token és az aktív lease predikátum eltávolítása bármelyik fenced SQL-ből a teszt pirosítását kell hogy okozza.

- [ ] **Step 2: Írd meg a heartbeat és token-egyediség teszteket**

```rust
#[tokio::test]
async fn heartbeat_extends_only_a_live_owned_lease() {
    let pool = test_pool().await;
    let policy = SourcePolicy {
        source_slug: "demo-synthetic".into(),
        transport: Transport::Fixture,
        mode: SourceMode::Shadow,
        daily_budget: 10,
        min_delay_ms: 0,
        jitter_ms: 0,
        breaker_threshold: 3,
        breaker_cooldown_secs: 60,
        adapter_version: "1.0".into(),
        parser_version: "1.0".into(),
        domains: vec!["match".into()],
        events: vec![],
        enabled: true,
        max_bytes: 5_242_880,
    };
    repo::upsert_policy(&pool, &policy).await.unwrap();
    repo::insert_job(&pool, &repo::NewJob {
        source_slug: policy.source_slug.clone(),
        domain: "match".into(),
        entity_key: "heartbeat".into(),
        event_type: "discovery".into(),
        time_window: "2026-11-28".into(),
        due_at: Utc::now() - Duration::seconds(1),
        priority: 0,
    }).await.unwrap();
    let claimed = repo::claim_due_jobs(&pool, Uuid::now_v7(), Utc::now(), 1, 1)
        .await.unwrap().remove(0);

    assert!(repo::heartbeat_job(&pool, &claimed.lease, 30).await.unwrap());
    let renewed: DateTime<Utc> = sqlx::query_scalar(
        "SELECT leased_until FROM ops.job WHERE id = $1",
    ).bind(claimed.job.id).fetch_one(&pool).await.unwrap();
    assert!(renewed > claimed.lease.leased_until);

    sqlx::query("UPDATE ops.job SET leased_until = clock_timestamp() - interval '1 second' WHERE id = $1")
        .bind(claimed.job.id).execute(&pool).await.unwrap();
    assert!(!repo::heartbeat_job(&pool, &claimed.lease, 30).await.unwrap());
}
```

A claim tesztben két külön egymás után claimolt job tokenjét hasonlítsd össze, és állítsd `assert_ne!`-re. Ez a zárolt/claimelt token generálás hibáját is elkapja.

- [ ] **Step 3: Futtasd a piros tesztet**

Futtasd:

```bash
cd "/Users/susleczandras/Documents/Code/football-app-v2-ops-o1-worker-fencing/rust"
cargo test -p football-storage --test worker_repos stale_lease_cannot_complete_requeue_or_write_coverage -- --exact --nocapture
```

Várt piros: exit `101`; a még nem létező `JobLease`/`LeasedJob` és fenced repository API hiányzik. Ha már létezik az API, a tesztnek a régi, token nélküli SQL tényleges elfogadására kell buknia, nem fordítási typo-ra.

- [ ] **Step 4: Add hozzá a 0033 migrációt**

Hozd létre `db/migrations/0033_worker_fencing.sql` fájlt:

```sql
BEGIN;

ALTER TABLE ops.job
    ADD COLUMN IF NOT EXISTS lease_token uuid;

CREATE UNIQUE INDEX IF NOT EXISTS idx_job_lease_token
    ON ops.job (lease_token)
    WHERE lease_token IS NOT NULL;

ALTER TABLE ops.job
    DROP CONSTRAINT IF EXISTS job_lease_token_status_check;
ALTER TABLE ops.job
    ADD CONSTRAINT job_lease_token_status_check
    CHECK (lease_token IS NULL OR status = 'leased');

ALTER TABLE ops.fetch_ledger
    DROP CONSTRAINT IF EXISTS fetch_ledger_outcome_check;
ALTER TABLE ops.fetch_ledger
    ADD CONSTRAINT fetch_ledger_outcome_check
    CHECK (outcome IN (
        'success', 'http_error', 'timeout', 'oversize', 'robots_blocked',
        'quota_blocked', 'breaker_open', 'transport_error', 'parse_error',
        'reserved'
    ));

COMMIT;
```

A constraint csak a nem-null token státuszát köti; a migráció előtti, token nélküli `leased` sorokat nem írja át és nem törli.

- [ ] **Step 5: Add hozzá a domain típusokat és a `reserved` outcome-ot**

`policy.rs`-ben a `FetchJob` mellé írd:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobLease {
    pub job_id: Uuid,
    pub token: Uuid,
    pub leased_until: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LeasedJob {
    pub job: FetchJob,
    pub lease: JobLease,
}
```

A `FetchOutcome` enumhoz add:

```rust
Reserved,
```

Ne változtasd a `FetchJob` mezőit vagy a `SourceAdapter` aláírását.

- [ ] **Step 6: Valósítsd meg a fenced storage műveleteket**

`worker.rs`-ben a claim SQL legyen:

```sql
UPDATE ops.job
SET status = 'leased',
    worker_id = $2,
    leased_until = $1 + make_interval(secs => $3),
    lease_token = gen_random_uuid(),
    attempts = attempts + 1
WHERE id IN (
    SELECT id FROM ops.job
    WHERE status = 'pending' AND due_at <= $1
    ORDER BY priority, due_at
    FOR UPDATE SKIP LOCKED
    LIMIT $4
)
RETURNING id, source_slug, domain, entity_key, event_type, time_window,
          due_at, priority, lease_token, leased_until
```

A `RETURNING lease_token` és `leased_until` alkotja `JobLease` értéket; `gen_random_uuid()` minden sorhoz más tokent ad.

A heartbeat SQL:

```sql
UPDATE ops.job
SET leased_until = clock_timestamp() + make_interval(secs => $3)
WHERE id = $1
  AND lease_token = $2
  AND status = 'leased'
  AND leased_until > clock_timestamp()
```

Az új `requeue_leased` és `complete_leased` predikátuma mindkét esetben:

```sql
WHERE id = $1
  AND lease_token = $2
  AND status = 'leased'
  AND leased_until > clock_timestamp()
```

Az új fenced műveletek a lezáráskor `lease_token = NULL`, `worker_id = NULL`, `leased_until = NULL`; `requeue_leased` a `due_at`-ot is beállítja. A meglévő, régi `requeue_with_delay` és `complete_job` a Task 4 eltávolításáig változatlanul megmarad, hogy a worker addig is buildelhessen. A `requeue_expired_leases` szintén törölje a tokent. A `release_worker_leases` a worker_id alapján törölje a tokent is.

A coverage írás egyetlen SQL-ben zárja le a job sort, majd csak aktív saját lease mellett upsertol:

```sql
WITH leased AS (
    SELECT id FROM ops.job
    WHERE id = $7
      AND lease_token = $8
      AND status = 'leased'
      AND leased_until > clock_timestamp()
    FOR UPDATE
)
INSERT INTO ops.coverage_slot
    (source_slug, entity_type, data_kind, time_window, state, last_job_id, updated_at)
SELECT $1, $2, $3, $4, $5, $7, clock_timestamp()
FROM leased
ON CONFLICT (source_slug, entity_type, data_kind, time_window)
DO UPDATE SET state = EXCLUDED.state,
              last_job_id = EXCLUDED.last_job_id,
              updated_at = EXCLUDED.updated_at
```

A `fetch_one`/`fetch_optional` érintett sorai adják a `bool` eredményt.

- [ ] **Step 7: Igazítsd a repository teszteket, de tartsd buildelhetővé a worker réteget**

A storage `claim_due_jobs` eredménye `Vec<LeasedJob>`. A scheduler `claim` wrapper ebben a feladatban átalakítja vissza `Vec<FetchJob>`-re, hogy a meglévő tick addig is buildelhessen:

```rust
let leased = repo::claim_due_jobs(pool, worker_id, now, lease_secs, limit).await?;
Ok(leased.into_iter().map(|item| item.job).collect())
```

A meglévő storage tesztekben `claimed[0].id` helyett `claimed[0].job.id`, a lease pedig `claimed[0].lease`. Az új fenced repository tesztek `complete_leased` és `requeue_leased` hívásokat használnak. A régi completion/requeue/coverage API-t és a tick régi hívásait a Task 4 távolítja el; Task 1 nem hagyhatja el a workspace buildjét.

- [ ] **Step 8: Futasd zöldre a storage és schema teszteket**

```bash
cd "/Users/susleczandras/Documents/Code/football-app-v2-ops-o1-worker-fencing"
./scripts/db-reset.sh
cd rust
cargo test -p football-storage --test worker_repos -- --test-threads=1
cargo test -p football-storage --test worker_schema -- --test-threads=1
```

Várt: mindkét parancs exit `0`; a stale lease teszt `1 passed / 0 failed`.

- [ ] **Step 9: Commitold meg a zöld repository réteget**

```bash
cd "/Users/susleczandras/Documents/Code/football-app-v2-ops-o1-worker-fencing"
git add db/migrations/0033_worker_fencing.sql \
  rust/crates/domain/src/policy.rs \
  rust/crates/storage/src/repositories/worker.rs \
  rust/crates/storage/tests/worker_repos.rs \
  rust/crates/storage/tests/worker_schema.rs \
  rust/crates/worker/src/scheduler.rs
git commit -m "feat(O1): lease token és fenced job véglegesítés"
git push -u origin feat/ops-o1-worker-fencing
```

---

### Task 2: Atomikus kvóta, min-delay és ledger reservation

**Files:**
- Modify: `rust/crates/storage/src/repositories/worker.rs`
- Modify: `rust/crates/worker/src/quota.rs`
- Modify: `rust/crates/worker/tests/quota_breaker.rs`
- Modify: `rust/crates/storage/tests/worker_schema.rs`

**Interfaces:**
- Consumes: `JobLease`, `FetchOutcome::Reserved`, a 0033 `reserved` constraint és a Task 1 claim API.
- Produces: `FetchReservation`, `reserve_fetch`, `quota::reserve`, `QuotaDecision::Allowed(Uuid)`.

- [ ] **Step 1: Írd meg a párhuzamos napi budget regressziót**

`quota_breaker.rs`-hez add a valódi `Barrier`-alapú tesztet. Minden feladatot ugyanaz az aktív lease és más request key képvisel, hogy a per-source lock a tényleges versenyhelyzetet kényszerítse:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reservations_never_exceed_daily_budget() {
    let pool = test_pool().await;
    seed_policy(&pool, 3, 0).await;
    let policy = repo::get_policy(&pool, "demo-synthetic").await.unwrap().unwrap();
    repo::insert_job(&pool, &repo::NewJob {
        source_slug: policy.source_slug.clone(),
        domain: "match".into(),
        entity_key: "quota-race".into(),
        event_type: "discovery".into(),
        time_window: Utc::now().date_naive().to_string(),
        due_at: Utc::now() - Duration::seconds(1),
        priority: 0,
    }).await.unwrap();
    let leased = repo::claim_due_jobs(&pool, Uuid::now_v7(), Utc::now(), 60, 1)
        .await.unwrap().remove(0);
    let now = base_time();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(12));

    let handles: Vec<_> = (0..12).map(|index| {
        let pool = pool.clone();
        let lease = leased.lease;
        let barrier = barrier.clone();
        tokio::spawn(async move {
            barrier.wait().await;
            quota::reserve(
                &pool,
                "demo-synthetic",
                &format!("race-{index}"),
                &lease,
                now,
            ).await.unwrap()
        })
    }).collect();

    let mut allowed = 0;
    let mut blocked = 0;
    for handle in handles {
        match handle.await.unwrap() {
            quota::QuotaDecision::Allowed(_) => allowed += 1,
            quota::QuotaDecision::QuotaBlocked => blocked += 1,
            other => panic!("váratlan kvótadöntés: {other:?}"),
        }
    }
    assert_eq!((allowed, blocked), (3, 9));
    let reserved: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.fetch_ledger WHERE outcome = 'reserved'",
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(reserved, 3);
}
```

A hiányzó per-source tranzakció vagy reservation számbavétele legalább 4 `Allowed` eredményt ad, ezért a teszt bizonyítottan piros.

- [ ] **Step 2: Írd meg a párhuzamos min-delay és crash-fail-closed tesztet**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reservations_obey_min_delay_and_reserved_survives_crash() {
    let pool = test_pool().await;
    seed_policy(&pool, 10, 60_000).await;
    let policy = repo::get_policy(&pool, "demo-synthetic").await.unwrap().unwrap();
    repo::insert_job(&pool, &repo::NewJob {
        source_slug: policy.source_slug.clone(),
        domain: "match".into(),
        entity_key: "delay-race".into(),
        event_type: "discovery".into(),
        time_window: Utc::now().date_naive().to_string(),
        due_at: Utc::now() - Duration::seconds(1),
        priority: 0,
    }).await.unwrap();
    let leased = repo::claim_due_jobs(&pool, Uuid::now_v7(), Utc::now(), 60, 1)
        .await.unwrap().remove(0);
    let now = base_time();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(8));
    let handles: Vec<_> = (0..8).map(|index| {
        let pool = pool.clone();
        let lease = leased.lease;
        let barrier = barrier.clone();
        tokio::spawn(async move {
            barrier.wait().await;
            quota::reserve(&pool, "demo-synthetic", &format!("delay-{index}"), &lease, now)
                .await.unwrap()
        })
    }).collect();

    let mut allowed = 0;
    let mut delayed = 0;
    for handle in handles {
        match handle.await.unwrap() {
            quota::QuotaDecision::Allowed(_) => allowed += 1,
            quota::QuotaDecision::DelayBlocked => delayed += 1,
            other => panic!("váratlan delay-döntés: {other:?}"),
        }
    }
    assert_eq!((allowed, delayed), (1, 7));

    let next = quota::reserve(
        &pool, "demo-synthetic", "after-crash", &leased.lease, now,
    ).await.unwrap();
    assert!(matches!(next, quota::QuotaDecision::DelayBlocked));
    let reserved: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.fetch_ledger WHERE outcome = 'reserved'",
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(reserved, 1);
}
```

Az előző `reserved` sort szándékosan nem finalizálja a teszt: a worker összeomlását és a fail-closed kvótavédelmet ellenőrzi.

- [ ] **Step 3: Futtasd a piros kvótateszteket**

```bash
cd "/Users/susleczandras/Documents/Code/football-app-v2-ops-o1-worker-fencing/rust"
cargo test -p football-worker --test quota_breaker concurrent_reservations -- --nocapture
```

Várt piros: exit `101`, a még hiányzó `quota::reserve` API. A race-teszteket ne módosítsd úgy, hogy a korábbi két külön `guard` lekérdezést hívdják; a végleges tesztnek magát a production reservation útvonalat kell mérnie.

- [ ] **Step 4: Valósítsd meg az atomi reservation-t storage rétegben**

`worker.rs`-ben egyetlen tranzakció:

1. `SELECT` a `ops.source_policy` sorra `FOR UPDATE`-val, és a zárolt sor aktuális `daily_budget`, `min_delay_ms` értékeit használd.
2. Ellenőrizd a job saját, aktív lease-t; ha nem aktív, `LeaseLost`.
3. Számold a mai `success`, `http_error`, `timeout`, `oversize`, `transport_error`, `parse_error` és `reserved` sorokat; `quota_blocked`, `breaker_open`, `robots_blocked` nem számít.
4. Ha a count eléri a budgetet, `QuotaBlocked`.
5. Keresd a számoló outcome-ok közül a legutóbbi `attempted_at` értéket; ha `last + min_delay > now`, `DelayBlocked`.
6. Szúrd be a `reserved` ledger sort a megadott `request_key`, `attempted_at`, `job_id` értékekkel, és add vissza a létrejött UUID-t.

A tranzakció pszeudokódja:

```rust
let mut tx = pool.begin().await?;
let policy_sql = format!(
    "SELECT {POLICY_COLUMNS} FROM ops.source_policy WHERE source_slug = $1 FOR UPDATE"
);
let policy_row = sqlx::query(&policy_sql)
    .bind(source_slug).fetch_one(&mut *tx).await?;
let policy = policy_from_row(&policy_row)?;
let lease_active = sqlx::query_scalar::<_, bool>(
    "SELECT EXISTS (SELECT 1 FROM ops.job WHERE id=$1 AND lease_token=$2 \
     AND status='leased' AND leased_until > clock_timestamp())",
).bind(job_id).bind(lease_token).fetch_one(&mut *tx).await?;
if !lease_active {
    tx.rollback().await?;
    return Ok(FetchReservation::LeaseLost);
}
// A count és last_attempt ugyanarra a tx-re futnak.
// A delay és budget döntés után szúrd be a reserved sort ugyanebben a tx-be.
tx.commit().await?;
```

A ledger beszúrás helperje fogadjon `&mut PgConnection`-t vagy `&mut Transaction`-t, hogy ne nyíljon ki új pool-kapcsolat. Az `attempted_at` a tick `now` paramétere; a lease predikátum egyedül adatbázis órát használ.

- [ ] **Step 5: Cseréld le a worker guard API-t**

`quota.rs`-ben add hozzá az új `reserve` függvényt, amely csak leképezi a storage döntést. A meglévő `guard` ebben a feladatban megmarad a még régi tick hívásához, és a storage count/last segédfüggvényeit használja; Task 4 távolítja el a guardot és a tick teljes átszerelését végzi:

```rust
match repo::reserve_fetch(pool, source_slug, request_key, lease.job_id, lease.token, now).await? {
    FetchReservation::Reserved(id) => QuotaDecision::Allowed(id),
    FetchReservation::QuotaBlocked => QuotaDecision::QuotaBlocked,
    FetchReservation::DelayBlocked => QuotaDecision::DelayBlocked,
    FetchReservation::LeaseLost => QuotaDecision::LeaseLost,
}
```

A meglévő kvótatesztek `Allowed(_)` mintára íródnak. A `quota_blocked` és `robots_blocked` nem-számláló viselkedés, valamint a határon `last + delay == now` engedélyezés megmarad.

- [ ] **Step 6: Bővítsd a schema tesztet**

A `worker_schema.rs` ellenőrizze, hogy `ops.job.lease_token` `uuid`, hogy a partial unique index létezik, és hogy egy valódi `reserved` ledger beszúrás el fog fogadni. Ne ellenőrizd csak a migráció szövegét.

- [ ] **Step 7: Futtasd zöldre a kvóta és schema teszteket**

```bash
cd "/Users/susleczandras/Documents/Code/football-app-v2-ops-o1-worker-fencing/rust"
cargo test -p football-worker --test quota_breaker -- --test-threads=1
cargo test -p football-storage --test worker_repos -- --test-threads=1
cargo test -p football-storage --test worker_schema -- --test-threads=1
```

Várt: exit `0`; a két race-teszt `2 passed / 0 failed`.

- [ ] **Step 8: Commitold meg az atomi kvótát**

```bash
cd "/Users/susleczandras/Documents/Code/football-app-v2-ops-o1-worker-fencing"
git add rust/crates/storage/src/repositories/worker.rs \
  rust/crates/worker/src/quota.rs \
  rust/crates/worker/tests/quota_breaker.rs \
  rust/crates/storage/tests/worker_schema.rs
git commit -m "feat(O1): atomi fetch reservation és fail-closed kvóta"
git push
```

---

### Task 3: Zárolt breaker-átmenetek

**Files:**
- Modify: `rust/crates/storage/src/repositories/worker.rs`
- Modify: `rust/crates/worker/src/breaker.rs`
- Modify: `rust/crates/worker/tests/quota_breaker.rs`

**Interfaces:**
- Consumes: a meglévő `BreakerState`, `BreakerRow`, `SourcePolicy`, audit író és a Task 1 schema.
- Produces: `update_breaker_locked`, `BreakerWrite` és a meglévő `breaker::allow/on_failure/on_success/on_auth_failure` API zárolt implementációja.

- [ ] **Step 1: Írd meg a párhuzamos half-open probe tesztet**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn only_one_concurrent_half_open_probe_is_allowed() {
    let pool = test_pool().await;
    let policy = seed_policy(&pool, 10, 0).await;
    let now = base_time();
    repo::update_breaker_locked(&pool, &policy.source_slug, |_| {
        ((), repo::BreakerWrite::Set(BreakerRow {
            state: BreakerState::Open,
            failure_count: 2,
            opened_at: Some(now - Duration::seconds(60)),
            next_probe_at: Some(now),
        }))
    }).await.unwrap();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(12));
    let handles: Vec<_> = (0..12).map(|_| {
        let pool = pool.clone();
        let policy = policy.clone();
        let barrier = barrier.clone();
        tokio::spawn(async move {
            barrier.wait().await;
            breaker::allow(&pool, &policy, now).await.unwrap()
        })
    }).collect();

    let mut probes = 0;
    for handle in handles {
        if handle.await.unwrap() {
            probes += 1;
        }
    }
    assert_eq!(probes, 1);
    let row = repo::get_breaker(&pool, &policy.source_slug).await.unwrap().unwrap();
    assert_eq!(row.state, BreakerState::HalfOpen);
}
```

A production-törések: a külön `get_breaker` után blind upsert, vagy az open→half_open átmenet zárolás nélküle.

- [ ] **Step 2: Írd meg a párhuzamos failure-count tesztet**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_breaker_failures_do_not_lose_updates() {
    let pool = test_pool().await;
    let policy = seed_policy(&pool, 10, 0).await;
    let now = base_time();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(10));
    let handles: Vec<_> = (0..10).map(|_| {
        let pool = pool.clone();
        let policy = policy.clone();
        let barrier = barrier.clone();
        tokio::spawn(async move {
            barrier.wait().await;
            breaker::on_failure(&pool, &policy, now).await.unwrap()
        })
    }).collect();
    for handle in handles {
        handle.await.unwrap();
    }
    let row = repo::get_breaker(&pool, &policy.source_slug).await.unwrap().unwrap();
    assert_eq!(row.failure_count, 10);
    assert_eq!(row.state, BreakerState::Open);
}
```

- [ ] **Step 3: Futtasd a piros breaker teszteket**

```bash
cd "/Users/susleczandras/Documents/Code/football-app-v2-ops-o1-worker-fencing/rust"
cargo test -p football-worker --test quota_breaker concurrent_breaker -- --nocapture
cargo test -p football-worker --test quota_breaker only_one_concurrent_half_open_probe -- --exact --nocapture
```

Várt: legalább az egyik teszt nem determinisztikus race miatt piros; a futtatást addig ismételd, amíg a régó berész eredményt látod, de fix sleep-del nem használhatsz.

- [ ] **Step 4: Valósítsd meg a storage sorzárat**

`update_breaker_locked` tranzakcióban:

1. `INSERT ... ON CONFLICT DO NOTHING` zárolható `closed` sort hoz létre.
2. `SELECT ... FOR UPDATE` olvassa a sort.
3. Meghívja a kapott closure-t `Some(BreakerRow)` értékkel.
4. `BreakerWrite::Set` esetén ugyanabban a tranzakcióban írja a sort; `Keep` nem ír.
5. Commit, majd visszaadja a closure `T` eredményét.

Ez a primitív tesztseedelésre és production átmenetekre is használható; a vak `set_breaker` production útvonala nem marad meg.

- [ ] **Step 5: Használd a sorzárat a worker állapotgépből**

`allow` closure-ja:

```rust
repo::update_breaker_locked(pool, &policy.source_slug, |current| {
    let row = current.unwrap_or(BreakerRow {
        state: BreakerState::Closed,
        failure_count: 0,
        opened_at: None,
        next_probe_at: None,
    });
    if row.state == BreakerState::Open {
        if row.next_probe_at.is_some_and(|probe| probe <= now) {
            let probe = BreakerRow {
                state: BreakerState::HalfOpen,
                ..row
            };
            (true, BreakerWrite::Set(probe))
        } else {
            (false, BreakerWrite::Keep)
        }
    } else if row.state == BreakerState::HalfOpen {
        (false, BreakerWrite::Keep)
    } else {
        (true, BreakerWrite::Keep)
    }
}).await
```

`on_failure` ugyanebben a zárolásban számolja ki `next_state`-et és írja a sort. `on_success` zároltan `closed`, `failure_count = 0`, `opened_at = None`, `next_probe_at = None` értékekre állít. `on_auth_failure` storage oldja meg a zárolt növelést és az audit beszúrását ugyanabban a tranzakcióban; a régi előzetes read + későbbi upsert megszűnik.

- [ ] **Step 6: Futtasd zöldre a breaker teszteket**

```bash
cd "/Users/susleczandras/Documents/Code/football-app-v2-ops-o1-worker-fencing/rust"
cargo test -p football-worker --test quota_breaker -- --test-threads=1
```

Várt: exit `0`; a két új race-teszt `2 passed / 0 failed`, a meglévő auth/threshold/probe teszt is zöld.

- [ ] **Step 7: Commitold meg a zárolt breakert**

```bash
cd "/Users/susleczandras/Documents/Code/football-app-v2-ops-o1-worker-fencing"
git add rust/crates/storage/src/repositories/worker.rs \
  rust/crates/worker/src/breaker.rs \
  rust/crates/worker/tests/quota_breaker.rs
git commit -m "fix(O1): zárolt breaker-átmenetek és egyetlen probe"
git push
```

---

### Task 4: Singleton cycle, heartbeat és fenced tick

**Files:**
- Modify: `rust/crates/worker/src/runner.rs`
- Modify: `rust/crates/worker/src/tick.rs`
- Modify: `rust/crates/worker/src/coverage.rs`
- Modify: `rust/crates/worker/src/scheduler.rs`
- Create: `rust/crates/worker/src/lease.rs`
- Modify: `rust/crates/worker/src/lib.rs`
- Modify: `rust/bins/footballd/src/main.rs`
- Modify: `rust/bins/footballctl/src/main.rs`
- Modify: `rust/crates/worker/tests/worker_loop.rs`
- Modify: `rust/crates/worker/tests/tick_e2e.rs`

**Interfaces:**
- Consumes: `LeasedJob`, fenced repository, `FetchReservation`, zárolt breaker.
- Produces: `CycleOutcome`, `run_cycle -> Result<Option<CycleOutcome>>`, `heartbeat_loop -> bool`; daemon és CLI közös production belépése.

- [ ] **Step 1: Írd meg a singleton busy/available tesztet**

`worker_loop.rs` meglévő fixture helperjeit használd. A teszt először policy-t seedel, de nem generál jobot. Egy külön, poolon kívüli kapcsolon kézzel tartja a production lock nevét; a zárolt körnek nem generálhatna és nem dolgozhatna:

```rust
#[tokio::test]
async fn singleton_skips_busy_cycle_and_runs_after_release() {
    let pool = test_pool().await;
    let policy = policy();
    repo::upsert_policy(&pool, &policy).await.unwrap();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let fetch_dir = tempfile::tempdir().unwrap();
    let fetcher = fixture_fetcher(fetch_dir.path());
    let adapters = adapters();
    let worker_id = Uuid::now_v7();

    let connection = pool.connect_options().connect().await.unwrap();
    let lock = sqlx::PgAdvisoryLock::new("football-app-v2:worker-cycle:v1");
    let guard = lock.try_acquire(connection).await.unwrap().left().unwrap();

    let busy = runner::run_cycle(&pool, &store, &as_fetchers(fetcher.clone()), &adapters, worker_id)
        .await.unwrap();
    assert!(busy.is_none());
    assert_eq!(count(&pool, "SELECT count(*) FROM ops.job").await, 0);

    let connection = guard.release_now().await.unwrap();
    drop(connection);
    let outcome = runner::run_cycle(&pool, &store, &as_fetchers(fetcher), &adapters, worker_id)
        .await.unwrap().expect("a lock elengedése után a kör futnia kell");
    assert_eq!(outcome.generated, 1);
    assert_eq!(outcome.processed, 1);
}
```

A `store_dir` és `fetch_dir` változók a kör mindkét hívásáig életben maradnak; ne hozz létre ideiglenes mappát, amely a kör indulása előtt megszűnik.

- [ ] **Step 2: Írd meg a heartbeat-loop integraciós tesztet**

```rust
#[tokio::test]
async fn heartbeat_loop_renews_lease_and_reports_lost_ownership() {
    let pool = test_pool().await;
    let policy = policy();
    repo::upsert_policy(&pool, &policy).await.unwrap();
    repo::insert_job(&pool, &repo::NewJob {
        source_slug: policy.source_slug.clone(),
        domain: "match".into(),
        entity_key: "heartbeat-loop".into(),
        event_type: "discovery".into(),
        time_window: Utc::now().date_naive().to_string(),
        due_at: Utc::now() - Duration::seconds(1),
        priority: 0,
    }).await.unwrap();
    let leased = repo::claim_due_jobs(&pool, Uuid::now_v7(), Utc::now(), 1, 1)
        .await.unwrap().remove(0);
    let (stop, shutdown) = tokio::sync::watch::channel(false);
    let handle = tokio::spawn(lease::heartbeat_loop(
        pool.clone(), leased.lease, Duration::from_millis(25), 1, shutdown,
    ));

    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let renewed = loop {
        let value: DateTime<Utc> = sqlx::query_scalar(
            "SELECT leased_until FROM ops.job WHERE id = $1",
        ).bind(leased.job.id).fetch_one(&pool).await.unwrap();
        if value > leased.lease.leased_until {
            break value;
        }
        assert!(tokio::time::Instant::now() < deadline, "a heartbeat nem újította a lease-t");
        tokio::time::sleep(Duration::from_millis(5)).await;
    };
    assert!(renewed > leased.lease.leased_until);

    sqlx::query("UPDATE ops.job SET leased_until = clock_timestamp() - interval '1 second' WHERE id = $1")
        .bind(leased.job.id).execute(&pool).await.unwrap();
    let alive = tokio::time::timeout(Duration::from_secs(2), handle).await.unwrap().unwrap();
    assert!(!alive);
    let _ = stop.send(true);
}
```

A rövid intervallum csak tesztparaméter; a production tick 15 mp heartbeat és 60 mp lease értéket használ.

- [ ] **Step 3: Futtasd a piros singleton és heartbeat teszteket**

```bash
cd "/Users/susleczandras/Documents/Code/football-app-v2-ops-o1-worker-fencing/rust"
cargo test -p football-worker --test worker_loop singleton_skips_busy_cycle -- --exact --nocapture
cargo test -p football-worker --test worker_loop heartbeat_loop_renews_lease -- --exact --nocapture
```

Várt piros: exit `101`; a még privát `run_cycle` és a `lease` modul/API hiányzik.

- [ ] **Step 4: Valósítsd meg a poolon kívüli singleton lockot**

`runner.rs`-ben a lock neve és a kör eredménye:

```rust
const WORKER_CYCLE_LOCK: &str = "football-app-v2:worker-cycle:v1";

#[derive(Debug, Clone, Copy)]
pub struct CycleOutcome {
    pub generated: usize,
    pub processed: usize,
    pub blocked: usize,
    pub failed: usize,
    pub deferred: usize,
}
```

A publikus `run_cycle` szerkezete:

```rust
let connection = pool.connect_options().connect().await?;
let lock = sqlx::PgAdvisoryLock::new(WORKER_CYCLE_LOCK);
let guard = match lock.try_acquire(connection).await? {
    Either::Right(_) => return Ok(None),
    Either::Left(guard) => guard,
};
let result = run_cycle_unlocked(pool, store, fetchers, adapters, worker_id).await;
let _connection = guard.release_now().await?;
let outcome = result?;
Ok(Some(outcome))
```

A belső `run_cycle_unlocked(pool: &PgPool, store: &ObjectStore, fetchers: &Fetchers, adapters: &HashMap<String, Box<dyn SourceAdapter>>, worker_id: Uuid) -> Result<CycleOutcome>` végzi a policy-listázást, generálást és ticket. A külön kapcsolat nem tér vissza a poolba. A `try_acquire` future cancellation esetén a standalone `PgConnection` lezárása oldja a session lockot; ezért poolozott connection itt tilos. A `release_now` hiba nem nyelhető el, de a kör eredményét sem szabad felülírni: először release, majd a kör eredményét add vissza.

- [ ] **Step 5: Valósítsd meg a heartbeat-loopot**

`lease.rs`:

```rust
pub async fn heartbeat_loop(
    pool: PgPool,
    lease: JobLease,
    interval: Duration,
    lease_secs: i64,
    mut shutdown: watch::Receiver<bool>,
) -> bool {
    loop {
        tokio::select! {
            _ = shutdown.changed() => return true,
            _ = tokio::time::sleep(interval) => {
                if *shutdown.borrow() {
                    return true;
                }
                match repo::heartbeat_job(&pool, &lease, lease_secs).await {
                    Ok(true) => {}
                    Ok(false) => {
                        tracing::warn!(job_id = %lease.job_id, "worker lease elveszett");
                        return false;
                    }
                    Err(err) => {
                        tracing::error!(job_id = %lease.job_id, error = %err, "worker heartbeat hiba");
                        return false;
                    }
                }
            }
        }
    }
}
```

`lib.rs`-ben `pub mod lease;`.

- [ ] **Step 6: Közd a heartbeatet a tick minden jobára**

`tick.rs`-ben `LEASE_SECS = 60`, új `HEARTBEAT_INTERVAL = Duration::from_secs(15)`. Minden `LeasedJob` feldolgozás előtt indíts `watch` csatornás heartbeat taskot. A `process_job` befejezése után először állítsd le és várd meg a taskot, csak aztán végezd a fenced completion/requeue műveletet:

```rust
let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
let heartbeat_pool = pool.clone();
let heartbeat_lease = leased.lease;
let heartbeat = tokio::spawn(lease::heartbeat_loop(
    heartbeat_pool,
    heartbeat_lease,
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
result
```

A fetch és a jobon belüli min-delay várakozás is a `process_job` future része, ezért mindkettő alatt fut a heartbeat. Egy hosszabb szinkron parse blokkolhatja az adott runtime workerét, de a multi-thread Tokio másik workerén a heartbeat futhat.

- [ ] **Step 7: Vidd át a lease contextet a ticken**

A scheduler `claim` wrapper most a storage `Vec<LeasedJob>` eredményét továbbadja, nem `Vec<FetchJob>`-re képezi le. A `tick` claim eredménye `Vec<LeasedJob>`. A `process_job` aláírása:

```rust
async fn process_job(
    pool: &PgPool,
    store: &ObjectStore,
    fetchers: &Fetchers,
    adapters: &HashMap<String, Box<dyn SourceAdapter>>,
    leased: &LeasedJob,
    now: DateTime<Utc>,
) -> Result<JobResult>
```

A `let job = &leased.job;` és `let lease = &leased.lease;` helyi kötésekkel az adapter API változatlan marad. A `scheduler::complete` a storage `complete_leased` hívását kapcsolja össze, a tick közvetlen `repo::requeue_leased` hívást használ; mindkettő `&JobLease`-et kap. A `scheduler::complete` visszatérése `Result<bool>`. A régi `repo::complete_job`, `repo::requeue_with_delay` és `repo::upsert_coverage` ebben a lépésben törlendő, a storage tesztek pedig az új fenced nevekre íródnak.

- [ ] **Step 8: Vidd át a coverage írást a saját tokenre**

`coverage::set` új aláírása:

```rust
pub async fn set(
    pool: &PgPool,
    leased: &LeasedJob,
    state: CoverageState,
) -> Result<bool>
```

A repository hívás:

```rust
repo::upsert_job_coverage(
    pool,
    &leased.lease,
    &leased.job.domain,
    &leased.job.entity_key,
    &leased.job.time_window,
    state,
).await
```

Ellenőrizd a meglévő coverage entity értékeket a tényleges tickben; ne cseréld fel az `entity_key`-t az `entity_type`-fel. Minden coverage hívásnál, ha az eredmény `false`, a job feldolgozása `Blocked` eredménnyel ér véget, és nem következik completion.

- [ ] **Step 9: Fences minden completion és requeue ágat**

A tick összes completion helyére a következő helper használandó:

```rust
async fn finish(
    pool: &PgPool,
    leased: &LeasedJob,
    status: JobStatus,
    last_error: Option<&str>,
) -> Result<JobResult> {
    let changed = scheduler::complete(pool, &leased.lease, status, last_error).await?;
    Ok(match (changed, status) {
        (true, JobStatus::Done) => JobResult::Processed,
        (true, JobStatus::Failed) => JobResult::Failed,
        _ => JobResult::Blocked,
    })
}
```

A helpert add hozzá a tickhez. A következő ágakat cseréld:

- váratlan `process_job` hiba: `Failed` completion, de a tényleges `finish` eredménye határozza meg a számlálót;
- hiányzó/letiltott policy: `Cancelled` → `Blocked`;
- hiányzó adapter, üres fetch-terv, hiányzó fetcher: `Failed`;
- robots tiltás: előbb fenced coverage, majd `Failed` → `Blocked`;
- breaker nyitva: `Failed` → `Blocked`;
- parse karantén: előbb fenced coverage, majd `Failed`;
- sikeres véglegesítés: fenced coverage, majd `Done` → `Processed`;
- best-effort error coverage: csak saját aktív lease esetén;
- fetch error végső `Failed` és a transient `requeue_leased`: mindkettő tokennel és aktív lease predikátummal.

A tick felső számlálója a `process_job` eredményét ne az eredeti `Ok/Err` alapján, hanem a heartbeat és a fenced finish utáni `JobResult` alapján növelje. A lease elvesztése `Blocked`, nem sikeres `Processed` és nem újabb worker hibája.

- [ ] **Step 10: Cseréld le a kvótakaput reservationre**

A robots után a breaker gate menjen előbb, hogy egy nyitott breaker ne fogyassza el a napi kvótát. Ezután:

```rust
let ledger_id = if local_file {
    repo::insert_ledger(pool, &repo::LedgerEntry {
        source_slug: job.source_slug.clone(),
        request_key: req.request_key.clone(),
        attempted_at: now,
        outcome: FetchOutcome::TransportError,
        http_status: None,
        byte_size: None,
        latency_ms: None,
        artifact_hash: None,
        job_id: Some(job.id),
    }).await?
} else if index == 0 {
    match quota::reserve(pool, &job.source_slug, &req.request_key, lease, now).await? {
        QuotaDecision::Allowed(id) => id,
        decision @ (QuotaDecision::QuotaBlocked | QuotaDecision::DelayBlocked) => {
            return requeue_for_gate(pool, leased, &policy, req, decision, now).await;
        }
        QuotaDecision::LeaseLost => return Ok(JobResult::Blocked),
    }
} else {
    match wait_for_delay(pool, &job.source_slug, &req.request_key, lease).await? {
        QuotaDecision::Allowed(id) => id,
        decision @ (QuotaDecision::QuotaBlocked | QuotaDecision::DelayBlocked) => {
            return requeue_for_gate(pool, leased, &policy, req, decision, now).await;
        }
        QuotaDecision::LeaseLost => return Ok(JobResult::Blocked),
    }
};
```

A `LocalFile` transzport szándékosan továbbra is megkerüli robots/quota/delay kaput, de továbbra is kap időben kiírt `transport_error` ledger placeholder-t. A `wait_for_delay` most `quota::reserve`-et hív, nem a régi guardot; az így létrejött `reserved` UUID lesz a későbbi finalizálandó ledger sor. A fetch után a meglévő `update_ledger_outcome` ezt a sort zárja le.

A `quota::guard` ebben a lépésben törlendő. A két új helper pontos viselkedése:

```rust
async fn wait_for_delay(
    pool: &PgPool,
    source_slug: &str,
    request_key: &str,
    lease: &JobLease,
) -> Result<QuotaDecision> {
    let deadline = tokio::time::Instant::now() + StdDuration::from_secs(15);
    loop {
        let decision = quota::reserve(pool, source_slug, request_key, lease, Utc::now()).await?;
        if decision != QuotaDecision::DelayBlocked {
            return Ok(decision);
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok(decision);
        }
        tokio::time::sleep(StdDuration::from_millis(250)).await;
    }
}

async fn requeue_for_gate(
    pool: &PgPool,
    leased: &LeasedJob,
    policy: &SourcePolicy,
    req: &FetchRequest,
    decision: QuotaDecision,
    now: DateTime<Utc>,
) -> Result<JobResult> {
    block_ledger(pool, &leased.job, req, Utc::now(), FetchOutcome::QuotaBlocked).await?;
    let due_at = match decision {
        QuotaDecision::QuotaBlocked => next_quota_reset(now),
        QuotaDecision::DelayBlocked => {
            let delay_ms = policy.min_delay_ms.max(1_000);
            let jitter = if policy.jitter_ms > 0 {
                rand::rng().random_range(0..policy.jitter_ms)
            } else {
                0
            };
            now + Duration::milliseconds(delay_ms + jitter)
        }
        _ => return Ok(JobResult::Blocked),
    };
    let changed = repo::requeue_leased(pool, &leased.lease, due_at).await?;
    if changed {
        tracing::debug!(job_id = %leased.job.id, %due_at, "job visszasorolva a kapu miatt");
    }
    Ok(JobResult::Blocked)
}
```

A gate továbbra is `pending` jobba és következő UTC-kvótavisszaállításra vagy min-delay lejártára ütemez, nem `failed` jobot hoz létre. A `changed` csak a naplózást szabályozza; a viselkedés mindkét esetben `Blocked`.

- [ ] **Step 11: Egyesítsd a daemon és CLI körét**

`footballd` a `run_loop`-ot használja; a `run_loop` belsőleg a publikus `run_cycle`-t hívja. `None` esetén naplózza, hogy egy másik worker tartja a lockot, de a daemon nem hibázik le.

`footballctl::worker_tick` helyett hívd a közös belépést:

```rust
match runner::run_cycle(&pool, &store, &fetchers, &adapters, Uuid::now_v7()).await? {
    Some(outcome) => println!(
        "generated={} processed={} blocked={} failed={} deferred={}",
        outcome.generated, outcome.processed, outcome.blocked,
        outcome.failed, outcome.deferred,
    ),
    None => println!("worker tick kihagyva: egy másik kör már fut"),
}
```

A CLI saját policy-listázása, `scheduler::generate_jobs` hívása és közvetlen `tick::tick` hívása törlendő.

- [ ] **Step 12: Igazítsd és futtasd a worker E2E teszteket**

A `tick_e2e.rs` claimhez kapcsolódó részeit `LeasedJob` szerint igazítsd. A delay, multi-request, parse/error és requeue teszteknek meg kell vizsgálniuk a tényleges ledger `reserved` → final outcome átmenetét és a job státuszt. Ne változtasd a várt domain eredményeket vagy fixture payloadokat.

```bash
cd "/Users/susleczandras/Documents/Code/football-app-v2-ops-o1-worker-fencing/rust"
cargo test -p football-worker --test worker_loop -- --test-threads=1
cargo test -p football-worker --test tick_e2e -- --test-threads=1
cargo test -p football-worker --test quota_breaker -- --test-threads=1
cargo build -p footballd -p footballctl --features obscura
```

Várt: minden parancs exit `0`.

- [ ] **Step 13: Commitold meg a közös worker körét**

```bash
cd "/Users/susleczandras/Documents/Code/football-app-v2-ops-o1-worker-fencing"
git add rust/crates/worker/src/runner.rs \
  rust/crates/worker/src/tick.rs \
  rust/crates/worker/src/coverage.rs \
  rust/crates/worker/src/scheduler.rs \
  rust/crates/worker/src/lease.rs \
  rust/crates/worker/src/lib.rs \
  rust/bins/footballd/src/main.rs \
  rust/bins/footballctl/src/main.rs \
  rust/crates/worker/tests/worker_loop.rs \
  rust/crates/worker/tests/tick_e2e.rs
git commit -m "feat(O1): singleton worker kör, heartbeat és fenced tick"
git push
```

---

### Task 5: Teljes kapu, független review, checkpoint és lezárás

**Files:**
- Modify: `docs/checkpoints/2026-11-28/checkpoint.md`
- Modify: csak a review által indokott O1 fájlok

**Interfaces:**
- Consumes: a Task 1–4 zöld commitjai és branch.
- Produces: teljes workspace bizonyíték, független verdikt, checkpoint, FF merge és pushed záró állapot.

- [ ] **Step 1: Futtasd a teljes alap- és workspace-kapot sorban**

```bash
cd "/Users/susleczandras/Documents/Code/football-app-v2-ops-o1-worker-fencing"
./scripts/db-reset.sh
./scripts/check-foundation.sh
cd rust
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo build -p footballd -p footballctl --features obscura
cd ..
git diff --check
```

Várt: minden parancs exit `0`; a workspace-teszt kimenetében `0 failed`, `0 ignored`; a kimenetben nincs tényleges `FAILED`, `panic`, `error:` vagy `warning:` marker. A parancsokat ne futtasd párhuzamosan, mert a `db-reset` és a workspace-tesztek ugyanazt a teszt-adatbázist használják.

- [ ] **Step 2: Ellenőrizd a teszt-adatbázis invariánsait**

A `football_v2_test` adatbázisban mérd:

- `0033_worker_fencing.sql` alkalmazva `1`;
- `ops.job.lease_token` oszlop `1`, partial unique index `1`;
- `reserved` outcome elfogadó constraint `1`;
- lezárt O1 tesztek után fennmaradó `leased` job `0`;
- token nélküli, fennmaradó `leased` job `0`;
- `reserved` ledger sor, amelyhez nem tartozik fennmaradó leased job `0` a teszt tisztasága után.

A valódi SQL-t read-only lekérdezésként vagy a projekt meglévő, rollback-védett ellenőrzési mintájával futtasd. Ne írj a dev adatbázisra.

- [ ] **Step 3: Kérj független kódreviewt**

A kódot nem író `general-purpose` ágensnek add át:

- a spec `docs/superpowers/specs/2026-11-27-uzemeltetes-triage-console-design.md` §8.2/§13.1/§15;
- a teljes O1 diffet a bázis `79273b62ce5f949cc18daf919d55d9cf11729ca1` és az aktuális HEAD között;
- a plan `docs/superpowers/plans/2026-11-28-uzemeltetes-o1.md`;
- a kötelező kérdéseket: a singleton valóban letiltja-e a generálást is, minden completion/requeue/coverage fenced-e, a heartbeat lejárat és cancellation után nem enged-e írást, a reservation valóban ugyanabban a tranzakcióban lockol/countol/insertál, a breaker half-open pontosan egy probe-t enged, a CLI nem hagy meg bypass-t.

Az ellenőr nem ír kódot. A `Critical` és `Important` findingokat javítsd, minden javításhoz célzott teszt és ismételt kapu kell. A `Minor` finding csak indokolt esetben javítható; a verdiktet ne a builder adja.

- [ ] **Step 4: Írd meg a checkpointot**

A `docs/checkpoints/2026-11-28/checkpoint.md` O1 szakasza kötelezően tartalmazza:

- induló branch/base és a read-only dev/daemon állapot;
- a módosított fájlok végleges listáját;
- minden piros teszt konkrét `passed/failed` és exit kimenetét;
- a race-tesztek tényleges párhuzamát és a várt számokat;
- a teljes workspace tesztcsomag- és `passed/failed/ignored` számát;
- clippy, fmt, build, check-foundation és `git diff --check` eredményét;
- a teszt DB `0033`, index, constraint és maradék lease/ledger ellenőrzését;
- a független review verdiktjét és finding-jeit;
- a magyar szövegek Unicode ellenőrzését;
- a nem ellenőrzött részeket és a fennmaradó kockázatokat;
- a kód `git revert` visszaállítását és azt, hogy a 0033 séma visszaállítás után használatlanul maradhat;
- hogy a dev DB és a futó daemon nem volt érintve.

- [ ] **Step 5: Ellenőrizd a magyar szöveget és a végleges diffet**

A módosított és untracked fájlok szövegét olvasd vissza. Keresd a CJK/kelet-ázsiai és `U+FFFD` karaktereket; a találat `0` kell legyen. Nézd át a `git status --short` listát és a teljes diffet; idegen, titkos vagy generált fájl nem kerülhet stagingre.

- [ ] **Step 6: Commitold meg a checkpointot és a esetleges review-javításokat**

```bash
cd "/Users/susleczandras/Documents/Code/football-app-v2-ops-o1-worker-fencing"
git add docs/checkpoints/2026-11-28/checkpoint.md
git commit -m "docs(O1): worker fencing kapu, review és visszaállás"
git push
```

- [ ] **Step 7: FF merge és branch lezárás**

Ellenőrizd, hogy a `feat/ops-triage-console` branch a bázis óta nem vált el. Ha a bázis az O1 branch őse, FF merge és push engedélyezett. Ha nem FF-mergehető, állj meg és ne kényszerítsd.

```bash
cd "/Users/susleczandras/Documents/Code/football-app-v2-ops-o1-worker-fencing"
git fetch origin
git merge-base --is-ancestor feat/ops-triage-console feat/ops-o1-worker-fencing
git merge-base --is-ancestor origin/feat/ops-triage-console feat/ops-triage-console
git switch feat/ops-triage-console
git merge --ff-only feat/ops-o1-worker-fencing
git push origin feat/ops-triage-console
git branch -d feat/ops-o1-worker-fencing
git push origin --delete feat/ops-o1-worker-fencing
```

A branch törlés csak a teljes zöld kapu és a független review után történik. A dev DB migrációja és a daemon telepítése/újraindítása nem része a merge-nek.

## Spec Coverage Self-Review

| Spec követelmény | Terv fedezete |
|---|---|
| Daemon és manual tick párhuzamos futása kizárva | Task 4 singleton busy/available teszt, daemon/CLI közös `run_cycle` |
| Job claim egyedi fencing token | Task 1 `gen_random_uuid()`, token-egyediség assertion |
| Completion csak saját tokennel | Task 1 stale completion teszt, Task 4 `finish` helper |
| Requeue csak saját tokennel | Task 1 stale requeue teszt, Task 4 transient requeue |
| Coverage csak saját tokennel | Task 1 stale coverage teszt, `WITH leased ... FOR UPDATE` |
| Heartbeat és stale worker védelem | Task 4 heartbeat-loop teszt és tickre kötés; DB `clock_timestamp()` predikátum |
| Kvóta/min-delay/placeholder egy tranzakcióban | Task 2 `FOR UPDATE` policy lock, két valódi Barrier race teszt |
| Breaker CAS vagy lock | Task 3 storage sorzár, két valódi Barrier race teszt |
| Párhuzamos tick nem lépi túl budgetet | Task 2 reservation race + Task 4 singleton |
| Kód visszaállítható, adat nem törlődik | 0033 additív migráció, Task 5 checkpoint és FF branch lezárás |
| Builder ≠ verifier | Task 5 független read-only review |

### Type Consistency Review

- `JobLease.job_id` és `LeasedJob.job.id` ugyanaz az UUID; a fenced repository és a quota reservation ugyanazt használja.
- A token minden repository predikátumban `JobLease.token`; a storage reservation a `job_id` és `lease_token` külön bindja.
- `FetchOutcome::Reserved` a domain enum, SQL constraint és a count/last lekérdezés pontos ugyanaz a sztring.
- `QuotaDecision::Allowed(Uuid)` minden non-local fetch előtt létrehozott reservation UUID-ra utal; `LocalFile` továbbra is külön placeholder-utat használ.
- `CycleOutcome.generated` `usize`, a tick meglévő számlálói is `usize`; a daemon/CLI nem egymástól eltérő eredménytípusra támaszkodik.
- `coverage::set` a teljes `LeasedJob`-ot kapja, így az entity mezők és a lease token nem külön paraméterekben állíthatók el egymástól.

### Executions Notes

A felhasználó állandó autonóm felhatalmazása alapján a terv végrehajtása inline, Task 1-től Task 5-ig folytatódik újabb végrehajtási kérdés nélkül. A független review és a zöld kapu után a branch a meglévő slice-closure szabály szerint FF-mergelődik; a dev DB és a futó daemon továbbra is külön, explicit döntés.
