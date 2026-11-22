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
//! A nemzetközi/top-ligás `LEAGUES` mellett a `GENERAL_LEAGUES` tábla
//! fedi a v1-ben már bejáratott többi ligát (S36a, adatvezérelt bővítés).
//! A női szekciók külön `WOMEN_LEAGUES` táblát kapnak (S94): a Tipplap női
//! címei országonként más szöveget használnak (pl. „Német bajnokság, női" a
//! férfi „Bundesliga 1." helyett), ezért a férfi kulcs önmagában nem elég.

use std::borrow::Cow;

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use chrono_tz::Europe::Budapest;
use football_domain::error::ParseError;
use football_domain::identity::fold_name;
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

/// Tipplap ligacím → v2 kanonikus verseny — bővített tábla (S36a).
///
/// A v1 `tipplap_league_alias` (434 reviewed sor) + a v2-ben már meccset
/// tartó versenyek alapján, adatvezérelt ellenőrzéssel: csak olyan cím
/// került be, amelyhez a v1 Tipplap-események legalább két egzakt
/// csapatpár-egyezést adnak egy v2 versenyre (lásd docs/decisions).
/// A v2 ország-nevek a meglévő `core.country` sorokból jönnek (GB esetén
/// a történelmi "Scotland" név — ismert adathiba, külön döntés).
const GENERAL_LEAGUES: [League; 26] = [
    League {
        key: "Angol Liga 1.",
        name: "League One",
        country: Some(("GB", "Scotland")),
        tier: 3,
        priority: 14,
        international: false,
    },
    League {
        key: "Angol Liga 2.",
        name: "League Two",
        country: Some(("GB", "Scotland")),
        tier: 4,
        priority: 16,
        international: false,
    },
    League {
        key: "Angol Liga bajnokság",
        name: "Championship",
        country: Some(("GB", "Scotland")),
        tier: 1,
        priority: 50,
        international: false,
    },
    League {
        key: "Angol Nemzeti Liga",
        name: "National League",
        country: Some(("GB", "Scotland")),
        tier: 5,
        priority: 18,
        international: false,
    },
    League {
        key: "Ausztrál Championship, D csop.",
        name: "Australian Championship",
        country: Some(("AU", "Australia")),
        tier: 1,
        priority: 50,
        international: false,
    },
    League {
        key: "Bundesliga 2.",
        name: "2. Bundesliga",
        country: Some(("DE", "Germany")),
        tier: 1,
        priority: 60,
        international: false,
    },
    League {
        key: "Egyiptomi Kupa",
        name: "Egyiptomi Kupa",
        country: Some(("EG", "Egypt")),
        tier: 0,
        priority: 40,
        international: false,
    },
    League {
        key: "Francia 2.",
        name: "Ligue 2",
        country: Some(("FR", "France")),
        tier: 1,
        priority: 50,
        international: false,
    },
    League {
        key: "Georgiai bajnokság",
        name: "Erovnuli Liga",
        country: Some(("GE", "Georgia")),
        tier: 1,
        priority: 50,
        international: false,
    },
    League {
        key: "Holland bajnokság",
        name: "Eredivisie",
        country: Some(("NL", "Netherlands")),
        tier: 1,
        priority: 60,
        international: false,
    },
    League {
        key: "Indiai Szuperliga",
        name: "Indian Super League",
        country: Some(("IN", "India")),
        tier: 1,
        priority: 50,
        international: false,
    },
    League {
        key: "Iraki bajnokság",
        name: "Stars League",
        country: Some(("IQ", "Iraq")),
        tier: 1,
        priority: 50,
        international: false,
    },
    League {
        key: "Iráni bajnokság",
        name: "Persian Gulf Pro League",
        country: None,
        tier: 1,
        priority: 50,
        international: false,
    },
    League {
        key: "Katari bajnokság",
        name: "Qatar Stars League",
        country: Some(("QA", "Qatar")),
        tier: 1,
        priority: 50,
        international: false,
    },
    League {
        key: "Lengyel bajnokság",
        name: "Ekstraklasa",
        country: Some(("PL", "Poland")),
        tier: 1,
        priority: 60,
        international: false,
    },
    League {
        key: "Norvég bajnokság",
        name: "Eliteserien",
        country: Some(("NO", "Norway")),
        tier: 1,
        priority: 60,
        international: false,
    },
    League {
        key: "Olasz B",
        name: "Serie B",
        country: Some(("IT", "Italy")),
        tier: 1,
        priority: 60,
        international: false,
    },
    League {
        key: "Osztrák 2.",
        name: "2. Liga",
        country: Some(("AT", "Austria")),
        tier: 1,
        priority: 60,
        international: false,
    },
    League {
        key: "Portugál bajnokság",
        name: "Primeira Liga",
        country: Some(("PT", "Portugal")),
        tier: 1,
        priority: 60,
        international: false,
    },
    League {
        key: "Skót Liga 1.",
        name: "Scottish League One",
        country: Some(("GB", "Scotland")),
        tier: 3,
        priority: 16,
        international: false,
    },
    League {
        key: "Skót Liga 2.",
        name: "Scottish League Two",
        country: Some(("GB", "Scotland")),
        tier: 4,
        priority: 18,
        international: false,
    },
    League {
        key: "Skót bajnokság",
        name: "Scottish Premiership",
        country: Some(("GB", "Scotland")),
        tier: 1,
        priority: 60,
        international: false,
    },
    League {
        key: "Spanyol 2.",
        name: "LaLiga 2",
        country: Some(("ES", "Spain")),
        tier: 2,
        priority: 12,
        international: false,
    },
    League {
        key: "Spanyol Kupa",
        name: "Copa del Rey",
        country: Some(("ES", "Spain")),
        tier: 0,
        priority: 40,
        international: false,
    },
    League {
        key: "Török bajnokság",
        name: "Süper Lig",
        country: Some(("TR", "Turkey")),
        tier: 1,
        priority: 60,
        international: false,
    },
    League {
        key: "Walesi bajnokság",
        name: "Cymru Premier",
        country: Some(("GB", "Scotland")),
        tier: 1,
        priority: 50,
        international: false,
    },
];

