//! Footballdata (football-data.co.uk) CSV-adapter — S3b-2.
//!
//! Az 5 demo liga statikus CSV-je (mmz4281/2627/{liga}.csv). A parse a közös
//! observation-sémát állítja elő (`team_identity`, `match_core`, `match_odds`
//! quotes-szal); a kanonikus út `NormalizedDay::from_observations`-en megy át.
//! A napi kérés a policy `domain`-jéből választ ligát (e0, sp1, i1, d1, f1).

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Europe::London;
use football_domain::error::ParseError;
use football_domain::policy::{FetchJob, Transport};

use crate::adapter::{AdapterMeta, Observation, ParseCtx, SourceAdapter};
use crate::fetch::{FetchRequest, RawPayload};

pub const SOURCE_SLUG: &str = "footballdata";

/// liga-kód → (ország-kód, ország-név, verseny-név, tier, priority)
///
/// S22: a demo kör 5 ligája mellett a football-data.co.uk fő divíziói
/// (másodosztályok és további országok). A csv-adapter ugyanaz marad; a napi
/// job egy kérést indít ligánként. A kiemelt kör a priority ≤10.
const LEAGUES: [(&str, &str, &str, &str, i16, i32); 22] = [
    ("e0", "GB", "England", "Premier League", 1, 6),
    ("sp1", "ES", "Spain", "La Liga", 1, 6),
    ("i1", "IT", "Italy", "Serie A", 1, 6),
    ("d1", "DE", "Germany", "Bundesliga", 1, 6),
    ("f1", "FR", "France", "Ligue 1", 1, 6),
    // Anglia: Championship → National League
    ("e1", "GB", "England", "Championship", 2, 12),
    ("e2", "GB", "England", "League One", 3, 14),
    ("e3", "GB", "England", "League Two", 4, 16),
    ("ec", "GB", "England", "National League", 5, 18),
    // Skócia
    ("sc0", "GB", "Scotland", "Scottish Premiership", 1, 12),
    ("sc1", "GB", "Scotland", "Scottish Championship", 2, 14),
    ("sc2", "GB", "Scotland", "Scottish League One", 3, 16),
    ("sc3", "GB", "Scotland", "Scottish League Two", 4, 18),
    // Másodosztályok
    ("d2", "DE", "Germany", "2. Bundesliga", 2, 12),
    ("i2", "IT", "Italy", "Serie B", 2, 12),
    ("sp2", "ES", "Spain", "LaLiga 2", 2, 12),
    ("f2", "FR", "France", "Ligue 2", 2, 12),
    // További első osztályok
    ("n1", "NL", "Netherlands", "Eredivisie", 1, 12),
    ("b1", "BE", "Belgium", "Belgian Pro League", 1, 12),
    ("p1", "PT", "Portugal", "Primeira Liga", 1, 12),
    ("t1", "TR", "Turkey", "Süper Lig", 1, 12),
    ("g1", "GR", "Greece", "Super League Greece", 1, 12),
];

/// A backoff-ablak: a job dátuma ±7 nap.
const WINDOW_DAYS: i64 = 7;

/// S23: a `new/{LIGA}.csv` fájlok (egy fájl = több szezon) liga-kódjai és
/// helyi időzónái. A verseny-nevet a sor `League` mezője adja.
const NEW_LEAGUES: [(&str, &str); 15] = [
    ("arg", "America/Argentina/Buenos_Aires"),
    ("aut", "Europe/Vienna"),
    ("bra", "America/Sao_Paulo"),
    ("chn", "Asia/Shanghai"),
    ("dnk", "Europe/Copenhagen"),
    ("fin", "Europe/Helsinki"),
    ("irl", "Europe/Dublin"),
    ("jpn", "Asia/Tokyo"),
    ("mex", "America/Mexico_City"),
    ("nor", "Europe/Oslo"),
    ("pol", "Europe/Warsaw"),
    ("rou", "Europe/Bucharest"),
    ("rus", "Europe/Moscow"),
    ("swe", "Europe/Stockholm"),
    ("usa", "America/New_York"),
];

