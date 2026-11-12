use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use football_domain::error::FetchError;
use sha2::Digest;

#[derive(Debug, Clone)]
pub struct FetchRequest {
    pub url: String,
    pub request_key: String,
    pub endpoint_key: String,
}

#[derive(Debug, Clone)]
pub struct RawPayload {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub http_status: u16,
    pub fetched_at: DateTime<Utc>,
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub struct FetchLimits {
    pub max_bytes: u64,
    pub timeout: Duration,
}

#[async_trait]
pub trait Fetcher: Send + Sync {
    async fn fetch(
        &self,
        req: &FetchRequest,
        limits: &FetchLimits,
    ) -> Result<RawPayload, FetchError>;
}

#[derive(Debug, Clone)]
pub struct FixtureFetcher {
    files: HashMap<String, PathBuf>,
}

impl FixtureFetcher {
    pub fn new(files: HashMap<String, PathBuf>) -> Self {
        Self { files }
    }

    /// Ha a könyvtár nem létezik, üres mapet adunk vissza, nem hibát:
    /// a daemon indulása nem bukhat el a worker-fixtures mappa hiányán.
    pub fn from_dir(dir: &Path) -> Result<Self> {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::new(HashMap::new()));
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("fixture könyvtár olvasása: {}", dir.display()));
            }
        };

        let mut files = HashMap::new();
        for entry in entries {
            let entry =
                entry.with_context(|| format!("fixture bejegyzés olvasása: {}", dir.display()))?;
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            files.insert(stem.to_string(), path);
        }
        Ok(Self { files })
    }
}

#[async_trait]
impl Fetcher for FixtureFetcher {
    async fn fetch(
        &self,
        req: &FetchRequest,
        limits: &FetchLimits,
    ) -> Result<RawPayload, FetchError> {
        let path = self.files.get(&req.request_key).ok_or_else(|| {
            FetchError::NotFound(format!("nincs fixture a kulcshoz: {}", req.request_key))
        })?;
        let bytes = std::fs::read(path).map_err(|err| {
            FetchError::Transport(format!("fixture olvasása ({}): {err}", path.display()))
        })?;
        let byte_size = bytes.len() as u64;
        if byte_size > limits.max_bytes {
            return Err(FetchError::Oversize(byte_size));
        }
        Ok(RawPayload {
            sha256: hex::encode(sha2::Sha256::digest(&bytes)),
            bytes,
            content_type: "application/json".to_string(),
            http_status: 200,
            fetched_at: Utc::now(),
        })
    }
}
