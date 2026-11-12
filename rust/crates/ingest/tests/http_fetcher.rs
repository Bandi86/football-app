use std::time::Duration;

use football_domain::error::FetchError;
use football_ingest::fetch::{FetchLimits, FetchRequest, Fetcher};
use football_ingest::http::RustHttpFetcher;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn limits() -> FetchLimits {
    FetchLimits {
        max_bytes: 1024,
        timeout: Duration::from_secs(2),
    }
}

#[tokio::test]
async fn fetch_200_with_honest_ua() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/data.json"))
        .and(header("user-agent", "football-app-v2/0.1"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"ok":true}"#))
        .mount(&server)
        .await;
    let fetcher = RustHttpFetcher::new("football-app-v2/0.1").unwrap();
    let req = FetchRequest {
        url: format!("{}/data.json", server.uri()),
        request_key: "k".into(),
        endpoint_key: "match".into(),
    };
    let raw = fetcher.fetch(&req, &limits()).await.unwrap();
    assert_eq!(raw.http_status, 200);
    assert_eq!(raw.sha256.len(), 64);
}

#[tokio::test]
async fn http_error_status_is_typed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let fetcher = RustHttpFetcher::new("football-app-v2/0.1").unwrap();
    let req = FetchRequest {
        url: server.uri(),
        request_key: "k".into(),
        endpoint_key: "m".into(),
    };
    let err = fetcher.fetch(&req, &limits()).await.unwrap_err();
    assert!(matches!(err, FetchError::HttpStatus(403)));
}

#[tokio::test]
async fn oversize_body_rejected() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("x".repeat(2048)))
        .mount(&server)
        .await;
    let fetcher = RustHttpFetcher::new("football-app-v2/0.1").unwrap();
    let req = FetchRequest {
        url: server.uri(),
        request_key: "k".into(),
        endpoint_key: "m".into(),
    };
    let err = fetcher.fetch(&req, &limits()).await.unwrap_err();
    assert!(matches!(err, FetchError::Oversize(_)));
}

#[tokio::test]
async fn timeout_is_typed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
        .mount(&server)
        .await;
    let fetcher = RustHttpFetcher::new("football-app-v2/0.1").unwrap();
    let req = FetchRequest {
        url: server.uri(),
        request_key: "k".into(),
        endpoint_key: "m".into(),
    };
    let err = fetcher.fetch(&req, &limits()).await.unwrap_err();
    assert!(matches!(err, FetchError::Timeout));
}

/// Streaming méretplafon: a válasz nem tartalmaz Content-Length fejlécet
/// (chunked átvitel), így a plafont letöltés közben, chunkonként kell érvényesíteni.
#[tokio::test]
async fn oversize_chunked_without_content_length() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("transfer-encoding", "chunked")
                .set_body_string("x".repeat(4096)),
        )
        .mount(&server)
        .await;
    let fetcher = RustHttpFetcher::new("football-app-v2/0.1").unwrap();
    let req = FetchRequest {
        url: server.uri(),
        request_key: "k".into(),
        endpoint_key: "m".into(),
    };
    let err = fetcher.fetch(&req, &limits()).await.unwrap_err();
    assert!(matches!(err, FetchError::Oversize(_)));
}
