use std::collections::HashMap;
use std::time::Duration;

use football_domain::error::FetchError;
use football_ingest::fetch::Fetcher;
use football_ingest::fetch::{FetchLimits, FetchRequest, FixtureFetcher};

fn limits() -> FetchLimits {
    FetchLimits {
        max_bytes: 1024,
        timeout: Duration::from_secs(5),
    }
}

fn req(key: &str) -> FetchRequest {
    FetchRequest {
        url: format!("fixture://{key}"),
        request_key: key.into(),
        endpoint_key: "match".into(),
    }
}

#[tokio::test]
async fn fixture_fetch_returns_payload_with_hash() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("day.json"), br#"{"ok":true}"#).unwrap();
    let fetcher = FixtureFetcher::from_dir(dir.path()).unwrap();
    let raw = fetcher.fetch(&req("day"), &limits()).await.unwrap();
    assert_eq!(raw.http_status, 200);
    assert_eq!(raw.bytes, br#"{"ok":true}"#);
    assert_eq!(raw.sha256.len(), 64);
}

#[tokio::test]
async fn missing_key_is_not_found() {
    let fetcher = FixtureFetcher::new(HashMap::new());
    let err = fetcher.fetch(&req("nincs"), &limits()).await.unwrap_err();
    assert!(matches!(err, FetchError::NotFound(_)));
}

#[tokio::test]
async fn oversize_payload_rejected() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("big.json"), vec![b'x'; 2048]).unwrap();
    let fetcher = FixtureFetcher::from_dir(dir.path()).unwrap();
    let err = fetcher.fetch(&req("big"), &limits()).await.unwrap_err();
    assert!(matches!(err, FetchError::Oversize(2048)));
}