/// A `new/` liga-kódok (backfill-tervezéshez).
pub fn new_league_codes() -> Vec<&'static str> {
    NEW_LEAGUES.iter().map(|(code, _)| *code).collect()
}

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
            domains: vec!["day".into(), "backfill".into()],
            transport: Transport::RustHttp,
        }
    }

    fn plan(&self, job: &FetchJob) -> Vec<FetchRequest> {
        // S23d: "new" ligák — egy fájl = sok szezon (más séma).
        if job.domain == "backfill" && job.entity_key.starts_with("new-") {
            let code = job
                .entity_key
                .trim_start_matches("new-")
                .to_ascii_lowercase();
            return vec![FetchRequest {
                url: format!(
                    "https://www.football-data.co.uk/new/{}.csv",
                    code.to_ascii_uppercase()
                ),
                request_key: format!("{SOURCE_SLUG}:new:{code}:all"),
                endpoint_key: code,
                ready_marker: None,
                bearer_token: None,
            }];
        }
        // S23: backfill job = egy liga-szezon CSV; a liga a job entity_key-ében,
        // a szezonkód (pl. 2526) a time_window-ban utazik.
        if job.domain == "backfill" {
            let league = job.entity_key.to_ascii_uppercase();
            return vec![FetchRequest {
                url: format!(
                    "https://www.football-data.co.uk/mmz4281/{}/{league}.csv",
                    job.time_window
                ),
                request_key: format!(
                    "{SOURCE_SLUG}:backfill:{}:{}",
                    job.entity_key.to_ascii_lowercase(),
                    job.time_window
                ),
                endpoint_key: job.entity_key.to_ascii_lowercase(),
                ready_marker: None,
                bearer_token: None,
            }];
        }
        // S9: egy napi job, ligánként egy CSV-kérés; a liga a kérés
        // request_key-ében utazik (a parse onnan oldja fel).
        LEAGUES
            .iter()
            .map(|(code, ..)| {
                let league = code.to_ascii_uppercase();
                FetchRequest {
                    url: format!("https://www.football-data.co.uk/mmz4281/2627/{league}.csv"),
                    request_key: format!("{SOURCE_SLUG}:{code}:{}", job.time_window),
                    endpoint_key: (*code).to_string(),
                    ready_marker: None,
                    bearer_token: None,
                }
            })
            .collect()
    }

    fn parse(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError> {
        if ctx.request_key.split(':').nth(1) == Some("new") {
            return parse_new_league(raw, ctx);
        }
        let (country_code, country_name, competition_name, tier, priority) =
            Self::league_token(ctx)?;
        // S23: backfill módban a kérés a teljes szezon-CSV-t hozza; a szezon
        // címkéje és dátum-sávja a request_key-ből jön, nincs ±7 napos ablak.
        let season_scope = parse_season_scope(&ctx.request_key)?;
        let season_label = match &season_scope {
            Some(scope) => scope.label.clone(),
            None => season_label_for(ctx.requested_date),
        };
        let text = match std::str::from_utf8(&raw.bytes) {
            Ok(text) => text.to_string(),
            // Régi archívum: Latin-1 kódolású CSV-k (EC/SC 2019 előtt).
            Err(_) => {
                tracing::warn!("nem UTF-8 CSV: Latin-1 dekódolás");
                raw.bytes.iter().map(|b| *b as char).collect::<String>()
            }
        };
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
        // Régi szezonok CSV-jében nincs Time oszlop — ilyenkor date_only.
        let c_time = headers.iter().position(|h| h.trim() == "Time");
        // Régi archívum (pl. G1 2000 előtt): HT/AT oszlopnevek.
        let c_home = headers
            .iter()
            .position(|h| h.trim() == "HomeTeam")
            .or_else(|| headers.iter().position(|h| h.trim() == "HT"))
            .ok_or_else(|| ParseError::SchemaDrift("hiányzó CSV-oszlop: HomeTeam".into()))?;
        let c_away = headers
            .iter()
            .position(|h| h.trim() == "AwayTeam")
            .or_else(|| headers.iter().position(|h| h.trim() == "AT"))
            .ok_or_else(|| ParseError::SchemaDrift("hiányzó CSV-oszlop: AwayTeam".into()))?;
        let c_fthg = column("FTHG")?;
        let c_ftag = column("FTAG")?;
        // Régi archívum: félidő-oszlopok hiányozhatnak.
        let c_hthg = headers.iter().position(|h| h.trim() == "HTHG");
        let c_htag = headers.iter().position(|h| h.trim() == "HTAG");

        let captured_at = Utc::now();
        let mut teams: std::collections::BTreeMap<String, String> = Default::default();
        let mut observations = Vec::new();
        let mut competition = None;
        let mut skipped_outside = 0usize;
        let mut seen_rows = 0usize;
        let mut skipped_malformed = 0usize;

        for record in reader.records() {
            let record = record.map_err(|e| ParseError::Parse(format!("CSV sor: {e}")))?;
            seen_rows += 1;
            let field = |i: usize| record.get(i).unwrap_or("").trim().to_string();
            // Hibás/hiányos sor (üres dátum vagy csapat): kihagyjuk, nem
            // buktatjuk el az egész szezont (a teljesen üres fájlt a
            // decoy-őr fogja).
            let date_text = field(c_date);
            if date_text.is_empty() || field(c_home).is_empty() || field(c_away).is_empty() {
                skipped_malformed += 1;
                continue;
            }
            let mut date = NaiveDate::parse_from_str(&date_text, "%d/%m/%Y")
                .map_err(|e| ParseError::Parse(format!("dátum ({date_text}): {e}")))?;
            if date.year() < 1900 {
                date = NaiveDate::parse_from_str(&date_text, "%d/%m/%y")
                    .map_err(|e| ParseError::Parse(format!("dátum ({date_text}): {e}")))?;
            }
            match &season_scope {
                Some(scope) => {
                    if date < scope.from || date > scope.to {
                        skipped_outside += 1;
                        continue;
                    }
                }
                None => {
                    if (date - ctx.requested_date).num_days().abs() > WINDOW_DAYS {
                        continue;
                    }
                }
            }
            let time_field = c_time.map(field).unwrap_or_default();
            let kickoff_at = match NaiveTime::parse_from_str(&time_field, "%H:%M") {
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
                    "home_ht": c_hthg
                        .and_then(|i| parse_number(Some(&field(i))))
                        .map(|v| v as i16),
                    "away_ht": c_htag
                        .and_then(|i| parse_number(Some(&field(i))))
                        .map(|v| v as i16),
                    "home_et": null, "away_et": null,
                    "home_pens": null, "away_pens": null,
                })
            });
            // Az observation-kulcs a meccs természetes azonosítója, NEM az
            // ablaké: ugyanaz a forduló több napi körben is megfigyelhető,
            // és az ablakfüggő kulcs ablakonként új observation-t gyártana.
            let source_match_id = format!("footballdata-{}-{}-{}", date, home_slug, away_slug);
            let season = serde_json::json!({ "id": null, "label": &season_label });
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

            // Stats: opcionális oszlopok; ha bármelyik érték jelen van, emittálunk.
            let stats = |name: &str| -> Option<i32> {
                headers
                    .iter()
                    .position(|h| h.trim() == name)
                    .and_then(|i| record.get(i))
                    .and_then(|v| v.trim().parse::<i32>().ok())
            };
            let stats_payload = serde_json::json!({
                "home_shots": stats("HS"),
                "away_shots": stats("AS"),
                "home_shots_on_target": stats("HST"),
                "away_shots_on_target": stats("AST"),
                "home_corners": stats("HC"),
                "away_corners": stats("AC"),
                "home_yellow": stats("HY"),
                "away_yellow": stats("AY"),
                "home_red": stats("HR"),
                "away_red": stats("AR"),
                "home_fouls": stats("HF"),
                "away_fouls": stats("AF"),
                "home_xg": null,
                "away_xg": null,
            });
            if stats_payload
                .as_object()
                .is_some_and(|o| o.values().any(|v| !v.is_null()))
            {
                observations.push(Observation {
                    entity_kind: "match".into(),
                    aspect: "match_stats".into(),
                    natural_key: source_match_id.clone(),
                    source_record_key: source_match_id.clone(),
                    observed_at: captured_at,
                    payload: stats_payload,
                });
            }
            let referee = headers
                .iter()
                .position(|h| h.trim() == "Referee")
                .and_then(|i| record.get(i))
                .map(|v| v.trim().to_string())
                .unwrap_or_default();
            if !referee.is_empty() {
                observations.push(Observation {
                    entity_kind: "match".into(),
                    aspect: "match_referee".into(),
                    natural_key: source_match_id.clone(),
                    source_record_key: source_match_id.clone(),
                    observed_at: captured_at,
                    payload: serde_json::json!({ "name": referee }),
                });
            }

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

        if season_scope.is_some() && seen_rows > 0 && observations.is_empty() {
            return Err(ParseError::SchemaDrift(format!(
                "egyetlen sor sem esik a kért szezonba (olvasott sorok: {seen_rows}, sávon kívül: {skipped_outside}, hibás sor: {skipped_malformed})"
            )));
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
        // A request_key formátuma: footballdata:{liga}:{ablak} vagy
        // footballdata:backfill:{liga}:{szezonkód}.
        let parts: Vec<&str> = ctx.request_key.split(':').collect();
        let domain = match parts.as_slice() {
            [_, "backfill", league, _] => *league,
            [_, league, _] => *league,
            _ => {
                return Err(ParseError::Parse(
                    "a request_key-ből nem derül ki a liga".into(),
                ));
            }
        };
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
    // A régi archívum tartalmaz 0/érvénytelen odds-cellákat: csak >1.0 megy.
    let get = |name: &str| -> Option<f64> {
        headers
            .iter()
            .position(|h| h.trim() == name)
            .and_then(|i| record.get(i))
            .and_then(|v| parse_number(Some(v)))
            .filter(|price| *price > 1.0)
    };
    for bookmaker in ["B365", "Avg", "Max", "GB", "IW"] {
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

#[derive(Debug, Clone)]
struct SeasonScope {
    label: String,
    from: NaiveDate,
    to: NaiveDate,
}

/// Szezonkód (`YYZZ`, pl. `2526` = 2025/26) → címke.
pub fn season_label_from_code(code: &str) -> Option<String> {
    if code.len() != 4 || !code.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let yy: u32 = code[..2].parse().ok()?;
    let start = if yy >= 90 { 1900 + yy } else { 2000 + yy };
    Some(format!("{start}/{:02}", (start + 1) % 100))
}

/// A megfigyelt közvetlen dátum alapján a szezon-címke (júliustól új szezon).
fn season_label_for(date: NaiveDate) -> String {
    use chrono::Datelike;
    let (start, end) = if date.month() >= 7 {
        (date.year(), date.year() + 1)
    } else {
        (date.year() - 1, date.year())
    };
    format!("{start}/{:02}", end % 100)
}

fn parse_season_scope(request_key: &str) -> Result<Option<SeasonScope>, ParseError> {
    let parts: Vec<&str> = request_key.split(':').collect();
    let code = match parts.as_slice() {
        [_, "backfill", _, season] => *season,
        _ => return Ok(None),
    };
    let label = season_label_from_code(code)
        .ok_or_else(|| ParseError::SchemaDrift(format!("érvénytelen szezonkód: {code}")))?;
    let start_year: i32 = code[..2]
        .parse::<i32>()
        .map(|yy| if yy >= 90 { 1900 + yy } else { 2000 + yy })
        .map_err(|_| ParseError::SchemaDrift(format!("érvénytelen szezonkód: {code}")))?;
    // Szezon-sáv: július 1. – következő június 30., ±2 hónap tűrés.
    let from = NaiveDate::from_ymd_opt(start_year, 5, 1)
        .ok_or_else(|| ParseError::SchemaDrift(format!("érvénytelen szezonkezdet: {code}")))?;
    let to = NaiveDate::from_ymd_opt(start_year + 1, 8, 31)
        .ok_or_else(|| ParseError::SchemaDrift(format!("érvénytelen szezonvég: {code}")))?;
    Ok(Some(SeasonScope { label, from, to }))
}

/// Az adapter által kezelt liga-kódok (backfill-tervezéshez).
pub fn league_codes() -> Vec<&'static str> {
    LEAGUES.iter().map(|(code, ..)| *code).collect()
}

/// Liga-kód → első elérhető szezon kezdőéve (footballdata archívum).
pub fn first_season(code: &str) -> Option<u16> {
    const FIRST: [(&str, u16); 22] = [
        ("e0", 1993),
        ("sp1", 1993),
        ("i1", 1993),
        ("d1", 1993),
        ("f1", 1993),
        ("e1", 1993),
        ("e2", 1993),
        ("e3", 1993),
        ("ec", 2005),
        ("sc0", 1994),
        ("sc1", 1994),
        ("sc2", 1997),
        ("sc3", 1997),
        ("d2", 1993),
        ("i2", 1997),
        ("sp2", 1996),
        ("f2", 1996),
        ("n1", 1993),
        ("b1", 1995),
        ("p1", 1994),
        ("t1", 1994),
        ("g1", 1994),
    ];
    FIRST
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, year)| *year)
}

