//! Soccerway nap-feed adapter — S4d (Obscura-út).
//!
//! A Soccerway JS-renderelt nap-oldala (Flashscore-stílusú, minified DOM):
//! a nap a localizált útvonalon érhető el (`/` = ma, `/tegnap/` = tegnap);
//! a sorok kliens-oldalon renderelődnek → a fetcher markerre vár. A lista
//! lokalizált (hu) stage-szöveget ad, ezért a lezárt állapotot a pontszám-
//! jelenlét is erősíti, a decoy-200 hiboldalt pedig tartalmi őr fogja.
//!
//! Egy job = egy liga: a napi oldalon minden verseny szerepel, a parser a
//! `request_key` domainje (liga-slug) szerint szűr, mert a `NormalizedDay`
//! egy verseny-meta-t hordoz.

use chrono::{Datelike, Local, NaiveDate, NaiveTime, TimeZone, Timelike, Utc};
use football_domain::error::ParseError;
use football_domain::policy::{FetchJob, Transport};
use scraper::{ElementRef, Html, Selector};

use crate::adapter::{AdapterMeta, Observation, ParseCtx, SourceAdapter};
use crate::fetch::{FetchRequest, RawPayload};

pub const SOURCE_SLUG: &str = "soccerway";

/// A lista-markert a fetcher pollozza (kliens-oldali renderelés).
const READY_MARKER: &str = "div.event__match[data-event-row]";

/// A napi lista idő-bélyegzői a bepinnelt UTC render miatt UTC-ben vannak.
const COMPETITION_TIMEZONE: &str = "UTC";

#[derive(Debug, Clone)]
pub struct SoccerwayLeague {
    /// Stabil, lokalizált URL-slug (pl. `anglia/premier-league`).
    pub slug: String,
    pub country_code: String,
    pub country_name: String,
    pub name: String,
    pub tier: i16,
    pub priority: i32,
}

/// (slug, ország-kód, ország-név, verseny-név, tier, priority)
const LEAGUES: [(&str, &str, &str, &str, i16, i32); 7] = [
    (
        "anglia/premier-league",
        "GB",
        "England",
        "Premier League",
        1,
        6,
    ),
    ("spanyolorszag/laliga", "ES", "Spain", "La Liga", 1, 6),
    ("olaszorszag/serie-a", "IT", "Italy", "Serie A", 1, 6),
    (
        "nemetorszag/bundesliga",
        "DE",
        "Germany",
        "Bundesliga",
        1,
        6,
    ),
    ("franciaorszag/ligue-1", "FR", "France", "Ligue 1", 1, 6),
    ("magyarorszag/nb-i", "HU", "Hungary", "NB I", 1, 8),
    ("magyarorszag/nb-ii", "HU", "Hungary", "NB II", 2, 10),
];

pub struct SoccerwayAdapter {
    pub leagues: Vec<SoccerwayLeague>,
}

impl Default for SoccerwayAdapter {
    fn default() -> Self {
        Self {
            leagues: LEAGUES
                .iter()
                .map(|(slug, cc, cn, name, tier, priority)| SoccerwayLeague {
                    slug: (*slug).to_string(),
                    country_code: (*cc).to_string(),
                    country_name: (*cn).to_string(),
                    name: (*name).to_string(),
                    tier: *tier,
                    priority: *priority,
                })
                .collect(),
        }
    }
}

/// Nap-útvonal: ma `/`, tegnap `/tegnap/`; más dátum (backfill) egyelőre
/// nincs (a naptár-popup útvonala nincs mérve) → üres terv.
pub fn route_for(today: NaiveDate, requested: NaiveDate) -> Option<&'static str> {
    match (requested - today).num_days() {
        0 => Some("/"),
        -1 => Some("/tegnap/"),
        _ => None,
    }
}

/// Szezon-címke a dátumból (júliustól induló idény): 2026-11 → 2026/27.
fn season_label(date: NaiveDate) -> String {
    let year = if date.month() >= 7 {
        date.year()
    } else {
        date.year() - 1
    };
    format!("{year}/{:02}", (year + 1) % 100)
}

