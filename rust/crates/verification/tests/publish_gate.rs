use football_domain::{Finding, VerificationReport};
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::fixture::{FixtureAdapter, FixtureSource};
use football_storage::object_store::ObjectStore;
use football_storage::test_support::test_pool;
use football_verification::gate::evaluate_and_persist_gate;

#[tokio::test]
async fn blocker_report_creates_withheld_gate() {
    let pool = test_pool().await;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();
    FixtureAdapter::ingest(&pool, &store, &fixture_path)
        .await
        .unwrap();
    let fixture: FixtureSource =
        serde_json::from_slice(&std::fs::read(&fixture_path).unwrap()).unwrap();
    let materialized = Canonicalizer::materialize(
        &pool,
        &football_ingest::normalized::NormalizedDay::from(&fixture),
    )
    .await
    .unwrap();
    let match_id = materialized.matches[0].match_id;

    let report = VerificationReport {
        findings: vec![Finding::blocker(
            "MATERIAL_SOURCE_CONFLICT",
            "match",
            "Források eltérnek.",
        )],
    };

    let state = evaluate_and_persist_gate(
        &pool,
        match_id,
        chrono::NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
        &report,
        "verification-1.0",
    )
    .await
    .unwrap();

    assert_eq!(state, football_domain::PublishState::Withheld);
    let stored: String = sqlx::query_scalar("SELECT state FROM ops.publish_gate WHERE match_id=$1")
        .bind(match_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, "withheld");
}