// ---------------------------------------------------------------------------
// S23d: footballdata "new" ligák (más séma, egy fájl = több szezon).
// ---------------------------------------------------------------------------

fn is_women_league(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "women",
        "femenina",
        "női",
        "feminine",
        "frauen",
        "femminile",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_youth_league(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["u17", "u18", "u19", "u20", "u21", "u23", "youth", "junior"]
        .iter()
        .any(|needle| lower.contains(needle))
}

/// "2012/2013" → "2012/13"; "2014" → "2014".
pub fn normalize_season_label(raw: &str) -> String {
    match raw.split_once('/') {
        Some((start, end)) => {
            let tail = &end[end.len().saturating_sub(2)..];
            format!("{start}/{tail}")
        }
        None => raw.to_string(),
    }
}

fn parse_new_league(raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError> {
    let code = ctx
        .request_key
        .split(':')
        .nth(2)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let zone_name = NEW_LEAGUES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, zone)| *zone)
        .ok_or_else(|| ParseError::SchemaDrift(format!("ismeretlen new-liga: {code}")))?;
    let tz: chrono_tz::Tz = zone_name
        .parse()
        .map_err(|e| ParseError::Parse(format!("időzóna ({zone_name}): {e}")))?;

    let text = match std::str::from_utf8(&raw.bytes) {
        Ok(text) => text.to_string(),
        Err(_) => {
            tracing::warn!("nem UTF-8 new-liga CSV: Latin-1 dekódolás");
            raw.bytes.iter().map(|b| *b as char).collect::<String>()
        }
    };
    let text = text.trim_start_matches('\u{feff}');
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|e| ParseError::Parse(format!("CSV fejléc: {e}")))?
        .clone();
    let column = |name: &str| {
        headers
            .iter()
            .position(|h| h.trim() == name)
            .ok_or_else(|| ParseError::SchemaDrift(format!("hiányzó CSV-oszlop: {name}")))
    };
    let c_country = column("Country")?;
    let c_league = column("League")?;
    let c_season = column("Season")?;
    let c_date = column("Date")?;
    let c_time = headers.iter().position(|h| h.trim() == "Time");
    let c_home = column("Home")?;
    let c_away = column("Away")?;
    let c_hg = column("HG")?;
    let c_ag = column("AG")?;

    let captured_at = Utc::now();
    let mut observations: Vec<Observation> = Vec::new();
    let mut teams: std::collections::BTreeMap<String, serde_json::Value> = Default::default();
    let mut seen_rows = 0usize;
    let mut skipped = 0usize;

    for record in reader.records() {
        let record = record.map_err(|e| ParseError::Parse(format!("CSV sor: {e}")))?;
        let field = |i: usize| record.get(i).unwrap_or("").trim().to_string();
        seen_rows += 1;

        let home = field(c_home);
        let away = field(c_away);
        let date_text = field(c_date);
        let league = field(c_league).trim().to_string();
        if home.is_empty() || away.is_empty() || date_text.is_empty() || league.is_empty() {
            skipped += 1;
            continue;
        }
        let mut date = NaiveDate::parse_from_str(&date_text, "%d/%m/%Y")
            .map_err(|e| ParseError::Parse(format!("dátum ({date_text}): {e}")))?;
        if date.year() < 1900 {
            date = NaiveDate::parse_from_str(&date_text, "%d/%m/%y")
                .map_err(|e| ParseError::Parse(format!("dátum ({date_text}): {e}")))?;
        }
        if !(1990..=2035).contains(&date.year()) {
            return Err(ParseError::SchemaDrift(format!(
                "hihetetlen dátum a new-liga CSV-ben: {date_text}"
            )));
        }

        let time_text = c_time.map(field).unwrap_or_default();
        let (kickoff_at, kickoff_precision) = match NaiveTime::parse_from_str(&time_text, "%H:%M") {
            Ok(time) => {
                let local = NaiveDateTime::new(date, time);
                match tz.from_local_datetime(&local).earliest() {
                    Some(dt) => (dt.with_timezone(&Utc), "exact"),
                    // DST-rés (nem létező helyi idő): date_only fallback.
                    None => (
                        Utc.from_utc_datetime(&NaiveDateTime::new(
                            date,
                            NaiveTime::from_hms_opt(12, 0, 0).expect("12:00 érvényes"),
                        )),
                        "date_only",
                    ),
                }
            }
            Err(_) => (
                Utc.from_utc_datetime(&NaiveDateTime::new(
                    date,
                    NaiveTime::from_hms_opt(12, 0, 0).expect("12:00 érvényes"),
                )),
                "date_only",
            ),
        };

        let season_raw = field(c_season);
        let season_label = normalize_season_label(&season_raw);
        let country_name = field(c_country);
        let (country_code, country_name) = match crate::countries::by_english_name(&country_name) {
            Some((cc, cn)) => (cc.to_string(), cn.to_string()),
            None => (String::new(), country_name.clone()),
        };
        let (gender, branch) = if is_women_league(&league) {
            ("female", "women")
        } else if is_youth_league(&league) {
            ("male", "youth")
        } else {
            ("male", "senior")
        };

        let home_ft = field(c_hg).parse::<i16>().ok();
        let away_ft = field(c_ag).parse::<i16>().ok();
        let (status, result) = match (home_ft, away_ft) {
            (Some(h), Some(a)) => (
                "finished",
                Some(serde_json::json!({
                    "home_ft": h, "away_ft": a,
                    "home_ht": null, "away_ht": null,
                    "home_et": null, "away_et": null,
                    "home_pens": null, "away_pens": null,
                    "decided_by": "ft",
                })),
            ),
            _ => ("scheduled", None),
        };

        let home_slug = slug(&home);
        let away_slug = slug(&away);
        for (team_slug, team_name) in [(&home_slug, &home), (&away_slug, &away)] {
            teams.entry(team_slug.clone()).or_insert_with(|| {
                serde_json::json!({
                    "id": null,
                    "source_entity_id": team_slug,
                    "name": team_name,
                    "country_code": country_code,
                    "gender": gender,
                    "age_group": if branch == "youth" { "youth" } else { "senior" },
                    "branch": branch,
                })
            });
        }

        let source_match_id = format!("footballdata-new-{code}-{date}-{home_slug}-{away_slug}");
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
                "kickoff_precision": kickoff_precision,
                "status": status,
                "result": result,
                "round_label": null,
                "competition_timezone": zone_name,
                "competition": {
                    "id": null,
                    "name": league,
                    "country_code": country_code,
                    "country_name": country_name,
                    "tier": 1,
                    "priority": 60,
                },
                "season": { "id": null, "label": season_label },
                "source_code": SOURCE_SLUG,
                "captured_at": captured_at,
            }),
        });

        let price = |name: &str| -> Option<f64> {
            headers
                .iter()
                .position(|h| h.trim() == name)
                .and_then(|i| record.get(i))
                .and_then(|v| parse_number(Some(v)))
                .filter(|p| *p > 1.0)
        };
        let mut quotes = Vec::new();
        for (bookmaker, prefix) in [
            ("B365", "B365C"),
            ("Max", "MaxC"),
            ("Avg", "AvgC"),
            ("BFE", "BFEC"),
            ("PSC", "PSC"),
        ] {
            let outcome = |letter: &str| price(&format!("{prefix}{letter}"));
            if let (Some(h), Some(d), Some(a)) = (outcome("H"), outcome("D"), outcome("A")) {
                for (name, value) in [("home", h), ("draw", d), ("away", a)] {
                    quotes.push(serde_json::json!({
                        "bookmaker": bookmaker,
                        "market": "1x2",
                        "outcome": name,
                        "line": 0.0,
                        "phase": "closing",
                        "price": value,
                    }));
                }
            }
        }
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
                    "market_count": 1,
                }),
            });
        }
    }

    if seen_rows == 0 || observations.is_empty() {
        return Err(ParseError::SchemaDrift(format!(
            "üres new-liga CSV (sorok: {seen_rows}, kihagyva: {skipped})"
        )));
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