/// Női liga-tábla (S94): a Tipplap női szekcióinak tényleges cím-prefixei →
/// v2 kanonikus női versenyek.
///
/// A férfi `LEAGUES` kulcsa nem minden országban egyezik a női cím szövegével
/// (a 2026-11-21-i renderben pl. „Német bajnokság, női", „Olasz bajnokság, női",
/// „Angol bajnokság, női" szerepel), ezért itt a tényleges női cím a kulcs.
/// Ahol a Tipplap a férfi kulcsot is használhatja (pl. „Bundesliga 1., női"),
/// azt is felvesszük, hogy a napi cím-forma ne törje el a lefedettséget.
/// A `női` jelzőt a [`league_for`] a kulcs-egyezés előtt leválasztja.
const WOMEN_LEAGUES: [League; 10] = [
    League {
        key: "Spanyol bajnokság",
        name: "Liga F",
        country: Some(("ES", "Spain")),
        tier: 1,
        priority: 55,
        international: false,
    },
    League {
        key: "Francia bajnokság",
        name: "Première Ligue (női)",
        country: Some(("FR", "France")),
        tier: 1,
        priority: 55,
        international: false,
    },
    League {
        key: "Bundesliga 1.",
        name: "Frauen-Bundesliga",
        country: Some(("DE", "Germany")),
        tier: 1,
        priority: 55,
        international: false,
    },
    League {
        key: "Német bajnokság",
        name: "Frauen-Bundesliga",
        country: Some(("DE", "Germany")),
        tier: 1,
        priority: 55,
        international: false,
    },
    League {
        key: "Olasz A",
        name: "Women's Serie A",
        country: Some(("IT", "Italy")),
        tier: 1,
        priority: 55,
        international: false,
    },
    League {
        key: "Olasz bajnokság",
        name: "Women's Serie A",
        country: Some(("IT", "Italy")),
        tier: 1,
        priority: 55,
        international: false,
    },
    League {
        key: "NB I",
        name: "NB I (női)",
        country: Some(("HU", "Hungary")),
        tier: 1,
        priority: 55,
        international: false,
    },
    League {
        key: "Premier Liga",
        name: "FA Women's Super League",
        country: Some(("GB", "England")),
        tier: 1,
        priority: 55,
        international: false,
    },
    League {
        key: "Angol bajnokság",
        name: "FA Women's Super League",
        country: Some(("GB", "England")),
        tier: 1,
        priority: 55,
        international: false,
    },
    League {
        key: "Bajnokok Ligája",
        name: "Women's Champions League",
        country: None,
        tier: 1,
        priority: 50,
        international: true,
    },
];

