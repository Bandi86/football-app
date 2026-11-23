//! Forrás-független normalizált nap: a kanonikus út közös bemenete.
//!
//! A fixture-út a kanonikus id-hinteket kitölti (demo id-stabilitás); az élő
//! források `None`-t adnak, és a kanonikus id-k natural-key upserttel képződnek.

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::adapter::Observation;
use crate::fixture::{FixtureOdds, FixtureSource};

#[derive(Debug, Clone)]
pub struct NormalizedDay {
    pub source_code: String,
    pub captured_at: DateTime<Utc>,
    pub competition: NormalizedCompetition,
    pub season: NormalizedSeason,
    pub teams: Vec<NormalizedTeam>,
    pub matches: Vec<NormalizedMatch>,
    pub standings: Option<Vec<NormalizedStanding>>,
}

#[derive(Debug, Clone)]
pub struct NormalizedCompetition {
    pub id: Option<Uuid>,
    /// A forrás saját verseny-azonosítója (source_entity_ref kulcsa); a
    /// fixture/demo út nem ad ilyet.
    pub source_entity_id: Option<String>,
    pub name: String,
    pub country_code: String,
    pub country_name: String,
    pub tier: i16,
    pub priority: i32,
}

#[derive(Debug, Clone)]
pub struct NormalizedSeason {
    pub id: Option<Uuid>,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct NormalizedTeam {
    pub id: Option<Uuid>,
    pub source_entity_id: String,
    pub name: String,
    pub country_code: String,
    pub gender: String,
    pub age_group: String,
    pub branch: String,
}

#[derive(Debug, Clone)]
pub struct NormalizedMatch {
    pub id: Option<Uuid>,
    pub source_match_id: String,
    pub home_source_entity_id: String,
    pub away_source_entity_id: String,
    pub round_label: Option<String>,
    pub kickoff_at: DateTime<Utc>,
    /// `exact` vagy `date_only` (a listán nincs kezdési idő → 12:00 UTC helyőrző).
    pub kickoff_precision: String,
    pub competition_timezone: String,
    pub status: String,
    pub result: Option<NormalizedResult>,
    pub odds: Option<NormalizedOdds>,
    pub stats: Option<NormalizedStats>,
    pub referee: Option<String>,
    /// A forrás által kimondott bíró-ország; nem a verseny országa. Hiány → None.
    pub referee_country: Option<String>,
    /// S90: meccs-környezet (stadion + időjárás); hiányzó adat `None`.
    pub environment: Option<NormalizedEnvironment>,
    /// A meccs eseményei a forrás sorrendjében (üres = nincs esemény-adat).
    pub incidents: Vec<NormalizedIncident>,
}

/// Meccs-környezet (S90): a forrás venue- és weather-blokkja együtt.
///
/// A hiányzó adat `None`, soha nem 0; a venue nem kanonikus entitás, ezért
/// meccs-szintű tény. Üres környezet (minden adatmező `None`) nem ír DB-sort.
/// Az `observed_at` a forrás-observation rögzítési ideje: ez lesz a
/// `fact.match_environment.captured_at`.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedEnvironment {
    pub observed_at: DateTime<Utc>,
    pub venue_name: Option<String>,
    pub venue_city: Option<String>,
    pub venue_country: Option<String>,
    pub venue_capacity: Option<i32>,
    pub venue_surface: Option<String>,
    pub venue_lat: Option<f64>,
    pub venue_lon: Option<f64>,
    pub temperature_c: Option<f64>,
    pub weather_description: Option<String>,
    pub wind_speed: Option<f64>,
    pub wind_direction: Option<String>,
    pub humidity_pct: Option<i32>,
    pub precipitation: Option<f64>,
    pub cloud_cover_pct: Option<i32>,
    pub weather_observed_at: Option<DateTime<Utc>>,
}

