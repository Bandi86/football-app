#![cfg(feature = "obscura")]

use std::path::PathBuf;

use football_ingest::obscura::{ObscuraConfig, ObscuraError, server::ObscuraServer};

#[tokio::test]
async fn missing_binary_is_binary_not_found() {
    let cfg = ObscuraConfig {
        bin: PathBuf::from("/nonexistent/obscura"),
        url: None,
        port: 19222,
        timeout_ms: 5000,
    };
    let err = ObscuraServer::start(&cfg).await.unwrap_err();
    assert!(matches!(err, ObscuraError::BinaryNotFound(_)));
}

#[tokio::test]
async fn external_url_skips_process_start() {
    // External URL esetén nem indít processzt; ha a URL nem él,
    // health-check hibát ad. A bináris nem létezik, de external módban
    // nem BinaryNotFound (nem indít processzt).
    let cfg = ObscuraConfig {
        bin: PathBuf::from("/nonexistent"),
        url: Some("http://127.0.0.1:19999".into()),
        port: 19999,
        timeout_ms: 1000,
    };
    let err = ObscuraServer::start(&cfg).await.unwrap_err();
    assert!(matches!(err, ObscuraError::ServerStart(_)));
}
