//! S38: a football-research adaptr nem-DB teszjei (terv, napi envelope, odds).

use chrono::{NaiveDate, TimeZone, Utc};
use football_domain::policy::FetchJob;
use football_ingest::adapter::{ParseCtx, SourceAdapter};
use football_ingest::fetch::RawPayload;
use football_ingest::research::ResearchAdapter;
use serde_json::Value;
use uuid::Uuid;

fn raw(bytes: &str) -> RawPayload {
    RawPayload {
        bytes: bytes.as_bytes().to_vec(),
        content_type: "application/json".into(),
        http_status: 200,
        fetched_at: Utc.with_ymd_and_hms(2026, 11, 20, 17, 0, 0).unwrap(),
        sha256: "0".repeat(64),
    }
}

fn ctx(key: &str) -> ParseCtx {
    ParseCtx {
        requested_date: NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
        request_key: key.into(),
    }
}

const DAILY: &str = r#"{
  "meta": { "generated_at": "2026-11-20T16:41:32Z" },
  "data": [
    {
      "uid": "espn:401932578",
      "kickoff": { "utc": "2026-11-20T18:00:00Z", "timezone": "Europe/Budapest" },
      "status": { "code": "finished" },
      "competition": { "name": "Premier League", "country": "ENG", "country_name": "England", "round": "Round 12" },
      "teams": { "home": { "id": "359", "name": "Bayern Munich" },
                 "away": { "id": "132", "name": "1. FC Koln" } },
      "score": { "home": 2, "away": 1, "halftime": { "home": 1, "away": 1 } },
      "referee": { "name": "Dr. Felix Zwayer" },
      "links": { "self": "/api/v1/matches/2026/11/20/bayern.json" },
      "coverage": { "odds": true }
    },
    {
      "uid": "ls1796153",
      "kickoff": { "utc": "2026-11-20T19:30:00Z", "timezone": "Europe/Budapest" },
      "status": { "code": "scheduled" },
      "competition": { "name": "Cymru Premier", "country": "WAL", "country_name": "Wales" },
      "teams": { "home": { "id": "s1", "name": "Caernarfon Town" },
                 "away": { "id": "s2", "name": "Connah's Quay" } },
      "links": { "self": "/api/v1/matches/2026/11/20/caernarfon.json" },
      "coverage": {}
    },
    {
      "uid": "fm:999",
      "kickoff": { "utc": null },
      "status": { "code": "postponed" },
      "competition": { "name": "Regionalliga", "country": "GER", "country_name": "Germany" },
      "teams": { "home": { "id": "r1", "name": "Kickers" },
                 "away": { "id": "r2", "name": "Offenbach" } }
    }
  ]
}"#;

const DETAIL: &str = r#"{
  "uid": "espn:401932578",
  "odds": {
    "phase": "closing",
    "by_bookmaker": { "tipplap": { "home": 2.1 } },
    "markets": {
      "1X2": { "home": 2.1, "draw": 3.4, "away": 3.2 },
      "BTTS": { "yes": 1.8, "no": 2.0 },
      "OU2.5": { "over": 1.9, "under": 1.95 },
      "DC": { "hd": 1.4 }
    }
  }
}"#;

