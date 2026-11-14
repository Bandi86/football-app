//! Obscura browser-fetcher réteg (S2c) — opt-in, az `obscura` cargo feature
//! mögött. Default KI; a chromiumoxide nehéz függőség csak ezzel húzódik be.
//!
//! Az Obscura egy Rust headless engine (V8, CDP-kompatibilis), a JS-renderelt
//! forrásokhoz (pl. Soccerway). A szállítás opt-in: a worker a
//! `source_policy.transport = 'obscura'` alapján választja. Stealth KI
//! (charter 9. pont); a worker robots/budget/breaker őrei változatlanok.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use football_domain::error::FetchError;
use sha2::Digest;
use thiserror::Error;

use crate::fetch::{FetchLimits, FetchRequest, Fetcher, RawPayload};

mod cdp;
pub mod server;

use cdp::CdpClient;

/// Az Obscura réteg típusos hibái. A `Fetcher::fetch` ezeket `FetchError`-ré
/// képezi (`into_fetch`), hogy a worker a meglévő hibautakon kezelje.
#[derive(Debug, Error)]
pub enum ObscuraError {
    #[error("obscura szerver indítási hiba: {0}")]
    ServerStart(String),
    #[error("obscura bináris nem található: {0}")]
    BinaryNotFound(String),
    #[error("CDP hiba: {0}")]
    Cdp(String),
    #[error("nincs HTTP-státusz a CDP válaszból")]
    NoStatus,
    #[error("obscura fetch időtúllépés")]
    Timeout,
}

impl ObscuraError {
    /// A worker hibautakhoz: a Timeout kivételével minden Transport
    /// (fail-closed — a breaker státusz nélkül nem enged át).
    pub fn into_fetch(self) -> FetchError {
        match self {
            Self::Timeout => FetchError::Timeout,
            other => FetchError::Transport(other.to_string()),
        }
    }
}

/// Obscura konfiguráció (env). A `from_env_with` injektálható olvasóval
/// tesztelhető (nincs globális env-függőség a unit tesztekben).
#[derive(Debug, Clone)]
pub struct ObscuraConfig {
    pub bin: PathBuf,
    pub url: Option<String>,
    pub port: u16,
    pub timeout_ms: u64,
}

impl ObscuraConfig {
    pub fn from_env() -> Self {
        Self::from_env_with(|k| std::env::var(k).ok())
    }

    pub fn from_env_with(get: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            bin: PathBuf::from(
                get("OBSCURA_BIN").unwrap_or_else(|| "tools/obscura/obscura".into()),
            ),
            url: get("OBSCURA_URL").filter(|s| !s.is_empty()),
            port: get("OBSCURA_PORT")
                .and_then(|v| v.parse().ok())
                .unwrap_or(9222),
            timeout_ms: get("OBSCURA_TIMEOUT_MS")
                .and_then(|v| v.parse().ok())
                .unwrap_or(45000),
        }
    }
}

/// CDP-alapú browser-fetcher az Obscura serve mögött. A Fetcher trait mögé
/// illeszkedik, így a worker robots/budget/breaker/delay őrei változatlanok.
pub struct ObscuraFetcher {
    client: CdpClient,
    timeout: Duration,
}

impl ObscuraFetcher {
    pub async fn connect(cdp_url: &str, timeout: Duration) -> Result<Self, ObscuraError> {
        Ok(Self {
            client: CdpClient::connect(cdp_url).await?,
            timeout,
        })
    }
}

impl ObscuraFetcher {
    /// A renderelési lánc magja: oldal → Network/Page.enable → navigate →
    /// dokumentum-státusz → settle → renderelt DOM. A hívó lezárja az oldalt.
    async fn render(&self, req: &FetchRequest) -> Result<(u16, Vec<u8>), FetchError> {
        let page = self.client.open_page().await.map_err(|e| e.into_fetch())?;
        let outcome = async {
            self.client
                .enable_network(&page)
                .await
                .map_err(|e| e.into_fetch())?;
            self.client
                .enable_page(&page)
                .await
                .map_err(|e| e.into_fetch())?;
            self.client
                .navigate(&page, &req.url)
                .await
                .map_err(|e| e.into_fetch())?;

            // A státusz a fő dokumentum-válaszból jön (redirect-lánc miatt
            // nem URL-re illesztünk); ugyanez az olvasó várja a load
            // eseményt is, mert az Obscura a navigate válaszát a betöltésig
            // visszatarthatja — az esemény a biztos jel.
            let status = self
                .client
                .wait_for_page_ready(&page, Duration::from_secs(20))
                .await
                .map_err(|e| e.into_fetch())?;

            // A spike szerinti 1.5 s settle a stabil renderelt DOM-hoz
            // (előtte "Cannot find context" hibák).
            tokio::time::sleep(Duration::from_millis(1500)).await;

            let expression = "document.documentElement.outerHTML";
            let mut result = self.client.evaluate_string(&page, expression).await;
            for _ in 0..3 {
                if result.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
                result = self.client.evaluate_string(&page, expression).await;
            }
            let html = result.map_err(|e| e.into_fetch())?;
            Ok((status, html.into_bytes()))
        }
        .await;
        // A hibás fetch is zárja be az oldalt (ne szivárogjon target).
        self.client
            .close_page(&page)
            .await
            .map_err(|e| e.into_fetch())?;
        outcome
    }
}

#[async_trait]
impl Fetcher for ObscuraFetcher {
    async fn fetch(
        &self,
        req: &FetchRequest,
        limits: &FetchLimits,
    ) -> Result<RawPayload, FetchError> {
        let fut = async {
            self.client.drain_events().await;
            let (http_status, bytes) = self.render(req).await?;
            if bytes.len() as u64 > limits.max_bytes {
                return Err(FetchError::Oversize(bytes.len() as u64));
            }
            Ok(RawPayload {
                sha256: hex::encode(sha2::Sha256::digest(&bytes)),
                bytes,
                content_type: "text/html".to_string(),
                http_status,
                fetched_at: chrono::Utc::now(),
            })
        };
        match tokio::time::timeout(self.timeout.min(limits.timeout), fut).await {
            Ok(res) => res,
            Err(_) => Err(FetchError::Timeout),
        }
    }
}
