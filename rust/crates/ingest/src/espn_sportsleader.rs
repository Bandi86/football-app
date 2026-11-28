//! ESPN-soccer forrásadapter (`espn_sportsleader`) — auditált, dry-run váz.
//!
//! Sport=`soccer`, referencia-liga=`eng.1`. A sports-leader MCP ugyanezeket a
//! nyilvános ESPN végpontokat használja (`site.api.espn.com/apis/site/v2/sports`,
//! `sports.core.api.espn.com/v2/sports`); ez a modul a NYERS ESPN JSON-t veszi
//! bemenetként, így kicsi MCP-híd (vagy közvetlen `reqwest` fetch) táplálhatja.
//!
//! Szerződés (a meglévő forrás-union/dedupe szabályokat követve):
//! - külön named source: `espn_sportsleader`;
//! - **nem ír kanonikus entitást** (az adapter → `Observation`, a kanonizálás a
//!   `Canonicalizer` + `identity` réteg feladata);
//! - dry-run alap: ez a modul SOHA nem ír DB-t/fájlt, csak `Vec<Observation>`-t
//!   és [`EspnDryRunReport`]-ot ad;
//! - `source_url` + `captured_at` MINDEN rekordon ([`EspnProvenance`]);
//! - nincs fuzzy egyeztetés, nincs kitalált érték: hiányzó mező → `null` /
//!   nem emelt observation + számláló; nem-igazolt esemény → drop + számláló;
//! - stabil `source_record_key` + `natural_key` + (a `stage.observation`
//!   `ON CONFLICT`-szemantikával kompatibilis) determinisztikus kimenet:
//!   ugyanaz a bemenet ugyanazt a kimenetet adja; cross-source union és
//!   kanonikus mező-kiválasztás NEM itt történik;
//! - hiányzó adat `null`, soha nem 0.

use chrono::{DateTime, SecondsFormat, Utc};
use football_domain::error::ParseError;
use football_domain::policy::{FetchJob, Transport};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::adapter::{AdapterMeta, Observation, ParseCtx, SourceAdapter};
use crate::fetch::{FetchRequest, RawPayload};

/// Külön named source (nem fuzzy, nem research-forrás).
pub const SOURCE_CODE: &str = "espn_sportsleader";
/// ESPN sport-slug.
pub const SPORT_SOCCER: &str = "soccer";
/// Referencia-liga.
pub const REFERENCE_LEAGUE_ENG1: &str = "eng.1";
/// Nyilvános ESPN site-v2 base (sports-leader ezt használja).
pub const ESPN_SITE_API_BASE: &str = "https://site.api.espn.com/apis/site/v2/sports";
/// Adapter/parser verzió (provenance + séma-stabilitás).
pub const ADAPTER_VERSION: &str = "espn-sportsleader-0.1";
pub const PARSER_VERSION: &str = "espn-soccer-scoreboard-0.1";

const KNOWN_SOCCER_LEAGUES: &[&str] = &[
    "usa.1",
    "eng.1",
    "esp.1",
    "ger.1",
    "ita.1",
    "fra.1",
    "uefa.champions",
    "fifa.world",
];

/// A sports-leader által kurált soccer-ligák (mért katalógus).
pub fn curated_soccer_leagues() -> &'static [&'static str] {
    KNOWN_SOCCER_LEAGUES
}

/// Explicit, caller-megadott liga-kontextus. NEM scrape-elt, NEM kitalált:
/// a forrásból jön a verseny-név; a tier/priority/ország-konfig a hívóé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EspnSoccerLeague {
    pub sport: String,
    pub league: String,
    /// Tier a kanonikus versenyhez; explicit, nem a forrásból (ESPN soccer nem ad).
    pub tier: i16,
    /// Prioritás a kanonikus versenyhez; explicit.
    pub priority: i32,
    /// Verseny-időzóna; ESPN soccer UTC-időt ad → `UTC`.
    pub competition_timezone: String,
}

