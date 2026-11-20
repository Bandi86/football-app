//! Tool-call hurok: a helyi modell `web_search` hívást kérhet, a hívást a
//! helyi SearXNG szolgálja ki. A válasz mellett a felhasznált forrásokat is
//! visszaadjuk, hogy minden állítás mögött ellenőrizhető URL legyen.

use serde_json::json;

use crate::AiError;
use crate::config::AiConfig;
use crate::magnitude::{ChatMessage, MagnitudeClient, ToolCall, ToolDefinition};
use crate::searxng::{SearchResult, SearxngClient};

/// A modellnek felkínált egyetlen tool neve.
pub const WEB_SEARCH_TOOL_NAME: &str = "web_search";
/// Egy tool-hívásra visszaadott találatok felső korlátja.
pub const SEARCH_RESULTS_PER_CALL: usize = 6;
/// A válasz hosszkorlátja tokenben (a helyi modell gyors, de rövid).
const ANSWER_MAX_TOKENS: u32 = 700;

/// Rendszerprompt: magyar, forrásalapú, találgatás nélküli válasz.
const SYSTEM_PROMPT: &str = "Helyi labdarúgó-adat asszisztens vagy. Magyarul válaszolj, tömören. \
Minden állításod mögött legyen a web_search eredményeiből származó forrás URL, és a válasz végén \
sorold fel a forrásokat. Ha a keresés nem ad egyértelmű választ, mondd ki, hogy nem találtad meg; \
soha ne találj ki adatot, dátumot vagy eredményt.";

/// Egy lefutott tool-hívás rövid naplója.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStep {
    pub tool: String,
    pub query: String,
    pub results: usize,
}

/// A hurok eredménye: a válasz, a felhasznált források és a lépések.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchAnswer {
    pub answer: String,
    pub sources: Vec<SearchResult>,
    pub steps: Vec<AgentStep>,
}

/// Helyi modell + helyi kereső, tool-call hurokkal.
#[derive(Debug, Clone)]
pub struct AiAgent {
    model: MagnitudeClient,
    search: SearxngClient,
    max_steps: usize,
}

impl AiAgent {
    pub fn new(config: &AiConfig) -> Result<Self, AiError> {
        let model =
            MagnitudeClient::new(&config.magnitude_base_url, &config.model, config.timeout)?;
        let search = SearxngClient::new(&config.searxng_url, config.timeout)?;
        Ok(Self {
            model,
            search,
            max_steps: config.max_tool_steps,
        })
    }

    pub fn max_steps(mut self, max_steps: usize) -> Self {
        self.max_steps = max_steps.max(1);
        self
    }

    pub fn model_name(&self) -> &str {
        self.model.model()
    }

    /// Kérdés megválaszolása legfeljebb `max_steps` tool-hívással.
    pub async fn ask(&self, question: &str) -> Result<ResearchAnswer, AiError> {
        let question = question.trim();
        if question.is_empty() {
            return Err(AiError::Config("üres kérdés".to_string()));
        }
        let tools = vec![web_search_tool()];
        let mut messages = vec![
            ChatMessage::system(SYSTEM_PROMPT),
            ChatMessage::user(question),
        ];
        let mut steps = Vec::new();
        let mut sources: Vec<SearchResult> = Vec::new();

        for _ in 0..self.max_steps {
            let response = self
                .model
                .chat(&messages, Some(&tools), ANSWER_MAX_TOKENS)
                .await?;
            let choice = response
                .choices
                .into_iter()
                .next()
                .ok_or(AiError::NoAnswer)?;
            let tool_calls = choice.message.tool_calls.clone().unwrap_or_default();
            if tool_calls.is_empty() {
                return Ok(ResearchAnswer {
                    answer: final_text(choice.message.content),
                    sources,
                    steps,
                });
            }

            messages.push(ChatMessage::assistant_tool_calls(
                choice.message.content.clone(),
                tool_calls.clone(),
            ));
            for call in &tool_calls {
                let (content, step) = self.run_tool(call, &mut sources).await?;
                steps.push(step);
                messages.push(ChatMessage::tool(call.id.clone(), content));
            }
        }

        // A lépéskeret elfogyott: még egy tool nélküli forduló a végleges válaszért.
        messages.push(ChatMessage::user(
            "A keresési lehetőségek elfogytak. Válaszolj a fenti találatok alapján, forrásokkal.",
        ));
        let response = self.model.chat(&messages, None, ANSWER_MAX_TOKENS).await?;
        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or(AiError::NoAnswer)?;
        let answer = final_text(choice.message.content);
        if answer.is_empty() {
            return Err(AiError::NoAnswer);
        }
        Ok(ResearchAnswer {
            answer,
            sources,
            steps,
        })
    }

    /// Egy tool-hívás végrehajtása; ismeretlen toolra hibaüzenet megy vissza.
    async fn run_tool(
        &self,
        call: &ToolCall,
        sources: &mut Vec<SearchResult>,
    ) -> Result<(String, AgentStep), AiError> {
        if call.function.name != WEB_SEARCH_TOOL_NAME {
            let error = json!({ "error": format!("ismeretlen tool: {}", call.function.name) });
            return Ok((
                error.to_string(),
                AgentStep {
                    tool: call.function.name.clone(),
                    query: String::new(),
                    results: 0,
                },
            ));
        }
        let query = extract_query(&call.function.arguments);
        let results = if query.is_empty() {
            Vec::new()
        } else {
            self.search.search(&query, SEARCH_RESULTS_PER_CALL).await?
        };
        for result in &results {
            if !sources.iter().any(|known| known.url == result.url) {
                sources.push(result.clone());
            }
        }
        let step = AgentStep {
            tool: WEB_SEARCH_TOOL_NAME.to_string(),
            query,
            results: results.len(),
        };
        Ok((serde_json::to_string(&results).unwrap_or_default(), step))
    }
}

fn web_search_tool() -> ToolDefinition {
    ToolDefinition::function(
        WEB_SEARCH_TOOL_NAME,
        "Webes keresés a helyi SearXNG-vel. Akkor használd, ha a kérdéshez friss vagy \
         ellenőrizendő információ kell.",
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "A keresőkifejezés (magyarul vagy angolul)."
                }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
    )
}

/// A tool-hívás argumentumaiból a keresőkifejezés; hibás JSON-ra üres.
fn extract_query(arguments: &str) -> String {
    serde_json::from_str::<serde_json::Value>(arguments)
        .ok()
        .and_then(|value| {
            value
                .get("query")
                .and_then(|query| query.as_str())
                .map(|query| query.trim().to_string())
        })
        .unwrap_or_default()
}

fn final_text(content: Option<String>) -> String {
    content.unwrap_or_default().trim().to_string()
}
