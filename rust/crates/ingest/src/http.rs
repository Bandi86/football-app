use async_trait::async_trait;
use chrono::Utc;
use football_domain::error::FetchError;
use sha2::{Digest, Sha256};

use crate::fetch::{FetchLimits, FetchRequest, Fetcher, RawPayload};

pub struct RustHttpFetcher {
    client: reqwest::Client,
    user_agent: String,
}

impl RustHttpFetcher {
    pub fn new(user_agent: &str) -> Result<Self, FetchError> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| FetchError::Transport(e.to_string()))?;
        Ok(Self {
            client,
            user_agent: user_agent.to_string(),
        })
    }
}

fn map_reqwest_error(err: reqwest::Error) -> FetchError {
    if err.is_timeout() {
        FetchError::Timeout
    } else {
        FetchError::Transport(err.to_string())
    }
}

#[async_trait]
impl Fetcher for RustHttpFetcher {
    async fn fetch(
        &self,
        req: &FetchRequest,
        limits: &FetchLimits,
    ) -> Result<RawPayload, FetchError> {
        let mut response = self
            .client
            .get(&req.url)
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .timeout(limits.timeout)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(FetchError::HttpStatus(status));
        }
        // Előellenőrzés Content-Length alapján: ha a deklarált méret meghaladja
        // a plafont, a törzs letöltését el sem kezdjük.
        if let Some(declared) = response.content_length() {
            if declared > limits.max_bytes {
                return Err(FetchError::Oversize(declared));
            }
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        // Streaming méretplafon: chunkonként gyűjtünk, és a ténylegesen letöltött
        // méret túllépésekor azonnal megállunk (Content-Length nélkül is véd).
        let mut bytes: Vec<u8> = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(map_reqwest_error)? {
            let received = bytes.len() as u64 + chunk.len() as u64;
            if received > limits.max_bytes {
                return Err(FetchError::Oversize(received));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(RawPayload {
            sha256: hex::encode(Sha256::digest(&bytes)),
            bytes,
            content_type,
            http_status: status,
            fetched_at: Utc::now(),
        })
    }
}
