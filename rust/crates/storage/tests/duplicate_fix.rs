//! S216: a cross-source duplikátum-rendezés csak placeholder sorokat érint,
//! idempotens és auditálható.

use chrono::{DateTime, TimeZone, Utc};
use football_storage::repositories::duplicate_fix;
use football_storage::test_support::{seed_min_match, test_pool};
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_placeholders(pool: &PgPool, match_id: Uuid) -> (Uuid, Vec<Uuid>) {
    let (competition_id, season_id, home, away, _kickoff): (Uuid, Uuid, Uuid, Uuid, DateTime<Utc>) =
        sqlx::query_as(
            "SELECT competition_id, season_id, home_team_id, away_team_id, kickoff_at
           FROM core.match WHERE id=$1",
        )
        .bind(match_id)
        .fetch_one(pool)
        .await
        .unwrap();

    sqlx::query(
        "UPDATE core.match SET kickoff_precision='exact',
            kickoff_at=$2 WHERE id=$1",
    )
    .bind(match_id)
    .bind(Utc.with_ymd_and_hms(2026, 11, 25, 20, 0, 0).unwrap())
    .execute(pool)
    .await
    .unwrap();

    let entity_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id,kind,canonical_name) VALUES ($1,'competition','Paraguayan Primera')")
        .bind(entity_id)
        .execute(pool)
        .await
        .unwrap();
    let other_competition = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.competition (id,entity_id,name) VALUES ($1,$2,'Division Profesional')",
    )
    .bind(other_competition)
    .bind(entity_id)
    .execute(pool)
    .await
    .unwrap();

    let mut losers = Vec::new();
    for _ in 0..2 {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO core.match
             (id,competition_id,season_id,home_team_id,away_team_id,kickoff_at,
              kickoff_precision,status,competition_timezone)
             VALUES ($1,$2,$3,$4,$5,$6,'date_only','scheduled','UTC')",
        )
        .bind(id)
        .bind(other_competition)
        .bind(season_id)
        .bind(home)
        .bind(away)
        .bind(Utc.with_ymd_and_hms(2026, 11, 25, 0, 0, 0).unwrap())
        .execute(pool)
        .await
        .unwrap();
        losers.push(id);
    }
    assert!(competition_id != Uuid::nil());
    (other_competition, losers)
}

#[tokio::test]
async fn duplicate_fix_only_cancels_placeholders_and_is_idempotent() {
    let pool = test_pool().await;
    let (winner_id, _) = seed_min_match(&pool).await;
    let (_, losers) = seed_placeholders(&pool, winner_id).await;

    let candidates = duplicate_fix::candidates(&pool, None, None).await.unwrap();
    assert_eq!(candidates.len(), 2);
    assert!(candidates.iter().all(|c| c.winner_id == winner_id));

    let (groups, rows) = duplicate_fix::apply(&pool, None, None).await.unwrap();
    assert_eq!(groups, 1);
    assert_eq!(rows, 2);

    for loser in &losers {
        let status: String = sqlx::query_scalar("SELECT status FROM core.match WHERE id=$1")
            .bind(loser)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "canceled");
    }
    let winner_status: String = sqlx::query_scalar("SELECT status FROM core.match WHERE id=$1")
        .bind(winner_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(winner_status, "scheduled");

    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.audit_event WHERE action='duplicate_fixture_cancel'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 2);

    let second = duplicate_fix::apply(&pool, None, None).await.unwrap();
    assert_eq!(second, (0, 0));
}
