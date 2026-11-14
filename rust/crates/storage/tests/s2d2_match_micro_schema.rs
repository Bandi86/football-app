use football_storage::test_support::{seed_min_match, test_pool};
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_player_entity(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'player','Szoboszlai')",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn insert_team_stat(
    pool: &PgPool,
    match_id: Uuid,
    source_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO fact.match_team_stats
         (match_id, source_id, captured_at, side, period, stat_key, value)
         VALUES ($1,$2,now(),'home','all','possession',55)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(pool)
    .await
    .map(|_| ())
}

#[tokio::test]
async fn team_stats_constraints() {
    let pool = test_pool().await;
    let (match_id, source_id) = seed_min_match(&pool).await;
    insert_team_stat(&pool, match_id, source_id).await.unwrap();

    // PK-duplikátum (23505).
    match insert_team_stat(&pool, match_id, source_id).await {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23505")),
        other => panic!("PK-duplikátum hibát vártunk, ehelyett: {other:?}"),
    }

    // side CHECK (23514).
    let bad_side = sqlx::query(
        "INSERT INTO fact.match_team_stats
         (match_id, source_id, captured_at, side, period, stat_key, value)
         VALUES ($1,$2,now(),'neutral','all','possession',55)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_side {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (side), ehelyett: {other:?}"),
    }

    // period CHECK (23514).
    let bad_period = sqlx::query(
        "INSERT INTO fact.match_team_stats
         (match_id, source_id, captured_at, side, period, stat_key, value)
         VALUES ($1,$2,now(),'home','extra_time','possession',55)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_period {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (period), ehelyett: {other:?}"),
    }
}

#[tokio::test]
async fn player_stats_and_lineup_constraints() {
    let pool = test_pool().await;
    let (match_id, source_id) = seed_min_match(&pool).await;
    let player = seed_player_entity(&pool).await;

    sqlx::query(
        "INSERT INTO fact.match_player_stats
         (match_id, source_id, captured_at, source_player_id, player_entity_id, side, stat_key, value)
         VALUES ($1,$2,now(),'p-1',$3,'home','rating',7.4)",
    )
    .bind(match_id)
    .bind(source_id)
    .bind(player)
    .execute(&pool)
    .await
    .unwrap();
    // PK-duplikátum.
    let dup = sqlx::query(
        "INSERT INTO fact.match_player_stats
         (match_id, source_id, captured_at, source_player_id, side, stat_key, value)
         VALUES ($1,$2,now(),'p-1','home','rating',7.0)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match dup {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23505")),
        other => panic!("PK-duplikátum hibát vártunk, ehelyett: {other:?}"),
    }
    // Ismeretlen entitás-FK (23503).
    let bad_entity = sqlx::query(
        "INSERT INTO fact.match_player_stats
         (match_id, source_id, captured_at, source_player_id, player_entity_id, side, stat_key, value)
         VALUES ($1,$2,now(),'p-2',$3,'home','rating',7.0)",
    )
    .bind(match_id)
    .bind(source_id)
    .bind(Uuid::now_v7())
    .execute(&pool)
    .await;
    match bad_entity {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23503")),
        other => panic!("FK-hibát vártunk, ehelyett: {other:?}"),
    }

    sqlx::query(
        "INSERT INTO fact.match_lineups
         (match_id, source_id, captured_at, side, source_player_id, player_entity_id,
          role, position, shirt_number, rating)
         VALUES ($1,$2,now(),'home','p-1',$3,'starter','CM',8,7.4)",
    )
    .bind(match_id)
    .bind(source_id)
    .bind(player)
    .execute(&pool)
    .await
    .unwrap();
    // role CHECK (23514).
    let bad_role = sqlx::query(
        "INSERT INTO fact.match_lineups
         (match_id, source_id, captured_at, side, source_player_id, role)
         VALUES ($1,$2,now(),'home','p-2','reserve')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_role {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (role), ehelyett: {other:?}"),
    }
    // shirt_number CHECK (23514).
    let bad_shirt = sqlx::query(
        "INSERT INTO fact.match_lineups
         (match_id, source_id, captured_at, side, source_player_id, role, shirt_number)
         VALUES ($1,$2,now(),'home','p-3','sub',0)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_shirt {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (shirt_number), ehelyett: {other:?}"),
    }
}

#[tokio::test]
async fn shots_and_incidents_constraints() {
    let pool = test_pool().await;
    let (match_id, source_id) = seed_min_match(&pool).await;

    sqlx::query(
        "INSERT INTO fact.match_shots
         (match_id, source_id, captured_at, shot_no, side, source_player_id, minute, x, y, xg, xgot, shot_type)
         VALUES ($1,$2,now(),1,'home','p-1',23,84.5,42.1,0.31,0.18,'header')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    // shot_no > 0 CHECK (23514).
    let bad_shot = sqlx::query(
        "INSERT INTO fact.match_shots (match_id, source_id, captured_at, shot_no, side)
         VALUES ($1,$2,now(),0,'home')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_shot {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (shot_no), ehelyett: {other:?}"),
    }

    sqlx::query(
        "INSERT INTO fact.match_incidents
         (match_id, source_id, captured_at, incident_no, side, incident_type,
          player_source_id, minute, detail)
         VALUES ($1,$2,now(),1,'home','goal','p-1',23,'bal láb, tizenhatoson belül')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    // incident_no > 0 CHECK (23514).
    let bad_incident = sqlx::query(
        "INSERT INTO fact.match_incidents
         (match_id, source_id, captured_at, incident_no, side, incident_type)
         VALUES ($1,$2,now(),0,'home','goal')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_incident {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (incident_no), ehelyett: {other:?}"),
    }
}

#[tokio::test]
async fn match_delete_cascades_all_micro_tables() {
    let pool = test_pool().await;
    let (match_id, source_id) = seed_min_match(&pool).await;

    insert_team_stat(&pool, match_id, source_id).await.unwrap();
    sqlx::query(
        "INSERT INTO fact.match_player_stats
         (match_id, source_id, captured_at, source_player_id, side, stat_key, value)
         VALUES ($1,$2,now(),'p-1','home','rating',7.4)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO fact.match_lineups
         (match_id, source_id, captured_at, side, source_player_id, role)
         VALUES ($1,$2,now(),'home','p-1','starter')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO fact.match_shots (match_id, source_id, captured_at, shot_no, side)
         VALUES ($1,$2,now(),1,'home')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO fact.match_incidents
         (match_id, source_id, captured_at, incident_no, side, incident_type)
         VALUES ($1,$2,now(),1,'home','goal')",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("DELETE FROM core.match WHERE id=$1")
        .bind(match_id)
        .execute(&pool)
        .await
        .unwrap();
    for table in [
        "fact.match_team_stats",
        "fact.match_player_stats",
        "fact.match_lineups",
        "fact.match_shots",
        "fact.match_incidents",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "{table} kaszkádol a meccs-törlésre");
    }
}
