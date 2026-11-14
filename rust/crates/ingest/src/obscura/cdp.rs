//! Minimál CDP-kliens az Obscura serve-hez (S2c Task 3).
//!
//! A chromiumoxide tipizált esemény-deszerializációja elbukik az Obscura
//! payloadján (`Network.responseReceived`-ből hiányoznak a kötelező mezők:
//! charset, connectionReused, connectionId, encodedDataLength, securityState),
//! ezért az egész CDP-üzenet eldobódik. Ez a kliens a nyers JSON-t olvassa
//! (serde_json::Value), így az Obscura válasz-státusza és a renderelt DOM
//! egyaránt elérhető. A protokoll-lépések nyers CDP-probe-lal igazoltak
//! (2026-11-14): createTarget → attachToTarget (flatten) → Network.enable →
//! Page.navigate → Network.responseReceived (status) → Runtime.evaluate
//! (outerHTML) → Target.closeTarget.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_tungstenite::tungstenite::Message;
use futures::StreamExt;
use serde_json::{Value, json};
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::obscura::ObscuraError;

/// A `Page.navigate` válaszára várakozás felső korlátja (Obscura lassú load).
const NAVIGATE_WAIT: Duration = Duration::from_secs(20);
/// Egy `Runtime.evaluate` kérés felső korlátja (beragadó oldal ellen).
const EVALUATE_WAIT: Duration = Duration::from_secs(15);
/// A best-effort oldalbezárás felső korlátja.
const CLOSE_WAIT: Duration = Duration::from_secs(5);

/// Egy nyitott oldal (target + CDP-session) azonosítói.
pub struct CdpPage {
    target_id: String,
    session_id: String,
}

/// Websocket-alapú CDP-kliens: id-alapú kérés/válasz + eseménycsatorna.
pub struct CdpClient {
    cmd_tx: mpsc::UnboundedSender<(String, oneshot::Sender<Value>)>,
    events: Mutex<mpsc::UnboundedReceiver<Value>>,
    next_id: AtomicU64,
}

impl CdpClient {
    pub async fn connect(url: &str) -> Result<Self, ObscuraError> {
        let (ws, _) = async_tungstenite::tokio::connect_async(url)
            .await
            .map_err(|e| ObscuraError::ServerStart(format!("CDP kapcsolat: {e}")))?;
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<(String, oneshot::Sender<Value>)>();
        let (event_tx, event_rx) = mpsc::unbounded_channel::<Value>();

        tokio::spawn(async move {
            let (mut sink, mut stream) = ws.split();
            let mut pending: HashMap<u64, oneshot::Sender<Value>> = HashMap::new();
            loop {
                tokio::select! {
                    cmd = cmd_rx.recv() => {
                        let Some((text, tx)) = cmd else { break };
                        let id = serde_json::from_str::<Value>(&text)
                            .ok()
                            .and_then(|v| v.get("id").and_then(Value::as_u64));
                        if sink.send(Message::Text(text.into())).await.is_err() {
                            let _ = tx.send(connection_closed());
                            break;
                        }
                        if let Some(id) = id {
                            pending.insert(id, tx);
                        }
                    }
                    msg = stream.next() => {
                        let text = match msg {
                            Some(Ok(Message::Text(text))) => text.to_string(),
                            Some(Ok(Message::Binary(bytes))) => match String::from_utf8(bytes.to_vec()) {
                                Ok(text) => text,
                                Err(_) => continue,
                            },
                            Some(Ok(_)) => continue,
                            Some(Err(_)) | None => break,
                        };
                        let Ok(value) = serde_json::from_str::<Value>(&text) else { continue };
                        match value.get("id").and_then(Value::as_u64) {
                            Some(id) => {
                                if let Some(tx) = pending.remove(&id) {
                                    let _ = tx.send(value);
                                }
                            }
                            None => {
                                let _ = event_tx.send(value);
                            }
                        }
                    }
                }
            }
            for (_, tx) in pending {
                let _ = tx.send(connection_closed());
            }
        });

        Ok(Self {
            cmd_tx,
            events: Mutex::new(event_rx),
            next_id: AtomicU64::new(1),
        })
    }

