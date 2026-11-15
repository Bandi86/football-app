use chrono::{NaiveDate, TimeZone, Utc};
use football_ingest::adapter::{ParseCtx, SourceAdapter};
use football_ingest::fetch::RawPayload;
use football_ingest::fotmob::FotMobAdapter;
use football_ingest::normalized::NormalizedDay;

fn parse(date: &str) -> Vec<football_ingest::adapter::Observation> {
    let bytes = std::fs::read("tests/fixtures/fotmob_sample.json").unwrap();
    let raw = RawPayload {
        bytes,
        content_type: "application/json".into(),
        http_status: 200,
        fetched_at: Utc::now(),
        sha256: String::new(),
    };
    let adapter = FotMobAdapter {
        leagues: vec![("UEFA Nations League A Grp. 4".into(), "INT".into())],
    };
    adapter
        .parse(
            &raw,
            &ParseCtx {
                requested_date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
                request_key: format!("fotmob:nations:{date}"),
            },
        )
        .unwrap()
}

#[test]
fn parses_selected_league_with_utc_kickoff_and_result() {
    let observations = parse("2026-11-14");
    let day = NormalizedDay::from_observations(&observations).unwrap();
    assert_eq!(day.matches.len(), 2);
    assert_eq!(day.competition.name, "UEFA Nations League A Grp. 4");
    assert_eq!(day.competition.country_code, "INT");
    let first = &day.matches[0];
    assert_eq!(
        first.kickoff_at,
        Utc.with_ymd_and_hms(2026, 11, 14, 17, 0, 0).unwrap()
    );
    assert_eq!(first.status, "finished");
    let result = first.result.as_ref().expect("eredmény");
    assert_eq!((result.home_ft, result.away_ft), (Some(3), Some(1)));
}

#[test]
fn date_guard_rejects_wrong_day() {
    let bytes = std::fs::read("tests/fixtures/fotmob_sample.json").unwrap();
    let raw = RawPayload {
        bytes,
        content_type: "application/json".into(),
        http_status: 200,
        fetched_at: Utc::now(),
        sha256: String::new(),
    };
    let err = FotMobAdapter::default()
        .parse(
            &raw,
            &ParseCtx {
                requested_date: NaiveDate::from_ymd_opt(2026, 11, 13).unwrap(),
                request_key: "fotmob:epl:2026-11-13".into(),
            },
        )
        .expect_err("dátum-guard");
    assert!(format!("{err}").contains("dátum-guard"), "üzenet: {err}");
}
