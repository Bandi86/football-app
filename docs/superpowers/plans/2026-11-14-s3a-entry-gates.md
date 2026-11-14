# S3a — Belépési kapuk: implementációs terv

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task (subagent-típusok nem elérhetők ebben a sessionben → inline execution). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A 13 belépési kapuból 11 kód-szintű zárása (fetch-reziliencia, robots+ledger, scheduler/evidencia/CLI, identitás) élő forrás bevezetése előtt, migráció nélkül.

**Architecture:** A kapuk a meglévő worker/storage/identity rétegekben javítások: új `FetchError::RateLimited`, ledger-alapú backoff + delayed requeue, fetch-szintű breaker-siker, robots relaxációk + robots-ledger, scheduler-validáció, media_type, CLI repo-fn-ök, közös linkeres identitás-feloldás, cross-kind approve, crosswalk-uniqueness ellenőrzés.

**Tech Stack:** Rust (edition 2024, rust-version 1.85), sqlx 0.8, tokio, wiremock (tesztek).

**Spec:** `docs/superpowers/specs/2026-11-14-s3a-entry-gates-design.md`

## Global Constraints

- AGENTS.md §2: **minden task végén STOP** — összefoglaló + bizonyíték, következő task csak felhasználói engedéllyel.
- **Commit: zöld task után** (felhasználói utasítás, 2026-11-14) — csak a kapuk zöld állapotában; `git -c user.name='Bandi86' -c user.email='Bandi86@users.noreply.github.com'`; push nem történik.
- Munkaág: `feat/s3a-entry-gates` a main (6741319) csúcsáról; a létrehozás a végrehajtás első lépése.
- Dokumentumok/hibaüzenetek **magyarul**; kódazonosítók angolul. Magyar szöveg Write/Edit után visszaolvasandó.
- Kérésfeldolgozásban nincs unwrap/expect/elnémítás; tesztkódban megengedett.
- Tesztek: `TEST_DATABASE_URL` kötelező; integrációs tesztek egy szálon; hivatalos kapu `./scripts/check.sh`.
- **Nincs migráció**; **0 élő kérés**; a demo 20/20 állapot változatlan.
- Halasztva (nem része): #10 OpenFoot-számláló, #11 többworker fencing.

## Fájlstruktúra

| Fájl | Felelősség |
|---|---|
| `rust/crates/domain/src/error.rs` | `FetchError::RateLimited(Option<std::time::Duration>)` |
| `rust/crates/ingest/src/http.rs` | 429 → RateLimited + Retry-After parse |
| `rust/crates/storage/src/repositories/worker.rs` | `count_job_fetch_attempts`, `requeue_with_delay`, media_type param, doctor/status repo-fn-ök, kvóta-kizárás |
| `rust/crates/worker/src/tick.rs` | RateLimited/backoff ág, fetch-szintű breaker-siker, persist_raw media_type |
| `rust/crates/worker/src/robots.rs` | Sitemap-only/BOM, origin-kulcs + origin robots-URL, TTL-ágak, robots-ledger |
| `rust/crates/worker/src/scheduler.rs` | event_type/interval validáció |
| `rust/crates/storage/src/repositories/identity.rs` | `entity_kind` |
| `rust/crates/identity/src/review.rs` | cross-kind approve |
| `rust/crates/identity/src/lib.rs` | `crosswalk_conflicts` |
| `rust/bins/footballctl/src/main.rs` | CLI repo-fn-ök, közös feloldás, `identity crosswalk-check` |
| Tesztek | `ingest/tests/http_fetcher.rs`, `worker/tests/tick_e2e.rs`, `worker/tests/robots_gate.rs`, `worker/tests/scheduler_validation.rs` (új), `storage/tests/persist_raw_media_type.rs` (új), `identity/tests/review.rs`, `identity/tests/crosswalk_check.rs` (új) |

---

### Task 1: Fetch-reziliencia (#1, #2, #3)

**Files:**
- Modify: `rust/crates/domain/src/error.rs`, `rust/crates/ingest/src/http.rs`, `rust/crates/storage/src/repositories/worker.rs`, `rust/crates/worker/src/tick.rs`, `rust/crates/worker/Cargo.toml` (rand, ha nem production dep)
- Test: `rust/crates/ingest/tests/http_fetcher.rs`, `rust/crates/worker/tests/tick_e2e.rs` (új tesztek + `TestHttpAdapter` helper)

