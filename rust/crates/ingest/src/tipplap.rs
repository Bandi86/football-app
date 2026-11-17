//! Tipplap napi kalkulátor-oldal adapter — S18 (rust-http).
//!
//! Egy kérés/nap: `/kalkulator/labdarugas/{YYYY-MM-DD}`. A lap
//! szerver-renderelt desktop táblája (`#calcTableDesktop`) soronként
//! tartalmazza a meccset, a HU helyi kickoffot és a Tippmix 1X2 szorzókat.
//!
//! Őrök: dátum-guard (`#dates` selected option == kért nap, fail-closed),
//! üres lista → schema-drift, szimulált/speciális tippek (kérdőjel,
//! zárójel, hiányzó „ - ") kihagyása.
//!
//! Verseny-megfeleltetés **fix szótárral** (naming-stability szabály):
//! ismeretlen ligacím esetén a sor kimarad — nem hozunk létre új
//! verseny-entitást. A szótár kulcsa a Tipplap magyar címe, az érték a v2
//! kanonikus (Soccerway/footballdata) név, hogy a meccsek összeérjenek.

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use chrono_tz::Europe::Budapest;
use football_domain::error::ParseError;
use football_domain::policy::{FetchJob, Transport};
use scraper::{Html, Selector};

use crate::adapter::{AdapterMeta, Observation, ParseCtx, SourceAdapter};
use crate::countries;
use crate::fetch::{FetchRequest, RawPayload};

pub const SOURCE_SLUG: &str = "tipplap";
const BASE: &str = "https://www.tipplap.hu";
const TITLE_PREFIX: &str = "Labdarúgás – ";

struct League {
    /// A Tipplap ligacím előtagja (a csoport/stádium utótag levágása után).
    key: &'static str,
    /// v2 kanonikus verseny-név (egyeznie kell a Soccerway szótárral).
    name: &'static str,
    country: Option<(&'static str, &'static str)>,
    tier: i16,
    priority: i32,
    international: bool,
}

const LEAGUES: [League; 16] = [
    League {
        key: "Nemzetek Ligája, A div",
        name: "UEFA Nations League - League A",
        country: None,
        tier: 1,
        priority: 50,
        international: true,
    },
    League {
        key: "Nemzetek Ligája, B div",
        name: "UEFA Nations League - League B",
        country: None,
        tier: 1,
        priority: 50,
        international: true,
    },
    League {
        key: "Nemzetek Ligája, C div",
        name: "UEFA Nations League - League C",
        country: None,
        tier: 1,
        priority: 50,
        international: true,
    },
    League {
        key: "Nemzetek Ligája, D div",
        name: "UEFA Nations League - League D",
        country: None,
        tier: 1,
        priority: 50,
        international: true,
    },
    League {
        key: "Afrikai Nemzetek Kupája-sel.",
        name: "Africa Cup of Nations - Qualification",
        country: None,
        tier: 0,
        priority: 40,
        international: true,
    },
    League {
        key: "CONCACAF Nemz.Ligája A div",
        name: "CONCACAF Nations League - League A",
        country: None,
        tier: 1,
        priority: 50,
        international: true,
    },
    League {
        key: "CONCACAF Nemz.Ligája B div",
        name: "CONCACAF Nations League - League B",
        country: None,
        tier: 1,
        priority: 50,
        international: true,
    },
    League {
        key: "CONCACAF Nemz.Ligája C div",
        name: "CONCACAF Nations League - League C",
        country: None,
        tier: 1,
        priority: 50,
        international: true,
    },
    League {
        key: "Válogatott mérkőzés",
        name: "Friendly International",
        country: None,
        tier: 1,
        priority: 50,
        international: true,
    },
    League {
        key: "Premier Liga",
        name: "Premier League",
        country: Some(("GB", "England")),
        tier: 1,
        priority: 6,
        international: false,
    },
    League {
        key: "Spanyol bajnokság",
        name: "La Liga",
        country: Some(("ES", "Spain")),
        tier: 1,
        priority: 6,
        international: false,
    },
    League {
        key: "Olasz A",
        name: "Serie A",
        country: Some(("IT", "Italy")),
        tier: 1,
        priority: 6,
        international: false,
    },
    League {
        key: "Bundesliga 1.",
        name: "Bundesliga",
        country: Some(("DE", "Germany")),
        tier: 1,
        priority: 6,
        international: false,
    },
    League {
        key: "Francia bajnokság",
        name: "Ligue 1",
        country: Some(("FR", "France")),
        tier: 1,
        priority: 6,
        international: false,
    },
    League {
        key: "NB I",
        name: "NB I",
        country: Some(("HU", "Hungary")),
        tier: 1,
        priority: 8,
        international: false,
    },
    League {
        key: "NB II",
        name: "NB II",
        country: Some(("HU", "Hungary")),
        tier: 2,
        priority: 10,
        international: false,
    },
];

