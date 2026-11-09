use football_storage::{connect, ping, user_table_count};

#[tokio::test]
async fn empty_test_database_is_reachable() {
    let database_url =
        std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL kötelező a teszthez");
    let pool = connect(&database_url)
        .await
        .expect("a tesztadatbázis kapcsolat létrehozható");

    ping(&pool).await.expect("az adatbázis válaszol");

    let tables = user_table_count(&pool)
        .await
        .expect("a táblaszám lekérdezhető");
    assert_eq!(tables, 0, "az alapozási fázisban a tesztdb legyen üres");

    pool.close().await;
}
