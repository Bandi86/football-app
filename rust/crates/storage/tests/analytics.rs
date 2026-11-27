//! S196: analytics.metric upsert/idempotencia és lista.

use chrono::NaiveDate;
use football_storage::repositories::analytics::{self, NewMeasurement};
use football_storage::test_support::test_pool;

fn measurement(hash: &str) -> NewMeasurement {
    NewMeasurement {
        model_version: "test-v1".to_owned(),
        date_from: NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(),
        date_to: NaiveDate::from_ymd_opt(2026, 11, 27).unwrap(),
        market: "1x2",
        sample_size: 10,
        brier: 0.20,
        brier_baseline: 0.22,
        brier_market: 0.20,
        difference: -0.02,
        ci_low: -0.04,
        ci_high: 0.01,
        log_loss: 0.65,
        wilson_low: 0.40,
        wilson_high: 0.80,
        inputs_hash: hash.to_owned(),
    }
}

#[tokio::test]
async fn measurement_upsert_is_idempotent_and_keeps_provenance() {
    let pool = test_pool().await;
    let first = analytics::upsert_measurement(&pool, &measurement("hash-1"))
        .await
        .unwrap();
    let mut updated = measurement("hash-2");
    updated.brier = 0.18;
    let second = analytics::upsert_measurement(&pool, &updated)
        .await
        .unwrap();
    assert_eq!(
        first, second,
        "ugyanaz a (verzió, ablak, piac) egy sor marad"
    );

    let rows = analytics::list_measurements(&pool, 10).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].inputs_hash, "hash-2");
    assert!((rows[0].brier - 0.18).abs() < 1e-9);
    assert_eq!(rows[0].sample_size, 10);
}
