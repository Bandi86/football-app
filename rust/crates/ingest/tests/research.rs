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
    assert_eq!(payload["competition"]["country_code"], "ENG");
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
    assert_eq!(observations.len(), 1);
    let payload = &observations[0].payload;
    let quotes = payload["quotes"].as_array().expect("quotes");
    assert_eq!(quotes.len(), 8, "3 1X2 + 2 BTTS + 2 OU + 1 DC");
    assert_eq!(payload["market_count"], 4);
    assert_eq!(observations[0].natural_key, "espn:401932578");

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
fn plan_requests_day_then_only_odds_matches() {
    let dir = std::env::temp_dir().join(format!("research-plan-{}", Uuid::now_v7()));
    let daily_path = dir.join("daily/2026/11/2026-11-20.json");
    std::fs::create_dir_all(daily_path.parent().unwrap()).unwrap();
    std::fs::write(&daily_path, DAILY).unwrap();

    let adapter = ResearchAdapter::new(dir.clone());
    let requests = adapter.plan(&job("2026-11-20"));
    assert_eq!(
        requests.len(),
        2,
        "1 napi + 1 oddsos meccs (a WAL meccsnek nincs oddsa)"
    );
    assert_eq!(requests[0].url, "daily/2026/11/2026-11-20.json");
    assert_eq!(requests[1].url, "matches/2026/11/20/bayern.json");
    assert_eq!(requests[1].request_key, "odds:espn:401932578");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn plan_is_empty_for_invalid_window() {
    let adapter = ResearchAdapter::default();
    assert!(adapter.plan(&job("tegnap")).is_empty());
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