fn text_of(element: &ElementRef) -> String {
    element.text().collect::<String>().trim().to_string()
}

/// A csapathivatkozás utolsó két szegmense `<slug>-<Code>`; a kód a stabil
/// forrás-azonosító. A lokalizált útvonal-szó (pl. `merkozes`) nem számít.
fn team_codes(href: &str) -> Option<(String, String)> {
    let path = href.split('?').next().unwrap_or(href);
    let mut segments = path
        .trim_end_matches('/')
        .rsplit('/')
        .filter(|segment| !segment.is_empty());
    let away = segments.next()?;
    let home = segments.next()?;
    let code = |segment: &str| -> Option<String> {
        segment
            .rsplit('-')
            .next()
            .filter(|code| code.len() >= 4)
            .map(str::to_string)
    };
    Some((code(home)?, code(away)?))
}

#[derive(Debug, Default)]
struct RowScores {
    home: Option<i16>,
    away: Option<i16>,
    home_pens: Option<i16>,
    away_pens: Option<i16>,
}

fn parse_i16(text: &str) -> Option<i16> {
    let digits: String = text
        .trim()
        .chars()
        .take_while(|c| *c != '(' && c.is_ascii_digit())
        .collect();
    digits.parse::<i16>().ok()
}

/// Büntető-sup szövege zárójelben: `(4)`.
fn parse_i16_pens(text: &str) -> Option<i16> {
    text.trim()
        .trim_matches(|c| c == '(' || c == ')')
        .parse::<i16>()
        .ok()
}

fn parse_scores(row: &ElementRef) -> Option<RowScores> {
    let span_selector = Selector::parse("span[data-testid=wcl-tableScore]").ok()?;
    let pen_selector = Selector::parse("sup.wcl-penaltyScore").ok()?;
    let mut scores = RowScores::default();
    for span in row.select(&span_selector) {
        let side = span.value().attr("data-side");
        let main = parse_i16(&text_of(&span));
        let pens = span
            .select(&pen_selector)
            .next()
            .and_then(|sup| parse_i16_pens(&text_of(&sup)));
        match side {
            Some("1") => {
                scores.home = main;
                scores.home_pens = pens;
            }
            Some("2") => {
                scores.away = main;
                scores.away_pens = pens;
            }
            _ => {}
        }
    }
    if scores.home.is_some() || scores.away.is_some() {
        Some(scores)
    } else {
        None
    }
}

/// A lezárt sor eredmény-mappingje: a 90 perces FT-konvenció szerint a
/// hosszabbítás/párharc idején mért fő pontszám nem FT.
fn result_for(decided_by: Option<&str>, scores: Option<&RowScores>) -> Option<serde_json::Value> {
    let scores = scores?;
    let (home_ft, away_ft, home_et, away_et, home_pens, away_pens) = match decided_by {
        Some("et") => (None, None, scores.home, scores.away, None, None),
        Some("pens") => (
            None,
            None,
            scores.home,
            scores.away,
            scores.home_pens,
            scores.away_pens,
        ),
        _ => (scores.home, scores.away, None, None, None, None),
    };
    Some(serde_json::json!({
        "home_ft": home_ft,
        "away_ft": away_ft,
        "home_ht": null,
        "away_ht": null,
        "home_et": home_et,
        "away_et": away_et,
        "home_pens": home_pens,
        "away_pens": away_pens,
        "decided_by": decided_by,
    }))
}

/// Decoy-200 hiboldal (mért minta): valódi státusz nélkül, tartalmi őrrel.
pub fn is_decoy(html: &str) -> bool {
    html.contains("404_page")
        || html.contains("The requested page can&apos;t be displayed")
        || html.contains("The requested page can't be displayed")
}