/// Egy ligacím feldolgozott hozzárendelése.
///
/// A női ág a `WOMEN_LEAGUES` pontos táblájából vagy (ha nincs női pár) a férfi
/// ligából „… (női)" utótaggal jön, ezért a név `Cow` (a fallback tulajdonolt
/// `String`). A `round_label` soha nem tartalmazhat „női" tokent.
#[derive(Clone)]
struct LeagueAssignment {
    league: &'static League,
    /// A payloadba kerülő kanonikus verseny-név.
    name: Cow<'static, str>,
    /// Női meccs: a csapatok gender/branch konvenciója ebből dől el.
    women: bool,
    round_label: Option<String>,
}

/// Női jelző token („női" → fold „noi") a ligacímben, vesszős tagként vagy
/// önálló szóként; a cím többi része változatlanul marad az egyezéshez.
fn strip_women_marker(title: &str) -> (String, bool) {
    let parts: Vec<&str> = title.split(',').map(str::trim).collect();
    if parts.iter().any(|part| is_women_token(part)) {
        let kept: Vec<&str> = parts
            .into_iter()
            .filter(|part| !part.is_empty() && !is_women_token(part))
            .collect();
        return (kept.join(", "), true);
    }
    // Vessző nélküli forma: önálló „női" szó bárhol a címben.
    let tokens: Vec<&str> = title.split_whitespace().collect();
    if tokens.iter().any(|token| is_women_token(token)) {
        let kept: Vec<&str> = tokens
            .into_iter()
            .filter(|token| !is_women_token(token))
            .collect();
        return (kept.join(" "), true);
    }
    (title.to_string(), false)
}

fn is_women_token(text: &str) -> bool {
    fold_name(text) == "noi"
}

/// Női cím maradékából csak a forduló-jelölés marad: szám („12", „12.") vagy
/// „N. forduló"/„N. kör" alak (foldve is). A „női" tokent a hívó már levágta.
fn women_round_label(rest: &str) -> Option<String> {
    rest.split(',')
        .map(str::trim)
        .find(|token| is_round_token(token))
        .map(str::to_string)
}

fn is_round_token(token: &str) -> bool {
    let folded = fold_name(token);
    if folded.is_empty() {
        return false;
    }
    let digits = folded
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    if digits.is_empty() {
        return false;
    }
    let tail = &folded[digits.len()..];
    tail.is_empty() || tail == "fordulo" || tail == "kor"
}

/// Női tábla prefix-egyezése szóhatárral: a kulcs után csak cím vége, vessző
/// vagy pont/space jöhet, így az „NB I" kulcs nem nyeli el az „NB II" címet.
fn women_prefix<'a>(leagues: &'a [League], title: &str) -> Option<(&'a League, &'static str)> {
    let mut best: Option<(&'a League, &'static str)> = None;
    for league in leagues {
        if !title.starts_with(league.key) {
            continue;
        }
        let rest = &title[league.key.len()..];
        let boundary = rest.is_empty() || rest.starts_with([',', '.', ' ']);
        if boundary && best.is_none_or(|(_, key)| league.key.len() > key.len()) {
            best = Some((league, league.key));
        }
    }
    best
}

/// A tábla leghosszabb prefix-egyezése (pl. „Nemzetek Ligája, B div" előbb,
/// mint a rövidebb „Nemzetek Ligája").
fn longest_prefix<'a>(leagues: &'a [League], title: &str) -> Option<(&'a League, &'static str)> {
    let mut best: Option<(&'a League, &'static str)> = None;
    for league in leagues {
        if title.starts_with(league.key) && best.is_none_or(|(_, key)| league.key.len() > key.len())
        {
            best = Some((league, league.key));
        }
    }
    best
}

