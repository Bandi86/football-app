//! `research` forrás-adapter (S38): a football-research fájl-alapú JSON API-ját
//! olvassa (nincs hálózat, `Transport::LocalFile`).
//!
//! A napi envelope (`daily/YYYY/MM/<date>.json`) adja a meccseket, a
//! csapat-identitást, a játékvezetőt és a versenyt; az oddsok a meccs-detail
//! fájlokból (`matches/YYYY/MM/DD/<slug>.json`, `odds.markets`) jönnek.
//! A terv egy napi kérést és a naphoz tartozó, oddsot tartalmazó
//! detail-fájlokat tartalmaz.

use std::collections::BTreeSet;
use std::path::PathBuf;

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use football_domain::error::ParseError;
use football_domain::identity::normalize_country_code;
use football_domain::policy::{FetchJob, Transport};
use serde::Deserialize;
use serde_json::json;

use crate::adapter::{AdapterMeta, Observation, ParseCtx, SourceAdapter};
use crate::fetch::{FetchRequest, RawPayload};

pub const SOURCE_SLUG: &str = "research";
pub const ADAPTER_VERSION: &str = "1.0.0";
pub const PARSER_VERSION: &str = "1.0.0";

/// A football-research API gyökere (az `api/v1` könyvtár).
pub fn default_api_root() -> PathBuf {
    if let Ok(dir) = std::env::var("FOOTBALL_RESEARCH_API_DIR") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/susleczandras".into());
    PathBuf::from(home).join("Library/Application Support/FootballResearch/api/v1")
}

#[derive(Debug, Clone)]
pub struct ResearchAdapter {
    api_root: PathBuf,
}

impl Default for ResearchAdapter {
    fn default() -> Self {
        Self::new(default_api_root())
    }
}

impl ResearchAdapter {
    pub fn new(api_root: PathBuf) -> Self {
        Self { api_root }
    }

    /// `api/v1/daily/2026/11/2026-11-20.json` (a gyökérhez viszonyítva).
    pub fn daily_rel(date: NaiveDate) -> String {
        format!(
            "daily/{:04}/{:02}/{}.json",
            date.year(),
            date.month(),
            date.format("%Y-%m-%d")
        )
    }

    fn odds_rel(link: &str) -> Option<String> {
        let rel = link.trim_start_matches("/api/v1/").trim_start_matches('/');
        if rel.starts_with("matches/") && rel.ends_with(".json") {
            Some(rel.to_string())
        } else {
            None
        }
    }
}

#[derive(Debug, Deserialize)]
struct DailyDoc {
    meta: DailyMeta,
    data: Vec<DayMatch>,
}

#[derive(Debug, Deserialize)]
struct DailyMeta {
    generated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct DayMatch {
    uid: String,
    kickoff: Kickoff,
    status: Status,
    competition: Competition,
    teams: Teams,
    #[serde(default)]
    score: Option<Score>,
    #[serde(default)]
    referee: Option<Referee>,
    #[serde(default)]
    links: Option<Links>,
    #[serde(default)]
    coverage: Option<Coverage>,
}

#[derive(Debug, Deserialize)]
struct Kickoff {
    utc: Option<DateTime<Utc>>,
    #[serde(default)]
    timezone: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Status {
    code: String,
}

#[derive(Debug, Deserialize)]
struct Competition {
    name: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    country_name: Option<String>,
    #[serde(default)]
    round: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Teams {
    home: TeamRef,
    away: TeamRef,
}

#[derive(Debug, Deserialize)]
struct TeamRef {
    /// A research forrásonként hol szövegként, hol számként adja az id-t.
    #[serde(deserialize_with = "de_id_string")]
    id: String,
    name: String,
}

fn de_id_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as _;
    match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::String(value) => Ok(value),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        other => Err(D::Error::custom(format!(
            "csapat-id csak szöveg vagy szám lehet: {other}"
        ))),
    }
}

#[derive(Debug, Deserialize)]
struct Score {
    #[serde(default)]
    home: Option<i16>,
    #[serde(default)]
    away: Option<i16>,
    #[serde(default)]
    halftime: Option<Halftime>,
}

#[derive(Debug, Deserialize)]
struct Halftime {
    #[serde(default)]
    home: Option<i16>,
    #[serde(default)]
    away: Option<i16>,
}

#[derive(Debug, Deserialize)]
struct Referee {
    name: String,
}

#[derive(Debug, Deserialize)]
struct Links {
    #[serde(default)]
    #[serde(rename = "self")]
    self_link: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Coverage {
    #[serde(default)]
    odds: bool,
}

/// A detal-fájl (meccs) releváns része az oddsokhoz.
#[derive(Debug, Deserialize)]
struct DetailDoc {
    #[serde(default)]
    uid: Option<String>,
    #[serde(default)]
    odds: Option<OddsSection>,
}

#[derive(Debug, Deserialize)]
struct OddsSection {
    #[serde(default)]
    phase: Option<String>,
    #[serde(default)]
    observed_at: Option<String>,
    #[serde(default)]
    by_bookmaker: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default)]
    markets: Option<serde_json::Map<String, serde_json::Value>>,
}