/// A naptár-gomb angol aria-labelje a nap dátuma (locale-független guard).
pub fn page_date(html: &str) -> Option<NaiveDate> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("[data-testid=wcl-dayPickerButton]").ok()?;
    let label = document
        .select(&selector)
        .next()
        .and_then(|button| button.value().attr("aria-label"))?;
    NaiveDate::parse_from_str(label.trim(), "%A, %B %d, %Y").ok()
}

/// A nap-oldal egy verseny-csoportja (fejléc) az osztályozással.
#[derive(Debug, Clone)]
struct GroupMeta {
    name: String,
    country_code: String,
    country_name: String,
    tier: i16,
    priority: i32,
    gender: &'static str,
    branch: &'static str,
    /// A csapatok országa ebben a csoportban ("" = ismeretlen, pl. kupa).
    team_country: String,
}

/// Lokalizált országnév → (ISO, angol név). Ismeretlen → None (fail-open
/// a besorolásnál: a verseny ország nélkül is bekerülhet).
fn country_by_title(title: &str) -> Option<(&'static str, &'static str)> {
    let key = title.trim().to_lowercase();
    let pairs: &[(&str, &str, &str)] = &[
        ("anglia", "GB", "England"),
        ("skócia", "GB", "Scotland"),
        ("wales", "GB", "Wales"),
        ("észak-írország", "GB", "Northern Ireland"),
        ("spanyolország", "ES", "Spain"),
        ("olaszország", "IT", "Italy"),
        ("németország", "DE", "Germany"),
        ("franciaország", "FR", "France"),
        ("magyarország", "HU", "Hungary"),
        ("ausztria", "AT", "Austria"),
        ("svájc", "CH", "Switzerland"),
        ("hollandia", "NL", "Netherlands"),
        ("belgium", "BE", "Belgium"),
        ("portugália", "PT", "Portugal"),
        ("törökország", "TR", "Turkey"),
        ("görögország", "GR", "Greece"),
        ("horvátország", "HR", "Croatia"),
        ("szerbia", "RS", "Serbia"),
        ("szlovénia", "SI", "Slovenia"),
        ("bosznia-hercegovina", "BA", "Bosnia and Herzegovina"),
        ("montenegró", "ME", "Montenegro"),
        ("észak-macedónia", "MK", "North Macedonia"),
        ("albánia", "AL", "Albania"),
        ("lengyelország", "PL", "Poland"),
        ("csehország", "CZ", "Czech Republic"),
        ("szlovákia", "SK", "Slovakia"),
        ("románia", "RO", "Romania"),
        ("bulgária", "BG", "Bulgaria"),
        ("oroszország", "RU", "Russia"),
        ("ukrajna", "UA", "Ukraine"),
        ("fehéroroszország", "BY", "Belarus"),
        ("moldova", "MD", "Moldova"),
        ("grúzia", "GE", "Georgia"),
        ("örményország", "AM", "Armenia"),
        ("azerbajdzsán", "AZ", "Azerbaijan"),
        ("kazahsztán", "KZ", "Kazakhstan"),
        ("izrael", "IL", "Israel"),
        ("izland", "IS", "Iceland"),
        ("norvégia", "NO", "Norway"),
        ("svédország", "SE", "Sweden"),
        ("dánia", "DK", "Denmark"),
        ("finnország", "FI", "Finland"),
        ("észtország", "EE", "Estonia"),
        ("lettország", "LV", "Latvia"),
        ("litvánia", "LT", "Lithuania"),
        ("írország", "IE", "Ireland"),
        ("ciprus", "CY", "Cyprus"),
        ("málta", "MT", "Malta"),
        ("luxemburg", "LU", "Luxembourg"),
        ("brazília", "BR", "Brazil"),
        ("argentin", "AR", "Argentina"),
        ("egyesült államok", "US", "United States"),
        ("mexikó", "MX", "Mexico"),
        ("japán", "JP", "Japan"),
        ("kína", "CN", "China"),
        ("ausztrália", "AU", "Australia"),
        ("egyiptom", "EG", "Egypt"),
        ("marokkó", "MA", "Morocco"),
        ("algéria", "DZ", "Algeria"),
        ("tunézia", "TN", "Tunisia"),
        ("dél-afrika", "ZA", "South Africa"),
    ];
    pairs
        .iter()
        .find(|(name, _, _)| key.contains(name))
        .map(|(_, cc, cn)| (*cc, *cn))
}

