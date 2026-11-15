use football_storage::test_support::test_pool;

#[tokio::test]
async fn club_elo_provenance_boundary_and_uniqueness() {
    let pool = test_pool().await;
    let insert = |pool: sqlx::PgPool, as_of: &'static str, provenance: &'static str| async move {
        sqlx::query(
            "INSERT INTO fact.club_elo
             (country_code, club_name, as_of, elo, provenance, captured_at)
             VALUES ('HUN','Újpest',$1::date,1532.4,$2,now())",
        )
        .bind(as_of)
        .bind(provenance)
        .execute(&pool)
        .await
        .map(|_| ())
    };
    // Határ utáni observed: érvényes.
    insert(pool.clone(), "2026-11-01", "observed")
        .await
        .unwrap();
    // Határ előtti observed: tiltott (23514).
    match insert(pool.clone(), "2025-05-31", "observed").await {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("határ-CHECK hibát vártunk, ehelyett: {other:?}"),
    }
    // Határ előtti provisional: érvényes.
    insert(pool.clone(), "2025-05-31", "provisional")
        .await
        .unwrap();
    // Ismeretlen címke: tiltott (23514).
    match insert(pool.clone(), "2026-11-02", "guessed").await {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("címke-CHECK hibát vártunk, ehelyett: {other:?}"),
    }
    // PK-duplikátum (23505).
    match insert(pool.clone(), "2026-11-01", "observed").await {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23505")),
        other => panic!("PK-duplikátum hibát vártunk, ehelyett: {other:?}"),
    }
}
