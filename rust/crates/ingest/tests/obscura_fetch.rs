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
                .set_body_raw(
                    r#"<html><head><title>t</title></head><body><div class="wcl-tableScore">x</div><script>document.body.setAttribute('data-rendered','yes')</script></body></html>"#,
                    "text/html",
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
        ready_marker: None,
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

/// A readiness-marker: a kliens-oldalon késleltetve beszúrt lista-marker
/// megjelenéséig várakozik a fetcher (Soccerway nap-feed minta).
#[tokio::test]
async fn local_cdp_ready_marker_waits_for_client_render() {
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
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"<html><head><title>t</title></head><body><div id="app"></div>
<script>
setTimeout(function () {
  var d = document.createElement('div');
  d.className = 'event__match';
  d.setAttribute('data-event-row', 'true');
  document.getElementById('app').appendChild(d);
}, 3000);
</script></body></html>"#,
            "text/html",
        ))
        .mount(&server)
        .await;

    let cfg = ObscuraConfig {
        bin: bin.into(),
        url: None,
        port: 19228,
        timeout_ms: 30000,
    };
    let mut srv = ObscuraServer::start(&cfg).await.unwrap();
    let fetcher = ObscuraFetcher::connect(srv.cdp_url(), Duration::from_secs(30))
        .await
        .unwrap();

    let req = FetchRequest {
        url: server.uri(),
        request_key: "marker".into(),
        endpoint_key: "e".into(),
        ready_marker: Some("div.event__match[data-event-row]".into()),
    };
    let limits = FetchLimits {
        max_bytes: 5_000_000,
        timeout: Duration::from_secs(30),
    };
    let raw = fetcher.fetch(&req, &limits).await.unwrap();
    assert_eq!(raw.http_status, 200);
    let html = String::from_utf8_lossy(&raw.bytes);
    assert!(
        html.contains("data-event-row=\"true\""),
        "a marker-poll nem várta meg a kliens-renderelt sort"
    );
    srv.stop();
}
