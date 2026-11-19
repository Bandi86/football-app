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
    let day_a = Canonicalizer::materialize(
        &pool,
        &football_ingest::normalized::NormalizedDay::from(&fixture_a),
    )
    .await
    .unwrap();
    let day_c = Canonicalizer::materialize(
        &pool,
        &football_ingest::normalized::NormalizedDay::from(&fixture_c),
    )
    .await
    .unwrap();
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
        resolved_fields: &[],
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

#[tokio::test]
async fn resolved_material_conflict_no_longer_blocks_the_rule() {
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
    let day_a = Canonicalizer::materialize(
        &pool,
        &football_ingest::normalized::NormalizedDay::from(&fixture_a),
    )
    .await
    .unwrap();
    let match_id = day_a.matches[0].match_id;
    let winner: uuid::Uuid = sqlx::query_scalar(
        "SELECT observation_id FROM fact.provenance
          WHERE subject_kind='match' AND subject_id=$1 LIMIT 1",
    )
    .bind(match_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Üres indoklás és idegen megfigyelés: fail-closed.
    assert!(
        football_verification::conflict::resolve_conflict(&pool, match_id, "match", winner, "  ",)
            .await
            .is_err()
    );
    assert!(
        football_verification::conflict::resolve_conflict(
            &pool,
            match_id,
            "match",
            uuid::Uuid::now_v7(),
            "idegen megfigyelés",
        )
        .await
        .is_err()
    );

    football_verification::conflict::resolve_conflict(
        &pool,
        match_id,
        "match",
        winner,
        "S26: forrásprioritás alapján a megfigyelés nyert (teszt)",
    )
    .await
    .unwrap();
    let fields = football_verification::conflict::load_resolved_fields(&pool, match_id)
        .await
        .unwrap();
    assert!(fields.iter().any(|f| f == "match"));

    let values = load_material_values(&pool, match_id).await.unwrap();
    let context = VerificationContext {
        snapshot: &day_a.matches[0].snapshot,
        home_identity_exact: true,
        away_identity_exact: true,
        duplicate_match_count: 0,
        source_values: &values,
        source_trust: &[],
        odds_values: &[],
        odds_tolerance: 0.0,
        resolved_fields: &fields,
    };
    let report = VerificationEngine::run(&default_rules(), &context);
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.rule_key == "MATERIAL_SOURCE_CONFLICT")
    );
}