impl NormalizedEnvironment {
    /// Igaz, ha a forrás egyetlen ismert mezőt sem adott (ilyenkor nincs mit
    /// írni: üres környezet-sor nem keletkezik).
    pub fn is_empty(&self) -> bool {
        self.venue_name.is_none()
            && self.venue_city.is_none()
            && self.venue_country.is_none()
            && self.venue_capacity.is_none()
            && self.venue_surface.is_none()
            && self.venue_lat.is_none()
            && self.venue_lon.is_none()
            && self.temperature_c.is_none()
            && self.weather_description.is_none()
            && self.wind_speed.is_none()
            && self.wind_direction.is_none()
            && self.humidity_pct.is_none()
            && self.precipitation.is_none()
            && self.cloud_cover_pct.is_none()
            && self.weather_observed_at.is_none()
    }
}

/// Egy meccs-esemény (a forrás sorrendjében); a hiányzó adat `None`.
///
/// A `side` a forrás oldala (`home`/`away`), ismeretlen esetben `None`; a
/// kanonizálás képezi a DB-oldali értékre. A `score_after_*` csak akkor
/// kerül tovább, ha mindkét oldal jelen van.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedIncident {
    pub minute: Option<i32>,
    pub incident_type: String,
    pub side: Option<String>,
    pub player_name: Option<String>,
    pub related_player_name: Option<String>,
    pub detail: Option<String>,
    pub score_after_home: Option<i16>,
    pub score_after_away: Option<i16>,
}

/// Egy csapat statisztikája (hét metrika; hiányzó = NULL, soha nem 0).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NormalizedTeamStats {
    pub shots: Option<i32>,
    pub shots_on_target: Option<i32>,
    pub corners: Option<i32>,
    pub yellow: Option<i32>,
    pub red: Option<i32>,
    pub fouls: Option<i32>,
    pub xg: Option<f64>,
}

impl NormalizedTeamStats {
    /// Igaz, ha a forrás egyetlen ismert metrikát sem adott (ilyenkor nincs
    /// mit írni: üres statisztika-sor nem keletkezik).
    pub fn is_empty(&self) -> bool {
        self.shots.is_none()
            && self.shots_on_target.is_none()
            && self.corners.is_none()
            && self.yellow.is_none()
            && self.red.is_none()
            && self.fouls.is_none()
            && self.xg.is_none()
    }
}