/// Angol országnevek (nemzetközi meccsek csapatnevei) → (ISO, angol név).
fn country_by_english(name: &str) -> Option<(&'static str, &'static str)> {
    let pairs: &[(&str, &str, &str)] = &[
        ("england", "GB", "England"),
        ("scotland", "GB", "Scotland"),
        ("wales", "GB", "Wales"),
        ("northern ireland", "GB", "Northern Ireland"),
        ("spain", "ES", "Spain"),
        ("italy", "IT", "Italy"),
        ("germany", "DE", "Germany"),
        ("france", "FR", "France"),
        ("hungary", "HU", "Hungary"),
        ("austria", "AT", "Austria"),
        ("switzerland", "CH", "Switzerland"),
        ("netherlands", "NL", "Netherlands"),
        ("belgium", "BE", "Belgium"),
        ("portugal", "PT", "Portugal"),
        ("turkey", "TR", "Turkey"),
        ("greece", "GR", "Greece"),
        ("croatia", "HR", "Croatia"),
        ("serbia", "RS", "Serbia"),
        ("slovenia", "SI", "Slovenia"),
        ("bosnia & herzegovina", "BA", "Bosnia and Herzegovina"),
        ("bosnia and herzegovina", "BA", "Bosnia and Herzegovina"),
        ("montenegro", "ME", "Montenegro"),
        ("north macedonia", "MK", "North Macedonia"),
        ("albania", "AL", "Albania"),
        ("poland", "PL", "Poland"),
        ("czech republic", "CZ", "Czech Republic"),
        ("czechia", "CZ", "Czech Republic"),
        ("slovakia", "SK", "Slovakia"),
        ("romania", "RO", "Romania"),
        ("bulgaria", "BG", "Bulgaria"),
        ("russia", "RU", "Russia"),
        ("ukraine", "UA", "Ukraine"),
        ("belarus", "BY", "Belarus"),
        ("moldova", "MD", "Moldova"),
        ("georgia", "GE", "Georgia"),
        ("armenia", "AM", "Armenia"),
        ("azerbaijan", "AZ", "Azerbaijan"),
        ("kazakhstan", "KZ", "Kazakhstan"),
        ("israel", "IL", "Israel"),
        ("iceland", "IS", "Iceland"),
        ("norway", "NO", "Norway"),
        ("sweden", "SE", "Sweden"),
        ("denmark", "DK", "Denmark"),
        ("finland", "FI", "Finland"),
        ("estonia", "EE", "Estonia"),
        ("latvia", "LV", "Latvia"),
        ("lithuania", "LT", "Lithuania"),
        ("ireland", "IE", "Ireland"),
        ("cyprus", "CY", "Cyprus"),
        ("malta", "MT", "Malta"),
        ("luxembourg", "LU", "Luxembourg"),
        ("faroe islands", "FO", "Faroe Islands"),
        ("andorra", "AD", "Andorra"),
        ("gibraltar", "GI", "Gibraltar"),
        ("liechtenstein", "LI", "Liechtenstein"),
        ("san marino", "SM", "San Marino"),
        ("kosovo", "XK", "Kosovo"),
        ("brazil", "BR", "Brazil"),
        ("argentina", "AR", "Argentina"),
        ("uruguay", "UY", "Uruguay"),
        ("colombia", "CO", "Colombia"),
        ("peru", "PE", "Peru"),
        ("chile", "CL", "Chile"),
        ("ecuador", "EC", "Ecuador"),
        ("paraguay", "PY", "Paraguay"),
        ("bolivia", "BO", "Bolivia"),
        ("venezuela", "VE", "Venezuela"),
        ("mexico", "MX", "Mexico"),
        ("united states", "US", "United States"),
        ("usa", "US", "United States"),
        ("canada", "CA", "Canada"),
        ("japan", "JP", "Japan"),
        ("south korea", "KR", "South Korea"),
        ("china", "CN", "China"),
        ("australia", "AU", "Australia"),
        ("egypt", "EG", "Egypt"),
        ("morocco", "MA", "Morocco"),
        ("algeria", "DZ", "Algeria"),
        ("tunisia", "TN", "Tunisia"),
        ("south africa", "ZA", "South Africa"),
        ("nigeria", "NG", "Nigeria"),
        ("ghana", "GH", "Ghana"),
        ("cameroon", "CM", "Cameroon"),
        ("senegal", "SN", "Senegal"),
        ("ivory coast", "CI", "Ivory Coast"),
        ("mali", "ML", "Mali"),
        ("kenya", "KE", "Kenya"),
    ];
    let key = name.trim().to_lowercase();
    pairs
        .iter()
        .find(|(label, _, _)| key == *label)
        .map(|(_, cc, cn)| (*cc, *cn))
}

