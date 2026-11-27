use football_ingest::fixture::FixtureAdapter;
use football_storage::object_store::ObjectStore;
use football_storage::test_support::test_pool;

#[tokio::test]
async fn fixture_is_stored_raw_and_observed_once() {
    let pool = test_pool().await;
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");

    let first = FixtureAdapter::ingest(&pool, &store, &path).await.unwrap();
    let second = FixtureAdapter::ingest(&pool, &store, &path).await.unwrap();

    let artifacts: i64 = sqlx::query_scalar("SELECT count(*) FROM raw.artifact")
        .fetch_one(&pool)
        .await
        .unwrap();
    let captures: i64 = sqlx::query_scalar("SELECT count(*) FROM raw.capture")
        .fetch_one(&pool)
        .await
        .unwrap();
    let observations: i64 = sqlx::query_scalar("SELECT count(*) FROM stage.observation")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(artifacts, 1);
    assert_eq!(captures, 2);
    assert_eq!(observations, 3);
    assert_ne!(first.capture_id, second.capture_id);
    assert_eq!(first.observation_ids, second.observation_ids);
}
