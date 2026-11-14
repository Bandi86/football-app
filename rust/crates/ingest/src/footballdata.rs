//! Footballdata (football-data.co.uk) CSV-adapter — S3b-2.
//!
//! Az 5 demo liga statikus CSV-je (mmz4281/2627/{liga}.csv). A parse a közös
//! observation-sémát állítja elő (`team_identity`, `match_core`, `match_odds`
//! quotes-szal); a kanonikus út `NormalizedDay::from_observations`-en megy át.
//! A napi kérés a policy `domain`-jéből választ ligát (e0, sp1, i1, d1, f1).

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Europe::London;
use football_domain::error::ParseError;
use football_domain::policy::{FetchJob, Transport};

use crate::adapter::{AdapterMeta, Observation, ParseCtx, SourceAdapter};
use crate::fetch::{FetchRequest, RawPayload};

pub const SOURCE_SLUG: &str = "footballdata";

/// liga-kód → (ország-kód, ország-név, verseny-név, tier, priority)
const LEAGUES: [(&str, &str, &str, &str, i16, i32); 5] = [
    ("e0", "GB", "England", "Premier League", 1, 6),
    ("sp1", "ES", "Spain", "La Liga", 1, 6),
    ("i1", "IT", "Italy", "Serie A", 1, 6),
    ("d1", "DE", "Germany", "Bundesliga", 1, 6),
    ("f1", "FR", "France", "Ligue 1", 1, 6),
];

/// A backoff-ablak: a job dátuma ±7 nap.
const WINDOW_DAYS: i64 = 7;

pub struct FootballdataAdapter;

impl FootballdataAdapter {
    fn league(
        domain: &str,
    ) -> Option<(
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        i16,
        i32,
    )> {
        LEAGUES
            .into_iter()
            .find(|(code, ..)| *code == domain.to_ascii_lowercase())
    }
}

fn slug(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn parse_number(value: Option<&str>) -> Option<f64> {
    value.and_then(|v| v.trim().parse::<f64>().ok())
}

impl SourceAdapter for FootballdataAdapter {
    fn meta(&self) -> AdapterMeta {
        AdapterMeta {
            source_slug: SOURCE_SLUG.into(),
            adapter_version: "1.0".into(),
            parser_version: "1.0".into(),
            domains: LEAGUES
                .iter()
                .map(|(code, ..)| (*code).to_string())
                .collect(),
            transport: Transport::RustHttp,
        }
    }

    fn plan(&self, job: &FetchJob) -> Vec<FetchRequest> {
        let league = job.domain.to_ascii_uppercase();
        vec![FetchRequest {
            url: format!("https://www.football-data.co.uk/mmz4281/2627/{league}.csv"),
            request_key: format!("{SOURCE_SLUG}:{}:{}", job.domain, job.time_window),
            endpoint_key: job.domain.clone(),
        }]
    }

    fn parse(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError> {
        let (country_code, country_name, competition_name, tier, priority) =
            Self::league_token(ctx)?;
        let text = std::str::from_utf8(&raw.bytes)
            .map_err(|_| ParseError::Parse("a CSV nem érvényes UTF-8".into()))?;
        let text = text.trim_start_matches('\u{feff}');
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(text.as_bytes());
        let headers = reader
            .headers()
            .map_err(|e| ParseError::Parse(format!("CSV fejléc: {e}")))?
            .clone();
        let column = |name: &str| -> Result<usize, ParseError> {
            headers
                .iter()
                .position(|h| h.trim() == name)
                .ok_or_else(|| ParseError::SchemaDrift(format!("hiányzó CSV-oszlop: {name}")))
        };
        let c_date = column("Date")?;
        let c_time = column("Time")?;
        let c_home = column("HomeTeam")?;
        let c_away = column("AwayTeam")?;
        let c_fthg = column("FTHG")?;
        let c_ftag = column("FTAG")?;
        let c_hthg = column("HTHG")?;
        let c_htag = column("HTAG")?;

        let captured_at = Utc::now();
        let mut teams: std::collections::BTreeMap<String, String> = Default::default();
        let mut observations = Vec::new();
        let mut competition = None;

        for (index, record) in reader.records().enumerate() {
            let record = record.map_err(|e| ParseError::Parse(format!("CSV sor: {e}")))?;
            let field = |i: usize| record.get(i).unwrap_or("").trim().to_string();
            let date = NaiveDate::parse_from_str(&field(c_date), "%d/%m/%Y")
                .map_err(|e| ParseError::Parse(format!("dátum ({}): {e}", field(c_date))))?;
            if (date - ctx.requested_date).num_days().abs() > WINDOW_DAYS {
                continue;
            }
            let kickoff_at = match NaiveTime::parse_from_str(&field(c_time), "%H:%M") {
                Ok(time) => {
                    let local = NaiveDateTime::new(date, time);
                    London
                        .from_local_datetime(&local)
                        .earliest()
                        .map(|dt| dt.with_timezone(&Utc))
                        .ok_or_else(|| ParseError::Parse("érvénytelen londoni időpont".into()))?
                }
                // date_only: déli UTC-helyőrző (dokumentált konvenció).
                Err(_) => Utc.from_utc_datetime(&NaiveDateTime::new(
                    date,
                    NaiveTime::from_hms_opt(12, 0, 0).expect("12:00 érvényes"),
                )),
            };
            if competition.is_none() {
                competition = Some(serde_json::json!({
                    "id": null,
                    "name": competition_name,
                    "country_code": country_code,
                    "country_name": country_name,
                    "tier": tier,
                    "priority": priority,
                }));
            }
            let home = field(c_home);
            let away = field(c_away);
            let home_slug = slug(&home);
            let away_slug = slug(&away);
            teams.insert(home_slug.clone(), home.clone());
            teams.insert(away_slug.clone(), away.clone());

            let finished = !field(c_fthg).is_empty() && !field(c_ftag).is_empty();
            let result = finished.then(|| {
                serde_json::json!({
                    "home_ft": parse_number(Some(&field(c_fthg))).map(|v| v as i16),
                    "away_ft": parse_number(Some(&field(c_ftag))).map(|v| v as i16),
                    "home_ht": parse_number(Some(&field(c_hthg))).map(|v| v as i16),
                    "away_ht": parse_number(Some(&field(c_htag))).map(|v| v as i16),
                    "home_et": null, "away_et": null,
                    "home_pens": null, "away_pens": null,
                })
            });
            let source_match_id =
                format!("{}-{}-{}-{}", ctx.request_key, index, home_slug, away_slug);
            let season = serde_json::json!({ "id": null, "label": "2026/27" });
            observations.push(Observation {
                entity_kind: "match".into(),
                aspect: "match_core".into(),
                natural_key: source_match_id.clone(),
                source_record_key: source_match_id.clone(),
                observed_at: captured_at,
                payload: serde_json::json!({
                    "canonical_match_id": null,
                    "source_home_id": home_slug,
                    "source_away_id": away_slug,
                    "kickoff_at": kickoff_at,
                    "status": if finished { "finished" } else { "scheduled" },
                    "result": result,
                    "round_label": null,
                    "competition_timezone": "Europe/London",
                    "competition": competition.clone().unwrap_or(serde_json::Value::Null),
                    "season": season,
                    "source_code": SOURCE_SLUG,
                    "captured_at": captured_at,
                }),
            });

            let quotes = odds_quotes(&record, &headers);
            if !quotes.is_empty() {
                observations.push(Observation {
                    entity_kind: "match".into(),
                    aspect: "match_odds".into(),
                    natural_key: source_match_id.clone(),
                    source_record_key: source_match_id,
                    observed_at: captured_at,
                    payload: serde_json::json!({
                        "canonical_match_id": null,
                        "quotes": quotes,
                        "market_count": 2,
                    }),
                });
            }
        }

        // Csapat-observationök (a meccsek után, hogy a sorrend determinisztikus).
        let mut team_obs = Vec::new();
        for (ext, name) in &teams {
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
                    "country_code": country_code,
                    "gender": "male",
                    "age_group": "senior",
                    "branch": "senior",
                }),
            });
        }
        team_obs.extend(observations);
        Ok(team_obs)
    }
}