**Interfaces:**
- Produces: `FetchError::RateLimited(Option<std::time::Duration>)`; `repo::count_job_fetch_attempts(pool, job_id) -> Result<i64>`; `repo::requeue_with_delay(pool, job_id, due_at) -> Result<()>`; `backoff_delay(attempts: u32, jitter_ms: i64, rng: &mut impl FnMut() -> u32) -> chrono::Duration`.
- Consumes: `SourceAdapter`, `Fetchers`, `repo::upsert_policy`, `repo::insert_job`.

- [ ] **Step 1: Bukó tesztek**

`rust/crates/ingest/tests/http_fetcher.rs` végére:

```rust
#[tokio::test]
async fn http_429_reports_rate_limit_with_retry_after() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "120"))
        .mount(&server)
        .await;
    let fetcher = RustHttpFetcher::new("football-app-v2/0.1").unwrap();
    let req = FetchRequest {
        url: server.uri(),
        request_key: "k".into(),
        endpoint_key: "e".into(),
    };
    let err = fetcher.fetch(&req, &limits()).await.unwrap_err();
    match err {
        FetchError::RateLimited(Some(d)) => assert_eq!(d, Duration::from_secs(120)),
        other => panic!("RateLimited(120s)-t vártunk, ehelyett: {other:?}"),
    }
}
```

`rust/crates/worker/tests/tick_e2e.rs` végére (a helperek a fájl meglévő mintáját
követik; a `today_noon`, `adapters`, `policy` helperek már léteznek):

