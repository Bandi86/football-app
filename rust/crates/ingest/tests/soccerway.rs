use chrono::{NaiveDate, Utc};
use football_ingest::adapter::{ParseCtx as Ctx, SourceAdapter};
use football_ingest::fetch::RawPayload;
use football_ingest::soccerway::SoccerwayAdapter;

fn ctx(date: &str) -> Ctx {
    Ctx {
        requested_date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
        request_key: format!("soccerway:anglia/premier-league:{date}"),
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
    format!(
        "<div data-testid=\"wcl-headerLeague\"><a class=\"headerLeague__title\" href=\"/{slug}/\">{slug}</a></div>"
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
    assert_eq!(matches.len(), 5, "csak a cél-liga sorai jönnek");
    let teams: Vec<_> = obs.iter().filter(|o| o.aspect == "team_identity").collect();
    assert_eq!(teams.len(), 10);

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

    assert!(
        matches
            .iter()
            .all(|o| o.source_record_key != "soccerway-fff6")
    );
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