impl FootballdataAdapter {
    fn league_token(
        ctx: &ParseCtx,
    ) -> Result<(&'static str, &'static str, &'static str, i16, i32), ParseError> {
        // A request_key formátuma: footballdata:{domain}:{time_window}
        let domain = ctx
            .request_key
            .split(':')
            .nth(1)
            .ok_or_else(|| ParseError::Parse("a request_key-ből nem derül ki a liga".into()))?;
        let (_, country_code, country_name, competition, tier, priority) = Self::league(domain)
            .ok_or_else(|| {
                ParseError::SchemaDrift(format!("ismeretlen footballdata liga: {domain}"))
            })?;
        Ok((country_code, country_name, competition, tier, priority))
    }
}

/// B365/Avg/Max 1x2 (opening + closing) és O/U 2.5 quote-ok a sor oszlopaiból.
fn odds_quotes(record: &csv::StringRecord, headers: &csv::StringRecord) -> Vec<serde_json::Value> {
    let mut quotes = Vec::new();
    let get = |name: &str| -> Option<f64> {
        headers
            .iter()
            .position(|h| h.trim() == name)
            .and_then(|i| record.get(i))
            .and_then(|v| parse_number(Some(v)))
    };
    for bookmaker in ["B365", "Avg", "Max"] {
        for (suffix, phase) in [("", "opening"), ("C", "closing")] {
            let outcome = |letter: &str| get(&format!("{bookmaker}{suffix}{letter}"));
            if let (Some(home), Some(draw), Some(away)) = (outcome("H"), outcome("D"), outcome("A"))
            {
                for (name, price) in [("home", home), ("draw", draw), ("away", away)] {
                    quotes.push(quote(bookmaker, "1x2", name, 0.0, phase, price));
                }
            }
            let over = get(&format!("{bookmaker}{suffix}>2.5"));
            let under = get(&format!("{bookmaker}{suffix}<2.5"));
            if let (Some(over), Some(under)) = (over, under) {
                quotes.push(quote(bookmaker, "ou", "over", 2.5, phase, over));
                quotes.push(quote(bookmaker, "ou", "under", 2.5, phase, under));
            }
        }
    }
    quotes
}

fn quote(
    bookmaker: &str,
    market: &str,
    outcome: &str,
    line: f64,
    phase: &str,
    price: f64,
) -> serde_json::Value {
    serde_json::json!({
        "bookmaker": bookmaker,
        "market": market,
        "outcome": outcome,
        "line": line,
        "phase": phase,
        "price": price,
    })
}
