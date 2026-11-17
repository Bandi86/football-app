use chrono::NaiveDate;
use football_ingest::adapter::{ParseCtx, SourceAdapter};
use football_ingest::fetch::RawPayload;
use football_ingest::openfoot::OpenFootAdapter;

fn raw(bytes: &[u8]) -> RawPayload {
    RawPayload {
        bytes: bytes.to_vec(),
        content_type: "application/json".into(),
        http_status: 200,
        fetched_at: chrono::Utc::now(),
        sha256: String::new(),
    }
}

fn ctx(date: &str, key: &str) -> ParseCtx {
    ParseCtx {
        requested_date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
        request_key: key.into(),
    }
}

const REGISTRY: &str = r#"{"data":[
  {"id":"comp_premier_league_eng","name":"Premier League","country":"England"},
  {"id":"comp_uefa_nations_league","name":"UEFA Nations League","country":"Europe"},
  {"id":"comp_fa_cup_eng","name":"FA Cup","country":"England"}
],"meta":{"count":3}}"#;

const DAY: &str = r#"{"data":[
  {"id":"match_a","competitionId":"comp_premier_league_eng","season":"2026/27","round":"Round 12",
   "kickoffAt":"2026-11-21T15:00:00Z","kickoffTimeKnown":true,"status":"finished",
   "homeTeam":{"id":"team_fd_liverpool","name":"Liverpool","country":"England"},
   "awayTeam":{"id":"team_fd_arsenal","name":"Arsenal","country":"England"},
   "score":{"home":2,"away":1},"period":"FT"},
  {"id":"match_b","competitionId":"comp_premier_league_eng","season":"2026/27","round":"Round 12",
   "kickoffAt":"2026-11-21T17:30:00Z","kickoffTimeKnown":true,"status":"scheduled",
   "homeTeam":{"id":"team_fd_chelsea","name":"Chelsea","country":"England"},
   "awayTeam":{"id":"team_fd_everton","name":"Everton","country":"England"},
   "score":null,"period":null},
  {"id":"match_c","competitionId":"comp_uefa_nations_league","season":"2026/27","round":"Round 6",
   "kickoffAt":"2026-11-21T15:00:00Z","kickoffTimeKnown":true,"status":"finished",
   "homeTeam":{"id":"team_fmb_faroe","name":"Faroe Islands","country":"Europe"},
   "awayTeam":{"id":"team_fmb_moldova","name":"Moldova","country":"Europe"},
   "score":{"home":1,"away":0},"period":"FT"},
  {"id":"match_d","competitionId":"comp_premier_league_eng","season":"2026/27","round":"Round 12",
   "kickoffAt":"2026-11-21T15:00:00Z","kickoffTimeKnown":true,"status":"live",
   "homeTeam":{"id":"team_fd_wolves","name":"Wolves","country":"England"},
   "awayTeam":{"id":"team_fd_fulham","name":"Fulham","country":"England"},
   "score":{"home":0,"away":0},"period":"1H"},
  {"id":"match_e","competitionId":"comp_fa_cup_eng","season":"2026/27","round":"Round 1",
   "kickoffAt":"2026-11-21T13:00:00Z","kickoffTimeKnown":true,"status":"postponed",
   "homeTeam":{"id":"team_fd_hull","name":"Hull","country":"England"},
   "awayTeam":{"id":"team_fd_leeds","name":"Leeds","country":"England"},
   "score":null,"period":null}
],"meta":{"count":5}}"#;

fn adapter_with_registry() -> OpenFootAdapter {
    let adapter = OpenFootAdapter::new(Some("test-key".into()));
    adapter
        .parse(
            &raw(REGISTRY.as_bytes()),
            &ctx("2026-11-21", "openfoot:competitions:2026-11-21"),
        )
        .unwrap();
    adapter
}

