use football_storage::test_support::{seed_min_match, test_pool};
use sqlx::PgPool;
use uuid::Uuid;

async fn insert_quote(pool: &PgPool, match_id: Uuid, source_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO fact.odds_quote
         (match_id, source_id, bookmaker, market, outcome, line, phase, price)
         VALUES ($1,$2,'default','1x2','home',0,'closing',2.2)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(pool)
    .await
    .map(|_| ())
}

#[tokio::test]
async fn odds_quote_constraints_and_cascade() {
    let pool = test_pool().await;
    let (match_id, source_id) = seed_min_match(&pool).await;

    // Fejléc nélkül FK-hiba (23503).
    match insert_quote(&pool, match_id, source_id).await {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23503")),
        other => panic!("FK-hibát vártunk, ehelyett: {other:?}"),
    }

    sqlx::query(
        "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count)
         VALUES ($1,$2,now(),3)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    insert_quote(&pool, match_id, source_id).await.unwrap();

    // PK-duplikátum (23505).
    match insert_quote(&pool, match_id, source_id).await {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23505")),
        other => panic!("PK-duplikátum hibát vártunk, ehelyett: {other:?}"),
    }

    // price > 1 CHECK (23514).
    let bad_price = sqlx::query(
        "INSERT INTO fact.odds_quote
         (match_id, source_id, bookmaker, market, outcome, line, phase, price)
         VALUES ($1,$2,'default','1x2','draw',0,'closing',1.0)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_price {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (price), ehelyett: {other:?}"),
    }

    // line >= 0 CHECK (23514).
    let bad_line = sqlx::query(
        "INSERT INTO fact.odds_quote
         (match_id, source_id, bookmaker, market, outcome, line, phase, price)
         VALUES ($1,$2,'default','ou','over',-1,'closing',1.9)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_line {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (line), ehelyett: {other:?}"),
    }

    // phase enum CHECK (23514).
    let bad_phase = sqlx::query(
        "INSERT INTO fact.odds_quote
         (match_id, source_id, bookmaker, market, outcome, line, phase, price)
         VALUES ($1,$2,'default','1x2','away',0,'mid',3.2)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad_phase {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (phase), ehelyett: {other:?}"),
    }

    // Fejléc törlése kaszkádol a quote-okra.
    sqlx::query("DELETE FROM fact.odds_snapshot WHERE match_id=$1 AND source_id=$2")
        .bind(match_id)
        .bind(source_id)
        .execute(&pool)
        .await
        .unwrap();
    let quotes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fact.odds_quote WHERE match_id=$1 AND source_id=$2",
    )
    .bind(match_id)
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(quotes, 0, "a fejléc törlése kaszkádol");
}

#[tokio::test]
async fn match_stats_constraints() {
    let pool = test_pool().await;
    let (match_id, source_id) = seed_min_match(&pool).await;

    sqlx::query(
        "INSERT INTO fact.match_stats
         (match_id, source_id, captured_at, home_shots, away_shots, home_xg, away_xg)
         VALUES ($1,$2,now(),12,9,1.7,0.9)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();

    // PK-duplikátum (23505).
    let dup = sqlx::query(
        "INSERT INTO fact.match_stats (match_id, source_id, captured_at) VALUES ($1,$2,now())",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match dup {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23505")),
        other => panic!("PK-duplikátum hibát vártunk, ehelyett: {other:?}"),
    }

    // Negatív érték CHECK (23514): másik source-dal.
    let other_source = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,'neg','Negatív')")
        .bind(other_source)
        .execute(&pool)
        .await
        .unwrap();
    let bad = sqlx::query(
        "INSERT INTO fact.match_stats (match_id, source_id, captured_at, home_shots)
         VALUES ($1,$2,now(),-1)",
    )
    .bind(match_id)
    .bind(other_source)
    .execute(&pool)
    .await;
    match bad {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23514")),
        other => panic!("CHECK-hibát vártunk (home_shots), ehelyett: {other:?}"),
    }
}

#[tokio::test]
async fn match_referee_and_venue_entity_fk() {
    let pool = test_pool().await;
    let (match_id, _source_id) = seed_min_match(&pool).await;
    let referee_id = Uuid::now_v7();
    let venue_id = Uuid::now_v7();
    for (id, kind, name) in [
        (referee_id, "referee", "Kovács István"),
        (venue_id, "venue", "Puskás Aréna"),
    ] {
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,$2,$3)")
            .bind(id)
            .bind(kind)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
    }

    // Ismeretlen uuid → FK-hiba (23503).
    let bad = sqlx::query("UPDATE core.match SET referee_entity_id=$2 WHERE id=$1")
        .bind(match_id)
        .bind(Uuid::now_v7())
        .execute(&pool)
        .await;
    match bad {
        Err(sqlx::Error::Database(e)) => assert_eq!(e.code().as_deref(), Some("23503")),
        other => panic!("FK-hibát vártunk, ehelyett: {other:?}"),
    }

    sqlx::query("UPDATE core.match SET referee_entity_id=$2, venue_entity_id=$3 WHERE id=$1")
        .bind(match_id)
        .bind(referee_id)
        .bind(venue_id)
        .execute(&pool)
        .await
        .unwrap();
    let (r, v): (Option<Uuid>, Option<Uuid>) =
        sqlx::query_as("SELECT referee_entity_id, venue_entity_id FROM core.match WHERE id=$1")
            .bind(match_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((r, v), (Some(referee_id), Some(venue_id)));
}
