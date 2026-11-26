//! `research` forrás-adapter (S38): a football-research fájl-alapú JSON API-ját
//! olvassa (nincs hálózat, `Transport::LocalFile`).
//!
//! A napi envelope (`daily/YYYY/MM/<date>.json`) adja a meccseket, a
//! csapat-identitást, a játékvezetőt és a versenyt; a meccs-detail fájlokból
//! (`matches/YYYY/MM/DD/<slug>.json`) jön az odds (`odds.markets`), a
//! mérkőzés-események (`events[]`) és a csapatstatisztika
//! (`team_statistics.groups[].items[]`). A terv egy napi kérést és a naphoz
//! tartozó detail-fájlokat tartalmaz (S82: egységes `detail:<uid>` kulccsal).

use std::collections::BTreeSet;
use std::path::PathBuf;

use chrono::{DateTime, Datelike, NaiveDate, Timelike, Utc};
use football_domain::error::ParseError;
use football_domain::identity::normalize_country_code;
use football_domain::policy::{FetchJob, Transport};
use serde::Deserialize;
use serde_json::json;

use crate::adapter::{AdapterMeta, Observation, ParseCtx, SourceAdapter};
use crate::fetch::{FetchRequest, RawPayload};
use crate::normalized::NormalizedTeamStats;

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

    /// A napi `links.self` értékéből a detail-fájl gyökérhez viszonyított útja.
    fn detail_rel(link: &str) -> Option<String> {
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
    /// S90: a stadion-blokk; a hiányzó blokk `None` (a régi envelope-ban sincs).
    #[serde(default)]
    venue: Option<Venue>,
    /// S90: az időjárás-blokk; a forrás nem minden meccsnél adja.
    #[serde(default)]
    weather: Option<Weather>,
    #[serde(default)]
    links: Option<Links>,
    #[serde(default)]
    coverage: Option<Coverage>,
    /// `enriched` | `fixture_only`; a részletes forrás-adat jelzése.
    #[serde(default)]
    data_tier: Option<String>,
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
    /// Élő meccs játékideje a research pillanatképben (pl. 87). A kliens
    /// ennek hiányában a kezdésből becsül — S134.
    #[serde(default)]
    minute: Option<i32>,
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
    /// A forrás által kimondott bíró-ország; régi envelope-ekben nincs → None.
    #[serde(default)]
    country: Option<String>,
}

/// A napi envelope `venue` blokkja (S90). A stadion nem kanonikus entitás,
/// ezért a meccs-környezet része; minden mező opcionális (hiányzó = NULL).
#[derive(Debug, Deserialize)]
struct Venue {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    city: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    capacity: Option<i32>,
    #[serde(default)]
    surface: Option<String>,
    #[serde(default)]
    coordinates: Option<Coordinates>,
}

#[derive(Debug, Deserialize)]
struct Coordinates {
    #[serde(default)]
    lat: Option<f64>,
    /// A forrás `long` kulcsot használ (nem `lon`); a payload `venue_lon`.
    #[serde(default)]
    long: Option<f64>,
}

/// A napi envelope `weather` blokkja (S90). A hőmérséklet a forrás
/// szerződése szerint Celsius-fok; más egységet nem konvertálunk, mert a
/// DB-oszlop is Celsius. A forrás `unit`/`source` kulcsait szándékosan nem
/// olvassuk be (nincs DB-oszlopuk); a serde az ismeretlen kulcsokat elveti.
#[derive(Debug, Deserialize)]
struct Weather {
    #[serde(default)]
    temperature_c: Option<f64>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    wind_speed: Option<f64>,
    #[serde(default)]
    wind_direction: Option<String>,
    #[serde(default)]
    humidity_pct: Option<i32>,
    #[serde(default)]
    precipitation: Option<f64>,
    #[serde(default)]
    cloud_cover_pct: Option<i32>,
    #[serde(default)]
    observed_at: Option<DateTime<Utc>>,
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
    /// `ok` | `none`; hiányzó mező (idősebb envelope) → `None`.
    #[serde(default)]
    detail: Option<String>,
}

/// A detail-fájl (meccs) releváns része: odds, események, statisztika.
///
/// A séma-drift tolerancia: ismeretlen gyökér-mezőket a serde kihagy, a
/// hiányzó blokkok `None`/üres értéket adnak, a mezők hiánya NULL-lá válik.
#[derive(Debug, Deserialize)]
struct DetailDoc {
    #[serde(default)]
    meta: Option<DetailMeta>,
    #[serde(default)]
    uid: Option<String>,
    #[serde(default)]
    odds: Option<OddsSection>,
    #[serde(default)]
    events: Vec<DetailEvent>,
    #[serde(default)]
    team_statistics: Option<TeamStatistics>,
}

