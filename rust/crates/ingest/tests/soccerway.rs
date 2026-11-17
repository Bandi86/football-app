use chrono::{NaiveDate, Utc};
use football_ingest::adapter::{ParseCtx as Ctx, SourceAdapter};
use football_ingest::fetch::RawPayload;
use football_ingest::soccerway::SoccerwayAdapter;

fn ctx(date: &str) -> Ctx {
    Ctx {
        requested_date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
        request_key: format!("soccerway:day:{date}"),
    }
}

fn raw(html: &str) -> RawPayload {
    RawPayload {
        bytes: html.as_bytes().to_vec(),
        content_type: "text/html".into(),
        http_status: 200,
        fetched_at: Utc::now(),
        sha256: String::new(),
    }
}

#[allow(clippy::too_many_arguments)] // teszt-fixture segédfüggvény
fn row(
    mid: &str,
    classes: &str,
    home_slug: &str,
    home_code: &str,
    home: &str,
    away_slug: &str,
    away_code: &str,
    away: &str,
    stage: Option<&str>,
    time: Option<&str>,
    scores: Option<(&str, &str)>,
    pens: Option<(&str, &str)>,
) -> String {
    let score_html = match scores {
        Some((h, a)) => {
            let pens_html = |p: Option<&str>| match p {
                Some(value) => format!("<sup class=\"wcl-penaltyScore\">({value})</sup>"),
                None => String::new(),
            };
            format!(
                "<span class=\"event__score\" data-testid=\"wcl-tableScore\" data-side=\"1\">{h}{}</span><span class=\"event__score\" data-testid=\"wcl-tableScore\" data-side=\"2\">{a}{}</span>",
                pens_html(pens.map(|p| p.0)),
                pens_html(pens.map(|p| p.1))
            )
        }
        None => String::new(),
    };
    let stage_html = match stage {
        Some(text) => format!(
            "<div class=\"event__stage\"><div class=\"event__stage--block\">{text}</div></div>"
        ),
        None => String::new(),
    };
    let time_html = match time {
        Some(t) => format!("<div class=\"event__time\">{t}</div>"),
        None => String::new(),
    };
    format!(
        "<div id=\"g_1_{mid}\" class=\"event__match event__match--withRowLink {classes}\" data-event-row=\"true\">\
<a class=\"eventRowLink\" href=\"https://hu.soccerway.com/merkozes/{home_slug}-{home_code}/{away_slug}-{away_code}/?mid={mid}\" aria-label=\"{home} - {away}\"></a>\
{stage_html}{time_html}\
<div class=\"event__homeParticipant\" data-testid=\"wcl-matchRow-participant\"><span data-testid=\"wcl-simple-text-01\">{home}</span></div>\
<div class=\"event__awayParticipant\" data-testid=\"wcl-matchRow-participant\"><span data-testid=\"wcl-simple-text-01\">{away}</span></div>\
{score_html}</div>"
    )
}

fn header(slug: &str) -> String {
    header_full(slug, "", "")
}

fn header_full(slug: &str, title: &str, flag_title: &str) -> String {
    let name = if title.is_empty() { slug } else { title };
    format!(
        "<div data-testid=\"wcl-headerLeague\"><span class=\"headerLeague__flag\" title=\"{flag_title}\"></span><a class=\"headerLeague__title\" href=\"/{slug}/\">{name}</a></div>"
    )
}

fn page(body: &str) -> String {
    format!(
        "<html><head><title>t</title></head><body>\
<button data-testid=\"wcl-dayPickerButton\" aria-label=\"Saturday, November 14, 2026\">Ma</button>\
{body}</body></html>"
    )
}

