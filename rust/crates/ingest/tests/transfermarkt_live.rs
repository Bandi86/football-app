#![cfg(feature = "obscura")]

//! Élő spike (opt-in): Transfermarkt sérüléslista renderelése Obscurával.
//! Csak `OBSCURA_LIVE_TEST=1` esetén fut; default futásban 0 élő kérés.
//! A cél annak eldöntése, hogy a plain HTTP-n üres tbody JS-renderrel
//! megtelik-e (a katalógus 2026-11-11-i mérése óta a kiszolgálás változott).

use std::time::Duration;

use football_ingest::fetch::{FetchLimits, FetchRequest, Fetcher};
use football_ingest::obscura::{ObscuraConfig, ObscuraFetcher, server::ObscuraServer};

const TARGET: &str = "https://www.transfermarkt.com/laliga/verletztespieler/wettbewerb/ES1";
const MARKER: &str = "profil/spieler";

fn obscura_bin() -> String {
    std::env::var("OBSCURA_BIN").unwrap_or_else(|_| {
        format!(
            "{}/../../../../football-app/tools/obscura/obscura",
            env!("CARGO_MANIFEST_DIR")
        )
    })
}

#[tokio::test]
async fn live_transfermarkt_render_probe() {
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
        port: 19225,
        timeout_ms: 90000,
    };
    let mut srv = ObscuraServer::start(&cfg).await.unwrap();
    let fetcher = ObscuraFetcher::connect(srv.cdp_url(), Duration::from_millis(cfg.timeout_ms))
        .await
        .unwrap();
    let req = FetchRequest {
        url: TARGET.into(),
        request_key: "live".into(),
        endpoint_key: "injuries".into(),
        ready_marker: None,
        bearer_token: None,
    };
    let limits = FetchLimits {
        max_bytes: 20_000_000,
        timeout: Duration::from_secs(90),
    };
    let raw = fetcher.fetch(&req, &limits).await.unwrap();
    srv.stop();

    let html = String::from_utf8_lossy(&raw.bytes).to_string();
    let spieler_links = html.matches(MARKER).count();
    let rows =
        html.matches("<tr class=\"odd\"").count() + html.matches("<tr class=\"even\"").count();
    println!(
        "http={} bytes={} spieler-links={} rows={}",
        raw.http_status,
        raw.bytes.len(),
        spieler_links,
        rows
    );
    assert_eq!(raw.http_status, 200);
    assert!(
        spieler_links > 5,
        "a renderelt DOM sem adott sérülés-sorokat (spieler-links={spieler_links})"
    );
}