impl SourceAdapter for ResearchAdapter {
    fn meta(&self) -> AdapterMeta {
        AdapterMeta {
            source_slug: SOURCE_SLUG.into(),
            adapter_version: ADAPTER_VERSION.into(),
            parser_version: PARSER_VERSION.into(),
            domains: vec!["day".into()],
            transport: Transport::LocalFile,
        }
    }

    fn plan(&self, job: &FetchJob) -> Vec<FetchRequest> {
        let Ok(date) = NaiveDate::parse_from_str(&job.time_window, "%Y-%m-%d") else {
            return Vec::new();
        };
        let daily_rel = Self::daily_rel(date);
        let mut requests = vec![FetchRequest {
            url: daily_rel.clone(),
            request_key: format!("day:{date}"),
            endpoint_key: "daily".into(),
            ready_marker: None,
            bearer_token: None,
        }];

        // Az oddsot tartalmazó meccs-detail fájlok: a napi envelope-ből
        // tudjuk, melyik meccshez van odds (`coverage.odds`).
        let path = self.api_root.join(&daily_rel);
        if let Ok(bytes) = std::fs::read(&path)
            && let Ok(doc) = serde_json::from_slice::<DailyDoc>(&bytes)
        {
            let mut seen = BTreeSet::new();
            for m in &doc.data {
                if !m.coverage.as_ref().map(|c| c.odds).unwrap_or(false) {
                    continue;
                }
                let Some(link) = m.links.as_ref().and_then(|l| l.self_link.as_deref()) else {
                    continue;
                };
                let Some(rel) = Self::odds_rel(link) else {
                    continue;
                };
                if !seen.insert(rel.clone()) {
                    continue;
                }
                requests.push(FetchRequest {
                    url: rel.clone(),
                    request_key: format!("odds:{}", m.uid),
                    endpoint_key: "odds".into(),
                    ready_marker: None,
                    bearer_token: None,
                });
            }
        }
        requests
    }