/// Férfi liga-egyezés: prefix (van maradék) vagy a GENERAL_LEAGUES pontos,
/// ékezet/kisbetű-független címe (nincs maradék).
fn match_male(title: &str) -> Option<(&'static League, &str)> {
    if let Some((league, key)) = longest_prefix(&LEAGUES, title) {
        return Some((league, &title[key.len()..]));
    }
    let folded = fold_name(title);
    let league = GENERAL_LEAGUES
        .iter()
        .find(|league| fold_name(league.key) == folded)?;
    Some((league, ""))
}

/// A GENERAL_LEAGUES pontos cím-egyezése vesszős farokkal is: a cím elejéről
/// levágja a forduló/csoport tokent (női fallbackhez; a férfi ág változatlan).
fn match_general_with_rest(title: &str) -> Option<(&'static League, String)> {
    let parts: Vec<&str> = title
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    for end in (1..=parts.len()).rev() {
        let head = parts[..end].join(", ");
        let folded = fold_name(&head);
        if let Some(league) = GENERAL_LEAGUES
            .iter()
            .find(|league| fold_name(league.key) == folded)
        {
            return Some((league, parts[end..].join(", ")));
        }
    }
    None
}

/// Női fallback: férfi liga (prefix vagy pontos GENERAL cím) + a maradékból
/// nyert round_label. `None`, ha a férfi táblák egyike sem ismeri a címet.
fn women_fallback(clean: &str) -> Option<(&'static League, Option<String>)> {
    if let Some((league, rest)) = match_male(clean) {
        let rest = rest.trim_start_matches([',', '.', ' ']).trim();
        return Some((league, women_round_label(rest)));
    }
    let (league, rest) = match_general_with_rest(clean)?;
    Some((league, women_round_label(&rest)))
}

