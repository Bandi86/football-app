//! Magnitude helyi inference kliens (OpenAI-kompatibilis Chat Completions).
//!
//! A helyi szolgáltatás a `http://127.0.0.1:10100/inference/v1` alatt fut,
//! API-kulcs nélkül. A kliens csak a tool-call hurokhoz szükséges mezőket
//! modellezi; az ismeretlen válaszmezőket a serde figyelmen kívül hagyja.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::AiError;

/// Egy chat-üzenet (system / user / assistant / tool).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Asszisztens-válasz, amely tool-hívásokat kért.
    pub fn assistant_tool_calls(content: Option<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
            tool_calls: Some(tool_calls),
            tool_call_id: None,
        }
    }

    /// A tool futásának eredménye, a híváshoz tartozó azonosítóval.
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// A modell által kért függvényhívás.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// Tool-definíció, amit a modell elé adunk.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub kind: String,
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ToolDefinition {
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            kind: "function".to_string(),
            function: FunctionDefinition {
                name: name.into(),
                description: description.into(),
                parameters,
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [ToolDefinition]>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    #[serde(default)]
    pub choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ChatMessage,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// Magnitude kliens egy konkrét modellel.
#[derive(Debug, Clone)]
pub struct MagnitudeClient {
    http: reqwest::Client,
    base_url: String,
    model: String,
    reasoning_effort: Option<String>,
}

impl MagnitudeClient {
    pub fn new(base_url: &str, model: &str, timeout: Duration) -> Result<Self, AiError> {
        let parsed = Url::parse(base_url)
            .map_err(|e| AiError::Config(format!("érvénytelen Magnitude URL ({base_url}): {e}")))?;
        if model.trim().is_empty() {
            return Err(AiError::Config("üres FOOTBALL_AI_MODEL".to_string()));
        }
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| AiError::Config(format!("nem sikerült HTTP-klienst építeni: {e}")))?;
        Ok(Self {
            http,
            base_url: parsed.to_string().trim_end_matches('/').to_string(),
            model: model.trim().to_string(),
            reasoning_effort: None,
        })
    }

    /// Reasoning-modelleknél kikapcsolható a gondolkodási tokenhasználat.
    pub fn with_reasoning_effort(mut self, effort: Option<String>) -> Self {
        self.reasoning_effort = effort;
        self
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Egy chat-forduló; opcionális tool-készlettel.
    pub async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
        max_tokens: u32,
    ) -> Result<ChatResponse, AiError> {
        let url = format!("{}/chat/completions", self.base_url);
        let body = ChatRequest {
            model: &self.model,
            messages,
            tools,
            max_tokens,
            reasoning_effort: self.reasoning_effort.as_deref(),
        };
        // A workspace reqwest nem tartalmazza a `json` feature-t, ezért a
        // törzset kézzel szerializáljuk és JSON tartalomtípusként küldjük.
        let payload = serde_json::to_string(&body).map_err(|source| AiError::Decode {
            context: "Magnitude kérés törzse".to_string(),
            source,
        })?;
        let response = self
            .http
            .post(&url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(payload)
            .send()
            .await
            .map_err(|source| AiError::Http {
                url: url.clone(),
                source,
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(AiError::Status { url, status });
        }
        let text = response.text().await.map_err(|source| AiError::Http {
            url: url.clone(),
            source,
        })?;
        serde_json::from_str(&text).map_err(|source| AiError::Decode {
            context: "Magnitude chat-válasz".to_string(),
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_request_omits_effort_by_default_and_sends_it_when_set() {
        let messages = vec![ChatMessage::user("kérdés")];
        let plain = ChatRequest {
            model: "model",
            messages: &messages,
            tools: None,
            max_tokens: 100,
            reasoning_effort: None,
        };
        let value = serde_json::to_value(&plain).unwrap();
        assert!(value.get("reasoning_effort").is_none());
        assert_eq!(value["max_tokens"], 100);

        let without_reasoning = ChatRequest {
            model: "model",
            messages: &messages,
            tools: None,
            max_tokens: 100,
            reasoning_effort: Some("none"),
        };
        let value = serde_json::to_value(&without_reasoning).unwrap();
        assert_eq!(value["reasoning_effort"], "none");
    }
}