```rust
struct TestHttpAdapter {
    url: String,
    parse_fails: bool,
}

impl SourceAdapter for TestHttpAdapter {
    fn meta(&self) -> football_ingest::adapter::AdapterMeta {
        football_ingest::adapter::AdapterMeta {
            source_slug: "demo-synthetic".into(),
            adapter_version: "1.0".into(),
            parser_version: "1.0".into(),
            domains: vec!["match".into()],
            transport: Transport::RustHttp,
        }
    }

    fn plan(&self, _job: &FetchJob) -> Vec<FetchRequest> {
        vec![FetchRequest {
            url: self.url.clone(),
            request_key: "test:page".into(),
            endpoint_key: "match".into(),
        }]
    }

    fn parse(
        &self,
        _raw: &RawPayload,
        _ctx: &football_ingest::adapter::ParseCtx,
    ) -> Result<Vec<football_ingest::adapter::Observation>, ParseError> {
        if self.parse_fails {
            Err(ParseError::DecoyDetected("teszt: parse-karantén".into()))
        } else {
            Ok(vec![])
        }
    }
}

fn http_policy(jitter_ms: u32) -> SourcePolicy {
    SourcePolicy {
        source_slug: "demo-synthetic".into(),
        transport: Transport::RustHttp,
        mode: SourceMode::Shadow,
        daily_budget: 100,
        min_delay_ms: 0,
        jitter_ms,
        breaker_threshold: 3,
        breaker_cooldown_secs: 60,
        adapter_version: "1.0".into(),
        parser_version: "1.0".into(),
        domains: vec!["match".into()],
        events: vec![],
        enabled: true,
        max_bytes: 5_242_880,
    }
}

fn http_adapters(url: &str, parse_fails: bool) -> HashMap<String, Box<dyn SourceAdapter>> {
    HashMap::from([(
        "demo-synthetic".to_string(),
        Box::new(TestHttpAdapter {
            url: url.to_string(),
            parse_fails,
        }) as Box<dyn SourceAdapter>,
    )])
}

async fn seed_http_job(pool: &PgPool, now: DateTime<Utc>) {
    repo::insert_job(
        pool,
        &repo::NewJob {
            source_slug: "demo-synthetic".into(),
            domain: "match".into(),
            entity_key: "all".into(),
            event_type: "discovery".into(),
            time_window: now.date_naive().to_string(),
            due_at: now - Duration::minutes(1),
            priority: 10,
        },
    )
    .await
    .unwrap();
}

async fn robots_allow_mock(server: &wiremock::MockServer) {
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/robots.txt"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            "User-agent: *\nAllow: /\n",
        ))
        .mount(server)
        .await;
}

#[tokio::test]
async fn rate_limited_job_is_requeued_with_retry_after_delay() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let server = wiremock::MockServer::start().await;
    robots_allow_mock(&server).await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/page"))
        .respond_with(
            wiremock::ResponseTemplate::new(429).insert_header("retry-after", "300"),
        )
        .mount(&server)
        .await;
    repo::upsert_policy(&pool, &http_policy(0)).await.unwrap();
    seed_http_job(&pool, now).await;
    let fetchers = Fetchers::new(
        RustHttpFetcher::new("football-app-test").unwrap(),
        FixtureFetcher::new(HashMap::new()),
    );
    let adapters = http_adapters(&format!("{}/page", server.uri()), false);

    let outcome = tick::tick(&pool, &store, &fetchers, &adapters, Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(outcome.failed, 0);
    assert_eq!(outcome.blocked, 1, "requeue = blokkolt kimenet");

    let (status, due_at): (String, DateTime<Utc>) =
        sqlx::query_as("SELECT status, due_at FROM ops.job")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "pending");
    let expected = now + Duration::seconds(300);
    assert!(
        (due_at - expected).num_seconds().abs() <= 2,
        "due_at ≈ now+300s: {due_at}"
    );
    let (outcome_db, http_status): (String, Option<i16>) = sqlx::query_as(
        "SELECT outcome, http_status FROM ops.fetch_ledger WHERE job_id IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((outcome_db.as_str(), http_status), ("http_error", Some(429)));
}

#[tokio::test]
async fn transient_error_backoff_is_one_minute_on_first_attempt() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let server = wiremock::MockServer::start().await;
    robots_allow_mock(&server).await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/page"))
        .respond_with(wiremock::ResponseTemplate::new(503))
        .mount(&server)
        .await;
    repo::upsert_policy(&pool, &http_policy(0)).await.unwrap();
    seed_http_job(&pool, now).await;
    let fetchers = Fetchers::new(
        RustHttpFetcher::new("football-app-test").unwrap(),
        FixtureFetcher::new(HashMap::new()),
    );
    let adapters = http_adapters(&format!("{}/page", server.uri()), false);

    let outcome = tick::tick(&pool, &store, &fetchers, &adapters, Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(outcome.blocked, 1);

    let (status, due_at): (String, DateTime<Utc>) =
        sqlx::query_as("SELECT status, due_at FROM ops.job")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "pending");
    let expected = now + Duration::seconds(60);
    assert!((due_at - expected).num_seconds().abs() <= 2, "due_at ≈ now+60s");
}

#[tokio::test]
async fn permanent_error_is_not_requeued() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let server = wiremock::MockServer::start().await;
    robots_allow_mock(&server).await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/page"))
        .respond_with(wiremock::ResponseTemplate::new(404))
        .mount(&server)
        .await;
    repo::upsert_policy(&pool, &http_policy(0)).await.unwrap();
    seed_http_job(&pool, now).await;
    let fetchers = Fetchers::new(
        RustHttpFetcher::new("football-app-test").unwrap(),
        FixtureFetcher::new(HashMap::new()),
    );
    let adapters = http_adapters(&format!("{}/page", server.uri()), false);

    let outcome = tick::tick(&pool, &store, &fetchers, &adapters, Uuid::now_v7(), now)
        .await
        .unwrap();
    assert_eq!(outcome.failed, 1);
    let status: String = sqlx::query_scalar("SELECT status FROM ops.job")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "failed", "a 404 végleges, nincs requeue");
}

#[tokio::test]
async fn successful_fetch_closes_half_open_breaker_even_on_parse_quarantine() {
    let pool = test_pool().await;
    let now = today_noon();
    let store_dir = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(store_dir.path()).unwrap();
    let server = wiremock::MockServer::start().await;
    robots_allow_mock(&server).await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/page"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("not-json"))
        .mount(&server)
        .await;
    repo::upsert_policy(&pool, &http_policy(0)).await.unwrap();
    seed_http_job(&pool, now).await;
    // half_open breaker (a probe engedélyezve: next_probe_at = now)
    football_storage::repositories::worker::set_breaker(
        &pool,
        "demo-synthetic",
        &football_storage::repositories::worker::BreakerRow {
            state: football_domain::policy::BreakerState::Open,
            failure_count: 3,
            opened_at: Some(now - Duration::minutes(5)),
            next_probe_at: Some(now - Duration::seconds(1)),
        },
    )
    .await
    .unwrap();
    let fetchers = Fetchers::new(
        RustHttpFetcher::new("football-app-test").unwrap(),
        FixtureFetcher::new(HashMap::new()),
    );
    let adapters = http_adapters(&format!("{}/page", server.uri()), true);

    let _ = tick::tick(&pool, &store, &fetchers, &adapters, Uuid::now_v7(), now)
        .await
        .unwrap();

    let state: String = sqlx::query_scalar(
        "SELECT state FROM ops.breaker_state WHERE source_slug='demo-synthetic'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, "closed", "a sikeres fetch lezárja a half_open breakert");
}
```