/// S82: egy detail fájl odds + események + csapatstatisztika; a valós
/// forrásból vett mezőnevekkel, ismeretlen extra mezőkkel (schema-drift)
/// és hiányzó/százalékos értékekkel együtt.
const DETAIL_FULL: &str = r#"{
  "uid": "espn:401932578",
  "meta": { "generated_at": "2026-11-20T21:55:00Z" },
  "ismeretlen_gyoker_mezö": { "barmi": [1, 2, 3] },
  "events": [
    { "minute": 12, "type": "Goal", "team": "home", "player": "Harry Kane",
      "player_in": null, "player_out": null,
      "assist": "assist by Joshua Kimmich", "own_goal": false, "penalty": false,
      "score_after": [1, 0], "detail": null, "ismeretlen_esemeny_mezö": 1 },
    { "minute": 34, "type": "Card", "team": "away", "player": "Jan Thielmann",
      "score_after": null, "detail": "Yellow card" },
    { "minute": 46, "type": "Half", "team": null, "player": null,
      "score_after": [1, 1], "detail": null },
    { "minute": 60, "type": "Substitution", "team": "home", "player": "Leroy Sane",
      "player_in": "Kingsley Coman", "player_out": "Leroy Sane",
      "assist": "out: Leroy Sane", "score_after": null },
    { "minute": null, "type": null, "team": "home", "player": "Ghost" },
    { "minute": 88, "type": "Card", "team": "ismeretlen", "player": "X" }
  ],
  "team_statistics": {
    "period": "ALL",
    "groups": [
      { "group": "Top stats", "items": [
        { "title": "Ball Possesion", "key": "BallPossesion", "home": 44, "away": 56, "format": "integer" },
        { "title": "Shots", "key": "total_shots", "home": 16, "away": 8, "format": "integer" },
        { "title": "Shots on target", "key": "ShotsOnTarget", "home": 5, "away": 3, "format": "integer" },
        { "title": "Corners", "key": "corners", "home": 7, "away": 2, "format": "integer" },
        { "title": "Expected goals (xG)", "key": "expected_goals", "home": "1.32", "away": "2.87", "format": "double" }
      ]},
      { "group": "Discipline", "items": [
        { "title": "Yellow cards", "key": "yellow_cards", "home": 2, "away": 3, "format": "integer" },
        { "title": "Red cards", "key": "red_cards", "home": 0, "away": null, "format": "integer" },
        { "title": "Fouls committed", "key": "fouls", "home": 11, "away": 14, "format": "integer" },
        { "title": "Accurate passes", "key": "accurate_passes", "home": "311 (76%)", "away": "280 (68%)", "format": "integerWithPercentage" }
      ]}
    ]
  },
  "odds": {
    "phase": "closing",
    "by_bookmaker": { "tipplap": { "home": 2.1 } },
    "markets": { "1X2": { "home": 2.1, "draw": 3.4, "away": 3.2 } }
  }
}"#;

/// S148: a research éjfél UTC-vel jelölt kezdései valójában csak dátumot
/// hordoznak — `date_only` precízióval kell bejutniuk, hogy a kliens ne
/// mutasson hamis 01:00-as kezdést.
#[test]
fn midnight_kickoff_is_marked_date_only() {
    let daily = r#"{
      "meta": { "generated_at": "2026-11-25T08:00:00Z" },
      "data": [{
        "uid": "fm:1",
        "kickoff": { "utc": "2026-11-25T00:00:00Z", "timezone": "Europe/Budapest" },
        "status": { "code": "scheduled" },
        "competition": { "name": "Paraguayan Primera", "country": "PAR" },
        "teams": { "home": { "id": "1", "name": "Cerro Porteño" },
                   "away": { "id": "2", "name": "Guaraní" } }
      }, {
        "uid": "fm:2",
        "kickoff": { "utc": "2026-11-25T20:00:00Z", "timezone": "Europe/Budapest" },
        "status": { "code": "scheduled" },
        "competition": { "name": "Copa", "country": "PAR" },
        "teams": { "home": { "id": "3", "name": "Libertad" },
                   "away": { "id": "4", "name": "Olimpia" } }
      }]
}"#;
    let adapter = ResearchAdapter::default();
    let observations = adapter.parse(&raw(daily), &ctx("day:2026-11-25")).unwrap();
    let cores: Vec<_> = observations
        .iter()
        .filter(|o| o.aspect == "match_core")
        .collect();
    assert_eq!(cores.len(), 2);
    let precision = |source_key: &str| {
        cores
            .iter()
            .find(|o| o.source_record_key == source_key)
            .unwrap()
            .payload["kickoff_precision"]
            .as_str()
            .unwrap()
    };
    assert_eq!(precision("fm:1"), "date_only");
    assert_eq!(precision("fm:2"), "exact");
}

