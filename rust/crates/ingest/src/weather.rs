//! A fejléc időjárásának forrásfeldolgozása: kulcs nélküli, read-only Open-Meteo hívás.
//!
//! A hálózat és a payload ellenőrzése itt történik (ingest felelősség); az API
//! csak a kész, tipizált értéket és a backend által birtokolt frissítési időt adja vissza.

use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, NaiveDateTime, Utc};
use reqwest::header::{CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};

/// A cache-be írt forrás azonosítója.
pub const WEATHER_SOURCE: &str = "open-meteo";
/// A backend által birtokolt frissítési periódus (a kliens régi 900 mp-es ciklusa).
pub const WEATHER_REFRESH_SECS: i64 = 900;
/// Magyar alaphelyszín; a kliens saját pozíciója opcionálisan felülírja.
pub const DEFAULT_LATITUDE: f64 = 47.4979;
pub const DEFAULT_LONGITUDE: f64 = 19.0402;

const DEFAULT_ENDPOINT: &str = "https://api.open-meteo.com/v1/forecast";
const MAX_RESPONSE_BYTES: usize = 64 * 1024;

/// Az Open-Meteo `current` blokkja. A mezők azonosak a Swift DTO-val.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WeatherPayload {
    pub temperature_2m: f64,
    pub apparent_temperature: Option<f64>,
    pub relative_humidity_2m: Option<i32>,
    pub weather_code: i32,
    pub wind_speed_10m: Option<f64>,
    /// Open-Meteo `is_day`: 1 nappal, 0 éjszaka.
    pub is_day: i32,
    /// `timezone=UTC` mellett `YYYY-MM-DDTHH:MM` formátumú UTC idő.
    pub time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenMeteoEnvelope {
    current: Option<WeatherPayload>,
}

/// A cache kulcsa: 2 tizedesre kerekített szélesség/hosszúság (kb. 1 km).
pub fn weather_location_key(latitude: f64, longitude: f64) -> String {
    format!("{latitude:.2},{longitude:.2}")
}

/// Az Open-Meteo UTC megfigyelési idejének parse-olása; hiányos/rossz formátum → None.
pub fn parse_observed_at(time: Option<&str>) -> Option<DateTime<Utc>> {
    let text = time?;
    let naive = NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M").ok()?;
    Some(DateTime::from_naive_utc_and_offset(naive, Utc))
}

pub struct OpenMeteoClient {
    http: reqwest::Client,
    endpoint: String,
    user_agent: String,
}

impl Default for OpenMeteoClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenMeteoClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
            endpoint: DEFAULT_ENDPOINT.to_string(),
            user_agent: "football-app-v2/1.0 (local weather header)".to_string(),
        }
    }

    /// Tesztekhez: saját endpoint (wiremock) és rövidebb időkorlát.
    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            ..Self::new()
        }
    }

    pub async fn current(&self, latitude: f64, longitude: f64) -> Result<WeatherPayload> {
        if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
            bail!("érvénytelen időjárás-koordináta");
        }
        // A koordinátákat fent validáltuk, így az URL építése biztonságos;
        // a reqwest workspace-konfigja nem hordozza a `.query()` helperhez kellő feature-t.
        let url = format!(
            "{}?latitude={latitude}&longitude={longitude}&current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,wind_speed_10m,is_day&timezone=UTC",
            self.endpoint
        );
        let response = self
            .http
            .get(url)
            .header(USER_AGENT, &self.user_agent)
            .timeout(Duration::from_secs(5))
            .send()
            .await?;
        if !response.status().is_success() {
            bail!("időjárás forrás HTTP {}", response.status().as_u16());
        }
        if response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_none_or(|v| !v.contains("json"))
        {
            bail!("időjárás forrás nem JSON választ adott");
        }
        let bytes = response.bytes().await?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            bail!("időjárás válasz túl nagy ({} bájt)", bytes.len());
        }
        let decoded: OpenMeteoEnvelope = serde_json::from_slice(&bytes)?;
        decoded
            .current
            .ok_or_else(|| anyhow!("időjárás válasznak nincs current blokkja"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_key_is_rounded_and_stable() {
        assert_eq!(weather_location_key(47.4979, 19.0402), "47.50,19.04");
        assert_eq!(weather_location_key(47.4941, 19.0399), "47.49,19.04");
    }

    #[test]
    fn observed_at_parses_utc_minutes() {
        let parsed = parse_observed_at(Some("2026-11-23T08:15")).unwrap();
        assert_eq!(parsed.to_rfc3339(), "2026-11-23T08:15:00+00:00");
        assert_eq!(parse_observed_at(Some("nem idő")), None);
        assert_eq!(parse_observed_at(None), None);
    }

    #[tokio::test]
    async fn current_decodes_open_meteo_shape() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/v1/forecast"))
            .and(wiremock::matchers::query_param("latitude", "47.4979"))
            .and(wiremock::matchers::query_param("timezone", "UTC"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "current": {
                        "temperature_2m": 5.6,
                        "apparent_temperature": 2.1,
                        "relative_humidity_2m": 86,
                        "weather_code": 61,
                        "wind_speed_10m": 12.3,
                        "is_day": 1,
                        "time": "2026-11-23T08:15"
                    }
                })),
            )
            .mount(&server)
            .await;
        let client = OpenMeteoClient::with_endpoint(format!("{}/v1/forecast", server.uri()));
        let current = client
            .current(DEFAULT_LATITUDE, DEFAULT_LONGITUDE)
            .await
            .unwrap();
        assert_eq!(current.weather_code, 61);
        assert_eq!(current.temperature_2m, 5.6);
        assert!(parse_observed_at(current.time.as_deref()).is_some());
    }
}
