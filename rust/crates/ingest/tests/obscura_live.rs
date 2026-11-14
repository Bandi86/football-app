#![cfg(feature = "obscura")]

//! Élő render teszt (opt-in): csak `OBSCURA_LIVE_TEST=1` esetén fut.
//! Robots-engedett Soccerway /matches/ oldal (S1 spike: a Disallow lista nem
//! tartalmazza); 1 obscura renderelés + 1 reqwest összehasonlítás. Default
//! futásban 0 élő kérés.

use std::time::Duration;

use football_ingest::fetch::{FetchLimits, FetchRequest, Fetcher};
use football_ingest::obscura::{ObscuraConfig, ObscuraFetcher, server::ObscuraServer};

const TARGET: &str = "https://www.soccerway.com/matches/";
const MARKER: &str = "wcl-tableScore";

fn obscura_bin() -> String {
    std::env::var("OBSCURA_BIN").unwrap_or_else(|_| {
        format!(
            "{}/../../../../football-app/tools/obscura/obscura",
            env!("CARGO_MANIFEST_DIR")
        )
    })
}

#[tokio::test]
async fn live_soccerway_render_surplus() {
    if std::env::var("OBSCURA_LIVE_TEST").as_deref() != Ok("1") {
        return;
    }
    let bin = obscura_bin();
    if !std::path::Path::new(&bin).exists() {
        eprintln!("skip: nincs obscura bináris ({bin})");
        return;
    }

    let cfg = ObscuraConfig {
        bin: bin.into(),
        url: None,
        port: 19224,
        timeout_ms: 90000,
    };
    let mut srv = ObscuraServer::start(&cfg).await.unwrap();
    let fetcher = ObscuraFetcher::connect(srv.cdp_url(), Duration::from_millis(cfg.timeout_ms))
        .await
        .unwrap();
    let req = FetchRequest {
        url: TARGET.into(),
        request_key: "live".into(),
        endpoint_key: "matches".into(),
    };
    let limits = FetchLimits {
        max_bytes: 20_000_000,
        timeout: Duration::from_secs(90),
    };
    let raw = fetcher.fetch(&req, &limits).await.unwrap();
    srv.stop();

    let obscura_markers = String::from_utf8_lossy(&raw.bytes).matches(MARKER).count();
    assert_eq!(raw.http_status, 200);
    assert!(
        obscura_markers > 0,
        "az obscura renderelés nem adott markert"
    );

    // Összehasonlítás: a nyers HTTP válasz markerei (a tartalmi többlet igazolása).
    let client = reqwest::Client::builder()
        .user_agent("football-app-v2/0.1")
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    let http_html = client
        .get(TARGET)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let http_markers = http_html.matches(MARKER).count();
    eprintln!("obscura markerek: {obscura_markers}, reqwest markerek: {http_markers}");
    assert!(
        obscura_markers > http_markers,
        "nincs igazolt tartalmi többlet a nyers HTTP-hez képest"
    );
}
