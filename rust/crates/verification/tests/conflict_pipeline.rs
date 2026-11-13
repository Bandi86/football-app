use football_ingest::canonicalize::Canonicalizer;
use football_ingest::fixture::{FixtureAdapter, FixtureSource};
use football_storage::object_store::ObjectStore;
use football_storage::test_support::test_pool;
use football_verification::conflict::load_material_values;
use football_verification::engine::{VerificationContext, VerificationEngine};
use football_verification::rules::default_rules;

#[tokio::test]
async fn conflicting_sources_produce_blocker_and_withheld_gate() {
    let pool = test_pool().await;
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");
    let conflict_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_c_conflict.json");

    FixtureAdapter::ingest(&pool, &store, &fixture_path)
        .await
        .unwrap();
    FixtureAdapter::ingest(&pool, &store, &conflict_path)
        .await
        .unwrap();

    let fixture_a: FixtureSource =
        serde_json::from_slice(&std::fs::read(&fixture_path).unwrap()).unwrap();
    let fixture_c: FixtureSource =
        serde_json::from_slice(&std::fs::read(&conflict_path).unwrap()).unwrap();
    let day_a = Canonicalizer::materialize(&pool, &fixture_a).await.unwrap();
    let day_c = Canonicalizer::materialize(&pool, &fixture_c).await.unwrap();
    assert_eq!(day_a.matches[0].match_id, day_c.matches[0].match_id);

    let values = load_material_values(&pool, day_a.matches[0].match_id)
        .await
        .unwrap();
    let context = VerificationContext {
        snapshot: &day_a.matches[0].snapshot,
        home_identity_exact: true,
        away_identity_exact: true,
        duplicate_match_count: 0,
        source_values: &values,
        source_trust: &[],
        odds_values: &[],
        odds_tolerance: 0.0,
    };
    let report = VerificationEngine::run(&default_rules(), &context);

    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_key == "MATERIAL_SOURCE_CONFLICT")
    );
    let state = football_verification::gate::evaluate_and_persist_gate(
        &pool,
        day_a.matches[0].match_id,
        chrono::NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
        &report,
        "verification-1.0",
    )
    .await
    .unwrap();
    assert_eq!(state, football_domain::PublishState::Withheld);
}
