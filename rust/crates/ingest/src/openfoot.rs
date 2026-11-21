//! OpenFoot API adapter (S13) — Bearer-kulcsos JSON nap-feed.
//!
//! Kétféle kérés egy napi jobban: a verseny-registry (`/v1/competitions`,
//! id → név/ország) és a nap-feed (`/v1/matches?date=…&limit=100&page=N`).
//! A registry a parse-láncban cache-be kerül, a nap-feed parse ebből oldja
//! fel a verseny-metát (a feed csak `competitionId`-t ad). A nap UTC-alapú;
//! dátum-guard: minden sor kickoff-dátuma a kért nap. Eredmény csak
//! `finished` + két pontszám esetén; oddsot a feed nem ad.

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, TimeZone, Utc};
use football_domain::error::ParseError;
use football_domain::policy::{FetchJob, Transport};

use crate::adapter::{AdapterMeta, Observation, ParseCtx, SourceAdapter};
use crate::countries;
use crate::fetch::{FetchRequest, RawPayload};

pub const SOURCE_SLUG: &str = "openfoot";
const BASE: &str = "https://openfootapi.com/v1";
/// Hány oldalt kérünk a nap-feedből (100 sor/oldal; 3 oldal = 300 sor).
const DAY_PAGES: u32 = 3;

/// Ismert versenyek kanonikus meta-ja (a footballdata neveivel egyezően,
/// hogy a verseny-entitások összeérjenek).
const KNOWN: [(&str, &str, &str, &str, i16, i32); 7] = [
    (
        "comp_premier_league_eng",
        "Premier League",
        "GB",
        "England",
        1,
        6,
    ),
    ("comp_laliga_es", "La Liga", "ES", "Spain", 1, 6),
    ("comp_serie_a_it", "Serie A", "IT", "Italy", 1, 6),
    ("comp_bundesliga_de", "Bundesliga", "DE", "Germany", 1, 6),
    ("comp_ligue_1_fr", "Ligue 1", "FR", "France", 1, 6),
    ("comp_nb_1_hun", "NB I", "HU", "Hungary", 1, 8),
    ("comp_nb_2_hun", "NB II", "HU", "Hungary", 2, 10),
];

#[derive(Debug, Clone)]
struct CompetitionMeta {
    name: String,
    country_code: String,
    country_name: String,
    tier: i16,
    priority: i32,
    gender: &'static str,
    branch: &'static str,
    international: bool,
}

fn is_cup(text: &str) -> bool {
    ["kupa", "cup", "copa", "coupe", "pokal", "supercup"]
        .iter()
        .any(|needle| text.contains(needle))
}

fn is_women(text: &str) -> bool {
    ["women", "női", "feminine", "femenina", "femminile"]
        .iter()
        .any(|needle| text.contains(needle))
}

fn is_youth(text: &str) -> bool {
    ["u17", "u18", "u19", "u20", "u21", "u23", "youth", "junior"]
        .iter()
        .any(|needle| text.contains(needle))
}

pub struct OpenFootAdapter {
    api_key: Option<String>,
    registry: Mutex<HashMap<String, CompetitionMeta>>,
}

impl Default for OpenFootAdapter {
    fn default() -> Self {
        Self::new(None)
    }
}

impl OpenFootAdapter {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            api_key,
            registry: Mutex::new(HashMap::new()),
        }
    }

    /// Az API-kulcs a `OPENFOOT_API_KEY` env változóból (Keychain-injektálás
    /// a burkoló scriptben); hiányzó kulcsnál a fetch 401-et kap.
    pub fn from_env() -> Self {
        Self::new(
            std::env::var("OPENFOOT_API_KEY")
                .ok()
                .filter(|k| !k.is_empty()),
        )
    }

    fn meta_for(&self, competition_id: &str) -> CompetitionMeta {
        if let Some((_, name, cc, cn, tier, priority)) =
            KNOWN.iter().find(|(id, ..)| *id == competition_id)
        {
            return CompetitionMeta {
                name: (*name).to_string(),
                country_code: (*cc).to_string(),
                country_name: (*cn).to_string(),
                tier: *tier,
                priority: *priority,
                gender: "male",
                branch: "senior",
                international: false,
            };
        }
        let registry = self.registry.lock().expect("registry lock");
        if let Some(meta) = registry.get(competition_id) {
            return meta.clone();
        }
        drop(registry);
        tracing::warn!(
            source = SOURCE_SLUG,
            competition_id,
            "ismeretlen verseny a registry-ben: azonosítóval, ország nélkül"
        );
        CompetitionMeta {
            name: competition_id.to_string(),
            country_code: String::new(),
            country_name: String::new(),
            tier: 1,
            priority: 50,
            gender: "male",
            branch: "senior",
            international: false,
        }
    }
}

