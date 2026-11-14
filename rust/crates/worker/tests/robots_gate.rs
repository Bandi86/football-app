use async_trait::async_trait;
use football_domain::error::FetchError;
use football_ingest::fetch::{FetchLimits, FetchRequest, Fetcher, FixtureFetcher, RawPayload};
use football_storage::test_support::test_pool;
use football_worker::robots::{self, RobotsVerdict};
use std::collections::HashMap;

async fn seed_policy(pool: &sqlx::PgPool) {
    // ugyanaz a seed, mint a Task 3 tesztekben (source_policy insert fixture transporttal)
    football_storage::repositories::worker::upsert_policy(
        pool,
        &football_domain::policy::SourcePolicy {
            source_slug: "demo-synthetic".into(),
            transport: football_domain::policy::Transport::Fixture,
            mode: football_domain::policy::SourceMode::Shadow,
            daily_budget: 10,
            min_delay_ms: 0,
            jitter_ms: 0,
            breaker_threshold: 3,
            breaker_cooldown_secs: 60,
            adapter_version: "1.0".into(),
            parser_version: "1.0".into(),
            domains: vec![],
            events: vec![],
            enabled: true,
            max_bytes: 5_242_880,
        },
    )
    .await
    .unwrap();
}

fn fetcher_with(files: &[(&str, &[u8])]) -> (tempfile::TempDir, FixtureFetcher) {
    let dir = tempfile::tempdir().unwrap();
    let mut map = HashMap::new();
    for (key, content) in files {
        // Az origin-kulcs '/' és ':' karaktereket tartalmaz: fájlnévként sanitizáljuk.
        let safe = key.replace(|c: char| !c.is_ascii_alphanumeric(), "_");
        let path = dir.path().join(format!("{safe}.json"));
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
    let (_dir, fetcher) = fetcher_with(&[(
        "demo-synthetic:robots:https://example.test",
        b"User-agent: *\nDisallow: /private\n",
    )]);
    let v = robots::check(
        &pool,
        &fetcher,
        "demo-synthetic",
        "https://example.test/public",
    )
    .await
    .unwrap();
    assert_eq!(v, RobotsVerdict::Allowed);
    // cache-ből: a fetcher már nem kell (üres fetcher is jó)
    let empty = FixtureFetcher::new(HashMap::new());
    let v2 = robots::check(
        &pool,
        &empty,
        "demo-synthetic",
        "https://example.test/public",
    )
    .await
    .unwrap();
    assert_eq!(v2, RobotsVerdict::Allowed);
    let denied = robots::check(
        &pool,
        &empty,
        "demo-synthetic",
        "https://example.test/private",
    )
    .await
    .unwrap();
    assert_eq!(denied, RobotsVerdict::Blocked);
}

#[tokio::test]
async fn robots_404_means_no_robots() {
    let pool = test_pool().await;
    seed_policy(&pool).await;
    // a FixtureFetcher hiányzó kulcsa NotFound hibát ad — a 404-esetet a robots-kapu
    // a Fetcher HttpStatus(404) ágán kezeli; FixtureFetcher-rel a hiányzó robots-fájl
    // NoRobots verdictet ad (dokumentált viselkedés: fixture módban a nem létező robots = nincs robots)
    let empty = FixtureFetcher::new(HashMap::new());
    let v = robots::check(&pool, &empty, "demo-synthetic", "https://example.test/x")
        .await
        .unwrap();
    assert_eq!(v, RobotsVerdict::NoRobots);
}

#[tokio::test]
async fn unparseable_robots_fail_closed() {
    let pool = test_pool().await;
    seed_policy(&pool).await;
    // Adaptáció: a brief \xff-es str-literálja nem fordul Rustban (hex escape
    // csak bájtsztringben lehet > 0x7f), ezért a fixture bájtként íródik.
    let (_dir, fetcher) = fetcher_with(&[(
        "demo-synthetic:robots:https://example.test",
        b"\x00\x01\xff nem robots",
    )]);
    let v = robots::check(&pool, &fetcher, "demo-synthetic", "https://example.test/x")
        .await
        .unwrap();
    assert_eq!(v, RobotsVerdict::Blocked);
}

#[tokio::test]
async fn specific_ua_group_blocks_despite_permissive_star() {
    // Negatív teszt (UA-token ruling): a robots.txt a mi botunkat célzottan tiltó
    // "football-app" csoportot tartalmaz, a "*" csoport megengedő. A matcher
    // UA-tokenje számjegymentes ("football-app"), így a specifikus csoportnak
    // illeszkednie kell — Blocked, nem Allowed (robots-megkerülés tilos).
    let pool = test_pool().await;
    seed_policy(&pool).await;
    let (_dir, fetcher) = fetcher_with(&[(
        "demo-synthetic:robots:https://example.test",
        b"User-agent: football-app\nDisallow: /\n\nUser-agent: *\nDisallow:\n",
    )]);
    let v = robots::check(&pool, &fetcher, "demo-synthetic", "https://example.test/x")
        .await
        .unwrap();
    assert_eq!(v, RobotsVerdict::Blocked);
}

#[tokio::test]
async fn star_disallow_still_blocks() {
    // A "*" csoport teljes tiltása a számjegymentes UA-tokennel is Blocked.
    let pool = test_pool().await;
    seed_policy(&pool).await;
    let (_dir, fetcher) = fetcher_with(&[(
        "demo-synthetic:robots:https://example.test",
        b"User-agent: *\nDisallow: /\n",
    )]);
    let v = robots::check(&pool, &fetcher, "demo-synthetic", "https://example.test/x")
        .await
        .unwrap();
    assert_eq!(v, RobotsVerdict::Blocked);
}

/// Teszt-only fetcher: minden kérésre transport-hibát ad (élő DNS/időtúllépés
/// szimulációja hálózat nélkül).
struct TransportFailFetcher;

#[async_trait]
impl Fetcher for TransportFailFetcher {
    async fn fetch(
        &self,
        _req: &FetchRequest,
        _limits: &FetchLimits,
    ) -> Result<RawPayload, FetchError> {
        Err(FetchError::Transport("szimulált transport-hiba".into()))
    }
}

#[tokio::test]
async fn transport_error_fails_closed_and_cached() {
    // Élő fail-closed (RFC 9309): a transport-hiba Blocked verdictet ad,
    // és a blocked verdict a cache-be kerül (átmeneti hibánál rövid TTL-lel).
    let pool = test_pool().await;
    seed_policy(&pool).await;
    let v = robots::check(
        &pool,
        &TransportFailFetcher,
        "demo-synthetic",
        "https://example.test/x",
    )
    .await
    .unwrap();
    assert_eq!(v, RobotsVerdict::Blocked);
    // Cache-bizonyíték: ha a blocked verdict NEM cache-elődne, az üres
    // FixtureFetcher NotFound hibája NoRobots-ot adna — így viszont Blocked marad.
    let empty = FixtureFetcher::new(HashMap::new());
    let v2 = robots::check(&pool, &empty, "demo-synthetic", "https://example.test/x")
        .await
        .unwrap();
    assert_eq!(v2, RobotsVerdict::Blocked);
}

#[tokio::test]
async fn garbage_without_user_agent_fail_closed() {
    // Adaptáció-teszt: a robotstxt 0.3.0 matcher elnéző (a szemét sorokat kihagyja),
    // ezért a kapu saját fail-closed érvényesség-kapuja is kell: érvényes UTF-8,
    // de user-agent direktíva nélküli szöveg = Blocked.
    let pool = test_pool().await;
    seed_policy(&pool).await;
    let (_dir, fetcher) = fetcher_with(&[(
        "demo-synthetic:robots:https://example.test",
        b"ez nem robots direktiva",
    )]);
    let v = robots::check(&pool, &fetcher, "demo-synthetic", "https://example.test/x")
        .await
        .unwrap();
    assert_eq!(v, RobotsVerdict::Blocked);
}

#[tokio::test]
async fn sitemap_only_robots_is_allowed() {
    // Legitim, User-agent szekció nélküli fájl (csak Sitemap): nem tiltás.
    let pool = test_pool().await;
    seed_policy(&pool).await;
    let (_dir, fetcher) = fetcher_with(&[(
        "demo-synthetic:robots:https://example.test",
        b"Sitemap: https://example.test/sitemap.xml\n",
    )]);
    let v = robots::check(
        &pool,
        &fetcher,
        "demo-synthetic",
        "https://example.test/page",
    )
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
    let v = robots::check(
        &pool,
        &fetcher,
        "demo-synthetic",
        "https://example.test/page",
    )
    .await
    .unwrap();
    assert_eq!(v, RobotsVerdict::Allowed, "a BOM nem blokkol");
}

#[tokio::test]
async fn transient_error_uses_short_ttl() {
    let pool = test_pool().await;
    seed_policy(&pool).await;
    let v = robots::check(
        &pool,
        &TransportFailFetcher,
        "demo-synthetic",
        "https://example.test/page",
    )
    .await
    .unwrap();
    assert_eq!(v, RobotsVerdict::Blocked, "átmeneti hiba = fail-closed");
    let ttl_hours: f64 = sqlx::query_scalar(
        "SELECT (EXTRACT(EPOCH FROM (expires_at - fetched_at)) / 3600)::float8 FROM ops.robots_cache",
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
    robots::check(
        &pool,
        &fetcher,
        "demo-synthetic",
        "https://example.test/page",
    )
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
        chrono::Utc::now().date_naive(),
    )
    .await
    .unwrap();
    assert_eq!(counted, 0, "a robots-fetch nem számít a napi kvótába");
}
