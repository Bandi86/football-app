//! FotMob nap-feed adapter — S4b.
//!
//! A fotmob.com nap-feedje **nyílt JSON API** (rust_http, nem kell böngésző):
//! `/api/data/matches?date=YYYYMMDD&timezone=UTC&locale=en`. A robots.txt
//! explicit `Allow: /api/*` szabályt ad; a worker robots-kapuja dönt.
//! A `status.utcTime` közvetlenül UTC, a csapatok stabil FotMob-id-t adnak.
//! Liga-szűrő: a policy domain → (liganév, országkód) pár; a default a top-5.

use chrono::{DateTime, Utc};
use football_domain::error::ParseError;
use football_domain::policy::{FetchJob, Transport};

use crate::adapter::{AdapterMeta, Observation, ParseCtx, SourceAdapter};
use crate::fetch::{FetchRequest, RawPayload};

pub const SOURCE_SLUG: &str = "fotmob";

/// (domain, liganév a feedben, országkód)
const LEAGUES: [(&str, &str, &str); 5] = [
    ("epl", "Premier League", "ENG"),
    ("laliga", "La Liga", "ESP"),
    ("seriea", "Serie A", "ITA"),
    ("bundesliga", "Bundesliga", "GER"),
    ("ligue1", "Ligue 1", "FRA"),
];

pub struct FotMobAdapter {
    /// Teszt/konfigurálható liga-szűrő: (liganév, országkód).
    pub leagues: Vec<(String, String)>,
}

impl Default for FotMobAdapter {
    fn default() -> Self {
        Self {
            leagues: LEAGUES
                .iter()
                .map(|(_, name, cc)| ((*name).to_string(), (*cc).to_string()))
                .collect(),
        }
    }
}

fn slug(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
}

struct LeagueMeta {
    country_code: String,
    country_name: String,
    tier: i16,
    priority: i32,
    team_country: String,
    gender: &'static str,
    branch: &'static str,
}