#[test]
fn day_parse_emits_teams_matches_and_referee() {
    let adapter = ResearchAdapter::default();
    let observations = adapter.parse(&raw(DAILY), &ctx("day:2026-11-20")).unwrap();

    let teams = observations
        .iter()
        .filter(|o| o.aspect == "team_identity")
        .count();
    let matches = observations
        .iter()
        .filter(|o| o.aspect == "match_core")
        .count();
    let referees = observations
        .iter()
        .filter(|o| o.aspect == "match_referee")
        .count();
    // 3 meccs × 2 csapat (a kickoff nélküli meccs csapatai is), 2 match_core
    // (a kickoff nélkülit kihagyjuk), 1 játékvezető.
    assert_eq!(teams, 6, "minden meccs két csapat-identitása");
    assert_eq!(matches, 2, "csak a kickoffal rendelkező meccsek");
    assert_eq!(referees, 1);

    let bayern = observations
        .iter()
        .find(|o| o.aspect == "match_core" && o.natural_key == "espn:401932578")
        .expect("bayern meccs");
    let payload = &bayern.payload;
    assert_eq!(payload["source_home_id"], "espn:359");
    assert_eq!(payload["status"], "finished");
    assert_eq!(payload["result"]["home_ft"], 2);
    assert_eq!(payload["result"]["away_ft"], 1);
    assert_eq!(payload["result"]["home_ht"], 1);
    assert_eq!(payload["competition"]["name"], "Premier League");
    assert_eq!(
        payload["competition"]["country_code"], "GB",
        "ENG → GB (a v2 brit versenyei GB alatt élnek)"
    );
    assert_eq!(
        payload["competition"]["source_entity_id"], "Premier League|GB",
        "a verseny source-ref kulcsa ország-szkópolt (S89)"
    );
    assert_eq!(payload["season"]["label"], "2026/27");
    assert_eq!(payload["round_label"], "Round 12");

    let wales = observations
        .iter()
        .find(|o| o.aspect == "match_core" && o.natural_key == "ls1796153")
        .expect("walesi meccs");
    assert_eq!(
        wales.payload["competition"]["country_code"], "GB",
        "WAL → GB (a v2 brit versenyei GB alatt élnek)"
    );
    assert_eq!(
        wales.payload["competition"]["name"], "Cymru Premier",
        "a walesi Premier League a Cymru Premier (S38b alias)"
    );
    assert!(
        wales.payload["result"].is_null(),
        "scheduled meccsnek nincs eredménye"
    );
}

#[test]
fn odds_parse_maps_markets_outcomes_and_phase() {
    let adapter = ResearchAdapter::default();
    let observations = adapter
        .parse(&raw(DETAIL), &ctx("odds:espn:401932578"))
        .unwrap();
    let odds_obs = observations
        .iter()
        .find(|o| o.aspect == "match_odds")
        .expect("match_odds megfigyelés");
    let payload = &odds_obs.payload;
    let quotes = payload["quotes"].as_array().expect("quotes");
    assert_eq!(quotes.len(), 8, "3 1X2 + 2 BTTS + 2 OU + 1 DC");
    assert_eq!(payload["market_count"], 4);
    assert_eq!(odds_obs.natural_key, "espn:401932578");

    let find = |market: &str, outcome: &str| -> Option<&Value> {
        quotes
            .iter()
            .find(|q| q["market"] == market && q["outcome"] == outcome)
    };
    let home = find("1x2", "home").expect("1x2 home");
    assert_eq!(home["price"], 2.1);
    assert_eq!(home["phase"], "closing");
    assert_eq!(home["bookmaker"], "tipplap");
    let over = find("ou", "over").expect("ou over");
    assert_eq!(over["line"], 2.5);
    assert!(find("btts", "yes").is_some());
    assert!(find("dc", "home_draw").is_some());
}

