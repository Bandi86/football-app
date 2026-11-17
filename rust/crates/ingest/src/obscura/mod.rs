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
/// Opcionálisan kezelt Obscura-szerver: health-check + motor-újraindítás
/// (a launchd alatt futó automata körben a motor-gyermek elhalhat; a
/// fetcher ezt észleli és újraindítja, majd újracsatlakozik CDP-n).
struct ManagedServer {
    cfg: ObscuraConfig,
    server: tokio::sync::Mutex<Option<server::ObscuraServer>>,
}

pub struct ObscuraFetcher {
    client: tokio::sync::Mutex<CdpClient>,
    managed: Option<std::sync::Arc<ManagedServer>>,
    health_url: String,
    timeout: Duration,
}

fn http_base(cfg: &ObscuraConfig) -> String {
    cfg.url
        .clone()
        .unwrap_or_else(|| format!("http://127.0.0.1:{}", cfg.port))
        .trim_end_matches('/')
        .to_string()
}

impl ObscuraFetcher {
    /// Külső (már futó) szerverhez csatlakozás — nincs önjavítás.
    pub async fn connect(cdp_url: &str, timeout: Duration) -> Result<Self, ObscuraError> {
        let client = CdpClient::connect(cdp_url).await?;
        let base = cdp_url
            .trim_end_matches("/devtools/browser")
            .trim_end_matches('/')
            .replacen("ws://", "http://", 1)
            .replacen("wss://", "https://", 1);
        Ok(Self {
            client: tokio::sync::Mutex::new(client),
            managed: None,
            health_url: format!("{base}/json/version"),
            timeout,
        })
    }

    /// Saját életciklusú szerver: ha a motor elhal vagy a CDP-kapcsolat
    /// megszakad, a fetcher újraindítja és újracsatlakozik.
    pub async fn connect_managed(cfg: &ObscuraConfig) -> Result<Self, ObscuraError> {
        let server = server::ObscuraServer::start(cfg).await?;
        let client = CdpClient::connect(server.cdp_url()).await?;
        Ok(Self {
            client: tokio::sync::Mutex::new(client),
            managed: Some(std::sync::Arc::new(ManagedServer {
                cfg: cfg.clone(),
                server: tokio::sync::Mutex::new(Some(server)),
            })),
            health_url: format!("{}/json/version", http_base(cfg)),
            timeout: Duration::from_millis(cfg.timeout_ms),
        })
    }

    /// Motor leállítása (daemon/CLI leálláskor).
    pub async fn shutdown(&self) {
        if let Some(managed) = &self.managed
            && let Some(mut srv) = managed.server.lock().await.take()
        {
            srv.stop();
        }
    }

