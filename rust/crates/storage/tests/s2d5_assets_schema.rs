use football_storage::test_support::test_pool;
use uuid::Uuid;

#[tokio::test]
async fn asset_licence_and_uniqueness_constraints() {
    let pool = test_pool().await;
    let insert = |pool: sqlx::PgPool, url: &'static str, status: &'static str| async move {
        sqlx::query(
            "INSERT INTO ops.asset
             (id, entity_kind, entity_id, kind, url, source, licence_status, captured_at)
             VALUES ($1,'team',$2,'logo',$3,'logos-github',$4,now())",
        )
        .bind(Uuid::now_v7())
        .bind(Uuid::now_v7())
        .bind(url)
        .bind(status)
        .execute(&pool)
        .await
        .map(|_| ())
    };
    insert(pool.clone(), "https://example.test/a.png", "unknown")
        .await
        .unwrap();
    insert(pool.clone(), "https://example.test/b.png", "cleared")
        .await
        .unwrap();
    // licence_status CHECK (23514).
    match insert(pool.clone(), "https://example.test/c.png", "maybe").await {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (licence_status), ehelyett: {other:?}"),
    }
}

#[tokio::test]
async fn asset_uniqueness_prevents_duplicate_binding() {
    let pool = test_pool().await;
    let entity_id = Uuid::now_v7();
    let insert = |pool: sqlx::PgPool| async move {
        sqlx::query(
            "INSERT INTO ops.asset
             (id, entity_kind, entity_id, kind, url, source, captured_at)
             VALUES ($1,'team',$2,'logo','https://example.test/a.png','logos',now())",
        )
        .bind(Uuid::now_v7())
        .bind(entity_id)
        .execute(&pool)
        .await
        .map(|_| ())
    };
    insert(pool.clone()).await.unwrap();
    match insert(pool.clone()).await {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23505")),
        other => panic!("egyediségi hibát vártunk, ehelyett: {other:?}"),
    }
}
