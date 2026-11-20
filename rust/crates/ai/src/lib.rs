//! Helyi AI-asszisztens alapjai: Magnitude (OpenAI-kompatibilis helyi
//! inference) + SearXNG (helyi webkeresés) + tool-call hurok.
//!
//! A crate szándékosan hálózati kliens és nem tartalmaz adatbázis- vagy
//! szerverfüggőséget: a `footballctl ai ...` parancs és a későbbi
//! háttérfeladatok is erre épülnek.
//!
//! Helyi alapértékek (felülírhatók környezeti változóval):
//! - `FOOTBALL_AI_MAGNITUDE_URL` = `http://127.0.0.1:10100/inference/v1`
//! - `FOOTBALL_AI_MODEL` = a gépen telepített gemma 4 E4B Artemis (Q4_0)
//! - `FOOTBALL_AI_SEARXNG_URL` = `http://127.0.0.1:8888`
//! - `FOOTBALL_AI_TIMEOUT_SECS` = 120
//! - `FOOTBALL_AI_MAX_TOOL_STEPS` = 4

pub mod agent;
pub mod config;
pub mod magnitude;
pub mod searxng;

pub use agent::{AgentStep, AiAgent, ResearchAnswer};
pub use config::AiConfig;
pub use magnitude::{ChatMessage, MagnitudeClient, ToolCall, ToolDefinition};
pub use searxng::{SearchResult, SearxngClient};

/// Tipizált hibák a helyi AI-rétegen (nincs csendben elnyelt hiba).
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("konfigurációs hiba: {0}")]
    Config(String),
    #[error("HTTP hiba ({url}): {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("váratlan HTTP státusz ({url}): {status}")]
    Status {
        url: String,
        status: reqwest::StatusCode,
    },
    #[error("érvénytelen JSON ({context}): {source}")]
    Decode {
        context: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("a modell nem adott használható választ")]
    NoAnswer,
}
