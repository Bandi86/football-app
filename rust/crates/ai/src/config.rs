//! Helyi AI-konfiguráció környezeti változókból, ésszerű helyi
//! alapértékekkel. A hálózati végpontok csak 127.0.0.1-re mutatnak.

use std::time::Duration;

use crate::AiError;

pub const DEFAULT_MAGNITUDE_URL: &str = "http://127.0.0.1:10100/inference/v1";
pub const DEFAULT_MODEL: &str =
    "hf:turintech/gemma-4-E4B-it-Artemis-GGUF/gemma-4-E4B-it-Artemis-Q4_0.gguf";
pub const DEFAULT_SEARXNG_URL: &str = "http://127.0.0.1:8888";
pub const DEFAULT_TIMEOUT_SECS: u64 = 120;
pub const DEFAULT_MAX_TOOL_STEPS: usize = 4;

/// A helyi AI-réteg beállításai.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiConfig {
    pub magnitude_base_url: String,
    pub model: String,
    pub searxng_url: String,
    pub timeout: Duration,
    pub max_tool_steps: usize,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            magnitude_base_url: DEFAULT_MAGNITUDE_URL.to_string(),
            model: DEFAULT_MODEL.to_string(),
            searxng_url: DEFAULT_SEARXNG_URL.to_string(),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            max_tool_steps: DEFAULT_MAX_TOOL_STEPS,
        }
    }
}

impl AiConfig {
    /// Beállítások a folyamat környezetéből; üres vagy hiányzó érték az
    /// alapértéket adja, érvénytelen számérték tipizált hibát.
    pub fn from_env() -> Result<Self, AiError> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Tesztelhető változat: a kulcs→érték leképezés tetszőleges.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, AiError> {
        let mut config = Self::default();
        if let Some(url) = non_empty(lookup("FOOTBALL_AI_MAGNITUDE_URL")) {
            config.magnitude_base_url = url.trim_end_matches('/').to_string();
        }
        if let Some(model) = non_empty(lookup("FOOTBALL_AI_MODEL")) {
            config.model = model;
        }
        if let Some(url) = non_empty(lookup("FOOTBALL_AI_SEARXNG_URL")) {
            config.searxng_url = url.trim_end_matches('/').to_string();
        }
        if let Some(raw) = non_empty(lookup("FOOTBALL_AI_TIMEOUT_SECS")) {
            let secs: u64 = raw.parse().map_err(|_| {
                AiError::Config(format!("FOOTBALL_AI_TIMEOUT_SECS nem szám: {raw}"))
            })?;
            config.timeout = Duration::from_secs(secs.max(1));
        }
        if let Some(raw) = non_empty(lookup("FOOTBALL_AI_MAX_TOOL_STEPS")) {
            let steps: usize = raw.parse().map_err(|_| {
                AiError::Config(format!("FOOTBALL_AI_MAX_TOOL_STEPS nem szám: {raw}"))
            })?;
            config.max_tool_steps = steps.max(1);
        }
        Ok(config)
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_all_local() {
        let config = AiConfig::from_lookup(|_| None).expect("alapértékek");
        assert_eq!(config, AiConfig::default());
        assert!(config.magnitude_base_url.starts_with("http://127.0.0.1:"));
        assert!(config.searxng_url.starts_with("http://127.0.0.1:"));
    }

    #[test]
    fn lookup_overrides_and_trims_trailing_slash() {
        let config = AiConfig::from_lookup(|key| match key {
            "FOOTBALL_AI_MAGNITUDE_URL" => Some("http://127.0.0.1:10100/inference/v1/".into()),
            "FOOTBALL_AI_MODEL" => Some("  hf:test/model.gguf  ".into()),
            "FOOTBALL_AI_SEARXNG_URL" => Some("http://127.0.0.1:9999/".into()),
            "FOOTBALL_AI_TIMEOUT_SECS" => Some("15".into()),
            "FOOTBALL_AI_MAX_TOOL_STEPS" => Some("2".into()),
            _ => None,
        })
        .expect("felülírás");
        assert_eq!(
            config.magnitude_base_url,
            "http://127.0.0.1:10100/inference/v1"
        );
        assert_eq!(config.model, "hf:test/model.gguf");
        assert_eq!(config.searxng_url, "http://127.0.0.1:9999");
        assert_eq!(config.timeout, Duration::from_secs(15));
        assert_eq!(config.max_tool_steps, 2);
    }

    #[test]
    fn invalid_numbers_are_typed_errors() {
        let err = AiConfig::from_lookup(|key| match key {
            "FOOTBALL_AI_TIMEOUT_SECS" => Some("nincs".into()),
            _ => None,
        })
        .expect_err("érvénytelen időkorlát");
        assert!(matches!(err, AiError::Config(_)));
    }
}
