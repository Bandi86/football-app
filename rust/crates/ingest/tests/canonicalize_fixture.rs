use football_ingest::canonicalize::Canonicalizer;
use football_ingest::fixture::{FixtureAdapter, FixtureOdds, FixtureSource};
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

/// S2b-2 (Task 3 halasztott minor): újra-ingestnél az odds sor trust_state-je
/// provisional-re áll vissza — a verified állapotot csak a gate utáni
/// odds-kiértékelés adhatja vissza, a canonicalize nem őrizheti meg.
#[tokio::test]
async fn reingest_resets_odds_trust_state_to_provisional() {
    let pool = test_pool().await;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();

    // A golden fixture odds mezőkkel kiegészítve (a fájl érintetlen marad).
    let mut fixture: FixtureSource =
        serde_json::from_slice(&std::fs::read(&fixture_path).unwrap()).unwrap();
    fixture.matches[0].odds = Some(FixtureOdds {
        home: 2.0,
        draw: 3.2,
        away: 3.5,
        btts_yes: 1.8,
        btts_no: 1.95,
        over25: 1.9,
        under25: 1.85,
    });
    fixture.matches[0].market_count = Some(7);
    let odds_fixture_path = temp.path().join("fixture_a_odds.json");
    std::fs::write(&odds_fixture_path, serde_json::to_vec(&fixture).unwrap()).unwrap();

    FixtureAdapter::ingest(&pool, &store, &odds_fixture_path)
        .await
        .unwrap();
    Canonicalizer::materialize(&pool, &fixture).await.unwrap();
    let state: String = sqlx::query_scalar("SELECT trust_state FROM fact.odds_snapshot")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "provisional");

    // Mesterségesen verified állapot (mintha a gate utáni kiértékelés állította
    // volna be); az újra-materializálásnak vissza kell állítania provisional-re.
    sqlx::query("UPDATE fact.odds_snapshot SET trust_state='verified'")
        .execute(&pool)
        .await
        .unwrap();
    Canonicalizer::materialize(&pool, &fixture).await.unwrap();

    let state: String = sqlx::query_scalar("SELECT trust_state FROM fact.odds_snapshot")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "provisional");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.odds_snapshot")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
}