impl EspnSoccerLeague {
    /// Referencia: English Premier League (`eng.1`).
    pub fn eng_1() -> Self {
        Self {
            sport: SPORT_SOCCER.to_string(),
            league: REFERENCE_LEAGUE_ENG1.to_string(),
            tier: 1,
            priority: 100,
            competition_timezone: "UTC".to_string(),
        }
    }
}

/// Nyilvános ESPN scoreboard URL.
pub fn scoreboard_url(sport: &str, league: &str, dates: Option<&str>) -> String {
    let base = format!("{ESPN_SITE_API_BASE}/{sport}/{league}/scoreboard");
    match dates {
        Some(d) if !d.is_empty() => format!("{base}?dates={d}"),
        _ => base,
    }
}

/// Nyilvános ESPN summary URL egy eseményre.
pub fn summary_url(sport: &str, league: &str, event_id: &str) -> String {
    format!("{ESPN_SITE_API_BASE}/{sport}/{league}/summary?event={event_id}")
}

/// Mérés-kompatibilis provenance minden rekordon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EspnProvenance {
    pub source: String,
    pub source_url: String,
    pub captured_at: String,
    pub sport: String,
    pub league: String,
}

/// Dry-run számlálók. `dry_run` mindig `true`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EspnDryRunReport {
    pub source: String,
    pub sport: String,
    pub league: String,
    pub source_url: String,
    pub captured_at: Option<String>,
    pub dry_run: bool,
    pub events_seen: usize,
    pub observations_emitted: usize,
    pub match_core_emitted: usize,
    pub team_identity_emitted: usize,
    pub match_environment_emitted: usize,
    pub match_referee_emitted: usize,
    pub match_link_emitted: usize,
    pub events_dropped_missing_id: usize,
    pub events_dropped_missing_kickoff: usize,
    pub events_dropped_missing_competitor: usize,
    pub events_dropped_missing_season: usize,
    pub events_dropped_unmapped_status: usize,
    pub results_unknown: usize,
}

/// Adapter kimenete.
#[derive(Debug, Clone)]
pub struct EspnSoccerOutput {
    pub observations: Vec<Observation>,
    pub report: EspnDryRunReport,
}

fn rfc3339_z(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn s(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// ESPN soccer `status.type` → kanonikus meccsstátusz; ismeretlen → `None`.
pub fn map_espn_soccer_status(name: &str, state: &str, completed: bool) -> Option<&'static str> {
    match name {
        "STATUS_POSTPONED" => return Some("postponed"),
        "STATUS_CANCELED" | "STATUS_CANCELLED" => return Some("canceled"),
        "STATUS_ABANDONED" => return Some("abandoned"),
        // SUSP/INT/DELAYED: nincs kanonikus érték → drop + számláló (nem találgatás).
        _ => {}
    }
    match (name, state) {
        ("STATUS_HALFTIME", _) => Some("halftime"),
        ("STATUS_FULL_TIME" | "STATUS_FINAL", "post")
        | ("STATUS_FULL_TIME" | "STATUS_FINAL", _) => Some("finished"),
        (_, "post") if completed => Some("finished"),
        (_, "in") => Some("live"),
        ("STATUS_IN_PROGRESS", _) | ("STATUS_FIRST_HALF", _) | ("STATUS_SECOND_HALF", _) => {
            Some("live")
        }
        ("STATUS_SCHEDULED", _) | (_, "pre") => Some("scheduled"),
        _ => None,
    }
}

fn parse_score(competitor: &Value) -> Option<i16> {
    s(competitor, "score").and_then(|sc| sc.trim().parse::<i16>().ok())
}

/// ESPN időbélyeg parse: RFC3339 + ESPN `...THH:MMZ` (másodperc nélkül) + naive.
fn parse_espn_datetime(raw: &str) -> Option<DateTime<Utc>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(d) = DateTime::parse_from_rfc3339(raw) {
        return Some(d.with_timezone(&Utc));
    }
    // timezone-offsettel (pl. `+00:00`) nem-Z formák.
    for fmt in ["%Y-%m-%dT%H:%M%z", "%Y-%m-%dT%H:%M:%S%z"] {
        if let Ok(d) = DateTime::parse_from_str(raw, fmt) {
            return Some(d.with_timezone(&Utc));
        }
    }
    // ESPN soccer `2026-11-27T20:00Z` (másodperc nélkül) + `...T20:00:00Z`:
    // `Z` levágás → naive → UTC (ESPN zulu, nem kitalált zóna).
    let naive = raw
        .strip_suffix('Z')
        .or_else(|| raw.strip_suffix('z'))
        .unwrap_or(raw);
    if let Ok(n) = chrono::NaiveDateTime::parse_from_str(naive, "%Y-%m-%dT%H:%M") {
        return Some(n.and_utc());
    }
    if let Ok(n) = chrono::NaiveDateTime::parse_from_str(naive, "%Y-%m-%dT%H:%M:%S") {
        return Some(n.and_utc());
    }
    None
}

