use chrono::{NaiveDate, Utc};
use football_domain::error::ParseError;
use football_domain::policy::{FetchJob, Transport};
use football_ingest::adapter::{ParseCtx, SourceAdapter};
use football_ingest::demo::DemoAdapter;
use football_ingest::fetch::RawPayload;
use sha2::Digest;
use uuid::Uuid;

fn raw_from(name: &str) -> RawPayload {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/worker/")
        .join(name);
    let bytes = std::fs::read(path).unwrap();
    let sha256 = hex::encode(sha2::Sha256::digest(&bytes));
    RawPayload {
        bytes,
        content_type: "application/json".into(),
        http_status: 200,
        fetched_at: Utc::now(),
        sha256,
    }
}

fn ctx() -> ParseCtx {
    ParseCtx {
        requested_date: NaiveDate::from_ymd_opt(2026, 11, 12).unwrap(),
        request_key: "demo-synthetic:match:2026-11-12".into(),
    }
}

fn job() -> FetchJob {
    FetchJob {
        id: Uuid::now_v7(),
        source_slug: "demo-synthetic".into(),
        domain: "match".into(),
        entity_key: "day".into(),
        event_type: "discovery".into(),
        time_window: "2026-11-12".into(),
        due_at: Utc::now(),
        priority: 10,
    }
}

#[test]
fn happy_path_observations() {
    let adapter = DemoAdapter;
    let obs = adapter.parse(&raw_from("demo_day.json"), &ctx()).unwrap();
    // 2 team_identity + 2 match_core
    assert_eq!(
        obs.iter().filter(|o| o.aspect == "team_identity").count(),
        2
    );
    assert_eq!(obs.iter().filter(|o| o.aspect == "match_core").count(), 2);
    let meta = adapter.meta();
    assert_eq!(meta.source_slug, "demo-synthetic");
    assert_eq!(meta.transport, Transport::Fixture);
    let plan = adapter.plan(&job());
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].request_key, "demo-synthetic:match:2026-11-12");
}

#[test]
fn decoy_empty_matches_rejected() {
    let err = DemoAdapter
        .parse(&raw_from("demo_day_decoy.json"), &ctx())
        .unwrap_err();
    assert!(matches!(err, ParseError::DecoyDetected(_)));
}

#[test]
fn date_guard_mismatch_rejected() {
    let err = DemoAdapter
        .parse(&raw_from("demo_day_wrong_date.json"), &ctx())
        .unwrap_err();
    assert!(matches!(err, ParseError::DateGuardMismatch { .. }));
}

#[test]
fn aet_pens_result_quarantined() {
    let err = DemoAdapter
        .parse(&raw_from("demo_day_aet.json"), &ctx())
        .unwrap_err();
    assert!(matches!(err, ParseError::Parse(_)));
}

#[test]
fn schema_drift_rejected() {
    let err = DemoAdapter
        .parse(&raw_from("demo_day_drift.json"), &ctx())
        .unwrap_err();
    assert!(matches!(err, ParseError::SchemaDrift(_)));
}