Megjegyzés: a half_open teszt a breaker-sort Open + lejárt `next_probe_at`-tel
seedeli (az `allow` Open→HalfOpen-t lép, majd a probe fetch fut); a
parse-karantén után is closed-nak kell lennie — pontosan a #3 kapu.

- [ ] **Step 2: Futtatás — bukás.** `cd rust && cargo test -p football-ingest --test http_fetcher -- --test-threads=1` → FAIL (`RateLimited` nincs); `cargo test -p football-worker --test tick_e2e rate_limited -- --test-threads=1` → FAIL.

- [ ] **Step 3: Implementáció — domain + HTTP** (a spec 2.1 szerint)

`rust/crates/domain/src/error.rs`: új variáns
`#[error("a forrás kérte: várakozás {0:?}")] RateLimited(Option<std::time::Duration>)`.

`rust/crates/ingest/src/http.rs`:

```rust
const RETRY_AFTER: reqwest::header::HeaderName =
    reqwest::header::HeaderName::from_static("retry-after");

fn parse_retry_after(value: Option<&reqwest::header::HeaderValue>) -> Option<std::time::Duration> {
    let text = value?.to_str().ok()?.trim();
    if let Ok(secs) = text.parse::<u64>() {
        return Some(std::time::Duration::from_secs(secs));
    }
    let at = chrono::DateTime::parse_from_rfc2822(text).ok()?;
    (at.with_timezone(&chrono::Utc) - chrono::Utc::now())
        .to_std()
        .ok()
}
```

a nem-2xx ágban:

```rust
        let status = response.status().as_u16();
        if status == 429 {
            return Err(FetchError::RateLimited(parse_retry_after(
                response.headers().get(&RETRY_AFTER),
            )));
        }
        return Err(FetchError::HttpStatus(status));
```

- [ ] **Step 4: Implementáció — storage**

`rust/crates/storage/src/repositories/worker.rs`:

```rust
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
```

- [ ] **Step 5: Implementáció — tick**

`rust/crates/worker/src/tick.rs`:

```rust
/// Exponenciális backoff + jitter: min(60s·2^(n-1), 1h) + [0, jitter_ms).
fn backoff_delay(attempts: u32, jitter_ms: i64, rng: &mut impl FnMut() -> u32) -> chrono::Duration {
    let exponent = attempts.saturating_sub(1).min(6);
    let base = 60i64 << exponent;
    let capped = base.min(3600);
    let jitter = if jitter_ms == 0 { 0 } else { i64::from(rng() % jitter_ms) };
    chrono::Duration::milliseconds(capped * 1000 + jitter)
}
```

A fetch siker-ága (`let raw = match fetched { Ok(raw) => raw, ... }` után):

```rust
    // A breaker a fetch-egészséget tükrözi: a sikeres fetch azonnal zárja
    // (half-open probe sem ragadhat be parse-karantén/infra-hiba miatt).
    breaker::on_success(pool, &job.source_slug).await?;
```

A `fetch_failure` elején (a meglévő ledger-outcome számítás előtt):

```rust
    let retry_after = match err {
        FetchError::RateLimited(retry_after) => *retry_after,
        _ => None,
    };
    let transient = matches!(
        err,
        FetchError::Transport(_) | FetchError::Timeout | FetchError::RateLimited(_)
    ) || matches!(err, FetchError::HttpStatus(status) if *status >= 500);
```

A ledger-outcome mapping (a meglévő `(outcome, http_status)` match): a
`RateLimited` ág `FetchOutcome::HttpError` + `Some(429)`; a többi ág és a
breaker-hívás változatlan.

A függvény végén (a `scheduler::complete(... JobStatus::Failed, ...)` helyett):

```rust
    if transient {
        let attempts = repo::count_job_fetch_attempts(pool, job.id).await?;
        let delay = retry_after
            .and_then(chrono::Duration::from_std)
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
```

A fájl végéről a régi, kései `breaker::on_success` hívás törlése.

Megjegyzés: a `rand` production dep kell (a worker Cargo.toml-ban
`rand.workspace = true`; ha már van dev-dep, a duplikációt kerüld).

- [ ] **Step 6: Futtatás — zöld.** `cargo test -p football-ingest --test http_fetcher -- --test-threads=1 && cargo test -p football-worker -- --test-threads=1` → PASS.

- [ ] **Step 7: Teljes kapu + commit.** `cd .. && ./scripts/check.sh` → exit 0;
  commit `feat: add S3a fetch resilience (rate limit, backoff, breaker fix)`.

- [ ] **Step 8: STOP — jelentés.**

---

### Task 2: Robots + ledger (#4, #7)