/// Meccs-statisztika (football-data oszlopok; hiányzó = NULL).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NormalizedStats {
    pub home_shots: Option<i32>,
    pub away_shots: Option<i32>,
    pub home_shots_on_target: Option<i32>,
    pub away_shots_on_target: Option<i32>,
    pub home_corners: Option<i32>,
    pub away_corners: Option<i32>,
    pub home_yellow: Option<i32>,
    pub away_yellow: Option<i32>,
    pub home_red: Option<i32>,
    pub away_red: Option<i32>,
    pub home_fouls: Option<i32>,
    pub away_fouls: Option<i32>,
    pub home_xg: Option<f64>,
    pub away_xg: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct NormalizedResult {
    pub home_ft: Option<i16>,
    pub away_ft: Option<i16>,
    pub home_ht: Option<i16>,
    pub away_ht: Option<i16>,
    pub home_et: Option<i16>,
    pub away_et: Option<i16>,
    pub home_pens: Option<i16>,
    pub away_pens: Option<i16>,
    /// `ft` / `et` / `pens`; NULL = ismeretlen (S2d-4 séma).
    pub decided_by: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NormalizedOdds {
    pub market_count: Option<i32>,
    pub quotes: Vec<NormalizedQuote>,
}

#[derive(Debug, Clone)]
pub struct NormalizedQuote {
    pub bookmaker: String,
    pub market: String,
    pub outcome: String,
    pub line: f64,
    pub phase: String,
    pub price: f64,
}

#[derive(Debug, Clone)]
pub struct NormalizedStanding {
    pub team_source_entity_id: String,
    pub position: i16,
}

/// Flat 7 érték → kanonikus quote-ok (default bookmaker, closing).
#[allow(clippy::too_many_arguments)]
fn flat_quotes(
    home: f64,
    draw: f64,
    away: f64,
    btts_yes: f64,
    btts_no: f64,
    over25: f64,
    under25: f64,
) -> Vec<NormalizedQuote> {
    let quote = |market: &str, outcome: &str, line: f64, price: f64| NormalizedQuote {
        bookmaker: "default".into(),
        market: market.into(),
        outcome: outcome.into(),
        line,
        phase: "closing".into(),
        price,
    };
    vec![
        quote("1x2", "home", 0.0, home),
        quote("1x2", "draw", 0.0, draw),
        quote("1x2", "away", 0.0, away),
        quote("btts", "yes", 0.0, btts_yes),
        quote("btts", "no", 0.0, btts_no),
        quote("ou", "over", 2.5, over25),
        quote("ou", "under", 2.5, under25),
    ]
}

fn default_quotes(odds: &FixtureOdds) -> Vec<NormalizedQuote> {
    flat_quotes(
        odds.home,
        odds.draw,
        odds.away,
        odds.btts_yes,
        odds.btts_no,
        odds.over25,
        odds.under25,
    )
}

#[derive(Deserialize)]
struct TeamPayload {
    id: Option<Uuid>,
    source_entity_id: String,
    name: String,
    country_code: String,
    gender: String,
    age_group: String,
    branch: String,
}

#[derive(Deserialize)]
struct CompetitionPayload {
    id: Option<Uuid>,
    #[serde(default)]
    source_entity_id: Option<String>,
    name: String,
    country_code: String,
    country_name: String,
    tier: i16,
    priority: i32,
}

#[derive(Deserialize)]
struct SeasonPayload {
    id: Option<Uuid>,
    label: String,
}

#[derive(Deserialize)]
struct ResultPayload {
    home_ft: Option<i16>,
    away_ft: Option<i16>,
    home_ht: Option<i16>,
    away_ht: Option<i16>,
    home_et: Option<i16>,
    away_et: Option<i16>,
    home_pens: Option<i16>,
    away_pens: Option<i16>,
    #[serde(default)]
    decided_by: Option<String>,
}

#[derive(Deserialize)]
struct MatchCorePayload {
    canonical_match_id: Option<Uuid>,
    source_home_id: String,
    source_away_id: String,
    kickoff_at: DateTime<Utc>,
    #[serde(default)]
    kickoff_precision: Option<String>,
    status: String,
    result: Option<ResultPayload>,
    round_label: Option<String>,
    competition_timezone: String,
    competition: CompetitionPayload,
    season: SeasonPayload,
    source_code: String,
    captured_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct FlatOddsPayload {
    home: f64,
    draw: f64,
    away: f64,
    btts_yes: f64,
    btts_no: f64,
    over25: f64,
    under25: f64,
}

#[derive(Deserialize)]
struct QuotePayload {
    bookmaker: String,
    market: String,
    outcome: String,
    line: f64,
    phase: String,
    price: f64,
}

#[derive(Deserialize)]
struct OddsPayload {
    odds: Option<FlatOddsPayload>,
    quotes: Option<Vec<QuotePayload>>,
    market_count: Option<i32>,
}

#[derive(Deserialize)]
struct StandingPayload {
    position: i16,
}

#[derive(Deserialize)]
struct StatsPayload {
    home_shots: Option<i32>,
    away_shots: Option<i32>,
    home_shots_on_target: Option<i32>,
    away_shots_on_target: Option<i32>,
    home_corners: Option<i32>,
    away_corners: Option<i32>,
    home_yellow: Option<i32>,
    away_yellow: Option<i32>,
    home_red: Option<i32>,
    away_red: Option<i32>,
    home_fouls: Option<i32>,
    away_fouls: Option<i32>,
    home_xg: Option<f64>,
    away_xg: Option<f64>,
}

#[derive(Deserialize)]
struct RefereePayload {
    name: String,
    #[serde(default)]
    country: Option<String>,
}

/// S90: a `match_environment` payload az API-kontraktus mezőneveivel; minden
/// mező opcionális (a hiányzó adat `None`, nem 0).
#[derive(Deserialize)]
struct EnvironmentPayload {
    venue_name: Option<String>,
    venue_city: Option<String>,
    venue_country: Option<String>,
    venue_capacity: Option<i32>,
    venue_surface: Option<String>,
    venue_lat: Option<f64>,
    venue_lon: Option<f64>,
    temperature_c: Option<f64>,
    weather_description: Option<String>,
    wind_speed: Option<f64>,
    wind_direction: Option<String>,
    humidity_pct: Option<i32>,
    precipitation: Option<f64>,
    cloud_cover_pct: Option<i32>,
    weather_observed_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
struct IncidentsPayload {
    incidents: Vec<IncidentPayload>,
}

#[derive(Deserialize)]
struct IncidentPayload {
    minute: Option<i32>,
    incident_type: String,
    #[serde(default)]
    side: Option<String>,
    #[serde(default)]
    player_name: Option<String>,
    #[serde(default)]
    related_player_name: Option<String>,
    #[serde(default)]
    detail: Option<String>,
    #[serde(default)]
    score_after_home: Option<i16>,
    #[serde(default)]
    score_after_away: Option<i16>,
}

impl NormalizedDay {
    /// Az observation-aspketekből (verziózott payload-séma) épített nap.
    pub fn from_observations(observations: &[Observation]) -> Result<Self> {
        let mut teams = Vec::new();
        let mut matches = Vec::new();
        let mut standings = Vec::new();
        let mut odds_by_key: Vec<(String, NormalizedOdds)> = Vec::new();
        let mut stats_by_key: Vec<(String, NormalizedStats)> = Vec::new();
        let mut incidents_by_key: Vec<(String, Vec<NormalizedIncident>)> = Vec::new();
        let mut referee_by_key: Vec<(String, String, Option<String>)> = Vec::new();
        let mut environment_by_key: Vec<(String, NormalizedEnvironment)> = Vec::new();
        let mut meta: Option<(
            NormalizedCompetition,
            NormalizedSeason,
            String,
            DateTime<Utc>,
        )> = None;

        for obs in observations {
            match obs.aspect.as_str() {
                "team_identity" => {
                    let p: TeamPayload = serde_json::from_value(obs.payload.clone())
                        .context("team_identity payload")?;
                    teams.push(NormalizedTeam {
                        id: p.id,
                        source_entity_id: p.source_entity_id,
                        name: p.name,
                        country_code: p.country_code,
                        gender: p.gender,
                        age_group: p.age_group,
                        branch: p.branch,
                    });
                }
                "match_core" => {
                    let p: MatchCorePayload = serde_json::from_value(obs.payload.clone())
                        .context("match_core payload")?;
                    if meta.is_none() {
                        meta = Some((
                            NormalizedCompetition {
                                id: p.competition.id,
                                source_entity_id: p.competition.source_entity_id,
                                name: p.competition.name,
                                country_code: p.competition.country_code,
                                country_name: p.competition.country_name,
                                tier: p.competition.tier,
                                priority: p.competition.priority,
                            },
                            NormalizedSeason {
                                id: p.season.id,
                                label: p.season.label,
                            },
                            p.source_code,
                            p.captured_at,
                        ));
                    }
                    matches.push(NormalizedMatch {
                        id: p.canonical_match_id,
                        source_match_id: obs.source_record_key.clone(),
                        home_source_entity_id: p.source_home_id,
                        away_source_entity_id: p.source_away_id,
                        round_label: p.round_label,
                        kickoff_at: p.kickoff_at,
                        kickoff_precision: p.kickoff_precision.unwrap_or_else(|| "exact".into()),
                        competition_timezone: p.competition_timezone,
                        status: p.status,
                        result: p.result.map(|r| NormalizedResult {
                            home_ft: r.home_ft,
                            away_ft: r.away_ft,
                            home_ht: r.home_ht,
                            away_ht: r.away_ht,
                            home_et: r.home_et,
                            away_et: r.away_et,
                            home_pens: r.home_pens,
                            away_pens: r.away_pens,
                            decided_by: r.decided_by,
                        }),
                        odds: None,
                        stats: None,
                        referee: None,
                        referee_country: None,
                        environment: None,
                        incidents: Vec::new(),
                    });
                }
                "match_odds" => {
                    let p: OddsPayload = serde_json::from_value(obs.payload.clone())
                        .context("match_odds payload")?;
                    let quotes = match (p.quotes, p.odds) {
                        (Some(quotes), _) => quotes
                            .into_iter()
                            .map(|q| NormalizedQuote {
                                bookmaker: q.bookmaker,
                                market: q.market,
                                outcome: q.outcome,
                                line: q.line,
                                phase: q.phase,
                                price: q.price,
                            })
                            .collect(),
                        (None, Some(flat)) => flat_quotes(
                            flat.home,
                            flat.draw,
                            flat.away,
                            flat.btts_yes,
                            flat.btts_no,
                            flat.over25,
                            flat.under25,
                        ),
                        (None, None) => Vec::new(),
                    };
                    odds_by_key.push((
                        obs.source_record_key.clone(),
                        NormalizedOdds {
                            market_count: p.market_count,
                            quotes,
                        },
                    ));
                }
                "match_stats" => {
                    let p: StatsPayload = serde_json::from_value(obs.payload.clone())
                        .context("match_stats payload")?;
                    stats_by_key.push((
                        obs.source_record_key.clone(),
                        NormalizedStats {
                            home_shots: p.home_shots,
                            away_shots: p.away_shots,
                            home_shots_on_target: p.home_shots_on_target,
                            away_shots_on_target: p.away_shots_on_target,
                            home_corners: p.home_corners,
                            away_corners: p.away_corners,
                            home_yellow: p.home_yellow,
                            away_yellow: p.away_yellow,
                            home_red: p.home_red,
                            away_red: p.away_red,
                            home_fouls: p.home_fouls,
                            away_fouls: p.away_fouls,
                            home_xg: p.home_xg,
                            away_xg: p.away_xg,
                        },
                    ));
                }
                "match_events" => {
                    let p: IncidentsPayload = serde_json::from_value(obs.payload.clone())
                        .context("match_events payload")?;
                    incidents_by_key.push((
                        obs.source_record_key.clone(),
                        p.incidents
                            .into_iter()
                            .map(|i| NormalizedIncident {
                                minute: i.minute,
                                incident_type: i.incident_type,
                                side: i.side,
                                player_name: i.player_name,
                                related_player_name: i.related_player_name,
                                detail: i.detail,
                                score_after_home: i.score_after_home,
                                score_after_away: i.score_after_away,
                            })
                            .collect(),
                    ));
                }
                "match_referee" => {
                    let p: RefereePayload = serde_json::from_value(obs.payload.clone())
                        .context("match_referee payload")?;
                    referee_by_key.push((obs.source_record_key.clone(), p.name, p.country));
                }
                "match_environment" => {
                    let p: EnvironmentPayload = serde_json::from_value(obs.payload.clone())
                        .context("match_environment payload")?;
                    environment_by_key.push((
                        obs.source_record_key.clone(),
                        NormalizedEnvironment {
                            observed_at: obs.observed_at,
                            venue_name: p.venue_name,
                            venue_city: p.venue_city,
                            venue_country: p.venue_country,
                            venue_capacity: p.venue_capacity,
                            venue_surface: p.venue_surface,
                            venue_lat: p.venue_lat,
                            venue_lon: p.venue_lon,
                            temperature_c: p.temperature_c,
                            weather_description: p.weather_description,
                            wind_speed: p.wind_speed,
                            wind_direction: p.wind_direction,
                            humidity_pct: p.humidity_pct,
                            precipitation: p.precipitation,
                            cloud_cover_pct: p.cloud_cover_pct,
                            weather_observed_at: p.weather_observed_at,
                        },
                    ));
                }
                "standing" => {
                    let p: StandingPayload =
                        serde_json::from_value(obs.payload.clone()).context("standing payload")?;
                    standings.push(NormalizedStanding {
                        team_source_entity_id: obs.source_record_key.clone(),
                        position: p.position,
                    });
                }
                other => anyhow::bail!("ismeretlen observation-aspect: {other}"),
            }
        }

        for m in &mut matches {
            if let Some((_, odds)) = odds_by_key
                .iter()
                .find(|(key, _)| *key == m.source_match_id)
            {
                m.odds = Some(odds.clone());
            }
            if let Some((_, stats)) = stats_by_key
                .iter()
                .find(|(key, _)| *key == m.source_match_id)
            {
                m.stats = Some(stats.clone());
            }
            if let Some((_, name, country)) = referee_by_key
                .iter()
                .find(|(key, _, _)| *key == m.source_match_id)
            {
                m.referee = Some(name.clone());
                m.referee_country = country.clone();
            }
            if let Some((_, environment)) = environment_by_key
                .iter()
                .find(|(key, _)| *key == m.source_match_id)
            {
                m.environment = Some(environment.clone());
            }
            if let Some((_, incidents)) = incidents_by_key
                .iter()
                .find(|(key, _)| *key == m.source_match_id)
            {
                m.incidents = incidents.clone();
            }
        }

        let (competition, season, source_code, captured_at) =
            meta.context("hiányzó match_core meta az observationökből")?;
        Ok(Self {
            source_code,
            captured_at,
            competition,
            season,
            teams,
            matches,
            standings: if standings.is_empty() {
                None
            } else {
                Some(standings)
            },
        })
    }
}

impl NormalizedDay {
    /// S9: a napi observationöket **verseny szerint** bontja; minden csoport
    /// önálló `NormalizedDay` (a megszokott `from_observations` szerződésen).
    /// A csoport kulcsa az ország+név+tier+priority+szezon; a csapatok és a
    /// meccshez kötött aspketek (odds/stats/referee/tabella) a csoporthoz
    /// kerülnek. Üres bemenet → hiba (a hívó a nulla-napot külön kezeli).
    pub fn grouped_from_observations(observations: &[Observation]) -> Result<Vec<Self>> {
        use std::collections::{BTreeSet, HashMap, HashSet};

        #[derive(Clone, PartialEq, Eq, Hash)]
        struct GroupKey {
            country_code: String,
            name: String,
            tier: i16,
            priority: i32,
            season_label: String,
        }

        let mut team_payloads: HashMap<String, Observation> = HashMap::new();
        for obs in observations {
            if obs.aspect == "team_identity"
                && let Some(id) = obs.payload.get("source_entity_id").and_then(|v| v.as_str())
            {
                team_payloads
                    .entry(id.to_string())
                    .or_insert_with(|| obs.clone());
            }
        }

        let mut order: Vec<GroupKey> = Vec::new();
        let mut groups: HashMap<GroupKey, Vec<Observation>> = HashMap::new();
        for obs in observations {
            if obs.aspect != "match_core" {
                continue;
            }
            let competition = obs.payload.get("competition");
            let field = |key: &str| {
                competition
                    .and_then(|c| c.get(key))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            let key = GroupKey {
                country_code: field("country_code"),
                name: field("name"),
                tier: competition
                    .and_then(|c| c.get("tier"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(1) as i16,
                priority: competition
                    .and_then(|c| c.get("priority"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(100) as i32,
                season_label: obs
                    .payload
                    .pointer("/season/label")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            };
            if !groups.contains_key(&key) {
                order.push(key.clone());
            }
            groups.entry(key).or_default().push(obs.clone());
        }
        if order.is_empty() {
            anyhow::bail!("hiányzó match_core meta az observationökből");
        }

        let mut days = Vec::with_capacity(order.len());
        for key in order {
            let matches = groups.remove(&key).unwrap_or_default();
            let records: HashSet<String> = matches
                .iter()
                .map(|m| m.source_record_key.clone())
                .collect();
            let mut team_ids: BTreeSet<String> = BTreeSet::new();
            for m in &matches {
                for field in ["source_home_id", "source_away_id"] {
                    if let Some(id) = m.payload.get(field).and_then(|v| v.as_str()) {
                        team_ids.insert(id.to_string());
                    }
                }
            }
            let mut slice: Vec<Observation> = Vec::new();
            for id in &team_ids {
                if let Some(team) = team_payloads.get(id) {
                    slice.push(team.clone());
                }
            }
            slice.extend(matches);
            for obs in observations {
                match obs.aspect.as_str() {
                    "match_odds" | "match_stats" | "match_referee" | "match_events"
                    | "match_environment" => {
                        if records.contains(&obs.source_record_key) {
                            slice.push(obs.clone());
                        }
                    }
                    "standing" if team_ids.contains(obs.source_record_key.as_str()) => {
                        slice.push(obs.clone());
                    }
                    _ => {}
                }
            }
            days.push(Self::from_observations(&slice)?);
        }
        Ok(days)
    }
}

impl From<&FixtureSource> for NormalizedDay {
    fn from(fixture: &FixtureSource) -> Self {
        Self {
            source_code: fixture.source_code.clone(),
            captured_at: fixture.captured_at,
            competition: NormalizedCompetition {
                id: Some(fixture.competition.id),
                source_entity_id: None,
                name: fixture.competition.name.clone(),
                country_code: fixture.competition.country_code.clone(),
                country_name: fixture.competition.country_name.clone(),
                tier: fixture.competition.tier,
                priority: fixture.competition.priority,
            },
            season: NormalizedSeason {
                id: Some(fixture.season.id),
                label: fixture.season.label.clone(),
            },
            teams: fixture
                .teams
                .iter()
                .map(|team| NormalizedTeam {
                    id: Some(team.id),
                    source_entity_id: team.source_entity_id.clone(),
                    name: team.name.clone(),
                    country_code: team.country_code.clone(),
                    gender: team.gender.clone(),
                    age_group: team.age_group.clone(),
                    branch: team.branch.clone(),
                })
                .collect(),
            matches: fixture
                .matches
                .iter()
                .map(|m| NormalizedMatch {
                    id: Some(m.id),
                    source_match_id: m.source_match_id.clone(),
                    home_source_entity_id: m.home_source_entity_id.clone(),
                    away_source_entity_id: m.away_source_entity_id.clone(),
                    round_label: m.round_label.clone(),
                    kickoff_at: m.kickoff_at,
                    kickoff_precision: "exact".into(),
                    competition_timezone: m.competition_timezone.clone(),
                    status: m.status.clone(),
                    result: m.result.as_ref().map(|r| NormalizedResult {
                        home_ft: r.home_ft,
                        away_ft: r.away_ft,
                        home_ht: r.home_ht,
                        away_ht: r.away_ht,
                        home_et: r.home_et,
                        away_et: r.away_et,
                        home_pens: r.home_pens,
                        away_pens: r.away_pens,
                        decided_by: None,
                    }),
                    odds: m.odds.as_ref().map(|odds| NormalizedOdds {
                        market_count: m.market_count,
                        quotes: default_quotes(odds),
                    }),
                    stats: None,
                    referee: None,
                    referee_country: None,
                    environment: None,
                    incidents: Vec::new(),
                })
                .collect(),
            standings: fixture.standings.as_ref().map(|rows| {
                rows.iter()
                    .map(|standing| NormalizedStanding {
                        team_source_entity_id: standing.team_source_entity_id.clone(),
                        position: standing.position,
                    })
                    .collect()
            }),
        }
    }
}

/// clubelo-pillanatkép nap (nem meccs-scoped feed).
#[derive(Debug, Clone)]
pub struct ClubEloDay {
    pub source_code: String,
    pub captured_at: DateTime<Utc>,
    pub as_of: NaiveDate,
    pub rows: Vec<EloRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EloRow {
    pub country_code: String,
    pub club_name: String,
    pub elo: f64,
    pub provenance: String,
}

#[derive(Deserialize)]
struct EloPayload {
    country: String,
    club: String,
    elo: f64,
    as_of: NaiveDate,
    provenance: String,
}

impl ClubEloDay {
    /// Az `club_elo` aspketekből épített nap; a meta az első sorból.
    pub fn from_observations(observations: &[Observation]) -> Result<Self> {
        let mut rows = Vec::new();
        let mut as_of: Option<NaiveDate> = None;
        let mut captured_at: Option<DateTime<Utc>> = None;
        let mut source_code: Option<String> = None;
        for obs in observations {
            if obs.aspect != "club_elo" {
                continue;
            }
            let p: EloPayload =
                serde_json::from_value(obs.payload.clone()).context("club_elo payload")?;
            as_of.get_or_insert(p.as_of);
            captured_at.get_or_insert(obs.observed_at);
            source_code.get_or_insert_with(|| "clubelo".to_string());
            rows.push(EloRow {
                country_code: p.country,
                club_name: p.club,
                elo: p.elo,
                provenance: p.provenance,
            });
        }
        if rows.is_empty() {
            anyhow::bail!("nincs club_elo observation");
        }
        Ok(Self {
            source_code: source_code.expect("nem üres"),
            captured_at: captured_at.expect("nem üres"),
            as_of: as_of.expect("nem üres"),
            rows,
        })
    }
}
