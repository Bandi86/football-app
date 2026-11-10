use football_ingest::canonicalize::Canonicalizer;
use football_ingest::fixture::{FixtureAdapter, FixtureSource};
use football_storage::object_store::ObjectStore;
use football_storage::test_support::test_pool;

#[tokio::test]
async fn fixture_materializes_one_canonical_match_with_exact_identity_and_provenance() {
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

    let day = Canonicalizer::materialize(&pool, &fixture).await.unwrap();

    assert_eq!(day.matches.len(), 1);
    assert_eq!(
        day.matches[0].home_team_id.to_string(),
        "10000000-0000-7000-8000-000000000011"
    );
    assert_eq!(
        day.matches[0].away_team_id.to_string(),
        "10000000-0000-7000-8000-000000000012"
    );

    let refs: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.source_entity_ref")
        .fetch_one(&pool)
        .await
        .unwrap();
    let provenance: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.provenance")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(refs, 2);
    assert!(provenance >= 5);
}