#[test]
fn parses_finished_scheduled_live_and_postponed_rows() {
    let adapter = adapter_with_registry();
    let obs = adapter
        .parse(
            &raw(DAY.as_bytes()),
            &ctx("2026-11-21", "openfoot:day:2026-11-21:p1"),
        )
        .unwrap();
    let matches: Vec<_> = obs.iter().filter(|o| o.aspect == "match_core").collect();
    assert_eq!(matches.len(), 5);
    let by = |key: &str| {
        matches
            .iter()
            .find(|o| o.source_record_key == format!("openfoot-{key}"))
            .unwrap()
            .payload
            .clone()
    };

    let a = by("match_a");
    assert_eq!(a["status"], "finished");
    assert_eq!(a["result"]["home_ft"], 2);
    assert_eq!(a["result"]["decided_by"], "ft");
    assert_eq!(a["competition"]["name"], "Premier League");
    assert_eq!(a["competition"]["country_code"], "GB");
    assert_eq!(a["competition"]["priority"], 6);
    assert_eq!(a["round_label"], "Round 12");
    assert_eq!(a["season"]["label"], "2026/27");
    assert_eq!(a["kickoff_precision"], "exact");

    let b = by("match_b");
    assert_eq!(b["status"], "scheduled");
    assert!(b["result"].is_null());

    let c = by("match_c");
    assert_eq!(c["competition"]["name"], "UEFA Nations League");
    assert_eq!(c["competition"]["country_code"], "");
    assert_eq!(c["result"]["home_ft"], 1);

    let d = by("match_d");
    assert_eq!(d["status"], "live");
    assert!(
        d["result"].is_null(),
        "élő részeredmény nem kanonikus eredmény"
    );

    let e = by("match_e");
    assert_eq!(e["status"], "postponed");
    assert_eq!(e["competition"]["tier"], 0);
    assert_eq!(e["competition"]["priority"], 40);

    let team = |code: &str| {
        obs.iter()
            .find(|o| o.aspect == "team_identity" && o.payload["source_entity_id"] == code)
            .unwrap()
            .payload
            .clone()
    };
    assert_eq!(
        team("team_fd_liverpool")["country_code"],
        "GB",
        "hazai liga: csapat-ország"
    );
    assert_eq!(
        team("team_fmb_faroe")["country_code"],
        "FO",
        "nemzetközi: névből"
    );
    assert_eq!(team("team_fmb_moldova")["country_code"], "MD");
}

#[test]
fn date_guard_rejects_other_day() {
    let adapter = adapter_with_registry();
    let err = adapter
        .parse(
            &raw(DAY.as_bytes()),
            &ctx("2026-11-22", "openfoot:day:2026-11-22:p1"),
        )
        .unwrap_err();
    assert!(format!("{err}").contains("dátum-guard"), "{err}");
}

#[test]
fn unknown_competition_falls_back_to_id() {
    let adapter = OpenFootAdapter::new(None);
    let day = r#"{"data":[
      {"id":"match_x","competitionId":"comp_unknown_xx","season":"2026","round":"1",
       "kickoffAt":"2026-11-21T10:00:00Z","kickoffTimeKnown":true,"status":"scheduled",
       "homeTeam":{"id":"t1","name":"A","country":""},
       "awayTeam":{"id":"t2","name":"B","country":""},"score":null}
    ]}"#;
    let obs = adapter
        .parse(
            &raw(day.as_bytes()),
            &ctx("2026-11-21", "openfoot:day:2026-11-21:p1"),
        )
        .unwrap();
    let m = obs.iter().find(|o| o.aspect == "match_core").unwrap();
    assert_eq!(m.payload["competition"]["name"], "comp_unknown_xx");
    assert_eq!(m.payload["competition"]["priority"], 50);
    assert_eq!(m.payload["competition"]["country_code"], "");
}

#[test]
fn plan_requests_registry_and_day_pages_with_token() {
    let adapter = OpenFootAdapter::new(Some("secret".into()));
    let job = football_domain::policy::FetchJob {
        id: uuid::Uuid::now_v7(),
        source_slug: "openfoot".into(),
        domain: "day".into(),
        entity_key: "all".into(),
        event_type: "discovery".into(),
        time_window: "2026-11-17".into(),
        due_at: chrono::Utc::now(),
        priority: 10,
    };
    let plan = adapter.plan(&job);
    assert_eq!(plan.len(), 4, "1 registry + 3 nap-oldal");
    assert!(plan[0].url.contains("/competitions"));
    assert_eq!(plan[0].request_key, "openfoot:competitions:2026-11-17");
    assert_eq!(plan[1].request_key, "openfoot:day:2026-11-17:p1");
    assert!(
        plan.iter()
            .all(|r| r.bearer_token.as_deref() == Some("secret"))
    );
}