fn league_for(title: &str) -> Option<(&'static League, Option<String>)> {
    let trimmed = title
        .trim()
        .strip_prefix(TITLE_PREFIX)
        .unwrap_or(title.trim());
    // Leghosszabb egyező kulcs (pl. „Nemzetek Ligája, B div" előbb, mint „…").
    let mut best: Option<(&'static League, &str)> = None;
    for league in LEAGUES.iter() {
        if trimmed.starts_with(league.key)
            && best.is_none_or(|(_, key)| league.key.len() > key.len())
        {
            best = Some((league, league.key));
        }
    }
    let (league, key) = best?;
    let rest = trimmed[key.len()..]
        .trim_start_matches([',', '.', ' '])
        .trim()
        .to_string();
    let round = (!rest.is_empty()).then_some(rest);
    Some((league, round))
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

/// Szezon-címke a meccs dátuma alapján (júliustól új szezon).
fn season_label(date: NaiveDate) -> String {
    let (start, end) = if date.month() >= 7 {
        (date.year(), date.year() + 1)
    } else {
        (date.year() - 1, date.year())
    };
    format!("{start}/{:02}", end % 100)
}

/// MM.DD. + HH:MM (HU helyi idő) → UTC; a kért naphoz legközelebbi év.
fn kickoff_utc(
    requested: NaiveDate,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
) -> Option<DateTime<Utc>> {
    let year = requested.year();
    let mut date = NaiveDate::from_ymd_opt(year, month, day)?;
    let delta = (date - requested).num_days();
    if delta > 183 {
        date = NaiveDate::from_ymd_opt(year - 1, month, day)?;
    } else if delta < -183 {
        date = NaiveDate::from_ymd_opt(year + 1, month, day)?;
    }
    let local = date.and_hms_opt(hour, minute, 0)?;
    Budapest
        .from_local_datetime(&local)
        .earliest()
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_price(text: &str) -> Option<f64> {
    text.trim()
        .replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| *v > 1.0)
}

/// Nemzetközi versenyben a csapatnév ország: kanonikus EN név + ISO.
fn resolve_national(name: &str) -> Option<(&'static str, &'static str)> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.contains('?') || trimmed.contains('(') {
        return None;
    }
    countries::by_localized_title(trimmed).or_else(|| countries::by_english_name(trimmed))
}

/// Speciális/szimulált tipp-sor (nem valódi meccs).
fn is_special_bet(home: &str, away: &str) -> bool {
    [home, away].iter().any(|name| {
        let n = name.trim();
        n.is_empty() || n.contains('?') || n.contains('(') || n.contains("NL ")
    })
}

pub struct TipplapAdapter;

impl SourceAdapter for TipplapAdapter {
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
        vec![FetchRequest {
            url: format!("{BASE}/kalkulator/labdarugas/{}", job.time_window),
            request_key: format!("{SOURCE_SLUG}:day:{}", job.time_window),
            endpoint_key: "day".into(),
            ready_marker: None,
            bearer_token: None,
        }]
    }

    fn parse(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError> {
        let html = std::str::from_utf8(&raw.bytes)
            .map_err(|e| ParseError::Parse(format!("nem UTF-8 HTML: {e}")))?;
        let doc = Html::parse_document(html);

        let sel = |css: &str| {
            Selector::parse(css).map_err(|e| ParseError::Parse(format!("szelektor ({css}): {e}")))
        };

        // Dátum-guard: a lap a választóban jelöli a kiszolgált napot.
        let selected = sel("select#dates option[selected]")?;
        let served = doc
            .select(&selected)
            .next()
            .and_then(|opt| opt.value().attr("value").map(str::to_string))
            .ok_or_else(|| {
                ParseError::SchemaDrift("hiányzó #dates selected option (dátum-guard)".into())
            })?;
        if served != ctx.requested_date.to_string() {
            return Err(ParseError::DateGuardMismatch {
                requested: ctx.requested_date.to_string(),
                got: served,
            });
        }

        let table_sel = sel("table#calcTableDesktop")?;
        let table = doc
            .select(&table_sel)
            .next()
            .ok_or_else(|| ParseError::SchemaDrift("hiányzó desktop kalkulátor tábla".into()))?;

        let row_sel = sel("tr")?;
        let th_sel = sel("th.tleft")?;
        let name_sel = sel("a[data-event-name]")?;
        let odds_sel = |side: &str| sel(&format!("td[data-name=\"{side}\"] strong[data-odds]"));
        let odds_h = odds_sel("H")?;
        let odds_d = odds_sel("D")?;
        let odds_v = odds_sel("V")?;
        let time_sel = sel("td[width=\"72\"]")?;

        let captured_at = Utc::now();
        let mut observations: Vec<Observation> = Vec::new();
        let mut teams: std::collections::BTreeMap<String, serde_json::Value> = Default::default();
        let mut league: Option<(&'static League, Option<String>)> = None;
        let mut rows = 0usize;
        let mut skipped = 0usize;

        for row in table.select(&row_sel) {
            if let Some(th) = row.select(&th_sel).next() {
                league = league_for(&th.text().collect::<String>());
                continue;
            }
            let Some(event_id) = row.value().attr("data-event") else {
                continue;
            };
            rows += 1;
            let Some((league_meta, round_label)) = league.as_ref() else {
                skipped += 1;
                continue;
            };

            let Some(name_text) = row
                .select(&name_sel)
                .next()
                .map(|a| a.text().collect::<String>().trim().to_string())
            else {
                skipped += 1;
                continue;
            };
            let Some((home_raw, away_raw)) = name_text.split_once(" - ") else {
                skipped += 1;
                continue;
            };
            let (home_raw, away_raw) = (home_raw.trim(), away_raw.trim());
            if is_special_bet(home_raw, away_raw) {
                skipped += 1;
                continue;
            }

            // Idő: a két 72 px-es cella „MM.DD." és „HH:MM".
            let cells: Vec<String> = row
                .select(&time_sel)
                .map(|td| td.text().collect::<String>().trim().to_string())
                .collect();
            let (Some(date_cell), Some(time_cell)) = (cells.first(), cells.get(1)) else {
                skipped += 1;
                continue;
            };
            let date_parts: Vec<&str> = date_cell.trim_end_matches('.').split('.').collect();
            let time_parts: Vec<&str> = time_cell.split(':').collect();
            let kickoff_at = (|| {
                let month: u32 = date_parts.first()?.parse().ok()?;
                let day: u32 = date_parts.get(1)?.parse().ok()?;
                let hour: u32 = time_parts.first()?.parse().ok()?;
                let minute: u32 = time_parts.get(1)?.parse().ok()?;
                kickoff_utc(ctx.requested_date, month, day, hour, minute)
            })();
            let Some(kickoff_at) = kickoff_at else {
                skipped += 1;
                continue;
            };

            let price = |selector: &Selector| {
                row.select(selector)
                    .next()
                    .map(|e| e.text().collect::<String>())
                    .and_then(|t| parse_price(&t))
            };
            let (Some(home_odds), Some(draw_odds), Some(away_odds)) =
                (price(&odds_h), price(&odds_d), price(&odds_v))
            else {
                skipped += 1;
                continue;
            };

            // Csapatok: nemzetközi versenyben ország (HU→EN+ISO), hazai
            // ligában a megfigyelt név (a linker aliasai oldják fel).
            let team_pair: Option<((String, String), (String, String))> =
                if league_meta.international {
                    let home = resolve_national(home_raw);
                    let away = resolve_national(away_raw);
                    match (home, away) {
                        (Some((h_iso, h_name)), Some((a_iso, a_name))) => Some((
                            (h_name.to_string(), h_iso.to_string()),
                            (a_name.to_string(), a_iso.to_string()),
                        )),
                        _ => None,
                    }
                } else {
                    let country = league_meta
                        .country
                        .map(|(cc, _)| cc.to_string())
                        .unwrap_or_default();
                    Some((
                        (home_raw.to_string(), country.clone()),
                        (away_raw.to_string(), country),
                    ))
                };
            let Some(((home_name, home_country), (away_name, away_country))) = team_pair else {
                skipped += 1;
                continue;
            };

            let home_id = slug(&home_name);
            let away_id = slug(&away_name);
            let (country_code, country_name) = match league_meta.country {
                Some((cc, cn)) => (cc.to_string(), cn.to_string()),
                None => (String::new(), String::new()),
            };
            for (team_id, team_name, team_country) in [
                (&home_id, &home_name, &home_country),
                (&away_id, &away_name, &away_country),
            ] {
                teams.entry(team_id.clone()).or_insert_with(|| {
                    serde_json::json!({
                        "id": null,
                        "source_entity_id": team_id,
                        "name": team_name,
                        "country_code": team_country,
                        "gender": "male",
                        "age_group": "senior",
                        "branch": "senior",
                    })
                });
            }

            let source_match_id = format!("tipplap-{event_id}");
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
                    "kickoff_precision": "exact",
                    "status": "scheduled",
                    "result": null,
                    "round_label": round_label,
                    "competition_timezone": "Europe/Budapest",
                    "competition": {
                        "id": null,
                        "name": league_meta.name,
                        "country_code": country_code,
                        "country_name": country_name,
                        "tier": league_meta.tier,
                        "priority": league_meta.priority,
                    },
                    "season": { "id": null, "label": season_label(kickoff_at.date_naive()) },
                    "source_code": SOURCE_SLUG,
                    "captured_at": captured_at,
                }),
            });
            let quote = |outcome: &str, price: f64| {
                serde_json::json!({
                    "bookmaker": "Tippmix",
                    "market": "1x2",
                    "outcome": outcome,
                    "line": 0.0,
                    "phase": "opening",
                    "price": price,
                })
            };
            observations.push(Observation {
                entity_kind: "match".into(),
                aspect: "match_odds".into(),
                natural_key: source_match_id.clone(),
                source_record_key: source_match_id,
                observed_at: captured_at,
                payload: serde_json::json!({
                    "canonical_match_id": null,
                    "quotes": [
                        quote("home", home_odds),
                        quote("draw", draw_odds),
                        quote("away", away_odds),
                    ],
                    "market_count": 1,
                }),
            });
        }

        if rows == 0 {
            return Err(ParseError::SchemaDrift(
                "üres kalkulátor lista (decoy-gyanú)".into(),
            ));
        }
        if observations.is_empty() {
            tracing::warn!(
                source = SOURCE_SLUG,
                rows,
                skipped,
                "minden sor kimaradt (szótár/név/idő/odds)"
            );
        }
        tracing::info!(source = SOURCE_SLUG, rows, skipped, "tipplap nap parse");

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

#[cfg(test)]
mod league_map_tests {
    use super::league_for;

    #[test]
    fn maps_nb_two_and_concacaf_leagues() {
        let (league, round) = league_for("Labdarúgás – NB II").unwrap();
        assert_eq!(league.name, "NB II");
        assert_eq!(round, None);

        let (league, round) = league_for("Labdarúgás – CONCACAF Nemz.Ligája A div.Acs").unwrap();
        assert_eq!(league.name, "CONCACAF Nations League - League A");
        assert_eq!(round.as_deref(), Some("Acs"));

        let (league, _) = league_for("Labdarúgás – CONCACAF Nemz.Ligája C div.Bcs").unwrap();
        assert_eq!(league.name, "CONCACAF Nations League - League C");
    }
}