#[test]
fn parses_target_league_rows_with_statuses_and_results() {
    let body = format!(
        "{}{}{}{}{}{}{}",
        header("anglia/premier-league"),
        row(
            "aaa1",
            "event__match--scheduled",
            "arsenal",
            "CodeA001",
            "Arsenal",
            "chelsea",
            "CodeB002",
            "Chelsea",
            None,
            Some("15:00"),
            None,
            None
        ),
        row(
            "bbb2",
            "",
            "liverpool",
            "CodeC003",
            "Liverpool",
            "everton",
            "CodeD004",
            "Everton",
            Some("Vége"),
            None,
            Some(("2", "1")),
            None
        ),
        row(
            "ccc3",
            "",
            "brighton",
            "CodeE005",
            "Brighton",
            "hull",
            "CodeF006",
            "Hull",
            Some("Hossz. után"),
            None,
            Some(("1", "1")),
            None
        ),
        row(
            "ddd4",
            "",
            "bedford",
            "CodeG007",
            "Bedford",
            "chesham",
            "CodeH008",
            "Chesham",
            Some("Büntetők után"),
            None,
            Some(("1", "1")),
            Some(("4", "5"))
        ),
        row(
            "eee5",
            "",
            "bromley",
            "CodeI009",
            "Bromley",
            "notts",
            "CodeJ010",
            "Notts County",
            Some("Elhalasztva"),
            None,
            None,
            None
        ),
        header("spanyolorszag/laliga"),
    );
    let body = format!(
        "{}{}",
        body,
        row(
            "fff6",
            "",
            "madrid",
            "CodeK011",
            "Madrid",
            "barca",
            "CodeL012",
            "Barcelona",
            Some("Vége"),
            None,
            Some(("3", "0")),
            None
        )
    );

    let adapter = SoccerwayAdapter::default();
    let obs = adapter
        .parse(&raw(&page(&body)), &ctx("2026-11-14"))
        .unwrap();

    let matches: Vec<_> = obs.iter().filter(|o| o.aspect == "match_core").collect();
    assert_eq!(matches.len(), 6, "minden verseny sorai jönnek");
    let teams: Vec<_> = obs.iter().filter(|o| o.aspect == "team_identity").collect();
    assert_eq!(teams.len(), 12);

    let by_key = |key: &str| {
        matches
            .iter()
            .find(|o| o.source_record_key == format!("soccerway-{key}"))
            .unwrap()
            .payload
            .clone()
    };

    let scheduled = by_key("aaa1");
    assert_eq!(scheduled["status"], "scheduled");
    assert_eq!(scheduled["kickoff_precision"], "exact");
    assert_eq!(scheduled["kickoff_at"], "2026-11-14T15:00:00Z");
    assert!(scheduled["result"].is_null());
    assert_eq!(scheduled["source_home_id"], "CodeA001");
    assert_eq!(scheduled["source_away_id"], "CodeB002");
    assert_eq!(scheduled["competition"]["country_code"], "GB");
    assert_eq!(scheduled["season"]["label"], "2026/27");

    let finished = by_key("bbb2");
    assert_eq!(finished["status"], "finished");
    assert_eq!(finished["result"]["home_ft"], 2);
    assert_eq!(finished["result"]["away_ft"], 1);
    assert_eq!(finished["result"]["decided_by"], "ft");
    assert!(finished["result"]["home_et"].is_null());
    assert_eq!(finished["kickoff_precision"], "date_only");
    assert_eq!(finished["kickoff_at"], "2026-11-14T12:00:00Z");

    let aet = by_key("ccc3");
    assert_eq!(aet["result"]["decided_by"], "et");
    assert!(aet["result"]["home_ft"].is_null());
    assert_eq!(aet["result"]["home_et"], 1);

    let pens = by_key("ddd4");
    assert_eq!(pens["result"]["decided_by"], "pens");
    assert_eq!(pens["result"]["home_pens"], 4);
    assert_eq!(pens["result"]["away_pens"], 5);
    assert!(pens["result"]["home_ft"].is_null());
    assert_eq!(pens["result"]["home_et"], 1);

    let postponed = by_key("eee5");
    assert_eq!(postponed["status"], "postponed");
    assert!(postponed["result"].is_null());

    let foreign = by_key("fff6");
    assert_eq!(foreign["competition"]["name"], "La Liga");
    assert_eq!(foreign["competition"]["country_code"], "ES");
}

#[test]
fn date_guard_rejects_other_day() {
    let adapter = SoccerwayAdapter::default();
    let err = adapter
        .parse(&raw(&page("")), &ctx("2026-11-13"))
        .unwrap_err();
    assert!(format!("{err}").contains("dátum-guard"), "{err}");
}

