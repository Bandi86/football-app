//! Obscura szerver-lifecycle (S2c Task 2).
//!
//! Lazy indítás: ha `OBSCURA_URL` be van állítva, külső szervert használunk
//! (csak health-check); egyébként a lokális binárist indítjuk és
//! health-checkeljük. Leállításkor a `stop()` kill+wait-eli a child
//! processzt; external módban no-op.

use std::process::{Child, Command, Stdio};
use std::time::Duration;

use crate::obscura::{ObscuraConfig, ObscuraError};

/// Managed Obscura szerver: lokális child processz vagy külső URL.
#[derive(Debug)]
pub struct ObscuraServer {
    child: Option<Child>,
    cdp_url: String,
    external: bool,
}

impl ObscuraServer {
    /// Lazy indítás + health-check (`/json/version` poll, max ~10 s).
    pub async fn start(cfg: &ObscuraConfig) -> Result<Self, ObscuraError> {
        let (child, external, base) = match &cfg.url {
            Some(url) => (None, true, url.trim_end_matches('/').to_string()),
            None => {
                if !cfg.bin.exists() {
                    return Err(ObscuraError::BinaryNotFound(cfg.bin.display().to_string()));
                }
                let child = Command::new(&cfg.bin)
                    .args([
                        "serve",
                        "--port",
                        &cfg.port.to_string(),
                        "--host",
                        "127.0.0.1",
                        "--workers",
                        "1",
                        "--user-agent",
                        "football-app-v2",
                        // spec S2c terv-jegyzet: a lokális CDP teszthez
                        // (loopback wiremock) kell; élesben a worker robots
                        // + policy őre csak engedélyezett URL-t ad át.
                        "--allow-private-network",
                    ])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| ObscuraError::ServerStart(e.to_string()))?;
                (Some(child), false, format!("http://127.0.0.1:{}", cfg.port))
            }
        };
        let cdp_url = format!("{}/devtools/browser", ws_base(&base));
        let version_url = format!("{base}/json/version");
        let client = reqwest::Client::new();
        let mut ok = false;
        for _ in 0..20 {
            if let Ok(resp) = client
                .get(&version_url)
                .timeout(Duration::from_millis(500))
                .send()
                .await
            {
                if resp.status().is_success() {
                    ok = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        if !ok {
            if let Some(mut c) = child {
                let _ = c.kill();
                let _ = c.wait();
            }
            return Err(ObscuraError::ServerStart(format!(
                "health-check timeout: {version_url}"
            )));
        }
        Ok(Self {
            child,
            cdp_url,
            external,
        })
    }

    /// CDP endpoint URL (az ObscuraFetcher connect ehhez kapja).
    pub fn cdp_url(&self) -> &str {
        &self.cdp_url
    }

    /// Külső szerver módban van-e (OBSCURA_URL override).
    pub fn is_external(&self) -> bool {
        self.external
    }

    /// Leállítás: lokális child processz kill+wait; external módban no-op.
    pub fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

impl Drop for ObscuraServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// CDP-websocket alap a health-check HTTP-alapjából (http → ws, https → wss).
fn ws_base(base: &str) -> String {
    if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        base.to_string()
    }
}