**Files:**
- Modify: `rust/crates/worker/src/robots.rs`, `rust/crates/storage/src/repositories/worker.rs` (kvóta-kizárás)
- Test: `rust/crates/worker/tests/robots_gate.rs`

**Interfaces:**
- Produces: origin-kulcsú robots cache + origin robots-URL; Sitemap-only engedés; TTL-ágak; robots-ledger sorok.

- [ ] **Step 1: Meglévő tesztek frissítése (bukjanak a jelenlegi kódon)**

A `robots_gate.rs` fixture-kulcsai a régi host-kulcsot használják
(`demo-synthetic:robots:example.test`) — az origin-kulcs miatt frissítendők:

```rust
// a fetcher_with kulcsaiban:  demo-synthetic:robots:https://example.test
```

Új tesztek:

```rust
#[tokio::test]
async fn sitemap_only_robots_is_allowed() {
    let pool = test_pool().await;
    seed_policy(&pool).await;
    let (_dir, fetcher) = fetcher_with(&[(
        "demo-synthetic:robots:https://example.test",
        b"Sitemap: https://example.test/sitemap.xml\n",
    )]);
    let v = robots::check(&pool, &fetcher, "demo-synthetic", "https://example.test/page")
        .await
        .unwrap();
    assert_eq!(v, RobotsVerdict::NoRobots, "a Sitemap-only fájl nem tiltás");
}

#[tokio::test]
async fn bom_prefixed_robots_is_parsed() {
    let pool = test_pool().await;
    seed_policy(&pool).await;
    let (_dir, fetcher) = fetcher_with(&[(
        "demo-synthetic:robots:https://example.test",
        b"\xef\xbb\xbfUser-agent: *\nAllow: /\n",
    )]);
    let v = robots::check(&pool, &fetcher, "demo-synthetic", "https://example.test/page")
        .await
        .unwrap();
    assert_eq!(v, RobotsVerdict::Allowed, "a BOM nem blokkol");
}

struct FailingFetcher;

#[async_trait::async_trait]
impl Fetcher for FailingFetcher {
    async fn fetch(
        &self,
        _req: &FetchRequest,
        _limits: &FetchLimits,
    ) -> Result<RawPayload, FetchError> {
        Err(FetchError::Transport("teszt: átmeneti hiba".into()))
    }
}

#[tokio::test]
async fn transient_error_uses_short_ttl() {
    let pool = test_pool().await;
    seed_policy(&pool).await;
    let v = robots::check(
        &pool,
        &FailingFetcher,
        "demo-synthetic",
        "https://example.test/page",
    )
    .await
    .unwrap();
    assert_eq!(v, RobotsVerdict::Blocked, "átmeneti hiba = fail-closed");
    let ttl_hours: f64 = sqlx::query_scalar(
        "SELECT EXTRACT(EPOCH FROM (expires_at - fetched_at)) / 3600 FROM ops.robots_cache",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        (ttl_hours - 1.0).abs() < 0.1,
        "átmeneti hibánál 1 órás TTL várható, ehelyett: {ttl_hours}"
    );
}

#[tokio::test]
async fn robots_fetch_writes_non_counting_ledger_row() {
    let pool = test_pool().await;
    seed_policy(&pool).await;
    let (_dir, fetcher) = fetcher_with(&[(
        "demo-synthetic:robots:https://example.test",
        b"User-agent: *\nAllow: /\n",
    )]);
    let now = chrono::Utc::now();
    robots::check(&pool, &fetcher, "demo-synthetic", "https://example.test/page")
        .await
        .unwrap();
    let (request_key, outcome, job_id): (String, String, Option<uuid::Uuid>) = sqlx::query_as(
        "SELECT request_key, outcome, job_id FROM ops.fetch_ledger
         WHERE request_key LIKE 'demo-synthetic:robots:%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(request_key, "demo-synthetic:robots:https://example.test");
    assert_eq!(outcome, "success");
    assert_eq!(job_id, None, "a robots-sor nem jobhoz kötött");
    let counted = football_storage::repositories::worker::count_fetch_attempts_today(
        &pool,
        "demo-synthetic",
        now.date_naive(),
    )
    .await
    .unwrap();
    assert_eq!(counted, 0, "a robots-fetch nem számít a napi kvótába");
}
```

- [ ] **Step 2: Futtatás — bukás.** `cargo test -p football-worker --test robots_gate -- --test-threads=1` → FAIL.