#[derive(Debug, Deserialize)]
struct DetailMeta {
    #[serde(default)]
    generated_at: Option<DateTime<Utc>>,
}

/// Egy esemény a `events[]` tömbből; minden mező opcionális (hiányzó = NULL).
#[derive(Debug, Deserialize)]
struct DetailEvent {
    #[serde(default)]
    minute: Option<i32>,
    #[serde(default, rename = "type")]
    incident_type: Option<String>,
    #[serde(default)]
    team: Option<String>,
    #[serde(default)]
    player: Option<String>,
    #[serde(default)]
    player_in: Option<String>,
    #[serde(default)]
    player_out: Option<String>,
    #[serde(default)]
    assist: Option<String>,
    /// Stabil forrás-ID-k (R exporter 1.1) — a név bizonytalan, az ID nem.
    #[serde(default)]
    player_id: Option<String>,
    #[serde(default)]
    player_in_id: Option<String>,
    #[serde(default)]
    player_out_id: Option<String>,
    #[serde(default)]
    score_after: Option<Vec<i16>>,
    #[serde(default)]
    detail: Option<String>,
}

/// A `team_statistics` blokk (period-groups-items); csak a `groups` kell.
#[derive(Debug, Deserialize)]
struct TeamStatistics {
    #[serde(default)]
    groups: Vec<StatGroup>,
}

#[derive(Debug, Deserialize)]
struct StatGroup {
    #[serde(default)]
    items: Vec<StatItem>,
}

