use chrono::NaiveDate;
use football_storage::repositories::audit::audit_range;
use football_storage::test_support::{seed_min_match, test_pool};
use sqlx::PgPool;
use uuid::Uuid;

async fn duplicate_of(pool: &PgPool, match_id: Uuid) -> Uuid {
    let row: (Uuid, Uuid, Uuid, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        "SELECT competition_id, season_id, home_team_id, kickoff_at FROM core.match WHERE id = $1",
    )
    .bind(match_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let away: (Uuid,) = sqlx::query_as("SELECT away_team_id FROM core.match WHERE id = $1")
        .bind(match_id)
        .fetch_one(pool)
        .await
        .unwrap();
    let new_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, round_label, home_team_id, away_team_id,
          kickoff_at, competition_timezone, status)
         VALUES ($1,$2,$3,'1',$4,$5,$6,'Europe/Budapest','scheduled')",
    )
    .bind(new_id)
    .bind(row.0)
    .bind(row.1)
    .bind(row.2)
    .bind(away.0)
    .bind(row.3)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ops.publish_gate
         (id, match_id, publish_date, state, rule_set_version, blockers, evaluated_at)
         VALUES ($1,$2,($3 AT TIME ZONE 'Europe/Budapest')::date,'verified_publish','verification-1.3','[]',now())",
    )
    .bind(Uuid::now_v7())
    .bind(new_id)
    .bind(row.3)
    .execute(pool)
    .await
    .unwrap();
    new_id
}

#[tokio::test]
async fn editorial_hold_removes_match_from_audit() {
    let pool = test_pool().await;
    let (match_id, _source_id) = seed_min_match(&pool).await;
    let dup_id = duplicate_of(&pool, match_id).await;

    let report = audit_range(
        &pool,
        NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
        NaiveDate::from_ymd_opt(2026, 11, 30).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        report.clusters.len(),
        1,
        "a két azonos fixture klasztert alkot"
    );

    sqlx::query(
        "INSERT INTO ops.editorial_hold (match_id, reason, note)
         VALUES ($1,'SOURCE_CONTRADICTION','teszt')",
    )
    .bind(dup_id)
    .execute(&pool)
    .await
    .unwrap();

    let report = audit_range(
        &pool,
        NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
        NaiveDate::from_ymd_opt(2026, 11, 30).unwrap(),
    )
    .await
    .unwrap();
    assert!(
        report.clusters.is_empty(),
        "a holdolt sor kimarad az auditból"
    );
}