    fn parse(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError> {
        if ctx.request_key.starts_with("day:") {
            self.parse_day(raw)
        } else if ctx.request_key.starts_with("odds:") {
            self.parse_odds(raw, ctx)
        } else {
            Err(ParseError::Parse(format!(
                "ismeretlen research kérés: {}",
                ctx.request_key
            )))
        }
    }
}

impl ResearchAdapter {
    fn parse_day(&self, raw: &RawPayload) -> Result<Vec<Observation>, ParseError> {
        let doc: DailyDoc = serde_json::from_slice(&raw.bytes)
            .map_err(|e| ParseError::SchemaDrift(format!("research napi envelope: {e}")))?;
        let captured_at = doc.meta.generated_at;
        let mut observations = Vec::new();
        let mut seen_teams = BTreeSet::new();

        for m in &doc.data {
            let source_prefix = m.uid.split(':').next().unwrap_or("research");
            let home_key = format!("{source_prefix}:{}", m.teams.home.id);
            let away_key = format!("{source_prefix}:{}", m.teams.away.id);
            if home_key == away_key {
                continue;
            }
            let country_code = competition_country_code(&m.competition);
            let country_name = m
                .competition
                .country_name
                .clone()
                .or_else(|| m.competition.country.clone())
                .unwrap_or_default();
            let comp_name = m
                .competition
                .display_name
                .clone()
                .unwrap_or_else(|| m.competition.name.clone());
            let (gender, age_group, branch) = team_context(&comp_name);

            for (key, team) in [(&home_key, &m.teams.home), (&away_key, &m.teams.away)] {
                if !seen_teams.insert(key.clone()) {
                    continue;
                }
                observations.push(Observation {
                    entity_kind: "team".into(),
                    aspect: "team_identity".into(),
                    natural_key: key.clone(),
                    source_record_key: key.clone(),
                    observed_at: captured_at,
                    payload: json!({
                        "source_entity_id": key,
                        "name": team.name,
                        "country_code": country_code,
                        "gender": gender,
                        "age_group": age_group,
                        "branch": branch,
                    }),
                });
            }

            let Some(kickoff_at) = m.kickoff.utc else {
                continue;
            };
            let status = match m.status.code.as_str() {
                "finished" => "finished",
                "live" => "live",
                "postponed" => "postponed",
                "cancelled" => "cancelled",
                "awarded" => "awarded",
                _ => "scheduled",
            };
            let result = result_payload(status, m.score.as_ref());
            let season_label = season_label(kickoff_at);
            observations.push(Observation {
                entity_kind: "match".into(),
                aspect: "match_core".into(),
                natural_key: m.uid.clone(),
                source_record_key: m.uid.clone(),
                observed_at: captured_at,
                payload: json!({
                    "canonical_match_id": null,
                    "source_home_id": home_key,
                    "source_away_id": away_key,
                    "kickoff_at": kickoff_at,
                    "status": status,
                    "result": result,
                    "round_label": m.competition.round,
                    "competition_timezone": m
                        .kickoff
                        .timezone
                        .clone()
                        .unwrap_or_else(|| "Europe/Budapest".into()),
                    "competition": {
                        "name": comp_name,
                        "country_code": country_code,
                        "country_name": country_name,
                        "tier": 1,
                        "priority": 30,
                    },
                    "season": { "label": season_label },
                    "source_code": SOURCE_SLUG,
                    "captured_at": captured_at,
                }),
            });

            if let Some(referee) = &m.referee
                && !referee.name.trim().is_empty()
            {
                observations.push(Observation {
                    entity_kind: "match".into(),
                    aspect: "match_referee".into(),
                    natural_key: m.uid.clone(),
                    source_record_key: format!("{}:referee", m.uid),
                    observed_at: captured_at,
                    payload: json!({ "name": referee.name.trim() }),
                });
            }
        }

        Ok(observations)
    }

