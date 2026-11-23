//! `GET /v1/weather` — a fejléc időjárása a helyi API-ból.
//!
//! A kliens nem hív külső szolgáltatást: a backend tartja a cache-t és a
//! `next_refresh_at` időpontot, amire a fejléc visszaszámlálója épül.

use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use chrono::{DateTime, Utc};
use football_ingest::weather::{
    OpenMeteoClient, WEATHER_REFRESH_SECS, WEATHER_SOURCE, WeatherPayload, parse_observed_at,
    weather_location_key,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    AppState,
    auth::{Scope, authorize},
    error::{ApiError, AppError},
};

#[derive(Debug, Deserialize)]
pub struct WeatherQuery {
    #[serde(default = "default_latitude")]
    latitude: f64,
    #[serde(default = "default_longitude")]
    longitude: f64,
}

fn default_latitude() -> f64 {
    football_ingest::weather::DEFAULT_LATITUDE
}

fn default_longitude() -> f64 {
    football_ingest::weather::DEFAULT_LONGITUDE
}

/// Az időjárás pillanatképe, forrás- és frissítési adatokkal.
#[derive(Debug, Serialize, ToSchema)]
pub struct WeatherResponse {
    pub temperature_c: f64,
    pub apparent_c: Option<f64>,
    pub humidity_pct: Option<i32>,
    pub wind_speed_kmh: Option<f64>,
    pub weather_code: i32,
    pub is_day: bool,
    pub observed_at: Option<DateTime<Utc>>,
    pub captured_at: DateTime<Utc>,
    /// A backend által birtokolt következő frissítés időpontja (UTC).
    pub next_refresh_at: DateTime<Utc>,
    /// A forrás nem elérhető, de a cache még megjeleníthető.
    pub stale: bool,
    pub source: String,
}

impl WeatherResponse {
    fn from_payload(
        payload: &WeatherPayload,
        captured_at: DateTime<Utc>,
        next_refresh_at: DateTime<Utc>,
        stale: bool,
    ) -> Self {
        Self {
            temperature_c: payload.temperature_2m,
            apparent_c: payload.apparent_temperature,
            humidity_pct: payload.relative_humidity_2m,
            wind_speed_kmh: payload.wind_speed_10m,
            weather_code: payload.weather_code,
            is_day: payload.is_day != 0,
            observed_at: parse_observed_at(payload.time.as_deref()),
            captured_at,
            next_refresh_at,
            stale,
            source: WEATHER_SOURCE.to_string(),
        }
    }
}

#[utoipa::path(
    get,
    path = "/v1/weather",
    tag = "időjárás",
    params(
        ("latitude" = Option<f64>, Query, description = "Szélesség; alap: Budapest"),
        ("longitude" = Option<f64>, Query, description = "Hosszúság; alap: Budapest")
    ),
    responses(
        (status = 200, description = "Aktuális időjárás cache-ből vagy friss hívásból", body = WeatherResponse),
        (status = 400, description = "Érvénytelen koordináta", body = ApiError),
        (status = 401, description = "Hiányzó token", body = ApiError),
        (status = 503, description = "A forrás nem elérhető és nincs használható cache", body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn weather(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<WeatherQuery>,
) -> Result<Json<WeatherResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let (latitude, longitude) = (query.latitude, query.longitude);
    if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
        return Err(AppError::bad_request(
            "Érvénytelen időjárás-koordináta (szélesség -90..90, hosszúság -180..180).",
        ));
    }
    let key = weather_location_key(latitude, longitude);
    let cached = football_storage::repositories::weather::get(&state.pool, &key).await?;
    let now = Utc::now();
    if let Some(row) = &cached
        && row.next_refresh_at > now
    {
        let payload: WeatherPayload = serde_json::from_value(row.payload.clone())
            .map_err(|_| AppError::internal("Sérült időjárás-cache."))?;
        return Ok(Json(WeatherResponse::from_payload(
            &payload,
            row.captured_at,
            row.next_refresh_at,
            false,
        )));
    }

    match OpenMeteoClient::new().current(latitude, longitude).await {
        Ok(payload) => {
            let captured_at = Utc::now();
            let next_refresh_at = captured_at + chrono::Duration::seconds(WEATHER_REFRESH_SECS);
            let value = serde_json::to_value(&payload)
                .map_err(|_| AppError::internal("Időjárás-szerializációs hiba."))?;
            football_storage::repositories::weather::upsert(
                &state.pool,
                &key,
                latitude,
                longitude,
                &value,
                parse_observed_at(payload.time.as_deref()),
                captured_at,
                next_refresh_at,
            )
            .await?;
            Ok(Json(WeatherResponse::from_payload(
                &payload,
                captured_at,
                next_refresh_at,
                false,
            )))
        }
        Err(err) => {
            tracing::warn!(error = %err, "időjárás-forrás nem elérhető");
            if let Some(row) = cached {
                let payload: WeatherPayload = serde_json::from_value(row.payload.clone())
                    .map_err(|_| AppError::internal("Sérült időjárás-cache."))?;
                return Ok(Json(WeatherResponse::from_payload(
                    &payload,
                    row.captured_at,
                    row.next_refresh_at,
                    true,
                )));
            }
            Err(AppError::service_unavailable(
                "Az időjárás forrása nem elérhető, és nincs korábbi megfigyelés.",
            ))
        }
    }
}