#[test]
// S82: egy detail job mind a három megfigyelést előállítja, a forrás
// sorrendjét és a hiányzó/ismertelen mezőket tolerálva.
fn detail_parse_emits_events_and_team_statistics() {
    let adapter = ResearchAdapter::default();
    let observations = adapter
        .parse(&raw(DETAIL_FULL), &ctx("detail:espn:401932578"))
        .unwrap();
    assert_eq!(
        observations.len(),
        4,
        "odds + események + statisztika + insight"
    );
    assert!(
        observations.iter().any(|o| o.aspect == "match_insight"),
        "insight observation"
    );

    let by_aspect = |aspect: &str| {
        observations
            .iter()
            .find(|o| o.aspect == aspect)
            .unwrap_or_else(|| panic!("hiányzó {aspect} megfigyelés"))
    };

    let events = by_aspect("match_events");
    assert_eq!(events.natural_key, "espn:401932578");
    assert_eq!(
        events.observed_at,
        Utc.with_ymd_and_hms(2026, 11, 20, 21, 55, 0).unwrap(),
        "a rögzítési idő a detail meta.generated_at-ja"
    );
    let incidents = events.payload["incidents"].as_array().expect("incidents");
    // 6 forrás-esemény, ebből 1 NULL típusú kimarad.
    assert_eq!(incidents.len(), 5);
    assert_eq!(incidents[0]["incident_type"], "Goal");
    assert_eq!(incidents[0]["minute"], 12);
    assert_eq!(incidents[0]["side"], "home");
    assert_eq!(incidents[0]["player_name"], "Harry Kane");
    assert_eq!(
        incidents[0]["related_player_name"], "Joshua Kimmich",
        "az assist 'assist by ' prefixe levágva"
    );
    assert_eq!(incidents[0]["score_after_home"], 1);
    assert_eq!(incidents[0]["score_after_away"], 0);
    // Félidő: nincs csapat → side NULL, a score_after viszont megvan.
    let half = incidents
        .iter()
        .find(|i| i["incident_type"] == "Half")
        .expect("félidő");
    assert!(half["side"].is_null());
    assert_eq!(half["score_after_home"], 1);
    assert_eq!(half["score_after_away"], 1);
    // Csere: a bejövő a player_name, a kimenő a related_player_name.
    let sub = incidents
        .iter()
        .find(|i| i["incident_type"] == "Substitution")
        .expect("csere");
    assert_eq!(sub["player_name"], "Kingsley Coman");
    assert_eq!(sub["related_player_name"], "Leroy Sane");
    assert_eq!(sub["minute"], 60);
    // Ismeretlen oldal → NULL, az esemény nem vész el.
    let unknown_side = incidents.last().expect("utolsó esemény");
    assert_eq!(unknown_side["incident_type"], "Card");
    assert!(unknown_side["side"].is_null());

    let stats = by_aspect("match_stats");
    let payload = &stats.payload;
    assert_eq!(payload["home_shots"], 16);
    assert_eq!(payload["away_shots"], 8);
    assert_eq!(payload["home_shots_on_target"], 5);
    assert_eq!(payload["away_shots_on_target"], 3);
    assert_eq!(payload["home_corners"], 7);
    assert_eq!(payload["away_corners"], 2);
    assert_eq!(payload["home_yellow"], 2);
    assert_eq!(payload["away_yellow"], 3);
    assert_eq!(payload["home_red"], 0, "a 0 valós érték, nem hiányzó adat");
    assert!(payload["away_red"].is_null(), "hiányzó érték → NULL");
    assert_eq!(payload["home_fouls"], 11);
    assert_eq!(payload["away_fouls"], 14);
    assert_eq!(payload["home_xg"], 1.32);
    assert_eq!(payload["away_xg"], 2.87);

    let odds = by_aspect("match_odds");
    assert_eq!(odds.payload["market_count"], 1);
    assert_eq!(odds.payload["quotes"].as_array().expect("quotes").len(), 3);
}

#[test]
// S82: hiányzó mezők nem hibák; a formázott/százalékos értéket nem
// találgatjuk át, és üres statisztikából nem lesz megfigyelés.
fn detail_parse_tolerates_missing_fields_and_empty_stats() {
    let adapter = ResearchAdapter::default();
    let minimal = r#"{
      "uid": "fm:123",
      "events": [ { "type": "Goal" } ],
      "team_statistics": { "groups": [ { "group": "Top stats", "items": [
        { "title": "Corners", "key": "corners", "home": null, "away": 3, "format": "integer" },
        { "title": "Accurate passes", "key": "accurate_passes", "home": "311 (76%)", "away": "280 (68%)", "format": "integerWithPercentage" }
      ] } ] }
    }"#;
    let observations = adapter.parse(&raw(minimal), &ctx("detail:fm:123")).unwrap();
    assert_eq!(observations.len(), 2, "esemény + statisztika, odds nélkül");
    let incident = &observations
        .iter()
        .find(|o| o.aspect == "match_events")
        .expect("esemény")
        .payload["incidents"][0];
    assert_eq!(incident["incident_type"], "Goal");
    assert!(incident["minute"].is_null());
    assert!(incident["side"].is_null());
    assert!(incident["player_name"].is_null());
    assert!(incident["score_after_home"].is_null());

    let stats = &observations
        .iter()
        .find(|o| o.aspect == "match_stats")
        .expect("statisztika")
        .payload;
    assert!(stats["home_corners"].is_null());
    assert_eq!(stats["away_corners"], 3);
    assert!(stats["home_xg"].is_null());
    assert!(
        stats["home_shots"].is_null(),
        "amit a forrás nem ad, az NULL, nem 0"
    );

    // Csak formázott értékek → nincs mit kanonizálni.
    let formatted_only = r#"{
      "team_statistics": { "groups": [ { "group": "Top stats", "items": [
        { "title": "Accurate passes", "key": "accurate_passes", "home": "311 (76%)", "away": "280 (68%)", "format": "integerWithPercentage" },
        { "title": "Expected goals (xG)", "key": "expected_goals", "home": null, "away": null, "format": "double" }
      ] } ] }
    }"#;
    let empty = adapter
        .parse(&raw(formatted_only), &ctx("detail:fm:123"))
        .unwrap();
    assert!(empty.is_empty(), "nincs megfigyelés formázott adatokból");

    // Teljesen üres detail (ismeretlen extra mezőkkel) sem hiba.
    let none = adapter
        .parse(
            &raw(r#"{ "uid": "fm:9", "valami_uj": true }"#),
            &ctx("detail:fm:9"),
        )
        .unwrap();
    assert!(none.is_empty());
}

