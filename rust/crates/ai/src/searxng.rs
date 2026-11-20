//! Helyi SearXNG webkeresés (JSON API). Csak 127.0.0.1-re hív, nincs
//! API-kulcs és nincs külső szolgáltatás; a találatokat a modell
//! `web_search` toolja használja.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::AiError;

/// Egy keresési találat, a modellnek átadható minimális formában.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct SearxResponse {
    #[serde(default)]
    results: Vec<SearxItem>,
}

#[derive(Debug, Deserialize)]
struct SearxItem {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    content: String,
}

/// SearXNG kliens egy konfigurált helyi végponttal.
#[derive(Debug, Clone)]
pub struct SearxngClient {
    http: reqwest::Client,
    endpoint: Url,
}

impl SearxngClient {
    pub fn new(endpoint: &str, timeout: Duration) -> Result<Self, AiError> {
        let endpoint = Url::parse(endpoint)
            .map_err(|e| AiError::Config(format!("érvénytelen SearXNG URL ({endpoint}): {e}")))?;
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| AiError::Config(format!("nem sikerült HTTP-klienst építeni: {e}")))?;
        Ok(Self { http, endpoint })
    }

    /// Keresés a helyi SearXNG-n; üres kifejezésre üres találatlista.
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, AiError> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let mut url = self.endpoint.clone();
        url.set_path("/search");
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("format", "json");
        let url_text = url.to_string();
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|source| AiError::Http {
                url: url_text.clone(),
                source,
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(AiError::Status {
                url: url_text,
                status,
            });
        }
        let body = response.text().await.map_err(|source| AiError::Http {
            url: url_text.clone(),
            source,
        })?;
        let parsed: SearxResponse =
            serde_json::from_str(&body).map_err(|source| AiError::Decode {
                context: "SearXNG válasz".to_string(),
                source,
            })?;
        Ok(parsed
            .results
            .into_iter()
            .filter(|item| !item.url.trim().is_empty())
            .take(limit)
            .map(|item| SearchResult {
                title: item.title.trim().to_string(),
                url: item.url.trim().to_string(),
                content: item.content.trim().to_string(),
            })
            .collect())
    }
}
