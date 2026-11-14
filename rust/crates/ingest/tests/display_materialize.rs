use football_ingest::canonicalize::Canonicalizer;
use football_ingest::fixture::{FixtureAdapter, FixtureSource};
use football_storage::object_store::ObjectStore;
use football_storage::test_support::test_pool;

#[tokio::test]
async fn display_facts_materialize_idempotently() {
    let pool = test_pool().await;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_d_display.json");
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();

    FixtureAdapter::ingest(&pool, &store, &fixture_path)
        .await
        .unwrap();
    let fixture: FixtureSource =
        serde_json::from_slice(&std::fs::read(&fixture_path).unwrap()).unwrap();

    Canonicalizer::materialize(
        &pool,
        &football_ingest::normalized::NormalizedDay::from(&fixture),
    )
    .await
    .unwrap();
    Canonicalizer::materialize(
        &pool,
        &football_ingest::normalized::NormalizedDay::from(&fixture),
    )
    .await
    .unwrap();

    let (tier, priority): (i16, i32) = sqlx::query_as(
        "SELECT tier, priority FROM core.competition WHERE id='30000000-0000-7000-8000-000000000001'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((tier, priority), (1, 6));

    let odds_count: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.odds_snapshot")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(odds_count, 1, "ismételt materializálás nem duplikál");

    let (home, market_count): (String, i32) = sqlx::query_as(
        "SELECT (max(price) FILTER (WHERE market='1x2' AND outcome='home'))::float8::text,
                o.market_count
         FROM fact.odds_snapshot o
         JOIN fact.odds_quote q ON q.match_id=o.match_id AND q.source_id=o.source_id
          AND q.bookmaker='default' AND q.phase='closing'
         WHERE o.match_id='30000000-0000-7000-8000-000000000101'
         GROUP BY o.market_count",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(home, "2.2");
    assert_eq!(market_count, 18);

    let standing_count: i64 = sqlx::query_scalar("SELECT count(*) FROM core.standing")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(standing_count, 2);

    let position: i16 = sqlx::query_scalar(
        "SELECT position FROM core.standing WHERE team_id='30000000-0000-7000-8000-000000000011'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(position, 8);

    let source_code: String = sqlx::query_scalar(
        "SELECT s.code FROM fact.odds_snapshot o JOIN ops.source s ON s.id=o.source_id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(source_code, "fixture_d");
}
