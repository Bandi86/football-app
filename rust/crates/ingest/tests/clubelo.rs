use chrono::NaiveDate;
use football_ingest::adapter::{ParseCtx, SourceAdapter};
use football_ingest::clubelo::ClubEloAdapter;
use football_ingest::fetch::RawPayload;
use football_ingest::normalized::ClubEloDay;

fn payload(path: &str) -> RawPayload {
    let bytes = std::fs::read(path).expect("CSV fixture");
    RawPayload {
        bytes,
        content_type: "text/csv".into(),
        http_status: 200,
        fetched_at: chrono::Utc::now(),
        sha256: String::new(),
    }
}

fn ctx(date: &str) -> ParseCtx {
    ParseCtx {
        requested_date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
        request_key: format!("clubelo:elo:{date}"),
    }
}

#[test]
fn csv_parses_rows_and_builds_elo_day() {
    let raw = payload("tests/fixtures/clubelo_2026-11-14.csv");
    let observations = ClubEloAdapter.parse(&raw, &ctx("2026-11-14")).unwrap();
    assert_eq!(observations.len(), 3);
    let day = ClubEloDay::from_observations(&observations).unwrap();
    assert_eq!(day.as_of, NaiveDate::from_ymd_opt(2026, 11, 14).unwrap());
    assert_eq!(day.rows[0].club_name, "Barcelona");
    assert_eq!(day.rows[0].provenance, "observed");
    assert!(day.rows.iter().all(|r| r.provenance == "observed"));
}

#[test]
fn pre_boundary_dates_are_provisional() {
    let raw = payload("tests/fixtures/clubelo_2026-11-14.csv");
    let observations = ClubEloAdapter.parse(&raw, &ctx("2025-05-12")).unwrap();
    let day = ClubEloDay::from_observations(&observations).unwrap();
    assert!(day.rows.iter().all(|r| r.provenance == "provisional"));
}
