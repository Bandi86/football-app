use chrono::TimeZone;
use football_ingest::adapter::{ParseCtx, SourceAdapter};
use football_ingest::fetch::RawPayload;
use football_ingest::footballdata::FootballdataAdapter;
use football_ingest::normalized::NormalizedDay;

fn read_fixture() -> Vec<u8> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/footballdata_e0.csv");
    std::fs::read(path).expect("CSV fixture")
}

fn parse(ctx: ParseCtx) -> Vec<football_ingest::adapter::Observation> {
    let raw = RawPayload {
        bytes: read_fixture(),
        content_type: "text/csv".into(),
        http_status: 200,
        fetched_at: chrono::Utc::now(),
        sha256: String::new(),
    };
    FootballdataAdapter.parse(&raw, &ctx).unwrap()
}

fn ctx(date: &str) -> ParseCtx {
    ParseCtx {
        requested_date: chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
        request_key: format!("footballdata:e0:{date}"),
    }
}

#[test]
fn csv_parses_core_rows_within_window() {
    let observations = parse(ctx("2026-11-21"));
    let matches: Vec<_> = observations
        .iter()
        .filter(|o| o.aspect == "match_core")
        .collect();
    assert_eq!(matches.len(), 2, "a ±7 napos ablakon kívüli meccs kimarad");
    let teams: Vec<_> = observations
        .iter()
        .filter(|o| o.aspect == "team_identity")
        .collect();
    assert_eq!(teams.len(), 4, "a meccsek csapatai");

    let day = NormalizedDay::from_observations(&observations).unwrap();
    assert_eq!(day.matches.len(), 2);
    assert_eq!(day.competition.name, "Premier League");
    assert_eq!(day.competition.country_code, "GB");
    assert_eq!(day.season.label, "2026/27");
    assert_eq!(day.source_code, "footballdata");

    // 2026-11-21 15:00 London (GMT, téli idő) → 15:00 UTC.
    let arsenal = day
        .matches
        .iter()
        .find(|m| m.home_source_entity_id == "arsenal")
        .expect("Arsenal meccs");
    let expected = chrono::Utc
        .with_ymd_and_hms(2026, 11, 21, 15, 0, 0)
        .unwrap();
    assert_eq!(arsenal.kickoff_at, expected);
    assert_eq!(arsenal.status, "finished");
    let result = arsenal.result.as_ref().unwrap();
    assert_eq!((result.home_ft, result.away_ft), (Some(2), Some(1)));
    assert_eq!((result.home_ht, result.away_ht), (Some(1), Some(0)));

    // Odds: B365 opening + closing (1x2), Avg opening, B365 O/U opening+closing.
    let quotes = arsenal.odds.as_ref().expect("odds").quotes.as_slice();
    let home_closing = quotes
        .iter()
        .find(|q| {
            q.bookmaker == "B365"
                && q.market == "1x2"
                && q.outcome == "home"
                && q.phase == "closing"
        })
        .expect("B365 closing home");
    assert!((home_closing.price - 1.72).abs() < 1e-9);
    assert!(
        quotes
            .iter()
            .any(|q| q.market == "ou" && q.outcome == "over" && (q.price - 1.95).abs() < 1e-9)
    );
}

#[test]
fn missing_required_column_is_schema_drift() {
    let bad = b"Div,Date,Time,HomeTeam\nE0,21/11/2026,15:00,Arsenal\n".to_vec();
    let raw = RawPayload {
        bytes: bad,
        content_type: "text/csv".into(),
        http_status: 200,
        fetched_at: chrono::Utc::now(),
        sha256: String::new(),
    };
    let err = FootballdataAdapter
        .parse(&raw, &ctx("2026-11-21"))
        .expect_err("hiányzó oszlop = hiba");
    let msg = format!("{err}");
    assert!(msg.contains("hiányzó CSV-oszlop"), "üzenet: {msg}");
}