#[test]
// S82: a `data_tier = enriched` és a `coverage.detail != none` is detail
// jobot kér, akkor is, ha nincs odds; a request_key egységes `detail:<uid>`.
fn plan_requests_detail_for_enriched_or_detail_matches() {
    let daily = r#"{
      "meta": { "generated_at": "2026-11-20T16:41:32Z" },
      "data": [
        { "uid": "espn:1", "kickoff": { "utc": "2026-11-20T18:00:00Z", "timezone": "Europe/Budapest" },
          "status": { "code": "finished" }, "competition": { "name": "Bundesliga", "country": "GER" },
          "teams": { "home": { "id": "1", "name": "A" }, "away": { "id": "2", "name": "B" } },
          "links": { "self": "/api/v1/matches/2026/11/20/a.json" },
          "coverage": { "odds": false, "detail": "ok" }, "data_tier": "fixture_only" },
        { "uid": "espn:2", "kickoff": { "utc": "2026-11-20T19:00:00Z", "timezone": "Europe/Budapest" },
          "status": { "code": "scheduled" }, "competition": { "name": "Bundesliga", "country": "GER" },
          "teams": { "home": { "id": "3", "name": "C" }, "away": { "id": "4", "name": "D" } },
          "links": { "self": "/api/v1/matches/2026/11/20/c.json" },
          "coverage": { "odds": false }, "data_tier": "enriched" },
        { "uid": "espn:3", "kickoff": { "utc": "2026-11-20T20:00:00Z", "timezone": "Europe/Budapest" },
          "status": { "code": "scheduled" }, "competition": { "name": "Bundesliga", "country": "GER" },
          "teams": { "home": { "id": "5", "name": "E" }, "away": { "id": "6", "name": "F" } },
          "links": { "self": "/api/v1/matches/2026/11/20/e.json" },
          "coverage": { "odds": false, "detail": "none" }, "data_tier": "fixture_only" }
      ]
    }"#;
    let dir = std::env::temp_dir().join(format!("research-plan-detail-{}", Uuid::now_v7()));
    let daily_path = dir.join("daily/2026/11/2026-11-20.json");
    std::fs::create_dir_all(daily_path.parent().unwrap()).unwrap();
    std::fs::write(&daily_path, daily).unwrap();
    // S179: a plan csak a létező detail-fájlokat kérdí; a tesztben létre kell hozni őket.
    for name in ["a.json", "c.json"] {
        let detail_path = dir.join(format!("matches/2026/11/20/{name}"));
        std::fs::create_dir_all(detail_path.parent().unwrap()).unwrap();
        std::fs::write(&detail_path, "{}").unwrap();
    }

    let adapter = ResearchAdapter::new(dir.clone());
    let requests = adapter.plan(&job("2026-11-20"));
    // 1 napi + espn:1 (detail ok) + espn:2 (enriched); espn:3 nem kér.
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[1].request_key, "detail:espn:1");
    assert_eq!(requests[2].request_key, "detail:espn:2");
    assert_eq!(requests[1].url, "matches/2026/11/20/a.json");
    assert_eq!(requests[2].url, "matches/2026/11/20/c.json");

    std::fs::remove_dir_all(&dir).ok();
}

fn job(date: &str) -> FetchJob {
    FetchJob {
        id: Uuid::now_v7(),
        source_slug: "research".into(),
        domain: "day".into(),
        entity_key: "all".into(),
        event_type: "discovery".into(),
        time_window: date.into(),
        due_at: Utc.with_ymd_and_hms(2026, 11, 20, 6, 0, 0).unwrap(),
        priority: 0,
    }
}