/// Egy statisztika-sor; az érték bármilyen JSON lehet (szám vagy formázott
/// szöveg), a szöveges/formázott értékeket nem találgatjuk át.
#[derive(Debug, Deserialize)]
struct StatItem {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    home: Option<serde_json::Value>,
    #[serde(default)]
    away: Option<serde_json::Value>,
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
        // S45 catch-up: a scheduler a múltbeli napot az entity_key-ben hordozza,
        // a time_window a mai nap marad (napi coverage/idempotencia miatt).
        // A tervnek mégis a JELÖLT nap fájlját kell olvasnia, különben minden
        // catch-up job újra és újra a mai napot tölti be és a tegnapi eredmény
        // soha nem jut el a v2-be.
        let target = job
            .entity_key
            .strip_prefix("catchup-")
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
            .or_else(|| NaiveDate::parse_from_str(&job.time_window, "%Y-%m-%d").ok());
        let Some(date) = target else {
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

        // S82: a detail-fájlok egyetlen jobban hozzák az oddsot, az
        // eseményeket és a csapatstatisztikát, ezért a korábbi `odds:<uid>`
        // request_key helyett egységes `detail:<uid>` kulcs megy. A parse a
        // régi `odds:<uid>` kulcsot is elfogadja (a stádiumban maradt
        // payloadok és a korábban tervezett jobok miatt); a plan már csak az
        // új kulcsot állítja elő.
        //
        // Detail jobot akkor kérünk, ha a napi envelope szerint van odds
        // (`coverage.odds`), van részletes adat (`coverage.detail != "none"`),
        // vagy a meccs `data_tier` értéke `enriched`.
        let path = self.api_root.join(&daily_rel);
        if let Ok(bytes) = std::fs::read(&path)
            && let Ok(doc) = serde_json::from_slice::<DailyDoc>(&bytes)
        {
            let mut seen = BTreeSet::new();
            for m in &doc.data {
                if !wants_detail(m) {
                    continue;
                }
                let Some(link) = m.links.as_ref().and_then(|l| l.self_link.as_deref()) else {
                    continue;
                };
                let Some(rel) = Self::detail_rel(link) else {
                    continue;
                };
                if !seen.insert(rel.clone()) {
                    continue;
                }
                // S179: a napi index elavult detail-hivatkozást is hordozhat —
                // a meccs más napra került, de a `links.self` a régi útra mutat.
                // Egy hiányzó detail nem buktathatja az egész napi kört (a
                // catch-up így soha nem jutna el a többi meccs eredményéhez).
                if !self.api_root.join(&rel).is_file() {
                    tracing::warn!(%rel, "research detail-fájl hiányzik, kihagyva a tervből");
                    continue;
                }
                requests.push(FetchRequest {
                    url: rel,
                    request_key: format!("detail:{}", m.uid),
                    endpoint_key: "detail".into(),
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
        } else if ctx.request_key.starts_with("detail:") || ctx.request_key.starts_with("odds:") {
            self.parse_detail(raw, ctx)
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
            let country_name = m
                .competition
                .country_name
                .clone()
                .or_else(|| m.competition.country.clone())
                .unwrap_or_default();
            let (comp_name, country_code) = competition_identity(&m.competition);
            // A verseny source-ref kulcsa ország-szkópolt: a puszta név több
            // országban is él (Ligue 1 FR/DZ/BF, Premier League GB/KE/TZ...),
            // és a név-alapú ref az elsőként látott ország versenyére ragadtatta
            // a többi ország azonos nevű versenyét (S89 adatkorrupció).
            let competition_ref = format!("{comp_name}|{country_code}");
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
            // S148: a research éjfél UTC-t ír, ha a forrás csak dátumot ismer.
            // A kliens `exact`ként hamis 01:00-at mutatna — `date_only`-ként
            // helyette „időpont egyeztetés alatt"-ot jelenít meg (AGENTS.md §6).
            let kickoff_precision =
                if kickoff_at.hour() == 0 && kickoff_at.minute() == 0 && kickoff_at.second() == 0 {
                    "date_only"
                } else {
                    "exact"
                };
            let status = match m.status.code.as_str() {
                "finished" => "finished",
                "live" => "live",
                "postponed" => "postponed",
                // A kanonikus szótár a "canceled" írásmódot használja
                // (a DB CHECK is ezt engedi); a research "cancelled"-t ad.
                "cancelled" | "canceled" => "canceled",
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
                    "kickoff_precision": kickoff_precision,
                    "status": status,
                    "status_minute": m.status.minute,
                    "result": result,
                    "round_label": m.competition.round,
                    "competition_timezone": m
                        .kickoff
                        .timezone
                        .clone()
                        .unwrap_or_else(|| "Europe/Budapest".into()),
                    "competition": {
                        "source_entity_id": competition_ref.as_str(),
                        "name": comp_name,
                        "country_code": country_code,
                        "country_name": country_name,
                        // Ismeretlen tier: a kanonikus versenyszótár tudja
                        // csak megmondani (a forrás 1-es tierje mindenre igaz).
                        "tier": 0,
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
                    // S90: a bare `m.uid` a szerződés — a `normalized` a
                    // referee-t a meccs `source_match_id`-jével párosítja
                    // (a footballdata adapter is bare id-t ad). A korábbi
                    // `"{uid}:referee"` kulcs miatt a research játékvezető
                    // soha nem linkelődött a meccshez.
                    source_record_key: m.uid.clone(),
                    observed_at: captured_at,
                    payload: json!({
                        "name": referee.name.trim(),
                        "country": referee.country.as_deref()
                            .map(str::trim)
                            .filter(|code| !code.is_empty()),
                    }),
                });
            }

            // S90: a stadion és az időjárás a meccs-forrás-sorhoz tartozik,
            // ezért ugyanaz a natural_key/source_record_key, mint a
            // match_core-nál (a normalized a source_match_id-vel párosít).
            // Csak akkor emisszálunk, ha legalább az egyik blokk jelen van.
            if m.venue.is_some() || m.weather.is_some() {
                observations.push(Observation {
                    entity_kind: "match".into(),
                    aspect: "match_environment".into(),
                    natural_key: m.uid.clone(),
                    source_record_key: m.uid.clone(),
                    observed_at: captured_at,
                    payload: environment_payload(m.venue.as_ref(), m.weather.as_ref()),
                });
            }
        }

        Ok(observations)
    }

    /// Egy detail-fájl teljes feldolgozása: odds + események + statisztika.
    ///
    /// Az ismeretlen kulcsok a serde viselkedése miatt nem dobnak hibát; a
    /// hiányzó blokk kimarad, a hiányzó mező NULL lesz a payloadban.
    fn parse_detail(
        &self,
        raw: &RawPayload,
        ctx: &ParseCtx,
    ) -> Result<Vec<Observation>, ParseError> {
        let doc: DetailDoc = serde_json::from_slice(&raw.bytes)
            .map_err(|e| ParseError::SchemaDrift(format!("research detail: {e}")))?;
        let match_key = doc.uid.clone().unwrap_or_else(|| {
            ctx.request_key
                .trim_start_matches("detail:")
                .trim_start_matches("odds:")
                .to_string()
        });
        // A detail-fájl generálási ideje a közös rögzítési idő; ha hiányzik,
        // a feldolgozás ideje (a régi odds-viselkedéshez hasonlóan).
        let captured_at = doc
            .meta
            .as_ref()
            .and_then(|meta| meta.generated_at)
            .unwrap_or_else(Utc::now);
        let mut observations = Vec::new();
        if let Some(odds) = self.odds_observation(&doc, &match_key) {
            observations.push(odds);
        }
        if let Some(events) = self.events_observation(&doc, &match_key, captured_at) {
            observations.push(events);
        }
        if let Some(stats) = self.stats_observation(&doc, &match_key, captured_at) {
            observations.push(stats);
        }
        // S113: a research detail gazdag szekciói (preview, predikció, H2H,
        // tabella, sérülések, teljes odds, stadion) külön observationben.
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&raw.bytes)
            && let Some(payload) = crate::research_insight::insight_payload(&value)
        {
            observations.push(Observation {
                entity_kind: "match".into(),
                aspect: "match_insight".into(),
                natural_key: match_key.clone(),
                source_record_key: match_key.clone(),
                observed_at: captured_at,
                payload,
            });
        }
        // S181: a research detailben szereplő http(s) forráslinkek.
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&raw.bytes)
            && let Some(payload) = crate::research_insight::link_payload(&value)
        {
            observations.push(Observation {
                entity_kind: "match".into(),
                aspect: "match_link".into(),
                natural_key: match_key.clone(),
                source_record_key: match_key.clone(),
                observed_at: captured_at,
                payload,
            });
        }
        Ok(observations)
    }

