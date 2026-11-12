//! Robots-kapu: a tick része, ledgerbe NEM ír (az elszámolás a tickben történik).
//!
//! Éles (live) fail-closed szemantika (RFC 9309): ha a robots.txt lekérése
//! transport-hibába fut (DNS, időtúllépés, kapcsolat-elutasítás), a verdict
//! `blocked` — a hiányzó robots NEM jelent szabadságot. Csak a bizonyítottan
//! nem létező robots ad `no_robots`-ot: HTTP 404, illetve fixture módban a
//! `FetchError::NotFound` (hiányzó fixture-kulcs). HTTP 403 és minden egyéb
//! hiba szintén `blocked`.
//!
//! A matcher UA-tokenje számjegymentes (`football-app`), mert a robotstxt
//! parser a robots.txt User-agent értékeit [a-zA-Z_-] karakterekre csonkolja:
//! a "football-app-v2" csoportnév sosem illeszkedne a számjegyes tokenre,
//! így a célzottan tiltó oldal mellett is Allowed születne (robots-megkerülés).
//! A HTTP User-agent fejléc a RustHttpFetcherben verziózott marad.
//!
//! Adaptáció a briefhez képest: a robotstxt 0.3.0 crate-ben nincs `Robot::new` /
//! `parsable` / `matches` API (Google-parser port: `DefaultMatcher` +
//! `one_agent_allowed_by_robots`). A Google-parser elnéző: a szemét sorokat
//! kihagyja, parse-hibát nem jelez. Ezért a fail-closed ág saját érvényesség-kapu:
//! 1. a nyers robots-bájtoknak érvényes UTF-8-nak kell lenniük (különben blocked),
//! 2. a nem üres szabályszövegnek tartalmaznia kell legalább egy `user-agent:`
//!    direktívát (különben blocked).

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use football_domain::error::FetchError;
use football_ingest::fetch::{FetchLimits, FetchRequest, Fetcher};
use football_storage::repositories::worker::{RobotsCacheRow, get_robots_cache, put_robots_cache};
use sqlx::PgPool;
use std::time::Duration as StdDuration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RobotsVerdict {
    Allowed,
    NoRobots,
    Blocked,
}

// Számjegymentes token: a robotstxt a User-agent értékeket [a-zA-Z_-]-re
// csonkolja, ezért a "-v2" végződésű token sosem illeszkedne (lásd fejléc).
const UA: &str = "football-app";

/// Fail-closed érvényesség-kapu: a nem üres robots-szöveg csak akkor parse-olható,
/// ha van benne `user-agent:` direktíva. A matcher önmagában a direktíva nélküli
/// szemetet is megengedné, ezért ezt itt külön elzárjuk.
fn has_user_agent_directive(rules_text: &str) -> bool {
    rules_text
        .lines()
        .any(|line| line.trim().to_ascii_lowercase().starts_with("user-agent:"))
}

pub async fn check(
    pool: &PgPool,
    fetcher: &dyn Fetcher,
    source_slug: &str,
    url: &str,
) -> Result<RobotsVerdict> {
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
            let limits = FetchLimits {
                max_bytes: 5_242_880,
                timeout: StdDuration::from_secs(30),
            };
            let (verdict, rules_text) = match fetcher.fetch(&req, &limits).await {
                // Szigorú UTF-8: érvénytelen bájtsor = parse-olhatatlan robots → fail-closed.
                Ok(raw) => match String::from_utf8(raw.bytes) {
                    Ok(text) => ("allowed".to_string(), text),
                    Err(_) => ("blocked".to_string(), String::new()),
                },
                Err(FetchError::HttpStatus(404)) => ("no_robots".to_string(), String::new()),
                Err(FetchError::HttpStatus(403)) => ("blocked".to_string(), String::new()),
                // Fixture-módus: a hiányzó robots-fájl NotFound hibát ad = nincs robots.
                // Éles módban a RustHttpFetcher HttpStatus(404) ága adja ugyanazt.
                Err(FetchError::NotFound(_)) => ("no_robots".to_string(), String::new()),
                // Éles fail-closed (RFC 9309): a transport-hiba (DNS, időtúllépés,
                // kapcsolat-elutasítás) NEM jelenthet szabadságot → blocked.
                Err(FetchError::Transport(_)) => ("blocked".to_string(), String::new()),
                Err(_) => ("blocked".to_string(), String::new()),
            };
            let row = RobotsCacheRow {
                host: host.clone(),
                fetched_at: now,
                verdict,
                rules_text,
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
    if !has_user_agent_directive(&rules.rules_text) {
        return Ok(RobotsVerdict::Blocked);
    }
    let mut matcher = robotstxt::DefaultMatcher::default();
    if matcher.one_agent_allowed_by_robots(&rules.rules_text, UA, url) {
        Ok(RobotsVerdict::Allowed)
    } else {
        Ok(RobotsVerdict::Blocked)
    }
}
