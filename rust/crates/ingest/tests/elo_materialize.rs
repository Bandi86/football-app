use chrono::NaiveDate;
use football_ingest::adapter::{ParseCtx, SourceAdapter};
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::clubelo::ClubEloAdapter;
use football_ingest::fetch::RawPayload;
use football_ingest::normalized::ClubEloDay;
use football_storage::test_support::test_pool;

#[tokio::test]
async fn elo_day_materializes_and_is_idempotent() {
    let pool = test_pool().await;
    let bytes = std::fs::read("tests/fixtures/clubelo_2026-11-14.csv").unwrap();
    let raw = RawPayload {
        bytes,
        content_type: "text/csv".into(),
        http_status: 200,
        fetched_at: chrono::Utc::now(),
        sha256: String::new(),
    };
    let ctx = ParseCtx {
        requested_date: NaiveDate::from_ymd_opt(2026, 11, 14).unwrap(),
        request_key: "clubelo:elo:2026-11-14".into(),
    };
    let observations = ClubEloAdapter.parse(&raw, &ctx).unwrap();
    let day = ClubEloDay::from_observations(&observations).unwrap();

    let written = Canonicalizer::materialize_elo(&pool, &day).await.unwrap();
    assert_eq!(written, 3);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.club_elo")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);
    let provenance: String =
        sqlx::query_scalar("SELECT provenance FROM fact.club_elo WHERE club_name='Barcelona'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(provenance, "observed");

    // Újrafuttatás: as-of upsert, nincs duplikátum.
    Canonicalizer::materialize_elo(&pool, &day).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.club_elo")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);
}
