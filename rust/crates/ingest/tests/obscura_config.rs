#![cfg(feature = "obscura")]

use football_domain::error::FetchError;
use football_ingest::obscura::{ObscuraConfig, ObscuraError};

#[test]
fn config_defaults() {
    let cfg = ObscuraConfig::from_env_with(|_| None);
    assert_eq!(cfg.port, 9222);
    assert_eq!(cfg.timeout_ms, 45000);
    assert!(cfg.url.is_none());
    assert_eq!(cfg.bin.to_str().unwrap(), "tools/obscura/obscura");
}

#[test]
fn config_reads_env() {
    let cfg = ObscuraConfig::from_env_with(|k| match k {
        "OBSCURA_BIN" => Some("/opt/obscura".into()),
        "OBSCURA_URL" => Some("http://127.0.0.1:9333".into()),
        "OBSCURA_PORT" => Some("9333".into()),
        "OBSCURA_TIMEOUT_MS" => Some("60000".into()),
        _ => None,
    });
    assert_eq!(cfg.bin.to_str().unwrap(), "/opt/obscura");
    assert_eq!(cfg.url.as_deref(), Some("http://127.0.0.1:9333"));
    assert_eq!(cfg.port, 9333);
    assert_eq!(cfg.timeout_ms, 60000);
}

#[test]
fn config_empty_url_is_none() {
    let cfg = ObscuraConfig::from_env_with(|k| match k {
        "OBSCURA_URL" => Some("".into()),
        _ => None,
    });
    assert!(cfg.url.is_none());
}

#[test]
fn error_mapping() {
    assert!(matches!(
        ObscuraError::NoStatus.into_fetch(),
        FetchError::Transport(_)
    ));
    assert!(matches!(
        ObscuraError::Timeout.into_fetch(),
        FetchError::Timeout
    ));
    assert!(matches!(
        ObscuraError::Cdp("x".into()).into_fetch(),
        FetchError::Transport(_)
    ));
    assert!(matches!(
        ObscuraError::BinaryNotFound("x".into()).into_fetch(),
        FetchError::Transport(_)
    ));
    assert!(matches!(
        ObscuraError::ServerStart("x".into()).into_fetch(),
        FetchError::Transport(_)
    ));
}
