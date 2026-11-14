#![cfg(feature = "obscura")]

use std::time::Duration;

use football_ingest::fetch::{FetchLimits, FetchRequest, Fetcher};
use football_ingest::obscura::{ObscuraConfig, ObscuraFetcher, server::ObscuraServer};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Lokális CDP teszt: obscura serve + wiremock statikus HTML.
/// Csak ha az Obscura bináris elérhető (különben skip). Nincs élő külső kérés.
#[tokio::test]
async fn local_cdp_fetch_returns_status_and_content() {
    let default_bin = format!(
        "{}/../../../../football-app/tools/obscura/obscura",
        env!("CARGO_MANIFEST_DIR")
    );
    let bin = std::env::var("OBSCURA_BIN").unwrap_or(default_bin);
    if !std::path::Path::new(&bin).exists() {
        eprintln!("skip: nincs obscura bináris ({bin})");
        return;
    }

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html")
                .set_body_string(
                    r#"<html><body><div class="wcl-tableScore">x</div></body></html>"#,
                ),
        )
        .mount(&server)
        .await;

    let cfg = ObscuraConfig {
        bin: bin.into(),
        url: None,
        port: 19223,
        timeout_ms: 30000,
    };
    let mut srv = ObscuraServer::start(&cfg).await.unwrap();
    let fetcher = ObscuraFetcher::connect(srv.cdp_url(), Duration::from_secs(30))
        .await
        .unwrap();

    let req = FetchRequest {
        url: server.uri(),
        request_key: "k".into(),
        endpoint_key: "e".into(),
    };
    let limits = FetchLimits {
        max_bytes: 5_000_000,
        timeout: Duration::from_secs(30),
    };
    let raw = fetcher.fetch(&req, &limits).await.unwrap();
    assert_eq!(raw.http_status, 200);
    assert!(String::from_utf8_lossy(&raw.bytes).contains("wcl-tableScore"));
    assert_eq!(raw.sha256.len(), 64);
    srv.stop();
}