- [ ] **Step 3: Implementáció — robots.rs**

  - A robots-változó legyen az **origin**:
    `let origin = parsed.origin().ascii_serialization();`
    a robots-URL: `format!("{origin}/robots.txt")`, a cache-kulcs: `origin`.
  - BOM: `let rules_text = rules_text.trim_start_matches('\u{feff}');` a cache-be
    a strip-elt szöveg kerül.
  - Direktíva-kapu: a `has_user_agent_directive` → Blocked ág helyett: ha nincs
    `User-agent` szakasz → `NoRobots` (RFC: a szekción kívüli direktívák
    irrelevánsak).
  - TTL-ágak: a fetch-hiba ág jelölje, hogy átmeneti volt-e
    (`FetchError::Transport(_)` / 5xx → `transient = true`); a `blocked`
    verdictnél: `transient → now + 1h`, egyébként `24h`.
  - Ledger: a fetch után (verdicttől függetlenül) egy `LedgerEntry`
    (`request_key = {slug}:robots:{origin}`, `job_id: None`, outcome
    success/http_error/timeout/transport_error, http_status, byte_size,
    latency_ms) a `repo::insert_ledger`-rel.
  - Kvóta-kizárás: `count_fetch_attempts_today` SQL-jébe:
    `AND request_key NOT LIKE '%:robots:%'`.

- [ ] **Step 4: Futtatás — zöld + kapu + commit** (`feat: add S3a robots relaxations and robots ledger`).

- [ ] **Step 5: STOP — jelentés.**

---

### Task 3: Scheduler + media_type + CLI repo-függvények (#5, #6, #8)

**Files:**
- Modify: `rust/crates/worker/src/scheduler.rs`, `rust/crates/storage/src/repositories/worker.rs`, `rust/crates/worker/src/tick.rs`, `rust/bins/footballctl/src/main.rs`
- Test: `rust/crates/worker/tests/scheduler_validation.rs` (új), `rust/crates/storage/tests/persist_raw_media_type.rs` (új)

**Interfaces:**
- Produces: `repo::persist_raw(..., media_type: &str)`; `repo::worker_status_rows(pool)`; `repo::doctor_counts(pool)`.

- [ ] **Step 1: Bukó tesztek**

`rust/crates/worker/tests/scheduler_validation.rs` (új):

```rust
use chrono::Utc;
use football_domain::policy::{EventTemplate, SourceMode, SourcePolicy, Transport};
use football_storage::repositories::worker as repo;
use football_storage::test_support::test_pool;
use football_worker::scheduler;

fn policy_with(events: Vec<EventTemplate>) -> SourcePolicy {
    SourcePolicy {
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
        events,
        enabled: true,
        max_bytes: 5_242_880,
    }
}

#[tokio::test]
async fn unknown_event_type_generates_no_job() {
    let pool = test_pool().await;
    let policy = policy_with(vec![EventTemplate {
        event_type: "mystery".into(),
        offset_minutes: None,
        interval_minutes: Some(1440),
    }]);
    repo::upsert_policy(&pool, &policy).await.unwrap();
    let inserted = scheduler::generate_jobs(&pool, &[policy], Utc::now())
        .await
        .unwrap();
    assert_eq!(inserted, 0, "ismeretlen event_type → nincs job");
}

#[tokio::test]
async fn invalid_interval_generates_no_job() {
    let pool = test_pool().await;
    let policy = policy_with(vec![EventTemplate {
        event_type: "discovery".into(),
        offset_minutes: None,
        interval_minutes: Some(60),
    }]);
    repo::upsert_policy(&pool, &policy).await.unwrap();
    let inserted = scheduler::generate_jobs(&pool, &[policy], Utc::now())
        .await
        .unwrap();
    assert_eq!(inserted, 0, "60 perces interval nem engedett");
}

#[tokio::test]
async fn daily_interval_generates_one_job() {
    let pool = test_pool().await;
    let policy = policy_with(vec![EventTemplate {
        event_type: "discovery".into(),
        offset_minutes: None,
        interval_minutes: Some(1440),
    }]);
    repo::upsert_policy(&pool, &policy).await.unwrap();
    let inserted = scheduler::generate_jobs(&pool, &[policy], Utc::now())
        .await
        .unwrap();
    assert_eq!(inserted, 1);
}
```

`rust/crates/storage/tests/persist_raw_media_type.rs` (új):

