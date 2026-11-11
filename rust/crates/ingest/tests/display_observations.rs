use football_ingest::fixture::FixtureAdapter;
use football_storage::object_store::ObjectStore;
use football_storage::test_support::test_pool;

#[tokio::test]
async fn display_fields_become_observations() {
    let pool = test_pool().await;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_d_display.json");
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();

    FixtureAdapter::ingest(&pool, &store, &fixture_path)
        .await
        .unwrap();

    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM stage.observation")
        .fetch_one(&pool)
        .await
        .unwrap();
    let odds: i64 =
        sqlx::query_scalar("SELECT count(*) FROM stage.observation WHERE aspect='match_odds'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let standing: i64 =
        sqlx::query_scalar("SELECT count(*) FROM stage.observation WHERE aspect='standing'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let odds_home: String = sqlx::query_scalar(
        "SELECT normalized_payload->'odds'->>'home' FROM stage.observation WHERE aspect='match_odds'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let standing_position: i16 = sqlx::query_scalar(
        "SELECT (normalized_payload->>'position')::smallint FROM stage.observation
         WHERE aspect='standing' AND natural_key='30000000-0000-7000-8000-000000000011'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        total, 6,
        "2 team_identity + 1 match_core + 1 match_odds + 2 standing"
    );
    assert_eq!(odds, 1);
    assert_eq!(standing, 2);
    assert_eq!(odds_home, "2.2");
    assert_eq!(standing_position, 8);
}