#[test]
fn plan_requests_day_then_only_detail_matches() {
    let dir = std::env::temp_dir().join(format!("research-plan-{}", Uuid::now_v7()));
    let daily_path = dir.join("daily/2026/11/2026-11-20.json");
    std::fs::create_dir_all(daily_path.parent().unwrap()).unwrap();
    std::fs::write(&daily_path, DAILY).unwrap();
    let detail_path = dir.join("matches/2026/11/20/bayern.json");
    std::fs::create_dir_all(detail_path.parent().unwrap()).unwrap();
    std::fs::write(&detail_path, DETAIL).unwrap();

    let adapter = ResearchAdapter::new(dir.clone());
    let requests = adapter.plan(&job("2026-11-20"));
    assert_eq!(
        requests.len(),
        2,
        "1 napi + 1 detail-os meccs (a WAL meccsnek nincs oddsa/detailje)"
    );
    assert_eq!(requests[0].url, "daily/2026/11/2026-11-20.json");
    assert_eq!(requests[1].url, "matches/2026/11/20/bayern.json");
    assert_eq!(
        requests[1].request_key, "detail:espn:401932578",
        "S82: egységes detail:<uid> kulcs (a régi odds:<uid> parse-kulcs megmarad)"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
// S179: a napi index elavult detail-hivatkozása nem buktathatja az egész
// napi kört — a hiányzó detail kimarad, a többi meccs eredménye betöltődik.
fn plan_skips_missing_detail_file() {
    let daily = r#"{
      "meta": { "generated_at": "2026-11-20T16:41:32Z" },
      "data": [
        { "uid": "espn:1", "kickoff": { "utc": "2026-11-20T18:00:00Z", "timezone": "Europe/Budapest" },
          "status": { "code": "finished" }, "competition": { "name": "Bundesliga", "country": "GER" },
          "teams": { "home": { "id": "1", "name": "A" }, "away": { "id": "2", "name": "B" } },
          "links": { "self": "/api/v1/matches/2026/11/20/a.json" },
          "coverage": { "odds": true } },
        { "uid": "espn:2", "kickoff": { "utc": "2026-11-20T19:00:00Z", "timezone": "Europe/Budapest" },
          "status": { "code": "finished" }, "competition": { "name": "Bundesliga", "country": "GER" },
          "teams": { "home": { "id": "3", "name": "C" }, "away": { "id": "4", "name": "D" } },
          "links": { "self": "/api/v1/matches/2026/11/20/b.json" },
          "coverage": { "odds": true } }
      ]
    }"#;
    let dir = std::env::temp_dir().join(format!("research-plan-missing-{}", Uuid::now_v7()));
    let daily_path = dir.join("daily/2026/11/2026-11-20.json");
    std::fs::create_dir_all(daily_path.parent().unwrap()).unwrap();
    std::fs::write(&daily_path, daily).unwrap();
    let detail_path = dir.join("matches/2026/11/20/b.json");
    std::fs::create_dir_all(detail_path.parent().unwrap()).unwrap();
    std::fs::write(&detail_path, DETAIL).unwrap();

    let adapter = ResearchAdapter::new(dir.clone());
    let requests = adapter.plan(&job("2026-11-20"));
    assert_eq!(
        requests.len(),
        2,
        "1 napi + a létező detail; a hiányzó a.json kimarad"
    );
    assert_eq!(requests[1].url, "matches/2026/11/20/b.json");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn plan_is_empty_for_invalid_window() {
    let adapter = ResearchAdapter::default();
    assert!(adapter.plan(&job("tegnap")).is_empty());
}

#[test]
fn catchup_job_reads_target_day_from_entity_key() {
    let dir = std::env::temp_dir().join(format!("research-catchup-{}", Uuid::now_v7()));
    let daily_path = dir.join("daily/2026/11/2026-11-20.json");
    std::fs::create_dir_all(daily_path.parent().unwrap()).unwrap();
    std::fs::write(&daily_path, DAILY).unwrap();
    let detail_path = dir.join("matches/2026/11/20/bayern.json");
    std::fs::create_dir_all(detail_path.parent().unwrap()).unwrap();
    std::fs::write(&detail_path, DETAIL).unwrap();

    let adapter = ResearchAdapter::new(dir.clone());
    let mut catchup = job("2026-11-24"); // time_window = ma, mint a schedulerben
    catchup.entity_key = "catchup-2026-11-20".into();
    let requests = adapter.plan(&catchup);
    assert!(!requests.is_empty(), "a catchup napnak van fetch-terve");
    assert_eq!(requests[0].url, "daily/2026/11/2026-11-20.json");
    assert_eq!(requests[0].request_key, "day:2026-11-20");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn youth_competition_uses_canonical_youth_age_group() {
    // A forrás U17/U19 tokenjei nem lehetnek age_group értékek: a kanonikus
    // szótár csak `senior`/`youth`, különben párhuzamos entitások születnek.
    let daily = r#"{
      "meta": { "generated_at": "2026-11-21T08:00:00Z" },
      "data": [{
        "uid": "fm:6139560",
        "kickoff": { "utc": "2026-11-21T13:00:00Z", "timezone": "Europe/Budapest" },
        "status": { "code": "scheduled" },
        "competition": { "name": "World Cup U17 - Group I", "country": "INT", "country_name": "World" },
        "teams": { "home": { "id": "6585", "name": "Spain U17" },
                   "away": { "id": "7476", "name": "Costa Rica U17" } }
      }]
    }"#;
    let adapter = ResearchAdapter::default();
    let observations = adapter.parse(&raw(daily), &ctx("day:2026-11-21")).unwrap();
    for team in observations.iter().filter(|o| o.aspect == "team_identity") {
        assert_eq!(
            team.payload["age_group"], "youth",
            "{}",
            team.payload["name"]
        );
        assert_eq!(team.payload["branch"], "youth");
    }
}

#[test]
fn numeric_team_ids_are_accepted_and_prefixed() {
    let daily = r#"{
      "meta": { "generated_at": "2026-11-20T16:41:32Z" },
      "data": [{
        "uid": "fm:6580833",
        "kickoff": { "utc": "2026-11-20T23:00:00Z", "timezone": "America/New_York" },
        "status": { "code": "scheduled" },
        "competition": { "name": "Major League Soccer Playoff", "country": "USA", "country_name": "USA" },
        "teams": { "home": { "id": 267810, "name": "Orlando City" },
                   "away": { "id": 722265, "name": "FC Cincinnati" } }
      }]
    }"#;
    let adapter = ResearchAdapter::default();
    let observations = adapter.parse(&raw(daily), &ctx("day:2026-11-20")).unwrap();
    let teams: Vec<_> = observations
        .iter()
        .filter(|o| o.aspect == "team_identity")
        .collect();
    assert_eq!(teams.len(), 2);
    let home = teams
        .iter()
        .find(|o| o.payload["name"] == "Orlando City")
        .expect("hazai csapat");
    assert_eq!(home.payload["source_entity_id"], "fm:267810");
    let m = observations
        .iter()
        .find(|o| o.aspect == "match_core")
        .expect("meccs");
    assert_eq!(m.payload["competition"]["country_code"], "US");
    assert_eq!(m.payload["source_home_id"], "fm:267810");
}

/// S90: venue + weather + ismeretlen kulcsok; a második meccsnek nincs
/// környezet-blokkja, arra nem szabad observationt adni.
const ENV_DAILY: &str = r#"{
  "meta": { "generated_at": "2026-11-21T00:30:00Z" },
  "data": [
    {
      "uid": "espn:401932578",
      "kickoff": { "utc": "2026-11-21T18:00:00Z", "timezone": "Europe/Budapest" },
      "status": { "code": "scheduled" },
      "competition": { "name": "Premier League", "country": "ENG", "country_name": "England" },
      "teams": { "home": { "id": "359", "name": "Bayern Munich" },
                 "away": { "id": "132", "name": "1. FC Koln" } },
      "venue": {
        "name": "Estadio Ricardo Gregor",
        "city": "Luque",
        "country": "Paraguay",
        "capacity": 4000,
        "surface": "grass",
        "coordinates": { "lat": -25.275764744, "long": -57.551740408 },
        "ismeretlen_mezo": "drift"
      },
      "weather": {
        "temperature_c": 27,
        "unit": "celsius",
        "description": "Thunder",
        "wind_speed": 2,
        "humidity_pct": 86,
        "precipitation": 0,
        "cloud_cover_pct": 54,
        "observed_at": "2026-11-21T00:30:00Z",
        "source": "fotmob"
      }
    },
    {
      "uid": "ls1796153",
      "kickoff": { "utc": "2026-11-21T19:30:00Z", "timezone": "Europe/Budapest" },
      "status": { "code": "scheduled" },
      "competition": { "name": "Cymru Premier", "country": "WAL", "country_name": "Wales" },
      "teams": { "home": { "id": "s1", "name": "Caernarfon Town" },
                 "away": { "id": "s2", "name": "Connah's Quay" } }
    }
  ]
}"#;