```rust
use football_storage::repositories::worker::{self as repo, RawCapture};
use football_storage::test_support::{seed_min_match, test_pool};

#[tokio::test]
async fn persist_raw_uses_payload_media_type() {
    let pool = test_pool().await;
    let (match_id, source_id) = seed_min_match(&pool).await;
    let _ = match_id;
    let _ = source_id;
    // A capture-höz a forrás-slug a seed_min_match 'seed' forrása.
    let artifact = football_storage::object_store::ArtifactRef {
        sha256: "a".repeat(64),
        compression: "none".into(),
        byte_size: 123,
        storage_key: "ab/abc".into(),
    };
    let capture = RawCapture {
        endpoint_key: "match",
        request_url: "https://example.test/page",
        request_key: "test:page",
        requested_at: chrono::Utc::now(),
        received_at: chrono::Utc::now(),
        http_status: 200,
        adapter_version: "1.0",
        parser_version: "1.0",
    };
    repo::persist_raw(
        &pool,
        "seed",
        "Seed",
        &artifact,
        &capture,
        "text/html",
    )
    .await
    .unwrap();
    let media_type: String =
        sqlx::query_scalar("SELECT media_type FROM raw.artifact WHERE content_sha256=$1")
            .bind(&artifact.sha256)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(media_type, "text/html");
}
```

Megjegyzés: az `ArtifactRef` mezőnevei a storage `object_store` típusát követik
(`put_json` visszatérési típusa); implementációkor a tényleges mezőnevek
ellenőrizendők (a típus az `object_store::put_json` visszatérési értéke).

- [ ] **Step 2: Futtatás — bukás.**

- [ ] **Step 3: Implementáció**

  - `scheduler::generate_jobs`: az event-ciklus elején

```rust
                if !matches!(
                    event.event_type.as_str(),
                    "discovery" | "refresh" | "canary" | "drift" | "backfill"
                ) {
                    tracing::warn!(event_type = %event.event_type, "ismeretlen eseménytípus, kihagyva");
                    continue;
                }
                if let Some(interval) = event.interval_minutes {
                    if interval != 1440 && interval != 10080 {
                        tracing::warn!(interval, "nem engedett interval_minutes, kihagyva");
                        continue;
                    }
                }
```

  - `persist_raw` szignatúra: új `media_type: &str` paraméter; az artifact
    insert `'application/json'` helyett `$6` bind; a tick hívása
    `&raw.content_type`.
  - `worker_status` / `doctor` SQL → `repo::worker_status_rows` /
    `repo::doctor_counts` (a SELECT-ek változatlanok, storage-be mozgatva).

- [ ] **Step 4: Futtatás — zöld + kapu + commit** (`feat: add S3a scheduler validation, media type and CLI repo calls`).

- [ ] **Step 5: STOP — jelentés.**

---

### Task 4: Identitás-kapuk (#9, #12, #13) + zárás

**Files:**
- Modify: `rust/bins/footballctl/src/main.rs`, `rust/crates/identity/src/review.rs`, `rust/crates/identity/src/lib.rs`, `rust/crates/storage/src/repositories/identity.rs`, `rust/crates/identity/src/error.rs`
- Test: `rust/crates/identity/tests/review.rs`, `rust/crates/identity/tests/crosswalk_check.rs` (új)

**Interfaces:**
- Produces: `repo::entity_kind(pool, entity_id) -> Result<Option<String>>`; `IdentityError::KindMismatch(String)`; `identity::crosswalk_conflicts(pool) -> Result<Vec<CrosswalkConflict>>`.

- [ ] **Step 1: Bukó tesztek**

`rust/crates/identity/tests/review.rs` végére (a fájl `seed_*` helperjeivel):

```rust
#[tokio::test]
async fn approve_with_mismatched_entity_kind_is_rejected() {
    let pool = test_pool().await;
    let source = seed_source(&pool, "src").await;
    // queue-sor: team kind
    let queue_id = seed_queue_row(&pool, source, "team", "ext-1").await;
    // választott entitás: player kind
    let player_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'player','Teszt')")
        .bind(player_id)
        .execute(&pool)
        .await
        .unwrap();

    let err = review::approve(&pool, queue_id, player_id, "cli", None)
        .await
        .expect_err("kind-eltérés hibát kell adjon");
    assert!(matches!(err, IdentityError::KindMismatch(_)), "ehelyett: {err:?}");
    let refs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM identity.source_entity_ref WHERE source_entity_id='ext-1'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(refs, 0, "elutasítás után nincs ref");
}
```

Megjegyzés: a `seed_queue_row` helper a meglévő tesztek review-sor seedjének
kivonata (a fájlban jelenleg inline szerepel; a lépés része a helper
kiszervezése a meglévő seed-minta alapján).

`rust/crates/identity/tests/crosswalk_check.rs` (új):