/// Nemzetközi verseny-e (a csapatnév ekkor ország-név).
fn is_international(text: &str) -> bool {
    [
        "international",
        "nations league",
        "cup of nations",
        "qualification",
        "world cup",
        "friendly",
        "euro u",
        "copa america",
        "afcon",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

/// `Belgium U21` → (`Belgium`, youth); `Spain U17 W` → (`Spain`, youth, women).
fn team_hints(name: &str) -> (String, bool, bool) {
    let mut youth = false;
    let mut women = false;
    let mut base: Vec<&str> = Vec::new();
    for token in name.split_whitespace() {
        let upper = token.to_ascii_uppercase();
        let is_u = upper.len() >= 2
            && upper.starts_with('U')
            && upper[1..].chars().all(|c| c.is_ascii_digit());
        if is_u {
            youth = true;
            continue;
        }
        if upper == "W" || upper == "WOMEN" || token.eq_ignore_ascii_case("női") {
            women = true;
            continue;
        }
        base.push(token);
    }
    (base.join(" "), youth, women)
}

fn is_cup(text: &str) -> bool {
    [
        "kupa",
        "cup",
        "copa",
        "coupe",
        "pokal",
        "szuperkupa",
        "supercup",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn is_women(text: &str) -> bool {
    ["női", "-noi", "women", "feminine", "femenina", "femminile"]
        .iter()
        .any(|needle| text.contains(needle))
}

fn is_youth(text: &str) -> bool {
    [
        "u17",
        "u18",
        "u19",
        "u20",
        "u21",
        "u23",
        "ifjúsági",
        "youth",
        "junior",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

impl SoccerwayAdapter {
    /// Csoport-osztályozás: ismert liga → kanonikus meta; ismeretlen →
    /// megjelenített név + zászló-ország + kupa/női/utánpótlás heurisztika.
    fn group_meta(&self, slug: &str, title: &str, flag_title: &str) -> GroupMeta {
        let haystack = format!("{slug} {title}").to_lowercase();
        let (gender, branch) = if is_women(&haystack) {
            ("female", "women")
        } else if is_youth(&haystack) {
            ("male", "youth")
        } else {
            ("male", "senior")
        };
        if let Some(known) = self.leagues.iter().find(|l| l.slug == slug) {
            return GroupMeta {
                name: known.name.clone(),
                country_code: known.country_code.clone(),
                country_name: known.country_name.clone(),
                tier: known.tier,
                priority: known.priority,
                gender,
                branch,
                team_country: known.country_code.clone(),
            };
        }
        let cup = is_cup(&haystack);
        let (country_code, country_name) = country_by_title(flag_title).unwrap_or(("", ""));
        let priority = if gender == "female" {
            60
        } else if branch == "youth" {
            70
        } else if cup {
            40
        } else {
            50
        };
        let name = if title.trim().is_empty() {
            slug.rsplit('/').next().unwrap_or(slug).to_string()
        } else {
            title.trim().to_string()
        };
        GroupMeta {
            name,
            country_code: country_code.to_string(),
            country_name: country_name.to_string(),
            tier: if cup { 0 } else { 1 },
            priority,
            gender,
            branch,
            // Kupában/nemzetköziben a csapat-ország a versenyből nem
            // következik (a meccs két különböző országból is jöhet).
            team_country: if cup || country_code.is_empty() {
                String::new()
            } else {
                country_code.to_string()
            },
        }
    }
}

impl SourceAdapter for SoccerwayAdapter {
    fn meta(&self) -> AdapterMeta {
        AdapterMeta {
            source_slug: SOURCE_SLUG.into(),
            adapter_version: "1.0".into(),
            parser_version: "2.0".into(),
            domains: vec!["day".into()],
            transport: Transport::Obscura,
        }
    }

    fn plan(&self, job: &FetchJob) -> Vec<FetchRequest> {
        let Ok(requested) = NaiveDate::parse_from_str(&job.time_window, "%Y-%m-%d") else {
            return vec![];
        };
        let Some(path) = route_for(Local::now().date_naive(), requested) else {
            return vec![];
        };
        vec![FetchRequest {
            url: format!("https://www.soccerway.com{path}"),
            request_key: format!("{SOURCE_SLUG}:day:{}", job.time_window),
            endpoint_key: "day".into(),
            ready_marker: Some(READY_MARKER.into()),
        }]
    }

    fn parse(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError> {
        let html = String::from_utf8_lossy(&raw.bytes);
        if is_decoy(&html) {
            return Err(ParseError::DecoyDetected(
                "Soccerway hiboldal (200-zal)".into(),
            ));
        }
        let got = page_date(&html)
            .ok_or_else(|| ParseError::SchemaDrift("hiányzó nap-címke (aria-label)".into()))?;
        if got != ctx.requested_date {
            return Err(ParseError::DateGuardMismatch {
                requested: ctx.requested_date.to_string(),
                got: got.to_string(),
            });
        }

        let document = Html::parse_document(&html);
        let combined =
            Selector::parse("div[data-testid=wcl-headerLeague], div.event__match[data-event-row]")
                .map_err(|e| ParseError::Parse(format!("szelektor: {e}")))?;
        let header_link = Selector::parse("a.headerLeague__title")
            .map_err(|e| ParseError::Parse(format!("szelektor: {e}")))?;
        let header_flag = Selector::parse("span.headerLeague__flag")
            .map_err(|e| ParseError::Parse(format!("szelektor: {e}")))?;
        let row_link = Selector::parse("a.eventRowLink")
            .map_err(|e| ParseError::Parse(format!("szelektor: {e}")))?;
        let name_selector = Selector::parse("span[data-testid=wcl-simple-text-01]")
            .map_err(|e| ParseError::Parse(format!("szelektor: {e}")))?;
        let stage_selector = Selector::parse("div.event__stage--block")
            .map_err(|e| ParseError::Parse(format!("szelektor: {e}")))?;
        let time_selector = Selector::parse("div.event__time")
            .map_err(|e| ParseError::Parse(format!("szelektor: {e}")))?;
        let participant = |row: &ElementRef, class: &str| -> Option<String> {
            let selector = Selector::parse(&format!("div.{class}")).ok()?;
            let side = row.select(&selector).next()?;
            side.select(&name_selector)
                .next()
                .map(|span| text_of(&span))
                .filter(|name| !name.is_empty())
        };

        let captured_at = Utc::now();
        let mut current: Option<GroupMeta> = None;
        let mut teams: std::collections::BTreeMap<String, (String, GroupMeta)> = Default::default();
        let mut observations = Vec::new();

        for element in document.select(&combined) {
            if element.value().attr("data-testid") == Some("wcl-headerLeague") {
                let link = element.select(&header_link).next();
                let slug = link
                    .and_then(|a| a.value().attr("href"))
                    .map(|href| href.trim_matches('/').to_string())
                    .unwrap_or_default();
                let title = link.map(|a| text_of(&a)).unwrap_or_default();
                let flag_title = element
                    .select(&header_flag)
                    .next()
                    .and_then(|span| span.value().attr("title"))
                    .unwrap_or_default();
                current = Some(self.group_meta(&slug, &title, flag_title));
                continue;
            }

            let Some(group) = current.as_ref() else {
                continue;
            };
            let row = element;
            let Some(href) = row
                .select(&row_link)
                .next()
                .and_then(|a| a.value().attr("href").map(str::to_string))
            else {
                return Err(ParseError::SchemaDrift(
                    "meccssor a meccs-hivatkozás nélkül".into(),
                ));
            };
            let mid = row
                .value()
                .attr("id")
                .and_then(|id| id.strip_prefix("g_1_"))
                .or_else(|| {
                    href.split("mid=")
                        .nth(1)
                        .map(|rest| rest.split('&').next().unwrap_or(rest))
                })
                .ok_or_else(|| ParseError::SchemaDrift("meccssor mid nélkül".into()))?;
            let (home_code, away_code) = team_codes(&href)
                .ok_or_else(|| ParseError::SchemaDrift("csapatkód a hivatkozásból".into()))?;
            let home_name = participant(&row, "event__homeParticipant")
                .ok_or_else(|| ParseError::SchemaDrift("hazai csapatnév hiányzik".into()))?;
            let away_name = participant(&row, "event__awayParticipant")
                .ok_or_else(|| ParseError::SchemaDrift("vendég csapatnév hiányzik".into()))?;
            teams
                .entry(home_code.clone())
                .or_insert_with(|| (home_name.clone(), group.clone()));
            teams
                .entry(away_code.clone())
                .or_insert_with(|| (away_name.clone(), group.clone()));

            let classes: Vec<&str> = row.value().classes().collect();
            let is_live = classes.contains(&"event__match--live");
            let is_scheduled = classes.contains(&"event__match--scheduled");
            let stage = row.select(&stage_selector).next().map(|e| text_of(&e));
            let scores = parse_scores(&row);

            let (status, decided_by) = if is_live {
                ("live", None)
            } else if is_scheduled {
                ("scheduled", None)
            } else {
                match stage.as_deref() {
                    Some("Vége") => ("finished", Some("ft")),
                    Some("Hossz. után") => ("finished", Some("et")),
                    Some("Büntetők után") => ("finished", Some("pens")),
                    Some("Elhalasztva") => ("postponed", None),
                    Some("Törölve") => ("canceled", None),
                    Some("Elmarad") => ("abandoned", None),
                    Some(_) if scores.is_some() => ("finished", None),
                    Some(other) => {
                        tracing::warn!(
                            source = SOURCE_SLUG,
                            mid,
                            stage = other,
                            "ismeretlen stage-szöveg pontszám nélkül: a sor kimarad"
                        );
                        continue;
                    }
                    None if scores.is_some() => ("finished", None),
                    None => ("scheduled", None),
                }
            };

            // Pontos kezdés csak ütemezett sornál van (HH:MM); máskor a nap
            // déli helyőrzője marad, `date_only` precízióval.
            let kickoff_time = row
                .select(&time_selector)
                .next()
                .and_then(|e| NaiveTime::parse_from_str(&text_of(&e), "%H:%M").ok());
            let (kickoff_at, kickoff_precision) = match kickoff_time {
                Some(time) => {
                    let naive = ctx
                        .requested_date
                        .and_hms_opt(time.hour(), time.minute(), 0)
                        .ok_or_else(|| ParseError::Parse("érvénytelen kezdés".into()))?;
                    (Utc.from_utc_datetime(&naive), "exact")
                }
                None => {
                    let naive = ctx
                        .requested_date
                        .and_hms_opt(12, 0, 0)
                        .ok_or_else(|| ParseError::Parse("érvénytelen kezdés".into()))?;
                    (Utc.from_utc_datetime(&naive), "date_only")
                }
            };

            let result = if status == "finished" {
                result_for(decided_by, scores.as_ref())
            } else {
                None
            };

            observations.push(Observation {
                entity_kind: "match".into(),
                aspect: "match_core".into(),
                natural_key: format!("soccerway-{mid}"),
                source_record_key: format!("soccerway-{mid}"),
                observed_at: captured_at,
                payload: serde_json::json!({
                    "canonical_match_id": null,
                    "source_home_id": home_code,
                    "source_away_id": away_code,
                    "kickoff_at": kickoff_at,
                    "kickoff_precision": kickoff_precision,
                    "status": status,
                    "result": result,
                    "round_label": null,
                    "competition_timezone": COMPETITION_TIMEZONE,
                    "competition": {
                        "id": null,
                        "name": group.name,
                        "country_code": group.country_code,
                        "country_name": group.country_name,
                        "tier": group.tier,
                        "priority": group.priority,
                    },
                    "season": { "id": null, "label": season_label(ctx.requested_date) },
                    "source_code": SOURCE_SLUG,
                    "captured_at": captured_at,
                }),
            });
        }

        let mut team_obs = Vec::new();
        for (code, (name, group)) in &teams {
            let (base_name, name_youth, name_women) = team_hints(name);
            let comp_haystack = group.name.to_lowercase();
            // Nemzetközi meccsen a csapatnév ország-név (nemzeti csapat);
            // hazai ligában a csoport országa érvényes.
            let country = if is_international(&comp_haystack) {
                country_by_english(&base_name)
                    .map(|(cc, _)| cc.to_string())
                    .unwrap_or_default()
            } else {
                group.team_country.clone()
            };
            let gender = if name_women || group.gender == "female" {
                "female"
            } else {
                "male"
            };
            let branch = if gender == "female" {
                "women"
            } else if name_youth || group.branch == "youth" {
                "youth"
            } else {
                "senior"
            };
            team_obs.push(Observation {
                entity_kind: "team".into(),
                aspect: "team_identity".into(),
                natural_key: code.clone(),
                source_record_key: code.clone(),
                observed_at: captured_at,
                payload: serde_json::json!({
                    "id": null,
                    "source_entity_id": code,
                    "name": name,
                    "country_code": country,
                    "gender": gender,
                    "age_group": if branch == "youth" { "youth" } else { "senior" },
                    "branch": branch,
                }),
            });
        }
        team_obs.extend(observations);
        Ok(team_obs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_cover_today_and_yesterday_only() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 15).unwrap();
        assert_eq!(route_for(today, today), Some("/"));
        assert_eq!(
            route_for(today, NaiveDate::from_ymd_opt(2026, 11, 14).unwrap()),
            Some("/tegnap/")
        );
        assert_eq!(
            route_for(today, NaiveDate::from_ymd_opt(2026, 11, 13).unwrap()),
            None
        );
        assert_eq!(
            route_for(today, NaiveDate::from_ymd_opt(2026, 11, 16).unwrap()),
            None
        );
    }

    #[test]
    fn season_label_spans_july_boundary() {
        assert_eq!(
            season_label(NaiveDate::from_ymd_opt(2026, 11, 15).unwrap()),
            "2026/27"
        );
        assert_eq!(
            season_label(NaiveDate::from_ymd_opt(2027, 3, 1).unwrap()),
            "2026/27"
        );
        assert_eq!(
            season_label(NaiveDate::from_ymd_opt(2027, 8, 1).unwrap()),
            "2027/28"
        );
    }

    #[test]
    fn team_codes_take_the_trailing_code() {
        let href =
            "https://hu.soccerway.com/merkozes/belgium-GbB957na/olaszorszag-hlKvieGH/?mid=4YNzMsxg";
        assert_eq!(
            team_codes(href),
            Some(("GbB957na".into(), "hlKvieGH".into()))
        );
    }
}