fn status_of(container: &Value) -> (String, String, bool) {
    let t = container.get("status").and_then(|st| st.get("type"));
    let name = t.and_then(|t| s(t, "name")).unwrap_or_default();
    let state = t.and_then(|t| s(t, "state")).unwrap_or_default();
    let completed = t
        .and_then(|t| t.get("completed"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    (name, state, completed)
}

/// Nyers ESPN soccer scoreboard JSON → normalizált observation + provenance.
///
/// `source_url`: a pontos ESPN URL, ahonnan a payload jött. `captured_at`:
/// rögzítési idő. `league`: explicit liga-kontextus. Nem hálózat, nem DB.
pub fn parse_soccer_scoreboard(
    raw_json: &str,
    source_url: &str,
    captured_at: DateTime<Utc>,
    league: &EspnSoccerLeague,
) -> Result<EspnSoccerOutput, ParseError> {
    let root: Value = serde_json::from_str(raw_json).map_err(|e| {
        ParseError::SchemaDrift(format!("ESPN soccer scoreboard nem érvényes JSON: {e}"))
    })?;
    if !root.is_object() {
        return Err(ParseError::SchemaDrift(
            "ESPN soccer scoreboard nem objektum".into(),
        ));
    }

    let captured = rfc3339_z(captured_at);
    let provenance = |sport: &str, lg: &str| EspnProvenance {
        source: SOURCE_CODE.to_string(),
        source_url: source_url.to_string(),
        captured_at: captured.clone(),
        sport: sport.to_string(),
        league: lg.to_string(),
    };

    // Verseny + szezon a `leagues[0]`-ból; nincs kitalált mező.
    let league_meta = root
        .get("leagues")
        .and_then(|l| l.get(0))
        .cloned()
        .unwrap_or(Value::Null);
    let league_slug = s(&league_meta, "slug").unwrap_or_else(|| league.league.clone());
    let league_name = s(&league_meta, "name");
    let league_id = s(&league_meta, "id");
    let league_season = league_meta.get("season").cloned().unwrap_or(Value::Null);

    let prov = provenance(&league.sport, &league_slug);

    let events = root
        .get("events")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut report = EspnDryRunReport {
        source: SOURCE_CODE.to_string(),
        sport: league.sport.clone(),
        league: league_slug.clone(),
        source_url: source_url.to_string(),
        captured_at: Some(captured.clone()),
        dry_run: true,
        events_seen: events.len(),
        ..Default::default()
    };

    let mut observations: Vec<Observation> = Vec::new();

    for event in &events {
        // event id kötelező stabil kulcs.
        let Some(event_id) = s(event, "id") else {
            report.events_dropped_missing_id += 1;
            continue;
        };
        let source_record_key = format!("espn:event:{event_id}");
        let match_natural_key = format!("{}:{}:event:{event_id}", league.sport, league_slug);

        let competition = event
            .get("competitions")
            .and_then(|c| c.get(0))
            .cloned()
            .unwrap_or(Value::Null);

        // kickoff kötelező; `competition.date` → `event.date`; UTC RFC3339.
        let kickoff_raw = s(&competition, "date").or_else(|| s(event, "date"));
        let kickoff_at = kickoff_raw.as_deref().and_then(parse_espn_datetime);
        let Some(kickoff_at) = kickoff_at else {
            report.events_dropped_missing_kickoff += 1;
            continue;
        };

        // státusz: competition → event; ismeretlen → drop + számláló.
        let (c_name, c_state, c_completed) = status_of(&competition);
        let (e_name, e_state, e_completed) = status_of(event);
        let (st_name, st_state, st_completed) = if c_name.is_empty() && c_state.is_empty() {
            (e_name, e_state, e_completed)
        } else {
            (c_name, c_state, c_completed)
        };
        let Some(status) = map_espn_soccer_status(&st_name, &st_state, st_completed) else {
            report.events_dropped_unmapped_status += 1;
            continue;
        };

        // két csapat (home/away), team típus, stabil id + név kötelező.
        let competitors = competition
            .get("competitors")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let pick = |side: &str| -> Option<(String, String)> {
            competitors.iter().find_map(|c| {
                let home_away = s(c, "homeAway")?;
                if home_away != side {
                    return None;
                }
                if s(c, "type").as_deref().unwrap_or("team") != "team" {
                    return None;
                }
                let team = c.get("team")?;
                let id = s(team, "id").or_else(|| s(c, "id"))?;
                let name = s(team, "displayName").or_else(|| s(team, "name"))?;
                Some((id, name))
            })
        };
        let home_pair = pick("home");
        let away_pair = pick("away");
        let (Some((home_id, home_name)), Some((away_id, away_name))) = (home_pair, away_pair)
        else {
            report.events_dropped_missing_competitor += 1;
            continue;
        };
        let home_source_id = format!("espn:team:{home_id}");
        let away_source_id = format!("espn:team:{away_id}");

        // szezon címke kötelező a match_core-hoz; nincs kitalált.
        let season_label = s(&league_season, "displayName")
            .or_else(|| s(event.get("season").unwrap_or(&Value::Null), "slug"))
            .or_else(|| {
                league_season
                    .get("year")
                    .and_then(Value::as_i64)
                    .or_else(|| {
                        event
                            .get("season")
                            .and_then(|se| se.get("year"))
                            .and_then(Value::as_i64)
                    })
                    .map(|y| y.to_string())
            });
        let Some(season_label) = season_label else {
            report.events_dropped_missing_season += 1;
            continue;
        };

        let kickoff_precision = if competition
            .get("timeValid")
            .and_then(Value::as_bool)
            .unwrap_or(true)
        {
            "exact"
        } else {
            "date_only"
        };

        // eredmény csak lezárt + numerikus; különben null + számláló, nem kitalált 0.
        let score_container = competitors.clone();
        let result = if status == "finished" || status == "awarded" || st_state == "post" {
            let home_score = score_container
                .iter()
                .find(|c| s(c, "homeAway").as_deref() == Some("home"))
                .and_then(parse_score);
            let away_score = score_container
                .iter()
                .find(|c| s(c, "homeAway").as_deref() == Some("away"))
                .and_then(parse_score);
            match (home_score, away_score) {
                (Some(h), Some(a)) => Some((h, a)),
                _ => {
                    if status == "finished" || status == "awarded" {
                        report.results_unknown += 1;
                    }
                    None
                }
            }
        } else {
            None
        };

        let kickoff_str = rfc3339_z(kickoff_at);
        let prov_json = serde_json::to_value(&prov).unwrap_or(Value::Null);

        // team_identity (mindkét csapat; hiányzó adat üres/null, nem kitalált).
        for (team_id, name, source_id) in [
            (home_id.clone(), home_name.clone(), home_source_id.clone()),
            (away_id.clone(), away_name.clone(), away_source_id.clone()),
        ] {
            observations.push(Observation {
                entity_kind: "team".into(),
                aspect: "team_identity".into(),
                natural_key: format!("{}:{}:team:{team_id}", league.sport, league_slug),
                source_record_key: source_id.clone(),
                observed_at: captured_at,
                payload: json!({
                    "id": null,
                    "source_entity_id": source_id,
                    "name": name,
                    "country_code": "",
                    "gender": "male",
                    "age_group": "senior",
                    "branch": "senior",
                    "source": SOURCE_CODE,
                    "source_url": source_url,
                    "captured_at": captured,
                    "provenance": prov_json,
                }),
            });
            report.team_identity_emitted += 1;
        }

        // match_core.
        let result_json = match result {
            Some((h, a)) => json!({
                "home_ft": h, "away_ft": a,
                "home_ht": null, "away_ht": null,
                "home_et": null, "away_et": null,
                "home_pens": null, "away_pens": null,
                "decided_by": null,
            }),
            None => Value::Null,
        };
        observations.push(Observation {
            entity_kind: "match".into(),
            aspect: "match_core".into(),
            natural_key: match_natural_key.clone(),
            source_record_key: source_record_key.clone(),
            observed_at: captured_at,
            payload: json!({
                "canonical_match_id": null,
                "source_home_id": home_source_id,
                "source_away_id": away_source_id,
                "kickoff_at": kickoff_str,
                "kickoff_precision": kickoff_precision,
                "status": status,
                "result": result_json,
                "round_label": null,
                "competition_timezone": league.competition_timezone,
                "competition": {
                    "id": null,
                    "source_entity_id": league_id.as_ref().map(|id| format!("espn:league:{id}")),
                    "name": league_name.clone().unwrap_or_else(|| league.league.clone()),
                    // ESPN soccer nem ad liga-országot → üres, nem kitalált kód
                    // (`NormalizedCompetition` kötelező string; a canonicalizer
                    // üresen NULL-ként kezeli).
                    "country_code": "",
                    "country_name": "",
                    "tier": league.tier,
                    "priority": league.priority,
                },
                "season": { "id": null, "label": season_label },
                "source_code": SOURCE_CODE,
                "captured_at": captured,
                "source": SOURCE_CODE,
                "source_url": source_url,
                "provenance": prov_json,
            }),
        });
        report.match_core_emitted += 1;

        // match_environment csak igazolt venue esetén; minden adatmező null ha nincs.
        if let Some(venue) = competition.get("venue").filter(|v| v.is_object()) {
            let venue_name = s(venue, "fullName").or_else(|| s(venue, "displayName"));
            let addr = venue.get("address").cloned().unwrap_or(Value::Null);
            let venue_city = s(&addr, "city");
            let venue_country = s(&addr, "country");
            if venue_name.is_some() || venue_city.is_some() || venue_country.is_some() {
                observations.push(Observation {
                    entity_kind: "match".into(),
                    aspect: "match_environment".into(),
                    natural_key: format!("{match_natural_key}:environment"),
                    source_record_key: source_record_key.clone(),
                    observed_at: captured_at,
                    payload: json!({
                        "venue_name": venue_name,
                        "venue_city": venue_city,
                        "venue_country": venue_country,
                        "venue_capacity": null,
                        "venue_surface": null,
                        "venue_lat": null,
                        "venue_lon": null,
                        "temperature_c": null,
                        "weather_description": null,
                        "wind_speed": null,
                        "wind_direction": null,
                        "humidity_pct": null,
                        "precipitation": null,
                        "cloud_cover_pct": null,
                        "weather_observed_at": null,
                        "source": SOURCE_CODE,
                        "source_url": source_url,
                        "captured_at": captured,
                        "provenance": prov_json,
                    }),
                });
                report.match_environment_emitted += 1;
            }
        }

        // match_referee csak igazolt officials; ESPN soccer scoreboard általában nem ad.
        if let Some(officials) = competition.get("officials").and_then(Value::as_array) {
            let referee = officials.iter().find_map(|o| {
                let name = s(o, "displayName").or_else(|| s(o, "name"))?;
                let role = o
                    .get("type")
                    .and_then(|t| s(t, "name"))
                    .or_else(|| s(o, "role"))
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if role.contains("referee") || role.is_empty() {
                    Some(name)
                } else {
                    None
                }
            });
            if let Some(name) = referee {
                observations.push(Observation {
                    entity_kind: "match".into(),
                    aspect: "match_referee".into(),
                    natural_key: format!("{match_natural_key}:referee"),
                    source_record_key: source_record_key.clone(),
                    observed_at: captured_at,
                    payload: json!({
                        "name": name,
                        "country": null,
                        "source": SOURCE_CODE,
                        "source_url": source_url,
                        "captured_at": captured,
                        "provenance": prov_json,
                    }),
                });
                report.match_referee_emitted += 1;
            }
        }

        // match_link: forrás-URL provenance + ESPN summary link.
        let mut links = vec![json!({
            "source": SOURCE_CODE,
            "kind": "scoreboard",
            "url": source_url,
            "source_url": source_url,
            "captured_at": captured,
        })];
        let espn_summary = summary_url(&league.sport, &league_slug, &event_id);
        links.push(json!({
            "source": SOURCE_CODE,
            "kind": "summary",
            "url": espn_summary,
            "source_url": source_url,
            "captured_at": captured,
        }));
        if let Some(event_links) = event.get("links").and_then(Value::as_array) {
            for l in event_links {
                let rel = l.get("rel").and_then(Value::as_array);
                let is_summary =
                    rel.is_some_and(|r| r.iter().any(|x| x.as_str() == Some("summary")));
                if is_summary && let Some(href) = s(l, "href") {
                    links.push(json!({
                        "source": SOURCE_CODE,
                        "kind": "summary",
                        "url": href,
                        "source_url": source_url,
                        "captured_at": captured,
                    }));
                }
            }
        }
        observations.push(Observation {
            entity_kind: "match".into(),
            aspect: "match_link".into(),
            natural_key: format!("{match_natural_key}:links"),
            source_record_key: source_record_key.clone(),
            observed_at: captured_at,
            payload: Value::Array(links),
        });
        report.match_link_emitted += 1;
    }

    // stabil sorrend: determinisztikus kimenet (source-union/dedupe-barát).
    observations.sort_by(|a, b| {
        (&a.entity_kind, &a.aspect, &a.source_record_key).cmp(&(
            &b.entity_kind,
            &b.aspect,
            &b.source_record_key,
        ))
    });
    report.observations_emitted = observations.len();

    Ok(EspnSoccerOutput {
        observations,
        report,
    })
}

/// `SourceAdapter` impl — beköthető a meglévő ingest-útra; NEM ír canonical.
pub struct EspnSoccerAdapter {
    pub league: EspnSoccerLeague,
}

impl EspnSoccerAdapter {
    pub fn eng_1() -> Self {
        Self {
            league: EspnSoccerLeague::eng_1(),
        }
    }
}

impl SourceAdapter for EspnSoccerAdapter {
    fn meta(&self) -> AdapterMeta {
        AdapterMeta {
            source_slug: SOURCE_CODE.to_string(),
            adapter_version: ADAPTER_VERSION.to_string(),
            parser_version: PARSER_VERSION.to_string(),
            domains: vec![self.league.sport.clone(), self.league.league.clone()],
            transport: Transport::RustHttp,
        }
    }

    fn plan(&self, job: &FetchJob) -> Vec<FetchRequest> {
        // dates: `time_window`/`entity_key` `YYYY-MM-DD` → ESPN `YYYYMMDD`.
        let day = job
            .time_window
            .trim()
            .trim_start_matches("day:")
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>();
        let dates = if day.len() >= 8 {
            Some(day[..8].to_string())
        } else {
            None
        };
        let url = scoreboard_url(&self.league.sport, &self.league.league, dates.as_deref());
        vec![FetchRequest {
            url: url.clone(),
            request_key: url,
            endpoint_key: "scoreboard".to_string(),
            ready_marker: None,
            bearer_token: None,
        }]
    }

    fn parse(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError> {
        let json = std::str::from_utf8(&raw.bytes)
            .map_err(|e| ParseError::SchemaDrift(format!("ESPN payload nem UTF-8: {e}")))?;
        let source_url = if ctx.request_key.starts_with("http") {
            ctx.request_key.clone()
        } else {
            scoreboard_url(&self.league.sport, &self.league.league, None)
        };
        Ok(parse_soccer_scoreboard(json, &source_url, raw.fetched_at, &self.league)?.observations)
    }
}
