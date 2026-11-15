use football_storage::test_support::{seed_min_match, test_pool};
use sqlx::PgPool;
use uuid::Uuid;

async fn ids(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let (match_id, source_id) = seed_min_match(pool).await;
    let (season_id, team_id): (Uuid, Uuid) =
        sqlx::query_as("SELECT season_id, home_team_id FROM core.match WHERE id=$1")
            .bind(match_id)
            .fetch_one(pool)
            .await
            .unwrap();
    (season_id, team_id, source_id)
}

#[tokio::test]
async fn season_team_stats_constraints() {
    let pool = test_pool().await;
    let (season_id, team_id, source_id) = ids(&pool).await;
    let insert = |pool: PgPool| async move {
        sqlx::query(
            "INSERT INTO fact.season_team_stats
             (season_id, team_id, source_id, captured_at, stat_key, value)
             VALUES ($1,$2,$3,now(),'goals',31)",
        )
        .bind(season_id)
        .bind(team_id)
        .bind(source_id)
        .execute(&pool)
        .await
        .map(|_| ())
    };
    insert(pool.clone()).await.unwrap();
    match insert(pool.clone()).await {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23505")),
        other => panic!("PK-duplikátum hibát vártunk, ehelyett: {other:?}"),
    }
}

#[tokio::test]
async fn season_player_stats_constraints() {
    let pool = test_pool().await;
    let (season_id, _team_id, source_id) = ids(&pool).await;
    let insert = |pool: PgPool| async move {
        sqlx::query(
            "INSERT INTO fact.season_player_stats
             (season_id, source_id, captured_at, source_player_id, stat_key, value)
             VALUES ($1,$2,now(),'p-1','goals',7)",
        )
        .bind(season_id)
        .bind(source_id)
        .execute(&pool)
        .await
        .map(|_| ())
    };
    insert(pool.clone()).await.unwrap();
    match insert(pool.clone()).await {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23505")),
        other => panic!("PK-duplikátum hibát vártunk, ehelyett: {other:?}"),
    }
}

#[tokio::test]
async fn player_availability_constraints() {
    let pool = test_pool().await;
    let (_season_id, _team_id, source_id) = ids(&pool).await;
    sqlx::query(
        "INSERT INTO fact.player_availability
         (source_id, captured_at, source_player_id, kind, expected_return, note)
         VALUES ($1, now(), 'p-1', 'injury', '2026-12-01', 'comb')
         ON CONFLICT (source_id, source_player_id, kind) DO UPDATE SET
           expected_return = EXCLUDED.expected_return",
    )
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    // kind CHECK (23514).
    let bad = sqlx::query(
        "INSERT INTO fact.player_availability
         (source_id, captured_at, source_player_id, kind)
         VALUES ($1, now(), 'p-2', 'vacation')",
    )
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (kind), ehelyett: {other:?}"),
    }
}
