use football_storage::test_support::{seed_min_match, test_pool};
use sqlx::PgPool;
use uuid::Uuid;

async fn match_id(pool: &PgPool) -> Uuid {
    let (id, _source) = seed_min_match(pool).await;
    id
}

#[tokio::test]
async fn decided_by_allows_only_documented_values() {
    let pool = test_pool().await;
    let match_id = match_id(&pool).await;
    sqlx::query(
        "INSERT INTO core.match_result (match_id, home_ft, away_ft, decided_by)
         VALUES ($1, 1, 1, 'pens')",
    )
    .bind(match_id)
    .execute(&pool)
    .await
    .unwrap();
    let bad = sqlx::query("UPDATE core.match_result SET decided_by='lottery' WHERE match_id=$1")
        .bind(match_id)
        .execute(&pool)
        .await;
    match bad {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (decided_by), ehelyett: {other:?}"),
    }
}

#[tokio::test]
async fn kickoff_precision_defaults_exact_and_checks_values() {
    let pool = test_pool().await;
    let match_id = match_id(&pool).await;
    let precision: String =
        sqlx::query_scalar("SELECT kickoff_precision FROM core.match WHERE id=$1")
            .bind(match_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(precision, "exact", "alapérték: exact");
    sqlx::query("UPDATE core.match SET kickoff_precision='date_only' WHERE id=$1")
        .bind(match_id)
        .execute(&pool)
        .await
        .unwrap();
    let bad = sqlx::query("UPDATE core.match SET kickoff_precision='guess' WHERE id=$1")
        .bind(match_id)
        .execute(&pool)
        .await;
    match bad {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (kickoff_precision), ehelyett: {other:?}"),
    }
}