```rust
use football_identity::crosswalk_conflicts;
use football_storage::test_support::test_pool;
use uuid::Uuid;

#[tokio::test]
async fn conflicting_crosswalk_bindings_are_reported() {
    let pool = test_pool().await;
    for right in ["ext-a", "ext-b"] {
        sqlx::query(
            "INSERT INTO identity.crosswalk
             (id, left_source, left_external_id, right_source, right_external_id,
              rung, method, status, evidence)
             VALUES ($1,'left','L1','right',$2,'strong','import','active','teszt')",
        )
        .bind(Uuid::now_v7())
        .bind(right)
        .execute(&pool)
        .await
        .unwrap();
    }
    let conflicts = crosswalk_conflicts(&pool).await.unwrap();
    assert_eq!(conflicts.len(), 1, "egy ambiguous kötés");
    assert!(conflicts[0].conflicting_right_external_ids.len() >= 2);
}
```

- [ ] **Step 2: Futtatás — bukás.**

- [ ] **Step 3: Implementáció**

  - `repo::entity_kind`:

```rust
pub async fn entity_kind(pool: &PgPool, entity_id: Uuid) -> Result<Option<String>> {
    sqlx::query_scalar("SELECT kind FROM core.entity WHERE id = $1")
        .bind(entity_id)
        .fetch_optional(pool)
        .await
        .context("entitás kind lekérés")
}
```

  - `IdentityError::KindMismatch(String)` (error.rs) + `review::approve`-ban az
    `entity_exists` után a kind-ellenőrzés (a spec 4. szekció kódja).
  - `identity::crosswalk_conflicts`:

```rust
#[derive(Debug, Clone)]
pub struct CrosswalkConflict {
    pub left_source: String,
    pub left_external_id: String,
    pub conflicting_right_external_ids: Vec<String>,
}

pub async fn crosswalk_conflicts(pool: &sqlx::PgPool) -> anyhow::Result<Vec<CrosswalkConflict>> {
    let rows = sqlx::query(
        "SELECT left_source, left_external_id, array_agg(DISTINCT right_external_id) AS rights
         FROM identity.crosswalk
         WHERE status = 'active'
         GROUP BY left_source, left_external_id
         HAVING count(DISTINCT right_external_id) > 1",
    )
    .fetch_all(pool)
    .await
    .context("crosswalk konfliktus lekérés")?;
    rows.iter()
        .map(|row| {
            Ok(CrosswalkConflict {
                left_source: row.try_get("left_source").context("left_source")?,
                left_external_id: row.try_get("left_external_id").context("left_external_id")?,
                conflicting_right_external_ids: row.try_get("rights").context("rights")?,
            })
        })
        .collect()
}
```

  (sqlx `Row` + `try_get`; a teszt az egy konfliktust várja).
  - CLI: `IdentityAction::CrosswalkCheck` → a konfliktusok kiírása; ha nem
    üres, nem-0 exit; az IngestFixture `home/away_identity_exact` a
    `football_worker::tick::resolve_team_identity` eredményéből (a hardcode
    törlése).

- [ ] **Step 4: Zöld + teljes kapu.** `./scripts/check.sh` exit 0; demo
  `cargo run -q -p footballctl -- day --date 2026-11-11` → 20/20 (test DB-n a
  seedekkel, vagy a meglévő ellenőrzéssel).

- [ ] **Step 5: Checkpoint + commitok.** A checkpoint végére az S3a szekció
  (kapuk + bizonyítékok + halasztott #10/#11 + visszaállítás: branch-revert,
  séma érintetlen); commit `docs: close S3a entry gates checkpoint`.

- [ ] **Step 6: STOP — zárójelentés.**

---

## Önreview-jegyzet (a terv írójának)

- **Spec-lefedettség:** #1/#2/#3 → Task 1; #4/#7 → Task 2; #5/#6/#8 → Task 3; #9/#12/#13 → Task 4; a halasztott #10/#11 a spec szerint kimaradnak.
- **Típus-konzisztencia:** `RateLimited(Option<Duration>)`, `backoff_delay(u32, u32, impl FnMut() -> u32)`, `requeue_with_delay(pool, job_id, due_at)`, `count_job_fetch_attempts(pool, job_id)`, `persist_raw(..., media_type)`, `entity_kind`, `crosswalk_conflicts` mindenhol egységes.
- **Ismert egyszerűsítés:** a RateLimited a `HttpStatus(429)` helyére lép minden matchben; a robots-tesztek fixture-kulcsai az origin-formára váltanak; a crosswalk-uniqueness az S3b importjának előkészítése (önálló validáció + CLI).