#[test]
fn decoy_page_is_detected() {
    let adapter = SoccerwayAdapter::default();
    let html = "<html><body><p>Error: The requested page can't be displayed. \
                Please try again later.</p><script>dataLayer.push({'type': '404_page'})</script>\
                </body></html>";
    let err = adapter.parse(&raw(html), &ctx("2026-11-14")).unwrap_err();
    assert!(format!("{err}").contains("decoy"), "{err}");
}

#[test]
fn unknown_stage_with_scores_is_finished_without_decider() {
    let body = format!(
        "{}{}",
        header("anglia/premier-league"),
        row(
            "ggg7",
            "",
            "wolves",
            "CodeM013",
            "Wolves",
            "fulham",
            "CodeN014",
            "Fulham",
            Some("Ismeretlen állapot"),
            None,
            Some(("0", "0")),
            None
        )
    );
    let adapter = SoccerwayAdapter::default();
    let obs = adapter
        .parse(&raw(&page(&body)), &ctx("2026-11-14"))
        .unwrap();
    let m = obs
        .iter()
        .find(|o| o.aspect == "match_core")
        .unwrap()
        .payload
        .clone();
    assert_eq!(m["status"], "finished");
    assert!(m["result"]["decided_by"].is_null());
    assert_eq!(m["result"]["home_ft"], 0);
}

/// S9: ismeretlen, női és kupa csoportok osztályozása.
#[test]
fn classifies_unknown_women_and_cup_groups() {
    let body = format!(
        "{}{}{}{}{}{}",
        header_full("valahol/liga-x", "Liga X", "Spanyolország"),
        row(
            "x1",
            "",
            "a",
            "CodeX001",
            "A",
            "b",
            "CodeX002",
            "B",
            Some("Vége"),
            None,
            Some(("1", "0")),
            None
        ),
        header_full("magyarorszag/noi-liga", "Női Liga", "Magyarország"),
        row(
            "w1",
            "",
            "c",
            "CodeW001",
            "C",
            "d",
            "CodeW002",
            "D",
            None,
            Some("18:00"),
            None,
            None
        ),
        header_full("magyarorszag/magyar-kupa", "Magyar Kupa", "Magyarország"),
        row(
            "c1",
            "",
            "e",
            "CodeK001",
            "E",
            "f",
            "CodeK002",
            "F",
            None,
            Some("19:00"),
            None,
            None
        ),
    );
    let adapter = SoccerwayAdapter::default();
    let obs = adapter
        .parse(&raw(&page(&body)), &ctx("2026-11-14"))
        .unwrap();
    let matches: Vec<_> = obs.iter().filter(|o| o.aspect == "match_core").collect();
    assert_eq!(matches.len(), 3, "minden csoport bekerül");
    let by_key = |key: &str| {
        matches
            .iter()
            .find(|o| o.source_record_key == format!("soccerway-{key}"))
            .unwrap()
            .payload
            .clone()
    };
    let unknown = by_key("x1");
    assert_eq!(unknown["competition"]["name"], "Liga X");
    assert_eq!(unknown["competition"]["country_code"], "ES");
    assert_eq!(unknown["competition"]["tier"], 1);
    assert_eq!(unknown["competition"]["priority"], 50);

    let women = by_key("w1");
    assert_eq!(women["competition"]["priority"], 60);
    let women_team = obs
        .iter()
        .find(|o| o.aspect == "team_identity" && o.payload["source_entity_id"] == "CodeW001")
        .unwrap();
    assert_eq!(women_team.payload["gender"], "female");
    assert_eq!(women_team.payload["branch"], "women");
    assert_eq!(women_team.payload["country_code"], "HU");

    let cup = by_key("c1");
    assert_eq!(cup["competition"]["tier"], 0);
    assert_eq!(cup["competition"]["priority"], 40);
    assert_eq!(cup["competition"]["country_code"], "HU");
    let cup_team = obs
        .iter()
        .find(|o| o.aspect == "team_identity" && o.payload["source_entity_id"] == "CodeK001")
        .unwrap();
    assert_eq!(
        cup_team.payload["country_code"], "HU",
        "hazai kupa: a csapat-ország a verseny országa"
    );
}

