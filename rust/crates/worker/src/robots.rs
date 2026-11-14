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
use football_domain::policy::FetchOutcome;
use football_ingest::fetch::{FetchLimits, FetchRequest, Fetcher};
use football_storage::repositories::worker::{
    LedgerEntry, RobotsCacheRow, get_robots_cache, insert_ledger, put_robots_cache,
};
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

/// UA-szekció nélküli fájlban csak legitim, csoportfüggetlen direktívák
/// engedettek (Sitemap/Host/komment/üres). Más szemét sor → fail-closed.
fn only_benign_directives(rules_text: &str) -> bool {
    rules_text.lines().all(|line| {
        let lower = line.trim().to_ascii_lowercase();
        lower.is_empty()
            || lower.starts_with('#')
            || lower.starts_with("sitemap:")
            || lower.starts_with("host:")
    })
}

pub async fn check(
    pool: &PgPool,
    fetcher: &dyn Fetcher,
    source_slug: &str,
    url: &str,
) -> Result<RobotsVerdict> {
    let parsed = url::Url::parse(url).context("robots: URL-parse")?;
    let origin = parsed.origin().ascii_serialization();
    let now = Utc::now();

    let rules = match get_robots_cache(pool, &origin).await? {
        Some(row) if row.expires_at > now => row,
        _ => {
            let robots_url = format!("{origin}/robots.txt");
            let req = FetchRequest {
                url: robots_url,
                request_key: format!("{source_slug}:robots:{origin}"),
                endpoint_key: "robots".into(),
            };
            let limits = FetchLimits {
                max_bytes: 5_242_880,
                timeout: StdDuration::from_secs(30),
            };
            let started = std::time::Instant::now();
            let fetched = fetcher.fetch(&req, &limits).await;
            let latency_ms = i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX);
            let (verdict, rules_text, transient, outcome, http_status, byte_size) = match fetched {
                Ok(raw) => {
                    let size = i64::try_from(raw.bytes.len()).ok();
                    let status = i16::try_from(raw.http_status).ok();
                    match String::from_utf8(raw.bytes) {
                        // BOM-strip: a cache-be és a parsernek is tisztított szöveg megy.
                        Ok(text) => (
                            "allowed".to_string(),
                            text.trim_start_matches('\u{feff}').to_string(),
                            false,
                            FetchOutcome::Success,
                            status,
                            size,
                        ),
                        Err(_) => (
                            "blocked".to_string(),
                            String::new(),
                            false,
                            FetchOutcome::TransportError,
                            status,
                            size,
                        ),
                    }
                }
                Err(FetchError::HttpStatus(404)) => (
                    "no_robots".to_string(),
                    String::new(),
                    false,
                    FetchOutcome::HttpError,
                    Some(404),
                    None,
                ),
                Err(FetchError::HttpStatus(403)) => (
                    "blocked".to_string(),
                    String::new(),
                    false,
                    FetchOutcome::HttpError,
                    Some(403),
                    None,
                ),
                // Fixture-módus: a hiányzó robots-fájl NotFound hibát ad = nincs robots.
                // Éles módban a RustHttpFetcher HttpStatus(404) ága adja ugyanazt.
                Err(FetchError::NotFound(_)) => (
                    "no_robots".to_string(),
                    String::new(),
                    false,
                    FetchOutcome::TransportError,
                    None,
                    None,
                ),
                // Éles fail-closed (RFC 9309): a transport-hiba NEM jelenthet
                // szabadságot → blocked, de rövid TTL-lel (a helyreállás ne
                // maradjon blokkolva egy napig).
                Err(FetchError::Transport(_)) => (
                    "blocked".to_string(),
                    String::new(),
                    true,
                    FetchOutcome::TransportError,
                    None,
                    None,
                ),
                Err(FetchError::HttpStatus(status)) => (
                    "blocked".to_string(),
                    String::new(),
                    status >= 500,
                    FetchOutcome::HttpError,
                    i16::try_from(status).ok(),
                    None,
                ),
                Err(FetchError::Timeout) => (
                    "blocked".to_string(),
                    String::new(),
                    true,
                    FetchOutcome::Timeout,
                    None,
                    None,
                ),
                Err(_) => (
                    "blocked".to_string(),
                    String::new(),
                    true,
                    FetchOutcome::TransportError,
                    None,
                    None,
                ),
            };
            // Nem-számláló ledger-sor minden robots-kísérletre (a kvóta-számláló
            // a :robots: kulcsokat kizárja).
            let entry = LedgerEntry {
                source_slug: source_slug.to_string(),
                request_key: req.request_key.clone(),
                attempted_at: now,
                outcome,
                http_status,
                byte_size,
                latency_ms: Some(latency_ms),
                artifact_hash: None,
                job_id: None,
            };
            insert_ledger(pool, &entry).await?;
            let ttl_hours = if transient { 1 } else { 24 };
            let row = RobotsCacheRow {
                host: origin.clone(),
                fetched_at: now,
                verdict,
                rules_text,
                expires_at: now + Duration::hours(ttl_hours),
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
        // Nincs UA szekció: csak legitim direktívák (Sitemap/Host/komment)
        // esetén számít „nincs robots”-nak; más szemét fail-closed.
        return if only_benign_directives(&rules.rules_text) {
            Ok(RobotsVerdict::NoRobots)
        } else {
            Ok(RobotsVerdict::Blocked)
        };
    }
    let mut matcher = robotstxt::DefaultMatcher::default();
    if matcher.one_agent_allowed_by_robots(&rules.rules_text, UA, url) {
        Ok(RobotsVerdict::Allowed)
    } else {
        Ok(RobotsVerdict::Blocked)
    }
}