    /// Az odds-megfigyelés (a korábbi `odds:<uid>` parse tartalma).
    fn odds_observation(&self, doc: &DetailDoc, match_key: &str) -> Option<Observation> {
        let odds = doc.odds.as_ref()?;
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
        let markets = odds.markets.as_ref()?;
        let mut quotes = Vec::new();
        for (market_name, outcomes) in markets {
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
            return None;
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
        let market_count = quotes
            .iter()
            .filter_map(|q| q.get("market").and_then(|m| m.as_str()))
            .collect::<BTreeSet<_>>()
            .len() as i64;
        Some(Observation {
            entity_kind: "match".into(),
            aspect: "match_odds".into(),
            natural_key: match_key.to_string(),
            // A napi match_core observation `source_record_key`-jével kell
            // egyeznie, különben az összeállítás nem kapcsolja a meccshez.
            source_record_key: match_key.to_string(),
            observed_at,
            payload: json!({
                "canonical_match_id": null,
                "quotes": quotes,
                "market_count": market_count,
            }),
        })
    }

    /// A `match_events` megfigyelés: a forrás sorrendjében, csak a névvel
    /// rendelkező típusok (a NULL típusú sor kimarad, mert a DB-ben kötelező).
    fn events_observation(
        &self,
        doc: &DetailDoc,
        match_key: &str,
        captured_at: DateTime<Utc>,
    ) -> Option<Observation> {
        let incidents: Vec<serde_json::Value> =
            doc.events.iter().filter_map(incident_payload).collect();
        if incidents.is_empty() {
            return None;
        }
        Some(Observation {
            entity_kind: "match".into(),
            aspect: "match_events".into(),
            natural_key: match_key.to_string(),
            source_record_key: match_key.to_string(),
            observed_at: captured_at,
            payload: json!({ "incidents": incidents }),
        })
    }

    /// A `match_stats` megfigyelés: a `team_statistics` blokk kanonikus
    /// metrikái. Formázott/százalékos értéket nem találgatunk át.
    fn stats_observation(
        &self,
        doc: &DetailDoc,
        match_key: &str,
        captured_at: DateTime<Utc>,
    ) -> Option<Observation> {
        let groups = &doc.team_statistics.as_ref()?.groups;
        let home = team_stats_from_groups(groups, true);
        let away = team_stats_from_groups(groups, false);
        if home.is_empty() && away.is_empty() {
            return None;
        }
        Some(Observation {
            entity_kind: "match".into(),
            aspect: "match_stats".into(),
            natural_key: match_key.to_string(),
            source_record_key: match_key.to_string(),
            observed_at: captured_at,
            payload: json!({
                "home_shots": home.shots,
                "away_shots": away.shots,
                "home_shots_on_target": home.shots_on_target,
                "away_shots_on_target": away.shots_on_target,
                "home_corners": home.corners,
                "away_corners": away.corners,
                "home_yellow": home.yellow,
                "away_yellow": away.yellow,
                "home_red": home.red,
                "away_red": away.red,
                "home_fouls": home.fouls,
                "away_fouls": away.fouls,
                "home_xg": home.xg,
                "away_xg": away.xg,
            }),
        })
    }
}

/// S82: kér-e a nap meccse detail-fájlt?
///
/// Akkor igen, ha `coverage.odds` igaz, a `coverage.detail` értéke nem
/// `none` (tehát van részletes adat), vagy a meccs `data_tier` értéke
/// `enriched`. A hiányzó coverage/data_tier nem igényel detail jobot.
fn wants_detail(m: &DayMatch) -> bool {
    let odds = m.coverage.as_ref().map(|c| c.odds).unwrap_or(false);
    let has_detail = m
        .coverage
        .as_ref()
        .and_then(|c| c.detail.as_deref())
        .map(|value| !value.eq_ignore_ascii_case("none"))
        .unwrap_or(false);
    let enriched = m.data_tier.as_deref() == Some("enriched");
    odds || has_detail || enriched
}

/// Egy detail-esemény → `match_events` payload-elem; a NULL típusú sor
/// kimarad (a kanonikus `incident_type` kötelező).
fn incident_payload(event: &DetailEvent) -> Option<serde_json::Value> {
    let incident_type = event.incident_type.as_deref()?.trim();
    if incident_type.is_empty() {
        return None;
    }
    let side = match event.team.as_deref() {
        Some("home") => Some("home"),
        Some("away") => Some("away"),
        _ => None,
    };
    let (player_name, related_player_name) = match incident_type {
        // A cserénél a bejövő a főszereplő, a lecserélt a kapcsolódó játékos.
        "Substitution" => (
            event.player_in.clone().or_else(|| event.player.clone()),
            event.player_out.clone(),
        ),
        // A gólnál az assziszt a kapcsolódó név; a "player" a gólszerző.
        "Goal" => (event.player.clone(), assist_name(event.assist.as_deref())),
        _ => (event.player.clone(), None),
    };
    let (score_after_home, score_after_away) = match event.score_after.as_deref() {
        Some([home, away]) => (Some(*home), Some(*away)),
        _ => (None, None),
    };
    // A cserénél a bejövő ID a fő, a kimenő a kapcsolódó oldal; gólnál a
    // gólszerző ID-je. Az asszisztnak nincs külön ID mezője a forrásban.
    let (player_source_id, related_player_source_id) = match incident_type {
        "Substitution" => (
            event
                .player_in_id
                .clone()
                .or_else(|| event.player_id.clone()),
            event.player_out_id.clone(),
        ),
        _ => (event.player_id.clone(), None),
    };
    Some(json!({
        "minute": event.minute,
        "incident_type": incident_type,
        "side": side,
        "player_name": player_name,
        "related_player_name": related_player_name,
        "player_source_id": player_source_id,
        "related_player_source_id": related_player_source_id,
        "detail": event.detail,
        "score_after_home": score_after_home,
        "score_after_away": score_after_away,
    }))
}

/// "assist by Marcus Kirchheiner" → "Marcus Kirchheiner"; más forma → NULL.
fn assist_name(assist: Option<&str>) -> Option<String> {
    let value = assist?.trim();
    value
        .strip_prefix("assist by ")
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

/// A team_statistics egy kanonikus metrikája.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatMetric {
    Shots,
    ShotsOnTarget,
    Corners,
    Yellow,
    Red,
    Fouls,
    Xg,
}

/// `key` vagy `title` alapján felismert metrika; mindkettőt ékezet- és
/// írásjel-függetlenül hasonlítjuk (pl. `total_shots` = "Total shots").
fn metric_of(item: &StatItem) -> Option<StatMetric> {
    let folded = |value: &str| {
        value
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    for candidate in [&item.key, &item.title].into_iter().flatten() {
        let metric = match folded(candidate).as_str() {
            "totalshots" => StatMetric::Shots,
            "shotsontarget" => StatMetric::ShotsOnTarget,
            "corners" => StatMetric::Corners,
            "yellowcards" => StatMetric::Yellow,
            "redcards" => StatMetric::Red,
            "fouls" | "foulscommitted" => StatMetric::Fouls,
            "expectedgoals" | "expectedgoalsxg" => StatMetric::Xg,
            _ => continue,
        };
        return Some(metric);
    }
    None
}

/// A `groups[].items[]` listából az egyik oldal hét metrikája.
///
/// Determinisztikus: a csoportok és a sorok forrás-sorrendjében az első
/// értelmezhető érték nyer (a duplikált kulcsok értéke a forrásban egyezik;
/// a group-header sorok értéke NULL, az nem nyer).
fn team_stats_from_groups(groups: &[StatGroup], is_home: bool) -> NormalizedTeamStats {
    let mut stats = NormalizedTeamStats::default();
    for group in groups {
        for item in &group.items {
            let Some(metric) = metric_of(item) else {
                continue;
            };
            let value = if is_home { &item.home } else { &item.away };
            let Some(value) = value.as_ref().filter(|v| !v.is_null()) else {
                continue;
            };
            match metric {
                StatMetric::Shots => stats.shots = stats.shots.or_else(|| int_value(value)),
                StatMetric::ShotsOnTarget => {
                    stats.shots_on_target = stats.shots_on_target.or_else(|| int_value(value))
                }
                StatMetric::Corners => stats.corners = stats.corners.or_else(|| int_value(value)),
                StatMetric::Yellow => stats.yellow = stats.yellow.or_else(|| int_value(value)),
                StatMetric::Red => stats.red = stats.red.or_else(|| int_value(value)),
                StatMetric::Fouls => stats.fouls = stats.fouls.or_else(|| int_value(value)),
                StatMetric::Xg => stats.xg = stats.xg.or_else(|| xg_value(value)),
            }
        }
    }
    stats
}

/// Egész statisztika-érték; csak JSON-szám, a formázott szöveg NULL marad.
fn int_value(value: &serde_json::Value) -> Option<i32> {
    i32::try_from(value.as_i64()?).ok()
}

/// xG-érték: JSON-szám vagy tiszta tizedestört-szöveg ("1.32"); százalékos
/// vagy más formázott szöveg nem értelmezhető (NULL, nem találgatunk).
fn xg_value(value: &serde_json::Value) -> Option<f64> {
    if let Some(number) = value.as_f64() {
        return Some(number);
    }
    let text = value.as_str()?.trim();
    if text.is_empty()
        || !text
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
    {
        return None;
    }
    text.parse().ok()
}

/// A research `country` mezője lehet ISO3 (`GER`, `ENG`, `WAL`) vagy már
/// normalizált kód; a v2-ben a brit hazai nemzetek versenyei GB alatt élnek.
fn competition_country_code(raw_country: &str) -> String {
    let raw = raw_country.trim();
    let normalized = normalize_country_code(&raw.to_ascii_uppercase());
    match normalized.as_str() {
        "WAL" | "NIR" | "SCO" | "GB-SCT" | "GB-WLS" | "GB-NIR" | "ENG" => "GB".into(),
        other => other.into(),
    }
}

/// A research versenyneve és országa a v2 kanonikus versenyére képezve.
///
/// A research a `name` mezőben rövid, szponzor-szuffix nélküli nevet ad
/// (`Ligue 1`), a `display_name` a szponzoros változatot (`Ligue 1 McDonald's`).
/// A kanonikus név ezért elsődlegesen a `name`; a `display_name` csak fallback.
/// A brit hazai nemzetek ligái a v2-ben GB alatt, más néven élnek (S38b).
fn competition_identity(competition: &Competition) -> (String, String) {
    let base_name = if competition.name.trim().is_empty() {
        competition
            .display_name
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_string()
    } else {
        competition.name.trim().to_string()
    };
    let raw_country = competition
        .country
        .as_deref()
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .unwrap_or("INT");
    let name = base_name.trim();
    if let Some((canonical_name, canonical_country)) =
        competition_alias(raw_country, &name.to_ascii_lowercase())
    {
        return (canonical_name.to_string(), canonical_country.to_string());
    }
    let country_code = competition_country_code(raw_country);
    let name = if name.is_empty() {
        "Ismeretlen verseny".to_string()
    } else {
        name.to_string()
    };
    (name, country_code)
}

/// (research országkód, folded versenynév) → (kanonikus név, kanonikus országkód).
///
/// Csak dokumentált, egyértelmű átnevezések vannak itt: a brit hazai nemzetek
/// ligái (League 1/2 → League One/Two, Premiership → Scottish/NIFL Premiership,
/// walesi Premier League → Cymru Premier) és a spanyol LaLiga névforma.
/// Minden más azonos néven megy tovább (nincs találgatás).
const COMPETITION_ALIASES: &[(&str, &str, &str, &str)] = &[
    ("ENG", "league 1", "League One", "GB"),
    ("ENG", "league 2", "League Two", "GB"),
    ("ENG", "league one", "League One", "GB"),
    ("ENG", "league two", "League Two", "GB"),
    ("WAL", "premier league", "Cymru Premier", "GB"),
    ("WAL", "premier division", "Cymru Premier", "GB"),
    ("SCO", "premiership", "Scottish Premiership", "GB"),
    ("SCO", "championship", "Scottish Championship", "GB"),
    ("SCO", "league 1", "Scottish League One", "GB"),
    ("SCO", "league one", "Scottish League One", "GB"),
    ("SCO", "league 2", "Scottish League Two", "GB"),
    ("SCO", "league two", "Scottish League Two", "GB"),
    ("SCO", "swpl 1", "SWPL 1 - női", "GB"),
    ("NIR", "premiership", "NIFL Premiership", "GB"),
    ("NIR", "championship", "NIFL Championship", "GB"),
    ("ESP", "laliga", "La Liga", "ES"),
    ("ESP", "laliga2", "LaLiga 2", "ES"),
    ("ESP", "copa del rey - 1st round", "Copa del Rey", "ES"),
];

/// Nemzetközi sorozatok nevei: a research tagekben felbontva jönnek
/// (pl. „Africa Cup of Nations Qualification Grp. A"), a v2-ben viszont egy
/// kanonikus selejtező-verseny él. Itt a csoport-taget eldobjuk.
fn international_alias(folded_name: &str) -> Option<(&'static str, &'static str)> {
    if folded_name == "afcon qualifying"
        || folded_name.starts_with("africa cup of nations qualification")
    {
        Some(("Afrikai Nemzetek Kupája - Selejtező", "INT"))
    } else if folded_name == "mens international friendly" {
        Some(("Friendlies", "INT"))
    } else {
        None
    }
}

fn competition_alias(raw_country: &str, folded_name: &str) -> Option<(&'static str, &'static str)> {
    if let Some(alias) = international_alias(folded_name) {
        return Some(alias);
    }
    let code = raw_country.trim().to_ascii_uppercase();
    COMPETITION_ALIASES
        .iter()
        .find(|(country, name, _, _)| *country == code && *name == folded_name)
        .map(|(_, _, canonical, canonical_country)| (*canonical, *canonical_country))
}

fn team_context(competition_name: &str) -> (&'static str, &'static str, &'static str) {
    let lower = competition_name.to_lowercase();
    let women = [
        "(w)",
        "women",
        "női",
        "frauen",
        "vrouwen",
        "feminine",
        "féminine",
        "femenine",
        "femenina",
        "femenino",
        "femminile",
        "dames",
        "kvinde",
        "damallsvenskan",
        "nwsl",
        "wsl",
        "swpl",
        "we league",
        "liga f",
        "toppserien",
        "ncaaw",
        "primera division femenina",
        "a-league women",
        "uwcl",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    if women {
        return ("female", "senior", "women");
    }
    // A kanonikus age_group csak `senior` / `youth`: a forrás U15–U21 tokenjei
    // nem kerülhetnek be önálló értékként, mert azok párhuzamos (XX/u17 stb.)
    // csapatentitásokat hoznak létre a meglévő youth entitások mellett.
    for group in ["u19", "u21", "u20", "u18", "u17", "u16", "u15"] {
        if lower.contains(group) {
            return ("male", "youth", "youth");
        }
    }
    ("male", "senior", "senior")
}

/// A `match_environment` observation payloadja az API-kontraktus mezőneveivel
/// (S90). A hiányzó adat `null`, soha nem 0; az üres/whitespace szöveg is
/// `null`. A DB CHECK-jei miatt a tartományon kívüli kapacitás és százalék
/// `null` (nem hackelünk érvényes értéket).
fn environment_payload(venue: Option<&Venue>, weather: Option<&Weather>) -> serde_json::Value {
    let coordinates = venue.and_then(|venue| venue.coordinates.as_ref());
    json!({
        "venue_name": venue.and_then(|venue| non_empty(venue.name.as_deref())),
        "venue_city": venue.and_then(|venue| non_empty(venue.city.as_deref())),
        "venue_country": venue.and_then(|venue| non_empty(venue.country.as_deref())),
        "venue_capacity": venue
            .and_then(|venue| venue.capacity)
            .filter(|capacity| *capacity >= 0),
        "venue_surface": venue.and_then(|venue| non_empty(venue.surface.as_deref())),
        "venue_lat": coordinates.and_then(|coordinates| coordinates.lat),
        "venue_lon": coordinates.and_then(|coordinates| coordinates.long),
        // A forrás szerződése Celsius-fok; a `unit` mező csak jelzés, más
        // egységet nem konvertálunk (a DB-oszlop is Celsius).
        "temperature_c": weather.and_then(|weather| weather.temperature_c),
        "weather_description": weather.and_then(|weather| non_empty(weather.description.as_deref())),
        "wind_speed": weather.and_then(|weather| weather.wind_speed),
        "wind_direction": weather.and_then(|weather| non_empty(weather.wind_direction.as_deref())),
        "humidity_pct": weather
            .and_then(|weather| weather.humidity_pct)
            .filter(|pct| (0..=100).contains(pct)),
        "precipitation": weather.and_then(|weather| weather.precipitation),
        "cloud_cover_pct": weather
            .and_then(|weather| weather.cloud_cover_pct)
            .filter(|pct| (0..=100).contains(pct)),
        "weather_observed_at": weather.and_then(|weather| weather.observed_at),
    })
}

/// Üres vagy csak whitespace-ből álló szöveg → `None` (a payload `null`).
fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn competition(name: &str, display: Option<&str>, country: Option<&str>) -> Competition {
        Competition {
            name: name.into(),
            display_name: display.map(str::to_string),
            country: country.map(str::to_string),
            country_name: None,
            round: None,
        }
    }

    #[test]
    fn welsh_premier_league_is_cymru_premier_not_the_english_league() {
        let identity = competition_identity(&competition(
            "Premier League",
            Some("Premier League"),
            Some("WAL"),
        ));
        assert_eq!(identity, ("Cymru Premier".to_string(), "GB".to_string()));
    }

    #[test]
    fn english_and_scottish_division_names_are_canonicalised() {
        assert_eq!(
            competition_identity(&competition("League 1", None, Some("ENG"))),
            ("League One".to_string(), "GB".to_string())
        );
        assert_eq!(
            competition_identity(&competition("Premiership", None, Some("SCO"))),
            ("Scottish Premiership".to_string(), "GB".to_string())
        );
        assert_eq!(
            competition_identity(&competition("Premiership", None, Some("NIR"))),
            ("NIFL Premiership".to_string(), "GB".to_string())
        );
    }

    #[test]
    fn spanish_laliga_names_are_canonicalised() {
        assert_eq!(
            competition_identity(&competition("LaLiga", None, Some("ESP"))),
            ("La Liga".to_string(), "ES".to_string())
        );
        assert_eq!(
            competition_identity(&competition("LaLiga2", None, Some("ESP"))),
            ("LaLiga 2".to_string(), "ES".to_string())
        );
    }

    #[test]
    fn sponsor_suffix_of_display_name_is_not_used_when_name_exists() {
        let identity = competition_identity(&competition(
            "Ligue 1",
            Some("Ligue 1 McDonald's"),
            Some("FRA"),
        ));
        assert_eq!(identity, ("Ligue 1".to_string(), "FR".to_string()));
    }

    #[test]
    fn international_qualification_groups_fold_into_one_canonical_competition() {
        let a = competition_identity(&competition(
            "Africa Cup of Nations Qualification Grp. A",
            None,
            Some("INT"),
        ));
        let b = competition_identity(&competition("AFCON Qualifying", None, Some("INT")));
        assert_eq!(a.0, "Afrikai Nemzetek Kupája - Selejtező");
        assert_eq!(b.0, "Afrikai Nemzetek Kupája - Selejtező");
        assert_eq!(a.1, "INT");
        assert_eq!(b.1, "INT");
    }

    #[test]
    fn unknown_competition_passes_through_with_normalised_country() {
        let identity = competition_identity(&competition("FNL", None, Some("CZE")));
        assert_eq!(identity, ("FNL".to_string(), "CZ".to_string()));
    }

    #[test]
    fn women_and_youth_competitions_are_flagged() {
        assert_eq!(team_context("SWPL 1").0, "female");
        assert_eq!(team_context("WE League").0, "female");
        assert_eq!(team_context("NWSL").0, "female");
        // S96: a holland és a szó eleji „Women's" alakok is női kontextust adnak,
        // különben a női liga csapatai férfi entitásra oldódnak.
        assert_eq!(team_context("Eredivisie Vrouwen").0, "female");
        assert_eq!(team_context("Women's FA Cup").0, "female");
        assert_eq!(team_context("Première Ligue Féminine").0, "female");
        assert_eq!(team_context("Eredivisie").0, "male");
        assert_eq!(team_context("Premier League").0, "male");
        // A forrás U18 tokenje a kanonikus `youth` age_group-ra képeződik.
        assert_eq!(team_context("Premier League U18").1, "youth");
        assert_eq!(team_context("Premier League").2, "senior");
    }

    /// R1.1: a stabil esemény-ID-k eljutnak a match_events payloadba is, hogy
    /// a canonicalize a meglévő `player_source_id` oszlopokba írja őket.
    /// S134: a research `status.minute` megérkezik az ingestbe; a kliens
    /// ennek hiányában csak a kezdésből tud becsülni.
    #[test]
    fn status_minute_survives_deserialisation() {
        let live: Status = serde_json::from_str(r#"{"code":"live","minute":87}"#).unwrap();
        assert_eq!(live.minute, Some(87));
        let finished: Status = serde_json::from_str(r#"{"code":"finished"}"#).unwrap();
        assert_eq!(finished.minute, None);
    }

    #[test]
    fn incident_payload_carries_stable_player_ids() {
        let substitution = DetailEvent {
            minute: Some(46),
            incident_type: Some("Substitution".into()),
            team: Some("home".into()),
            player: Some("Ihab Goudani".into()),
            player_in: Some("Andrej Vasovic".into()),
            player_out: Some("Ihab Goudani".into()),
            assist: None,
            player_id: Some("2035207".into()),
            player_in_id: Some("2323116".into()),
            player_out_id: Some("1698721".into()),
            score_after: None,
            detail: None,
        };
        let payload = incident_payload(&substitution).unwrap();
        assert_eq!(payload["player_source_id"], "2323116");
        assert_eq!(payload["related_player_source_id"], "1698721");

        let goal = DetailEvent {
            incident_type: Some("Goal".into()),
            player_id: Some("2035207".into()),
            player_in_id: None,
            player_out_id: None,
            ..substitution
        };
        let payload = incident_payload(&goal).unwrap();
        assert_eq!(payload["player_source_id"], "2035207");
        assert!(payload["related_player_source_id"].is_null());
    }
}