fn ccode_iso(ccode: &str) -> Option<(&'static str, &'static str)> {
    match ccode {
        "ENG" => Some(("GB", "England")),
        "ESP" => Some(("ES", "Spain")),
        "ITA" => Some(("IT", "Italy")),
        "GER" => Some(("DE", "Germany")),
        "FRA" => Some(("FR", "France")),
        "NED" => Some(("NL", "Netherlands")),
        "POR" => Some(("PT", "Portugal")),
        "BEL" => Some(("BE", "Belgium")),
        "TUR" => Some(("TR", "Turkey")),
        "GRE" => Some(("GR", "Greece")),
        "SCO" => Some(("GB", "Scotland")),
        "WAL" => Some(("GB", "Wales")),
        "IRL" => Some(("IE", "Ireland")),
        "AUT" => Some(("AT", "Austria")),
        "SUI" => Some(("CH", "Switzerland")),
        "DEN" => Some(("DK", "Denmark")),
        "SWE" => Some(("SE", "Sweden")),
        "NOR" => Some(("NO", "Norway")),
        "FIN" => Some(("FI", "Finland")),
        "POL" => Some(("PL", "Poland")),
        "CZE" => Some(("CZ", "Czech Republic")),
        "CRO" => Some(("HR", "Croatia")),
        "SRB" => Some(("RS", "Serbia")),
        "RUS" => Some(("RU", "Russia")),
        "UKR" => Some(("UA", "Ukraine")),
        "BRA" => Some(("BR", "Brazil")),
        "ARG" => Some(("AR", "Argentina")),
        "USA" => Some(("US", "United States")),
        "MEX" => Some(("MX", "Mexico")),
        "JPN" => Some(("JP", "Japan")),
        "KOR" => Some(("KR", "South Korea")),
        "AUS" => Some(("AU", "Australia")),
        "EGY" => Some(("EG", "Egypt")),
        "MAR" => Some(("MA", "Morocco")),
        "ALG" => Some(("DZ", "Algeria")),
        "TUN" => Some(("TN", "Tunisia")),
        "RSA" => Some(("ZA", "South Africa")),
        _ => None,
    }
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

impl FotMobAdapter {
    /// Liga-osztályozás: a top-5 ismert (priority 6), a többi 50; kupa 40,
    /// női 60, utánpótlás 70; ismeretlen országkód → ismeretlen ország.
    fn league_meta(&self, name: &str, ccode: &str) -> LeagueMeta {
        let haystack = format!("{name} {ccode}").to_lowercase();
        let (gender, branch) = if is_women(&haystack) {
            ("female", "women")
        } else if is_youth(&haystack) {
            ("male", "youth")
        } else {
            ("male", "senior")
        };
        let cup = is_cup(&haystack);
        let known = self
            .leagues
            .iter()
            .any(|(ln, lc)| ln == name && lc == ccode);
        let (country_code, country_name) = ccode_iso(ccode).unwrap_or(("", ""));
        let priority = if known {
            6
        } else if gender == "female" {
            60
        } else if branch == "youth" {
            70
        } else if cup {
            40
        } else {
            50
        };
        LeagueMeta {
            country_code: country_code.to_string(),
            country_name: country_name.to_string(),
            tier: if cup { 0 } else { 1 },
            priority,
            team_country: if cup || country_code.is_empty() {
                String::new()
            } else {
                country_code.to_string()
            },
            gender,
            branch,
        }
    }
}

impl SourceAdapter for FotMobAdapter {
    fn meta(&self) -> AdapterMeta {
        AdapterMeta {
            source_slug: SOURCE_SLUG.into(),
            adapter_version: "1.0".into(),
            parser_version: "1.0".into(),
            domains: LEAGUES.iter().map(|(d, ..)| (*d).to_string()).collect(),
            transport: Transport::RustHttp,
        }
    }

    fn plan(&self, job: &FetchJob) -> Vec<FetchRequest> {
        // S9: egy napi kérés, minden liga (a nap-feed természetes egysége).
        let compact = job.time_window.replace('-', "");
        vec![FetchRequest {
            url: format!(
                "https://www.fotmob.com/api/data/matches?date={compact}&timezone=UTC&locale=en"
            ),
            request_key: format!("{SOURCE_SLUG}:day:{}", job.time_window),
            endpoint_key: "day".into(),
            ready_marker: None,
            bearer_token: None,
        }]
    }

    fn parse(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError> {
        let value: serde_json::Value = serde_json::from_slice(&raw.bytes)
            .map_err(|e| ParseError::Parse(format!("feed JSON: {e}")))?;
        // Dátum-guard: a feed a kért napot adja (fail-closed, decoy-védelem).
        let expected = ctx.requested_date.format("%Y%m%d").to_string();
        let got = value.get("date").and_then(|d| {
            d.as_str()
                .map(str::to_string)
                .or_else(|| d.as_i64().map(|n| n.to_string()))
        });
        if got.as_deref() != Some(expected.as_str()) {
            return Err(ParseError::DateGuardMismatch {
                requested: expected,
                got: got.unwrap_or_default(),
            });
        }

        let leagues = value
            .get("leagues")
            .and_then(|l| l.as_array())
            .ok_or_else(|| ParseError::SchemaDrift("hiányzó leagues tömb".into()))?;
        let captured_at = Utc::now();
        let mut observations = Vec::new();
        let mut seen_teams: std::collections::BTreeMap<
            String,
            (String, String, &'static str, &'static str),
        > = Default::default();

        for league in leagues {
            let name = league.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let ccode = league.get("ccode").and_then(|v| v.as_str()).unwrap_or("");
            let meta = self.league_meta(name, ccode);
            let matches = league
                .get("matches")
                .and_then(|m| m.as_array())
                .ok_or_else(|| ParseError::SchemaDrift("hiányzó matches tömb".into()))?;
            for m in matches {
                let id = m
                    .get("id")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| ParseError::SchemaDrift("meccs id hiányzik".into()))?;
                let side = |key: &str| -> Result<(String, String, Option<i32>), ParseError> {
                    let s = m
                        .get(key)
                        .ok_or_else(|| ParseError::SchemaDrift(format!("hiányzó {key}")))?;
                    let team_id = s
                        .get("id")
                        .and_then(|v| v.as_i64())
                        .ok_or_else(|| ParseError::SchemaDrift("csapat id hiányzik".into()))?;
                    let team_name = s
                        .get("name")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| ParseError::SchemaDrift("csapatnév hiányzik".into()))?;
                    let score = s.get("score").and_then(|v| v.as_i64()).map(|v| v as i32);
                    Ok((team_id.to_string(), team_name.to_string(), score))
                };
                let (home_id, home_name, home_score) = side("home")?;
                let (away_id, away_name, _away_score) = side("away")?;
                seen_teams.insert(
                    home_id.clone(),
                    (
                        home_name.clone(),
                        meta.team_country.clone(),
                        meta.gender,
                        meta.branch,
                    ),
                );
                seen_teams.insert(
                    away_id.clone(),
                    (
                        away_name.clone(),
                        meta.team_country.clone(),
                        meta.gender,
                        meta.branch,
                    ),
                );

                let status = m
                    .get("status")
                    .ok_or_else(|| ParseError::SchemaDrift("hiányzó status".into()))?;
                let utc_time = status
                    .get("utcTime")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ParseError::SchemaDrift("hiányzó utcTime".into()))?;
                let kickoff_at: DateTime<Utc> = DateTime::parse_from_rfc3339(utc_time)
                    .map_err(|e| ParseError::Parse(format!("utcTime: {e}")))?
                    .with_timezone(&Utc);
                let finished = status
                    .get("finished")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let cancelled = status
                    .get("cancelled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let match_status = if cancelled {
                    "canceled"
                } else if finished {
                    "finished"
                } else {
                    "scheduled"
                };
                let result = if finished && !cancelled {
                    side("away")?.2.map(|away_score| {
                        serde_json::json!({
                            "home_ft": home_score,
                            "away_ft": away_score,
                            "home_ht": null, "away_ht": null,
                            "home_et": null, "away_et": null,
                            "home_pens": null, "away_pens": null,
                        })
                    })
                } else {
                    None
                };
                let source_match_id = format!("fotmob-{id}");
                observations.push(Observation {
                    entity_kind: "match".into(),
                    aspect: "match_core".into(),
                    natural_key: source_match_id.clone(),
                    source_record_key: source_match_id.clone(),
                    observed_at: captured_at,
                    payload: serde_json::json!({
                        "canonical_match_id": null,
                        "source_home_id": home_id,
                        "source_away_id": away_id,
                        "kickoff_at": kickoff_at,
                        "status": match_status,
                        "result": result,
                        "round_label": null,
                        "competition_timezone": "UTC",
                        "competition": {
                            "id": null,
                            "name": name,
                            "country_code": meta.country_code,
                            "country_name": meta.country_name,
                            "tier": meta.tier,
                            "priority": meta.priority,
                        },
                        "season": { "id": null, "label": "2026/27" },
                        "source_code": SOURCE_SLUG,
                        "captured_at": captured_at,
                    }),
                });
            }
        }

        // Csapat-observationök (a meccsek után, determinisztikus sorrendben).
        let mut team_obs = Vec::new();
        for (ext, (name, country, gender, branch)) in &seen_teams {
            team_obs.push(Observation {
                entity_kind: "team".into(),
                aspect: "team_identity".into(),
                natural_key: ext.clone(),
                source_record_key: ext.clone(),
                observed_at: captured_at,
                payload: serde_json::json!({
                    "id": null,
                    "source_entity_id": ext,
                    "name": name,
                    "country_code": country,
                    "gender": gender,
                    "age_group": if *branch == "youth" { "youth" } else { "senior" },
                    "branch": branch,
                }),
            });
        }
        team_obs.extend(observations);
        let _ = slug; // a forrás-azonosító a stabil FotMob-id (slug nem kell)
        Ok(team_obs)
    }
}