    async fn request(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
    ) -> Result<Value, ObscuraError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            message["sessionId"] = json!(session);
        }
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send((message.to_string(), tx))
            .map_err(|_| connection_closed_err())?;
        let response = rx.await.map_err(|_| connection_closed_err())?;
        if let Some(error) = response.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("ismeretlen CDP hiba");
            return Err(ObscuraError::Cdp(format!("{method}: {message}")));
        }
        Ok(response)
    }

    /// Új oldal `about:blank`-kel + flatten attach.
    pub async fn open_page(&self) -> Result<CdpPage, ObscuraError> {
        let response = self
            .request("Target.createTarget", json!({ "url": "about:blank" }), None)
            .await?;
        let target_id = response
            .pointer("/result/targetId")
            .and_then(Value::as_str)
            .ok_or_else(|| ObscuraError::Cdp("createTarget: hiányzó targetId".into()))?
            .to_string();
        let response = self
            .request(
                "Target.attachToTarget",
                json!({ "targetId": target_id, "flatten": true }),
                None,
            )
            .await?;
        let session_id = response
            .pointer("/result/sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| ObscuraError::Cdp("attachToTarget: hiányzó sessionId".into()))?
            .to_string();
        Ok(CdpPage {
            target_id,
            session_id,
        })
    }

    pub async fn enable_network(&self, page: &CdpPage) -> Result<(), ObscuraError> {
        self.request("Network.enable", json!({}), Some(&page.session_id))
            .await
            .map(|_| ())
    }

    /// A Page domain engedélyezése (Obscura: Page.enable nélkül a
    /// Page.navigate válasza nem érkezik meg — probe-igazolt, 2026-11-14).
    pub async fn enable_page(&self, page: &CdpPage) -> Result<(), ObscuraError> {
        self.request("Page.enable", json!({}), Some(&page.session_id))
            .await
            .map(|_| ())
    }

    /// Navigáció. Az Obscura a `Page.navigate` válaszát a teljes betöltésig
    /// (időnként >45 s) visszatarthatja, ezért legfeljebb `NAVIGATE_WAIT`
    /// ideig várunk rá; utána a Document response esemény dönt a státuszról.
    pub async fn navigate(&self, page: &CdpPage, url: &str) -> Result<(), ObscuraError> {
        let response = tokio::time::timeout(
            NAVIGATE_WAIT,
            self.request(
                "Page.navigate",
                json!({ "url": url }),
                Some(&page.session_id),
            ),
        )
        .await;
        match response {
            Ok(result) => {
                let response = result?;
                if let Some(text) = response
                    .pointer("/result/errorText")
                    .and_then(Value::as_str)
                {
                    return Err(ObscuraError::Cdp(format!("navigáció: {text}")));
                }
                Ok(())
            }
            Err(_) => Ok(()),
        }
    }

    /// JS-kiértékelés (pl. `document.documentElement.outerHTML`) string eredménnyel.
    pub async fn evaluate_string(
        &self,
        page: &CdpPage,
        expression: &str,
    ) -> Result<String, ObscuraError> {
        let response = tokio::time::timeout(
            EVALUATE_WAIT,
            self.request(
                "Runtime.evaluate",
                json!({ "expression": expression, "returnByValue": true }),
                Some(&page.session_id),
            ),
        )
        .await
        .map_err(|_| ObscuraError::Timeout)??;
        response
            .pointer("/result/result/value")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| {
                ObscuraError::Cdp(format!("evaluate: nem string eredmény ({expression})"))
            })
    }

    /// Best-effort oldalbezárás (a hiba nem nyelődik el: a hívó dönt).
    pub async fn close_page(&self, page: &CdpPage) -> Result<(), ObscuraError> {
        let result = tokio::time::timeout(
            CLOSE_WAIT,
            self.request(
                "Target.closeTarget",
                json!({ "targetId": page.target_id }),
                None,
            ),
        )
        .await
        .map_err(|_| ObscuraError::Timeout)?;
        result.map(|_| ())
    }

    /// A `Page.loadEventFired` esemény a page-sessionen (best-effort,
    /// Korábbi oldalakról visszamaradt események eldobása.
    pub async fn drain_events(&self) {
        let mut events = self.events.lock().await;
        while events.try_recv().is_ok() {}
    }

    /// Megvárja a fő dokumentum-válasz státuszát és a load eseményt egyetlen
    /// esemény-olvasóval, maximum `timeout` ideig.
    ///
    /// A redirect-lánc miatt nem URL-re illesztünk, hanem a legutolsó
    /// `type == "Document"` válaszra (a /matches/ → / átirányítás így is
    /// 200-at ad). Ha a státusz nem-átirányítás és csendes ablak következik,
    /// a load esemény nélkül is továbblépünk (best-effort).
    pub async fn wait_for_page_ready(
        &self,
        page: &CdpPage,
        timeout: Duration,
    ) -> Result<u16, ObscuraError> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut events = self.events.lock().await;
        let mut last: Option<u16> = None;
        let mut loaded = false;
        while tokio::time::Instant::now() < deadline && !(loaded && last.is_some()) {
            match tokio::time::timeout(Duration::from_millis(300), events.recv()).await {
                Ok(Some(value)) => match value.get("method").and_then(Value::as_str) {
                    Some("Network.responseReceived")
                        if value.get("sessionId").and_then(Value::as_str)
                            == Some(page.session_id.as_str())
                            && value.pointer("/params/type").and_then(Value::as_str)
                                == Some("Document") =>
                    {
                        if let Some(status) = value
                            .pointer("/params/response/status")
                            .and_then(Value::as_u64)
                        {
                            last = Some(u16::try_from(status).map_err(|_| {
                                ObscuraError::Cdp(format!("érvénytelen státusz: {status}"))
                            })?);
                        }
                    }
                    Some("Page.loadEventFired")
                        if value.get("sessionId").and_then(Value::as_str)
                            == Some(page.session_id.as_str()) =>
                    {
                        loaded = true;
                    }
                    _ => {}
                },
                Ok(None) => break,
                Err(_) => {
                    // Csendes ablak: nem-átirányítás státusz esetén elegendő.
                    if matches!(last, Some(status) if !(300..400).contains(&status)) {
                        break;
                    }
                }
            }
        }
        last.ok_or(ObscuraError::NoStatus)
    }
}

fn connection_closed() -> Value {
    json!({ "error": { "message": "CDP kapcsolat lezárult" } })
}

fn connection_closed_err() -> ObscuraError {
    ObscuraError::Cdp("CDP kapcsolat lezárult".into())
}