    async fn health_ok(&self) -> bool {
        reqwest::Client::new()
            .get(&self.health_url)
            .timeout(Duration::from_millis(800))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Ha a motor halott vagy a CDP-kapcsolat bezárt: újraindítás + reconnect.
    async fn ensure_ready(&self) -> Result<(), FetchError> {
        let healthy = self.health_ok().await;
        let closed = self.client.lock().await.is_closed();
        if healthy && !closed {
            return Ok(());
        }
        self.restart().await
    }

    async fn restart(&self) -> Result<(), FetchError> {
        let Some(managed) = &self.managed else {
            return Err(FetchError::Transport(
                "obscura kapcsolat megszakadt (külső szerver)".into(),
            ));
        };
        tracing::warn!("obscura motor újraindítás (health/CDP hiba)");
        {
            let mut slot = managed.server.lock().await;
            if let Some(mut old) = slot.take() {
                old.stop();
            }
        }
        let server = server::ObscuraServer::start(&managed.cfg)
            .await
            .map_err(|e| FetchError::Transport(format!("motor újraindítás: {e}")))?;
        let cdp = server.cdp_url().to_string();
        *managed.server.lock().await = Some(server);
        let client = CdpClient::connect(&cdp)
            .await
            .map_err(|e| FetchError::Transport(format!("CDP újracsatlakozás: {e}")))?;
        *self.client.lock().await = client;
        Ok(())
    }

    /// A renderelési lánc magja: oldal → Network/Page.enable → navigate →
    /// dokumentum-státusz → settle → renderelt DOM. A hívó lezárja az oldalt.
    async fn render(
        &self,
        client: &CdpClient,
        req: &FetchRequest,
    ) -> Result<(u16, Vec<u8>), FetchError> {
        let page = client.open_page().await.map_err(|e| e.into_fetch())?;
        let outcome = async {
            client
                .enable_network(&page)
                .await
                .map_err(|e| e.into_fetch())?;
            client
                .enable_page(&page)
                .await
                .map_err(|e| e.into_fetch())?;
            client
                .navigate(&page, &req.url)
                .await
                .map_err(|e| e.into_fetch())?;

            let status = client
                .wait_for_page_ready(&page, Duration::from_secs(20))
                .await
                .map_err(|e| e.into_fetch())?;

            // A spike szerinti 1.5 s settle a stabil renderelt DOM-hoz.
            tokio::time::sleep(Duration::from_millis(1500)).await;

            // Kliens-oldali lista: marker-várakozás; ha nem jön meg,
            // TRANZIENS hiba (fél oldalt nem adunk a parsernek).
            if let Some(marker) = &req.ready_marker {
                let selector = serde_json::to_string(marker)
                    .map_err(|e| FetchError::Transport(format!("marker-szelektor: {e}")))?;
                let check =
                    format!("document.querySelector({selector}) !== null ? 'true' : 'false'");
                let wait = Duration::from_secs(20)
                    .min(self.timeout.saturating_sub(Duration::from_secs(2)));
                let deadline = tokio::time::Instant::now() + wait;
                let mut ready = false;
                loop {
                    match client.evaluate_string(&page, &check).await {
                        Ok(value) if value == "true" => {
                            ready = true;
                            break;
                        }
                        Ok(_) | Err(_) if tokio::time::Instant::now() < deadline => {
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                        _ => break,
                    }
                }
                if !ready {
                    return Err(FetchError::Timeout);
                }
            }

            let expression = "document.documentElement.outerHTML";
            let mut result = client.evaluate_string(&page, expression).await;
            for _ in 0..3 {
                if result.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
                result = client.evaluate_string(&page, expression).await;
            }
            let html = result.map_err(|e| e.into_fetch())?;
            Ok((status, html.into_bytes()))
        }
        .await;
        // A hibás fetch is zárja be az oldalt (ne szivárogjon target).
        client.close_page(&page).await.map_err(|e| e.into_fetch())?;
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
            self.ensure_ready().await?;
            let mut last_error: Option<FetchError> = None;
            for attempt in 0..2 {
                let client = self.client.lock().await;
                client.drain_events().await;
                match self.render(&client, req).await {
                    Ok((http_status, bytes)) => {
                        drop(client);
                        if bytes.len() as u64 > limits.max_bytes {
                            return Err(FetchError::Oversize(bytes.len() as u64));
                        }
                        return Ok(RawPayload {
                            sha256: hex::encode(sha2::Sha256::digest(&bytes)),
                            bytes,
                            content_type: "text/html".to_string(),
                            http_status,
                            fetched_at: chrono::Utc::now(),
                        });
                    }
                    Err(err @ FetchError::Transport(_)) if attempt == 0 => {
                        drop(client);
                        last_error = Some(err);
                        self.restart().await?;
                    }
                    Err(err) => return Err(err),
                }
            }
            Err(last_error.unwrap_or(FetchError::Transport("obscura hiba".into())))
        };
        match tokio::time::timeout(self.timeout.min(limits.timeout), fut).await {
            Ok(res) => res,
            Err(_) => Err(FetchError::Timeout),
        }
    }
}