fn league_for(title: &str) -> Option<LeagueAssignment> {
    let trimmed = title
        .trim()
        .strip_prefix(TITLE_PREFIX)
        .unwrap_or(title.trim());
    let (clean, women) = strip_women_marker(trimmed);

    if women {
        if let Some((league, key)) = women_prefix(&WOMEN_LEAGUES, &clean) {
            let rest = clean[key.len()..]
                .trim_start_matches([',', '.', ' '])
                .trim();
            return Some(LeagueAssignment {
                league,
                name: Cow::Borrowed(league.name),
                women: true,
                round_label: women_round_label(rest),
            });
        }
        // Nincs pontos női pár: a férfi verseny neve + „ (női)" utótag, hogy a
        // meccs ne vesszen el; a parse naplózza, hogy ez fallback volt.
        if let Some((league, round_label)) = women_fallback(&clean) {
            tracing::warn!(
                source = SOURCE_SLUG,
                title = trimmed,
                male_competition = league.name,
                "ismeretlen női ligacím: férfi verseny + (női) utótag"
            );
            return Some(LeagueAssignment {
                league,
                name: Cow::Owned(format!("{} (női)", league.name)),
                women: true,
                round_label,
            });
        }
        tracing::debug!(
            source = SOURCE_SLUG,
            title = trimmed,
            "ismeretlen női ligacím (nincs férfi pár sem) — a sor kimarad"
        );
        return None;
    }

    // Férfi ág: a viselkedés változatlan (prefix-egyezésnél a maradék a
    // round_label, a GENERAL táblánál pontos cím és nincs round).
    let (league, rest) = match_male(&clean)?;
    let rest = rest.trim_start_matches([',', '.', ' ']).trim();
    Some(LeagueAssignment {
        league,
        name: Cow::Borrowed(league.name),
        women: false,
        round_label: (!rest.is_empty()).then(|| rest.to_string()),
    })
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

/// Csapat-identitás payload: a női ág a research adapter konvencióját követi
/// (`"gender": "female"`, `"branch": "women"`), a férfi ág változatlan.
fn team_identity_payload(
    team_id: &str,
    name: &str,
    country_code: &str,
    women: bool,
) -> serde_json::Value {
    let (gender, branch) = if women {
        ("female", "women")
    } else {
        ("male", "senior")
    };
    serde_json::json!({
        "id": null,
        "source_entity_id": team_id,
        "name": name,
        "country_code": country_code,
        "gender": gender,
        "age_group": "senior",
        "branch": branch,
    })
}

pub struct TipplapAdapter;

impl SourceAdapter for TipplapAdapter {
    fn meta(&self) -> AdapterMeta {
        AdapterMeta {
            source_slug: SOURCE_SLUG.into(),
            adapter_version: "1.0".into(),
            parser_version: "1.1".into(),
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
        let mut league: Option<LeagueAssignment> = None;
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
            let Some(league_meta) = league.as_ref() else {
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
                if league_meta.league.international {
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
                        .league
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
            let (country_code, country_name) = match league_meta.league.country {
                Some((cc, cn)) => (cc.to_string(), cn.to_string()),
                None => (String::new(), String::new()),
            };
            let competition_name = league_meta.name.as_ref();
            for (team_id, team_name, team_country) in [
                (&home_id, &home_name, &home_country),
                (&away_id, &away_name, &away_country),
            ] {
                teams.entry(team_id.clone()).or_insert_with(|| {
                    team_identity_payload(team_id, team_name, team_country, league_meta.women)
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
                    "round_label": league_meta.round_label.as_deref(),
                    "competition_timezone": "Europe/Budapest",
                    "competition": {
                        "id": null,
                        "source_entity_id": (!competition_name.is_empty())
                            .then_some(competition_name),
                        "name": competition_name,
                        "country_code": country_code,
                        "country_name": country_name,
                        "tier": league_meta.league.tier,
                        "priority": league_meta.league.priority,
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
    use super::{league_for, team_identity_payload};

    #[test]
    fn maps_general_leagues_from_v1_alias_registry() {
        // S36a: a v1 tipplap_league_alias reviewed sorai + v2 egyezés.
        let assignment = league_for("Labdarúgás – Angol Liga 1.").unwrap();
        assert!(!assignment.women);
        assert_eq!(assignment.name.as_ref(), "League One");
        assert_eq!(assignment.league.country, Some(("GB", "Scotland")));
        assert_eq!(assignment.league.tier, 3);
        assert_eq!(assignment.round_label, None);

        let assignment = league_for("Labdarúgás – Spanyol 2.").unwrap();
        assert_eq!(assignment.name.as_ref(), "LaLiga 2");
        assert_eq!(assignment.league.tier, 2);

        let assignment = league_for("Labdarúgás – Török bajnokság").unwrap();
        assert_eq!(assignment.name.as_ref(), "Süper Lig");

        // Ékezet/kisbetű-független egyezés (a prefix levágása után).
        let assignment = league_for("Labdarúgás – OSZTRÁK 2.").unwrap();
        assert_eq!(assignment.name.as_ref(), "2. Liga");

        // Ismeretlen cím továbbra sem találgat.
        assert!(league_for("Labdarúgás – Szuper Liga, Mars").is_none());
    }

    #[test]
    fn maps_nb_two_and_concacaf_leagues() {
        let assignment = league_for("Labdarúgás – NB II").unwrap();
        assert_eq!(assignment.name.as_ref(), "NB II");
        assert_eq!(assignment.round_label, None);

        let assignment = league_for("Labdarúgás – CONCACAF Nemz.Ligája A div.Acs").unwrap();
        assert_eq!(
            assignment.name.as_ref(),
            "CONCACAF Nations League - League A"
        );
        assert_eq!(assignment.round_label.as_deref(), Some("Acs"));

        let assignment = league_for("Labdarúgás – CONCACAF Nemz.Ligája C div.Bcs").unwrap();
        assert_eq!(
            assignment.name.as_ref(),
            "CONCACAF Nations League - League C"
        );
    }

    #[test]
    fn maps_women_league_without_round_label() {
        // S94: a „női" jelző nem kerülhet round_labelbe.
        let assignment = league_for("Labdarúgás – Spanyol bajnokság, női").unwrap();
        assert!(assignment.women);
        assert_eq!(assignment.name.as_ref(), "Liga F");
        assert_eq!(assignment.league.country, Some(("ES", "Spain")));
        assert_eq!(assignment.league.tier, 1);
        assert_eq!(assignment.league.priority, 55);
        assert!(!assignment.league.international);
        assert_eq!(assignment.round_label, None);
    }

    #[test]
    fn maps_women_league_with_round_label() {
        let assignment = league_for("Labdarúgás – Spanyol bajnokság, női, 12. forduló").unwrap();
        assert!(assignment.women);
        assert_eq!(assignment.name.as_ref(), "Liga F");
        assert_eq!(assignment.round_label.as_deref(), Some("12. forduló"));

        // A „női" mellett más vesszős tag (pl. csoport) nem round_label.
        let assignment = league_for("Labdarúgás – Angol bajnokság, női, Dél").unwrap();
        assert_eq!(assignment.name.as_ref(), "FA Women's Super League");
        assert_eq!(assignment.round_label, None);
    }

    #[test]
    fn keeps_male_league_behaviour() {
        let assignment = league_for("Labdarúgás – Francia bajnokság").unwrap();
        assert!(!assignment.women);
        assert_eq!(assignment.name.as_ref(), "Ligue 1");
        assert_eq!(assignment.league.country, Some(("FR", "France")));
        assert_eq!(assignment.round_label, None);
    }

    #[test]
    fn maps_women_nb_one() {
        let assignment = league_for("Labdarúgás – NB I, női").unwrap();
        assert!(assignment.women);
        assert_eq!(assignment.name.as_ref(), "NB I (női)");
        assert_eq!(assignment.league.country, Some(("HU", "Hungary")));
        assert_eq!(assignment.league.priority, 55);
        assert_eq!(assignment.round_label, None);

        // Az „NB I" kulcs nem nyeli el az „NB II" címet (szóhatár).
        let assignment = league_for("Labdarúgás – NB II, női").unwrap();
        assert!(assignment.women);
        assert_eq!(assignment.name.as_ref(), "NB II (női)");
    }

    #[test]
    fn maps_women_champions_league() {
        let assignment = league_for("Labdarúgás – Bajnokok Ligája, női").unwrap();
        assert!(assignment.women);
        assert_eq!(assignment.name.as_ref(), "Women's Champions League");
        assert_eq!(assignment.league.country, None);
        assert!(assignment.league.international);
        assert_eq!(assignment.league.tier, 1);
        assert_eq!(assignment.league.priority, 50);
        assert_eq!(assignment.round_label, None);
    }

    #[test]
    fn women_title_without_exact_pair_falls_back_to_male_name() {
        // Nem kötelező lefedettség (pl. holland női): férfi név + „ (női)".
        let assignment = league_for("Labdarúgás – Holland bajnokság, női, 5. forduló").unwrap();
        assert!(assignment.women);
        assert_eq!(assignment.name.as_ref(), "Eredivisie (női)");
        assert_eq!(assignment.league.name, "Eredivisie");
        assert_eq!(assignment.round_label.as_deref(), Some("5. forduló"));
    }

    #[test]
    fn women_title_without_any_pair_is_skipped_not_panicking() {
        assert!(league_for("Labdarúgás – Szuper Liga, Mars, női").is_none());
    }

    #[test]
    fn male_tables_map_to_themselves_without_women_marker() {
        // S94 regresszió: a férfi táblák minden kulcsa (a TITLE_PREFIXszel
        // ellátva) önmagára képződjön, women=false és round_label nélkül.
        let titles = super::LEAGUES
            .iter()
            .chain(super::GENERAL_LEAGUES.iter())
            .map(|league| (format!("{}{}", super::TITLE_PREFIX, league.key), league));
        for (title, expected) in titles {
            let assignment = league_for(&title).unwrap_or_else(|| panic!("nem egyezett: {title}"));
            assert!(!assignment.women, "női jelző férfi címen: {title}");
            assert_eq!(assignment.name.as_ref(), expected.name, "név: {title}");
            assert_eq!(assignment.league.key, expected.key, "kulcs: {title}");
            assert_eq!(assignment.round_label, None, "round_label: {title}");
        }
    }

    #[test]
    fn team_identity_payload_follows_gender_convention() {
        let women = team_identity_payload("deportivo-alaves-w", "Deportivo Alaves W", "ES", true);
        assert_eq!(women["gender"], "female");
        assert_eq!(women["age_group"], "senior");
        assert_eq!(women["branch"], "women");
        assert_eq!(women["country_code"], "ES");

        let men = team_identity_payload("deportivo-alaves", "Deportivo Alaves", "ES", false);
        assert_eq!(men["gender"], "male");
        assert_eq!(men["age_group"], "senior");
        assert_eq!(men["branch"], "senior");
    }
}
