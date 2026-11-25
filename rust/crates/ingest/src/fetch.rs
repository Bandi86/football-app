use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use football_domain::error::FetchError;
use football_domain::policy::Transport;
use sha2::Digest;

use crate::http::RustHttpFetcher;

#[derive(Debug, Clone)]
pub struct FetchRequest {
    pub url: String,
    pub request_key: String,
    pub endpoint_key: String,
    /// Opcionális CSS-szelektor: a böngésző-fetcher addig polloz, amíg a
    /// renderelt DOM-ban meg nem jelenik (kliens-oldali lista-várakozás).
    /// A HTTP-fetcher figyelmen kívül hagyja.
    pub ready_marker: Option<String>,
    /// Opcionális Bearer token a kéréshez (pl. API-forrás). A naplóba és a
    /// nyers capture-be NEM kerül (csak a kérés fejlécébe).
    pub bearer_token: Option<String>,
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

/// Helyi, fájl-alapú forrás fetchere (S38): a kérés `url` mezője a gyökérhez
/// viszonyított relatív útvonal. Nincs hálózat, nincs robots, nincs késleltetés.
#[derive(Debug, Clone)]
pub struct LocalFileFetcher {
    root: PathBuf,
}

impl LocalFileFetcher {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    fn resolve(&self, rel: &str) -> Result<PathBuf, FetchError> {
        let rel = rel.trim_start_matches('/');
        if rel.is_empty() || rel.contains("..") {
            return Err(FetchError::Transport(format!(
                "érvénytelen helyi útvonal: {rel}"
            )));
        }
        Ok(self.root.join(rel))
    }

    /// A forrásfájl tényleges útja (S139). A frissesség-ellenőrzés használja:
    /// a catch-up job csak akkor dolgozható fel, ha a napi envelope a job
    /// létrehozása után frissült.
    pub fn source_path(&self, rel: &str) -> Result<PathBuf, FetchError> {
        self.resolve(rel)
    }
}

#[async_trait]
impl Fetcher for LocalFileFetcher {
    async fn fetch(
        &self,
        req: &FetchRequest,
        limits: &FetchLimits,
    ) -> Result<RawPayload, FetchError> {
        let path = self.resolve(&req.url)?;
        let bytes = std::fs::read(&path).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => {
                FetchError::NotFound(format!("nincs helyi fájl: {}", path.display()))
            }
            _ => FetchError::Transport(format!("helyi fájl olvasása ({}): {err}", path.display())),
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

/// Transzport szerinti fetcher-választó (S2c): a worker a
/// `source_policy.transport` alapján választ fetchert. Az obscura fetcher
/// csak akkor van jelen, ha az `obscura` feature be van kapcsolva és a
/// footballd inicializálta (lazy szerverindítás).
pub struct Fetchers {
    pub rust_http: RustHttpFetcher,
    pub fixture: FixtureFetcher,
    /// Helyi fájl-források gyökere (opcionális; a football-research API könyvtára).
    pub local_file: Option<LocalFileFetcher>,
    #[cfg(feature = "obscura")]
    pub obscura: Option<crate::obscura::ObscuraFetcher>,
}

impl Fetchers {
    pub fn new(rust_http: RustHttpFetcher, fixture: FixtureFetcher) -> Self {
        Self {
            rust_http,
            fixture,
            local_file: None,
            #[cfg(feature = "obscura")]
            obscura: None,
        }
    }

    /// Helyi fájl-forrás gyökerének beállítása (S38).
    pub fn with_local_file(mut self, root: &Path) -> Self {
        self.local_file = Some(LocalFileFetcher::new(root));
        self
    }

    #[cfg(feature = "obscura")]
    pub fn with_obscura(mut self, fetcher: crate::obscura::ObscuraFetcher) -> Self {
        self.obscura = Some(fetcher);
        self
    }

    /// A robots.txt kérései valódi forrásnál plain HTTP-vel mennek.
    pub fn rust_http(&self) -> &dyn Fetcher {
        &self.rust_http
    }

    pub fn for_transport(&self, transport: &Transport) -> Result<&dyn Fetcher, FetchError> {
        match transport {
            Transport::RustHttp => Ok(&self.rust_http),
            Transport::Fixture => Ok(&self.fixture),
            Transport::LocalFile => self
                .local_file
                .as_ref()
                .map(|f| f as &dyn Fetcher)
                .ok_or_else(|| FetchError::Transport("helyi fájl-forrás nincs beállítva".into())),
            #[cfg(feature = "obscura")]
            Transport::Obscura => self
                .obscura
                .as_ref()
                .map(|fetcher| fetcher as &dyn Fetcher)
                .ok_or_else(|| FetchError::Transport("obscura fetcher nincs inicializálva".into())),
            #[cfg(not(feature = "obscura"))]
            Transport::Obscura => Err(FetchError::Transport(
                "obscura feature nincs engedélyezve ebben a buildben".into(),
            )),
        }
    }
}
