use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

#[tokio::test]
async fn trust_schema_and_odds_composite_pk() {
    let pool = test_pool().await;
    let t: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_tables WHERE schemaname='ops' AND tablename='source_trust'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(t, 1);
    // composite PK: pontos oszloplista attnum szerint; a contains bővebb
    // halmazt is átengedne, ezért teljes egyenlőség kell.
    let pk: String = sqlx::query_scalar(
        "SELECT string_agg(a.attname, ',' ORDER BY a.attnum)
         FROM pg_index i JOIN pg_attribute a ON a.attrelid=i.indrelid AND a.attnum=ANY(i.indkey)
         WHERE i.indrelid='fact.odds_snapshot'::regclass AND i.indisprimary",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pk, "match_id,source_id");
    // Előbb érvényes ops.source sor: különben az FK-hiba elfedné a
    // trust_class CHECK-et, és a teszt nem azt bizonyítaná, amit kell.
    let source_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ops.source (id, code, display_name)
         VALUES (gen_random_uuid(),'trust-test','Trust teszt') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    // Érvénytelen trust_class: az egyetlen hibaok a CHECK lehet,
    // SQLSTATE 23514 (check_violation).
    let bad = sqlx::query(
        "INSERT INTO ops.source_trust (source_id, domain, trust_class)
         VALUES ($1,'result','unknown')",
    )
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad {
        Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23514") => {}
        other => panic!("a trust_class CHECK 23514-et várt, ehelyett: {other:?}"),
    }
}

#[tokio::test]
async fn trust_state_check_and_default() {
    let pool = test_pool().await;
    let (match_id, source_id) = seed_odds_row(&pool).await;
    // Friss sorban a trust_state alapértelmezése 'provisional'.
    let state: String = sqlx::query_scalar(
        "SELECT trust_state FROM fact.odds_snapshot WHERE match_id=$1 AND source_id=$2",
    )
    .bind(match_id)
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, "provisional");
    // Érvénytelen trust_state: az egyetlen hibaok a CHECK lehet,
    // SQLSTATE 23514 (check_violation).
    let bad = sqlx::query(
        "UPDATE fact.odds_snapshot SET trust_state='invalid' WHERE match_id=$1 AND source_id=$2",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await;
    match bad {
        Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("23514") => {}
        other => panic!("a trust_state CHECK 23514-et várt, ehelyett: {other:?}"),
    }
}

// Érvényes odds_snapshot sor minimális lánca: country → competition →
// season → csapatok → match → source → odds (a day_query teszt seedje).
async fn seed_odds_row(pool: &PgPool) -> (Uuid, Uuid) {
    let country_id = Uuid::now_v7();
    let competition_entity_id = Uuid::now_v7();
    let competition_id = Uuid::now_v7();
    let season_id = Uuid::now_v7();
    let home_entity_id = Uuid::now_v7();
    let home_team_id = Uuid::now_v7();
    let away_entity_id = Uuid::now_v7();
    let away_team_id = Uuid::now_v7();
    let match_id = Uuid::now_v7();
    let source_id = Uuid::now_v7();

    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Hungary')")
        .bind(country_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','NB I')",
    )
    .bind(competition_entity_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name)
         VALUES ($1,$2,$3,'NB I')",
    )
    .bind(competition_id)
    .bind(competition_entity_id)
    .bind(country_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season_id)
        .bind(competition_id)
        .execute(pool)
        .await
        .unwrap();

    for (entity_id, team_id, name) in [
        (home_entity_id, home_team_id, "Újpest"),
        (away_entity_id, away_team_id, "Győr"),
    ] {
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(entity_id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO core.team
             (id, entity_id, country_id, name, gender, age_group, branch)
             VALUES ($1,$2,$3,$4,'male','senior','senior')",
        )
        .bind(team_id)
        .bind(entity_id)
        .bind(country_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    }

    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, round_label, home_team_id, away_team_id,
          kickoff_at, competition_timezone, status)
         VALUES ($1,$2,$3,'12',$4,$5,now(),'Europe/Budapest','scheduled')",
    )
    .bind(match_id)
    .bind(competition_id)
    .bind(season_id)
    .bind(home_team_id)
    .bind(away_team_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ops.source (id, code, display_name) VALUES ($1,'trust-odds','Trust odds')",
    )
    .bind(source_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count)
         VALUES ($1,$2,now(),18)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(pool)
    .await
    .unwrap();

    (match_id, source_id)
}
