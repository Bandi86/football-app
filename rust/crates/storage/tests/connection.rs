use football_storage::{connect, ping, user_table_count};

#[tokio::test]
async fn migrated_test_database_is_reachable() {
    let database_url =
        std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL kötelező a teszthez");
    let pool = connect(&database_url)
        .await
        .expect("a tesztadatbázis kapcsolat létrehozható");

    ping(&pool).await.expect("az adatbázis válaszol");

    let tables = user_table_count(&pool)
        .await
        .expect("a táblaszám lekérdezhető");
    assert_eq!(
        tables, 40,
        "a 0001–0009 migráció után 39 terméktábla és a _sqlx_migrations várható"
    );

    pool.close().await;
}
