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
    let (_dir, fetcher) = fetcher_with(&[(
        "demo-synthetic:robots:example.test",
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
        "demo-synthetic:robots:example.test",
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
        "demo-synthetic:robots:example.test",
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
        "demo-synthetic:robots:example.test",
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
    // és a blocked verdict a cache-be kerül (24 órás TTL-lel).
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
        "demo-synthetic:robots:example.test",
        b"ez nem robots direktiva",
    )]);
    let v = robots::check(&pool, &fetcher, "demo-synthetic", "https://example.test/x")
        .await
        .unwrap();
    assert_eq!(v, RobotsVerdict::Blocked);
}
