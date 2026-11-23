//! A backend-időjárás cache-e. A frissítési időt a backend birtokolja
//! (`next_refresh_at`), a kliens csak olvassa — innen jön a valódi countdown.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{FromRow, PgPool};

#[derive(Debug, Clone, FromRow)]
pub struct WeatherCacheRow {
    pub payload: Value,
    pub captured_at: DateTime<Utc>,
    pub observed_at: Option<DateTime<Utc>>,
    pub next_refresh_at: DateTime<Utc>,
}

/// Cache-találat kulcs szerint. A hiány `None`, nem hamis adat.
pub async fn get(
    pool: &PgPool,
    location_key: &str,
) -> Result<Option<WeatherCacheRow>, sqlx::Error> {
    sqlx::query_as::<_, WeatherCacheRow>(
        "SELECT payload, captured_at, observed_at, next_refresh_at
           FROM ops.weather_cache WHERE location_key = $1",
    )
    .bind(location_key)
    .fetch_optional(pool)
    .await
}

/// Frissítési idő csak olvasása (a countdown a backendből jön).
pub async fn next_refresh_at(
    pool: &PgPool,
    location_key: &str,
) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
    sqlx::query_scalar::<_, DateTime<Utc>>(
        "SELECT next_refresh_at FROM ops.weather_cache WHERE location_key = $1",
    )
    .bind(location_key)
    .fetch_optional(pool)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert(
    pool: &PgPool,
    location_key: &str,
    latitude: f64,
    longitude: f64,
    payload: &Value,
    observed_at: Option<DateTime<Utc>>,
    captured_at: DateTime<Utc>,
    next_refresh_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO ops.weather_cache
            (location_key, latitude, longitude, payload, observed_at, captured_at, next_refresh_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (location_key) DO UPDATE SET
            latitude = EXCLUDED.latitude,
            longitude = EXCLUDED.longitude,
            payload = EXCLUDED.payload,
            observed_at = EXCLUDED.observed_at,
            captured_at = EXCLUDED.captured_at,
            next_refresh_at = EXCLUDED.next_refresh_at",
    )
    .bind(location_key)
    .bind(latitude)
    .bind(longitude)
    .bind(payload)
    .bind(observed_at)
    .bind(captured_at)
    .bind(next_refresh_at)
    .execute(&mut *tx)
    .await?;
    // A cache korlátos marad: a 7 napnál régebbi, már nem használt kulcsok mennek.
    sqlx::query("DELETE FROM ops.weather_cache WHERE captured_at < now() - interval '7 days'")
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(value: &str) -> Value {
        serde_json::from_str(value).unwrap()
    }

    #[tokio::test]
    async fn cache_roundtrip_and_replacement() {
        let pool = crate::test_support::test_pool().await;
        let key = "47.50,19.04";
        assert!(get(&pool, key).await.unwrap().is_none());

        let first = payload(r#"{"temperature_2m":5.6,"weather_code":61,"is_day":1}"#);
        let captured = Utc::now();
        let next = captured + chrono::Duration::seconds(900);
        upsert(&pool, key, 47.4979, 19.0402, &first, None, captured, next)
            .await
            .unwrap();
        let row = get(&pool, key).await.unwrap().unwrap();
        assert_eq!(row.payload, first);
        assert_eq!(row.next_refresh_at, next);
        assert_eq!(next_refresh_at(&pool, key).await.unwrap(), Some(next));

        let second = payload(r#"{"temperature_2m":7.1,"weather_code":3,"is_day":0}"#);
        let captured2 = captured + chrono::Duration::seconds(900);
        let next2 = captured2 + chrono::Duration::seconds(900);
        upsert(
            &pool, key, 47.4979, 19.0402, &second, None, captured2, next2,
        )
        .await
        .unwrap();
        let row = get(&pool, key).await.unwrap().unwrap();
        assert_eq!(row.payload["temperature_2m"], 7.1);
        assert_eq!(row.next_refresh_at, next2);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.weather_cache")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
}