impl SourceAdapter for OpenFootAdapter {
    fn meta(&self) -> AdapterMeta {
        AdapterMeta {
            source_slug: SOURCE_SLUG.into(),
            adapter_version: "1.0".into(),
            parser_version: "1.0".into(),
            domains: vec!["day".into()],
            transport: Transport::RustHttp,
        }
    }

    fn plan(&self, job: &FetchJob) -> Vec<FetchRequest> {
        let mut requests = Vec::with_capacity(1 + DAY_PAGES as usize);
        requests.push(FetchRequest {
            url: format!("{BASE}/competitions?limit=200"),
            request_key: format!("{SOURCE_SLUG}:competitions:{}", job.time_window),
            endpoint_key: "competitions".into(),
            ready_marker: None,
            bearer_token: self.api_key.clone(),
        });
        for page in 1..=DAY_PAGES {
            requests.push(FetchRequest {
                url: format!(
                    "{BASE}/matches?date={}&limit=100&page={page}",
                    job.time_window
                ),
                request_key: format!("{SOURCE_SLUG}:day:{}:p{page}", job.time_window),
                endpoint_key: "day".into(),
                ready_marker: None,
                bearer_token: self.api_key.clone(),
            });
        }
        requests
    }

    fn parse(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError> {
        if ctx.request_key.contains(":competitions:") {
            return self.parse_registry(raw);
        }
        self.parse_day(raw, ctx)
    }
}

impl OpenFootAdapter {
    /// Verseny-registry: id → (név, ország); observation nem keletkezik.
    fn parse_registry(&self, raw: &RawPayload) -> Result<Vec<Observation>, ParseError> {
        let value: serde_json::Value = serde_json::from_slice(&raw.bytes)
            .map_err(|e| ParseError::Parse(format!("registry JSON: {e}")))?;
        let rows = value
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| ParseError::SchemaDrift("hiányzó registry data tömb".into()))?;
        let mut registry = self.registry.lock().expect("registry lock");
        for row in rows {
            let Some(id) = row.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            let name = row.get("name").and_then(|v| v.as_str()).unwrap_or(id);
            let country = row.get("country").and_then(|v| v.as_str()).unwrap_or("");
            let haystack = format!("{name} {country}").to_lowercase();
            let (gender, branch) = if is_women(&haystack) {
                ("female", "women")
            } else if is_youth(&haystack) {
                ("male", "youth")
            } else {
                ("male", "senior")
            };
            let international = countries::is_international_text(&haystack)
                || countries::is_continent_flag(country);
            let (country_code, country_name) = match countries::by_english_name(country) {
                Some((cc, cn)) => (cc.to_string(), cn.to_string()),
                None if international => (String::new(), country.to_string()),
                None => (String::new(), country.to_string()),
            };
            let cup = is_cup(&haystack);
            let priority = if gender == "female" {
                60
            } else if branch == "youth" {
                70
            } else if cup {
                40
            } else {
                50
            };
            registry.insert(
                id.to_string(),
                CompetitionMeta {
                    name: name.to_string(),
                    country_code,
                    country_name,
                    tier: if cup { 0 } else { 1 },
                    priority,
                    gender,
                    branch,
                    international,
                },
            );
        }
        Ok(Vec::new())
    }

    /// Nap-feed: dátum-guard + meccs-observationök.
    fn parse_day(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError> {
        let value: serde_json::Value = serde_json::from_slice(&raw.bytes)
            .map_err(|e| ParseError::Parse(format!("feed JSON: {e}")))?;
        let rows = value
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| ParseError::SchemaDrift("hiányzó data tömb".into()))?;

        let captured_at = Utc::now();
        let mut observations = Vec::new();
        let mut teams: std::collections::BTreeMap<String, serde_json::Value> = Default::default();

        for row in rows {
            let id = row
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ParseError::SchemaDrift("meccs id hiányzik".into()))?;
            let kickoff_at: DateTime<Utc> = row
                .get("kickoffAt")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ParseError::SchemaDrift("kickoffAt hiányzik".into()))
                .and_then(|s| {
                    DateTime::parse_from_rfc3339(s)
                        .map(|d| d.with_timezone(&Utc))
                        .map_err(|e| ParseError::Parse(format!("kickoffAt: {e}")))
                })?;
            // Dátum-guard: a feed a kért napot adja (fail-closed).
            if kickoff_at.date_naive() != ctx.requested_date {
                return Err(ParseError::DateGuardMismatch {
                    requested: ctx.requested_date.to_string(),
                    got: kickoff_at.date_naive().to_string(),
                });
            }
            let kickoff_known = row
                .get("kickoffTimeKnown")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let (kickoff_at, kickoff_precision) = if kickoff_known {
                (kickoff_at, "exact")
            } else {
                let naive = ctx
                    .requested_date
                    .and_hms_opt(12, 0, 0)
                    .ok_or_else(|| ParseError::Parse("érvénytelen kezdés".into()))?;
                (Utc.from_utc_datetime(&naive), "date_only")
            };