/// S9: nemzetközi meccseknél a csapatnév ország; az U/W tokenek
/// korosztályt/nemet adnak.
#[test]
fn international_teams_get_country_and_youth_gender_hints() {
    let body = format!(
        "{}{}{}",
        header_full(
            "europa/friendly-international",
            "Friendly International",
            "Európa"
        ),
        row(
            "i1",
            "",
            "serbia",
            "CodeI001",
            "Serbia",
            "belgium-u21",
            "CodeI002",
            "Belgium U21",
            None,
            Some("18:00"),
            None,
            None
        ),
        row(
            "i2",
            "",
            "spain-u17-w",
            "CodeI003",
            "Spain U17 W",
            "france-u17-w",
            "CodeI004",
            "France U17 W",
            None,
            Some("19:00"),
            None,
            None
        ),
    );
    let adapter = SoccerwayAdapter::default();
    let obs = adapter
        .parse(&raw(&page(&body)), &ctx("2026-11-14"))
        .unwrap();
    let team = |code: &str| {
        obs.iter()
            .find(|o| o.aspect == "team_identity" && o.payload["source_entity_id"] == code)
            .unwrap()
            .payload
            .clone()
    };
    let serbia = team("CodeI001");
    assert_eq!(serbia["country_code"], "RS");
    assert_eq!(serbia["gender"], "male");
    assert_eq!(serbia["branch"], "senior");

    let belgium = team("CodeI002");
    assert_eq!(belgium["country_code"], "BE");
    assert_eq!(belgium["branch"], "youth");
    assert_eq!(belgium["age_group"], "youth");

    let spain = team("CodeI003");
    assert_eq!(spain["country_code"], "ES");
    assert_eq!(spain["gender"], "female");
    assert_eq!(spain["branch"], "women");
}

/// S9: angol zászló-nevek országot adnak; a kontinens-zászló nemzetközi
/// versenyt jelez (a csapatnév ország).
#[test]
fn english_flags_and_continent_flags_classify_teams() {
    let body = format!(
        "{}{}{}{}",
        header_full("peru/liga-1", "Liga 1 - Clausura", "Peru"),
        row(
            "p1",
            "",
            "cajamarca",
            "CodeP001",
            "Cajamarca",
            "cusco",
            "CodeP002",
            "Cusco",
            None,
            Some("20:00"),
            None,
            None
        ),
        header_full("europa/elite-league-u20", "Elite League U20", "Europe"),
        row(
            "e1",
            "",
            "czech-u20",
            "CodeE001",
            "Czech Republic U20",
            "england-u20",
            "CodeE002",
            "England U20",
            None,
            Some("18:00"),
            None,
            None
        ),
    );
    let adapter = SoccerwayAdapter::default();
    let obs = adapter
        .parse(&raw(&page(&body)), &ctx("2026-11-14"))
        .unwrap();
    let match_payload = |key: &str| {
        obs.iter()
            .find(|o| o.aspect == "match_core" && o.source_record_key == format!("soccerway-{key}"))
            .unwrap()
            .payload
            .clone()
    };
    let team = |code: &str| {
        obs.iter()
            .find(|o| o.aspect == "team_identity" && o.payload["source_entity_id"] == code)
            .unwrap()
            .payload
            .clone()
    };

    let peru = match_payload("p1");
    assert_eq!(peru["competition"]["country_code"], "PE");
    assert_eq!(team("CodeP001")["country_code"], "PE");

    let czech = team("CodeE001");
    assert_eq!(czech["country_code"], "CZ");
    assert_eq!(czech["branch"], "youth");
    let england = team("CodeE002");
    assert_eq!(england["country_code"], "GB");
}