#[test]
fn referee_observation_key_is_the_bare_match_record_key() {
    let adapter = ResearchAdapter::default();
    let observations = adapter.parse(&raw(DAILY), &ctx("day:2026-11-20")).unwrap();

    let referee = observations
        .iter()
        .find(|o| o.aspect == "match_referee")
        .expect("játékvezető observation");
    // B: a normalized a referee-t a meccs `source_match_id`-jével párosítja,
    // ezért a kulcs a bare uid — a korábbi "{uid}:referee" soha nem talált.
    assert_eq!(referee.source_record_key, "espn:401932578");
    assert_eq!(referee.natural_key, "espn:401932578");
    assert_eq!(
        referee.source_record_key, referee.natural_key,
        "a referee kulcsa pontosan a meccs rekord-kulcsa"
    );
    assert_eq!(referee.payload["name"], "Dr. Felix Zwayer");
}

#[test]
fn day_parse_emits_match_environment_with_contract_field_names() {
    let adapter = ResearchAdapter::default();
    let observations = adapter
        .parse(&raw(ENV_DAILY), &ctx("day:2026-11-21"))
        .unwrap();

    let environments: Vec<_> = observations
        .iter()
        .filter(|o| o.aspect == "match_environment")
        .collect();
    assert_eq!(
        environments.len(),
        1,
        "csak a venue/weather blokkot adó meccsre keletkezik observation"
    );

    let environment = environments[0];
    assert_eq!(environment.entity_kind, "match");
    assert_eq!(environment.natural_key, "espn:401932578");
    assert_eq!(
        environment.source_record_key, "espn:401932578",
        "ugyanaz a kulcs, mint a match_core-nál (a normalized ezzel párosít)"
    );

    // A payload pontosan az API-kontraktus mezőit tartalmazza.
    let payload = environment
        .payload
        .as_object()
        .expect("a környezet payload objektum");
    let mut keys: Vec<&str> = payload.keys().map(String::as_str).collect();
    keys.sort_unstable();
    let mut expected = vec![
        "venue_name",
        "venue_city",
        "venue_country",
        "venue_capacity",
        "venue_surface",
        "venue_lat",
        "venue_lon",
        "temperature_c",
        "weather_description",
        "wind_speed",
        "wind_direction",
        "humidity_pct",
        "precipitation",
        "cloud_cover_pct",
        "weather_observed_at",
    ];
    expected.sort_unstable();
    assert_eq!(keys, expected);

    assert_eq!(environment.payload["venue_name"], "Estadio Ricardo Gregor");
    assert_eq!(environment.payload["venue_city"], "Luque");
    assert_eq!(environment.payload["venue_country"], "Paraguay");
    assert_eq!(environment.payload["venue_capacity"], 4000);
    assert_eq!(environment.payload["venue_surface"], "grass");
    assert_eq!(
        environment.payload["venue_lat"].as_f64(),
        Some(-25.275764744)
    );
    assert_eq!(
        environment.payload["venue_lon"].as_f64(),
        Some(-57.551740408)
    );
    assert_eq!(environment.payload["temperature_c"].as_f64(), Some(27.0));
    assert_eq!(environment.payload["weather_description"], "Thunder");
    assert_eq!(environment.payload["wind_speed"].as_f64(), Some(2.0));
    assert_eq!(environment.payload["humidity_pct"], 86);
    assert_eq!(environment.payload["precipitation"].as_f64(), Some(0.0));
    assert_eq!(environment.payload["cloud_cover_pct"], 54);
    assert_eq!(
        environment.payload["weather_observed_at"],
        "2026-11-21T00:30:00Z"
    );
    // Hiányzó forrás-mező → null, soha nem 0.
    assert!(
        environment.payload["wind_direction"].is_null(),
        "a hiányzó szélirány null, nem üres string/0"
    );
}