            let status = match row.get("status").and_then(|v| v.as_str()).unwrap_or("") {
                "scheduled" => "scheduled",
                "live" => "live",
                "finished" => "finished",
                "postponed" => "postponed",
                "cancelled" | "canceled" => "canceled",
                other => {
                    tracing::warn!(
                        source = SOURCE_SLUG,
                        id,
                        status = other,
                        "ismeretlen státusz: a sor kimarad"
                    );
                    continue;
                }
            };
            let score = row.get("score");
            let home_ft = score
                .and_then(|s| s.get("home"))
                .and_then(|v| v.as_i64())
                .map(|v| v as i16);
            let away_ft = score
                .and_then(|s| s.get("away"))
                .and_then(|v| v.as_i64())
                .map(|v| v as i16);
            // Csak a lezárt, mindkét pontszámmal rendelkező meccs eredmény.
            let result = if status == "finished" {
                match (home_ft, away_ft) {
                    (Some(h), Some(a)) => Some(serde_json::json!({
                        "home_ft": h, "away_ft": a,
                        "home_ht": null, "away_ht": null,
                        "home_et": null, "away_et": null,
                        "home_pens": null, "away_pens": null,
                        "decided_by": "ft",
                    })),
                    _ => None,
                }
            } else {
                None
            };

            let side = |key: &str| -> Result<(String, String, String), ParseError> {
                let team = row
                    .get(key)
                    .ok_or_else(|| ParseError::SchemaDrift(format!("hiányzó {key}")))?;
                let team_id = team
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ParseError::SchemaDrift("csapat id hiányzik".into()))?;
                let name = team
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ParseError::SchemaDrift("csapatnév hiányzik".into()))?;
                let country = team
                    .get("country")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok((team_id.to_string(), name.to_string(), country))
            };
            let (home_id, home_name, home_country) = side("homeTeam")?;
            let (away_id, away_name, away_country) = side("awayTeam")?;

            let competition_id = row
                .get("competitionId")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ParseError::SchemaDrift("competitionId hiányzik".into()))?;
            let competition = self.meta_for(competition_id);

            // Csapat-ország: nemzetközi versenyben a névből, hazai ligában a
            // csapat saját ország-mezőjéből (angol név → ISO).
            let team_country = |name: &str, country: &str| -> String {
                let source = if competition.international {
                    name
                } else {
                    country
                };
                countries::by_english_name(source)
                    .map(|(cc, _)| cc.to_string())
                    .unwrap_or_default()
            };
            for (team_id, team_name, team_country_field) in [
                (&home_id, &home_name, &home_country),
                (&away_id, &away_name, &away_country),
            ] {
                let country = team_country(team_name, team_country_field);
                teams.entry(team_id.clone()).or_insert_with(|| {
                    serde_json::json!({
                        "id": null,
                        "source_entity_id": team_id,
                        "name": team_name,
                        "country_code": country,
                        "gender": competition.gender,
                        "age_group": if competition.branch == "youth" { "youth" } else { "senior" },
                        "branch": competition.branch,
                    })
                });
            }

            let season = row
                .get("season")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let round = row.get("round").and_then(|v| v.as_str());
            let source_match_id = format!("openfoot-{id}");
            observations.push(Observation {
                entity_kind: "match".into(),
                aspect: "match_core".into(),
                natural_key: source_match_id.clone(),
                source_record_key: source_match_id,
                observed_at: captured_at,
                payload: serde_json::json!({
                    "canonical_match_id": null,
                    "source_home_id": home_id,
                    "source_away_id": away_id,
                    "kickoff_at": kickoff_at,
                    "kickoff_precision": kickoff_precision,
                    "status": status,
                    "result": result,
                    "round_label": round,
                    "competition_timezone": "UTC",
                    "competition": {
                        "id": null,
                        "source_entity_id": (!competition_id.is_empty()).then_some(competition_id),
                        "name": competition.name,
                        "country_code": competition.country_code,
                        "country_name": competition.country_name,
                        "tier": competition.tier,
                        "priority": competition.priority,
                    },
                    "season": { "id": null, "label": season },
                    "source_code": SOURCE_SLUG,
                    "captured_at": captured_at,
                }),
            });
        }

        let mut team_obs: Vec<Observation> = teams
            .into_iter()
            .map(|(code, payload)| Observation {
                entity_kind: "team".into(),
                aspect: "team_identity".into(),
                natural_key: code.clone(),
                source_record_key: code,
                observed_at: captured_at,
                payload,
            })
            .collect();
        team_obs.extend(observations);
        Ok(team_obs)
    }
}