/// Locale-flip: a magyar oldal nemzeti csapatait és versenyeit angol
/// kanonikus névre képezzük (stabil entitások, nincs duplikáció).
#[test]
fn hungarian_locale_is_normalized_to_canonical_english() {
    let body = format!(
        "{}{}{}{}",
        header_full(
            "europa/uefa-nations-league",
            "UEFA Nemzetek Ligája - A-Liga",
            "Európa"
        ),
        row(
            "h1",
            "",
            "dania",
            "CodeH001",
            "Dánia",
            "norvegia",
            "CodeH002",
            "Norvégia",
            None,
            Some("20:45"),
            None,
            None
        ),
        header_full(
            "vilag/felkeszulesi-merkozes",
            "Felkészülési mérkőzés",
            "Világ"
        ),
        row(
            "h2",
            "",
            "belgium-u21",
            "CodeH003",
            "Belgium U21",
            "spanyolorszag-u17-w",
            "CodeH004",
            "Spanyolország U17 W",
            None,
            Some("18:00"),
            None,
            None
        ),
    );
    let adapter = SoccerwayAdapter::default();
    let obs = adapter
        .parse(&raw(&page(&body)), &ctx("2026-11-14"))
        .unwrap();
    let m = |key: &str| {
        obs.iter()
            .find(|o| o.aspect == "match_core" && o.source_record_key == format!("soccerway-{key}"))
            .unwrap()
            .payload
            .clone()
    };
    assert_eq!(
        m("h1")["competition"]["name"],
        "UEFA Nations League - League A"
    );
    assert_eq!(m("h2")["competition"]["name"], "Friendly International");

    let team = |code: &str| {
        obs.iter()
            .find(|o| o.aspect == "team_identity" && o.payload["source_entity_id"] == code)
            .unwrap()
            .payload
            .clone()
    };
    assert_eq!(team("CodeH001")["name"], "Denmark");
    assert_eq!(team("CodeH001")["country_code"], "DK");
    assert_eq!(team("CodeH002")["name"], "Norway");
    assert_eq!(team("CodeH003")["name"], "Belgium U21");
    assert_eq!(team("CodeH003")["country_code"], "BE");
    assert_eq!(team("CodeH004")["name"], "Spain U17 W");
    assert_eq!(team("CodeH004")["gender"], "female");
}

#[test]
fn flipped_display_row_keeps_slug_orientation_and_swaps_scores() {
    // A meccs-URL slugja a kanonikus sorrend (iran - iraq); a megjelenített
    // sor fordítva áll — a kód↔név páros és a pontszámok nem keveredhetnek.
    let body = format!(
        "{}\
<div id=\"g_1_flip1\" class=\"event__match event__match--withRowLink event__match--twoLine\" data-event-row=\"true\">\
<a class=\"eventRowLink\" href=\"https://www.soccerway.com/match/iran-CodeIR01/iraq-CodeIQ02/?mid=flip1\" aria-label=\"Iraq - Iran\"></a>\
<div class=\"event__homeParticipant\" data-testid=\"wcl-matchRow-participant\"><span data-testid=\"wcl-simple-text-01\">Iraq</span></div>\
<div class=\"event__awayParticipant\" data-testid=\"wcl-matchRow-participant\"><span data-testid=\"wcl-simple-text-01\">Iran</span></div>\
<span class=\"event__score\" data-testid=\"wcl-tableScore\" data-side=\"1\">1</span>\
<span class=\"event__score\" data-testid=\"wcl-tableScore\" data-side=\"2\">2</span></div>",
        header("svajc/super-league")
    );
    let obs = SoccerwayAdapter::default()
        .parse(&raw(&page(&body)), &ctx("2026-11-14"))
        .unwrap();
    let core = obs
        .iter()
        .find(|o| o.aspect == "match_core")
        .expect("match_core");
    assert_eq!(core.payload["source_home_id"], "CodeIR01");
    assert_eq!(core.payload["source_away_id"], "CodeIQ02");
    // A kijelzés szerinti 1–2 (Iraq–Iran) a slug-sorrendre fordítva: Iran 2 – Iraq 1.
    assert_eq!(core.payload["result"]["home_ft"], 2);
    assert_eq!(core.payload["result"]["away_ft"], 1);

    let teams: Vec<_> = obs.iter().filter(|o| o.aspect == "team_identity").collect();
    let iran = teams
        .iter()
        .find(|t| t.payload["source_entity_id"] == "CodeIR01")
        .expect("iran team");
    assert_eq!(iran.payload["name"], "Iran");
    let iraq = teams
        .iter()
        .find(|t| t.payload["source_entity_id"] == "CodeIQ02")
        .expect("iraq team");
    assert_eq!(iraq.payload["name"], "Iraq");
}