    fn parse_odds(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError> {
        let doc: DetailDoc = serde_json::from_slice(&raw.bytes)
            .map_err(|e| ParseError::SchemaDrift(format!("research detail: {e}")))?;
        let Some(odds) = doc.odds else {
            return Ok(Vec::new());
        };
        let phase = match odds.phase.as_deref() {
            Some("closing") => "closing",
            _ => "opening",
        };
        // Bookmaker: az első by_bookmaker kulcs (pl. tipplap), különben research.
        let bookmaker = odds
            .by_bookmaker
            .as_ref()
            .and_then(|m| m.keys().next())
            .cloned()
            .unwrap_or_else(|| SOURCE_SLUG.to_string());
        let Some(markets) = odds.markets else {
            return Ok(Vec::new());
        };
        let mut quotes = Vec::new();
        for (market_name, outcomes) in &markets {
            let Some(outcomes) = outcomes.as_object() else {
                continue;
            };
            let (market, line) = normalize_market(market_name);
            for (outcome_name, price) in outcomes {
                let Some(price) = price.as_f64() else {
                    continue;
                };
                if !(1.0..=1000.0).contains(&price) {
                    continue;
                }
                let Some(outcome) = normalize_outcome(&market, outcome_name) else {
                    continue;
                };
                quotes.push(json!({
                    "bookmaker": bookmaker,
                    "market": market,
                    "outcome": outcome,
                    "line": line,
                    "phase": phase,
                    "price": price,
                }));
            }
        }
        if quotes.is_empty() {
            return Ok(Vec::new());
        }
        // A forrás-oldali megfigyelési idő (pl. "2026-11-18 17:46:23");
        // ha nem értelmezhető, a feldolgozás ideje lesz a rögzítés.
        let observed_at = odds
            .observed_at
            .as_deref()
            .and_then(|value| {
                chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").ok()
            })
            .map(|naive| naive.and_utc())
            .unwrap_or_else(Utc::now);
        let match_key = doc
            .uid
            .unwrap_or_else(|| ctx.request_key.trim_start_matches("odds:").to_string());
        let market_count = quotes
            .iter()
            .filter_map(|q| q.get("market").and_then(|m| m.as_str()))
            .collect::<BTreeSet<_>>()
            .len() as i64;
        Ok(vec![Observation {
            entity_kind: "match".into(),
            aspect: "match_odds".into(),
            natural_key: match_key.clone(),
            // A napi match_core observation `source_record_key`-jével kell
            // egyeznie, különben az összeállítás nem kapcsolja a meccshez.
            source_record_key: match_key.clone(),
            observed_at,
            payload: json!({
                "canonical_match_id": null,
                "quotes": quotes,
                "market_count": market_count,
            }),
        }])
    }
}

fn competition_country_code(competition: &Competition) -> String {
    let raw = competition
        .country
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .unwrap_or("XX");
    let normalized = normalize_country_code(&raw.to_ascii_uppercase());
    // A research a brit hazai nemzeteket külön kóddal adja; a v2-ben ezek
    // a GB alatt élnek (a skót versenyek is).
    match normalized.as_str() {
        "WAL" | "NIR" | "SCO" | "GB-SCT" | "GB-WLS" | "GB-NIR" => "GB".into(),
        other => other.into(),
    }
}

fn team_context(competition_name: &str) -> (&'static str, &'static str, &'static str) {
    let lower = competition_name.to_lowercase();
    let women = [
        "(w)",
        " women",
        "női",
        "frauen",
        "femenina",
        "femminile",
        "dames",
        "kvinde",
        "damallsvenskan",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    if women {
        return ("female", "senior", "women");
    }
    for group in ["u19", "u21", "u20", "u18", "u17", "u16", "u15"] {
        if lower.contains(group) {
            return ("male", group, "youth");
        }
    }
    ("male", "senior", "senior")
}

fn season_label(kickoff_at: DateTime<Utc>) -> String {
    let year = kickoff_at.year();
    let month = kickoff_at.month();
    if month >= 7 {
        format!("{year}/{:02}", (year + 1) % 100)
    } else {
        format!("{}/{:02}", year - 1, year % 100)
    }
}

fn result_payload(status: &str, score: Option<&Score>) -> Option<serde_json::Value> {
    if !matches!(status, "finished" | "awarded") {
        return None;
    }
    let score = score?;
    let (home, away) = (score.home?, score.away?);
    let (home_ht, away_ht) = match score.halftime.as_ref() {
        Some(ht) => (ht.home, ht.away),
        None => (None, None),
    };
    Some(json!({
        "home_ft": home,
        "away_ft": away,
        "home_ht": home_ht,
        "away_ht": away_ht,
    }))
}

fn normalize_market(market: &str) -> (String, f64) {
    match market {
        "1X2" | "1x2" => ("1x2".into(), 0.0),
        "BTTS" | "btts" => ("btts".into(), 0.0),
        "DC" | "dc" => ("dc".into(), 0.0),
        other => {
            if let Some(line) = other
                .strip_prefix("OU")
                .or_else(|| other.strip_prefix("ou"))
                .and_then(|value| value.parse::<f64>().ok())
            {
                ("ou".into(), line)
            } else {
                (other.to_lowercase(), 0.0)
            }
        }
    }
}

/// A normalizált market-névből (1x2/btts/dc/ou) és a forrás kimeneteléből
/// képezi a v2 kanonikus kimenetel-kódot. Az ismeretlen kimeneteleket kihagyjuk.
fn normalize_outcome(market: &str, outcome: &str) -> Option<String> {
    let outcome = outcome.to_lowercase();
    let mapped = match (market, outcome.as_str()) {
        ("1x2", "home") => "home",
        ("1x2", "draw") => "draw",
        ("1x2", "away") => "away",
        ("btts", "yes") => "yes",
        ("btts", "no") => "no",
        ("dc", "hd") => "home_draw",
        ("dc", "ha") => "home_away",
        ("dc", "da") => "draw_away",
        ("ou", "over") => "over",
        ("ou", "under") => "under",
        _ => return None,
    };
    Some(mapped.to_string())
}
