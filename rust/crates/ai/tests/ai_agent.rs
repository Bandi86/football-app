//! A helyi AI-réteg integrációs tesztjei: mindkét helyi szolgáltatást
//! (Magnitude chat, SearXNG keresés) wiremock pótolja, így a teszt hálózat
//! és külső szolgáltatás nélkül, determinisztikusan fut.

use football_ai::{AiAgent, AiConfig, SearxngClient};
use serde_json::json;
use wiremock::matchers::{body_string_contains, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn tool_call_response(query: &str) -> serde_json::Value {
    json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "reasoning_content": "keresés kell",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "web_search",
                        "arguments": json!({ "query": query }).to_string()
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }]
    })
}

fn final_answer_response(answer: &str) -> serde_json::Value {
    json!({
        "choices": [{
            "message": { "role": "assistant", "content": answer },
            "finish_reason": "stop"
        }]
    })
}

#[tokio::test]
async fn searxng_maps_results_and_trims_to_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("q", "Debrecen Paks"))
        .and(query_param("format", "json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "query": "Debrecen Paks",
            "results": [
                { "title": "Debrecen találat", "url": "https://a.example/1", "content": "első" },
                { "title": "Paks találat", "url": "https://b.example/2", "content": "második" },
                { "title": "C találat", "url": "https://c.example/3", "content": "harmadik" }
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client =
        SearxngClient::new(&server.uri(), std::time::Duration::from_secs(5)).expect("kliens");
    let results = client.search("Debrecen Paks", 2).await.expect("keresés");

    assert_eq!(results.len(), 2, "a limit levágja a találatokat");
    assert_eq!(results[0].url, "https://a.example/1");
    assert_eq!(results[1].title, "Paks találat");
    server.verify().await;
}

#[tokio::test]
async fn empty_query_does_not_hit_the_network() {
    let server = MockServer::start().await;
    let client =
        SearxngClient::new(&server.uri(), std::time::Duration::from_secs(5)).expect("kliens");
    let results = client.search("   ", 5).await.expect("üres keresés");
    assert!(results.is_empty());
    assert!(
        server
            .received_requests()
            .await
            .unwrap_or_default()
            .is_empty(),
        "üres kifejezésre nem indul HTTP-hívás"
    );
}

#[tokio::test]
async fn agent_calls_web_search_and_returns_sources() {
    let magnitude = MockServer::start().await;
    let searxng = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                { "title": "Debrecen–Paks", "url": "https://a.example/meccs", "content": "19:00" },
                { "title": "NB I Paks forduló", "url": "https://b.example/fordulo", "content": "program" }
            ]
        })))
        .expect(1)
        .mount(&searxng)
        .await;

    // A második forduló már tartalmazza a tool eredményét.
    Mock::given(method("POST"))
        .and(body_string_contains("\"role\":\"tool\""))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(final_answer_response(
                "A Debrecen–Paks mérkőzés 19:00-kor kezdődik. Forrás: https://a.example/meccs",
            )),
        )
        .with_priority(1)
        .expect(1)
        .mount(&magnitude)
        .await;
    // Első forduló: a modell keresést kér.
    Mock::given(method("POST"))
        .and(path("/inference/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(tool_call_response("Debrecen Paks meccs")),
        )
        .with_priority(2)
        .expect(1)
        .mount(&magnitude)
        .await;

    let config = AiConfig {
        magnitude_base_url: format!("{}/inference/v1", magnitude.uri()),
        searxng_url: searxng.uri(),
        max_tool_steps: 3,
        ..AiConfig::default()
    };
    let agent = AiAgent::new(&config).expect("agent");
    let answer = agent
        .ask("Mikor kezdődik a Debrecen–Paks mérkőzés?")
        .await
        .expect("válasz");

    assert!(answer.answer.contains("19:00"), "válasz: {}", answer.answer);
    assert_eq!(answer.steps.len(), 1, "egy tool-hívás történt");
    assert_eq!(answer.steps[0].tool, "web_search");
    assert_eq!(answer.steps[0].query, "Debrecen Paks meccs");
    assert_eq!(
        answer.steps[0].results, 2,
        "két találat ment vissza a modellnek"
    );
    assert_eq!(answer.sources.len(), 2, "a források összegyűltek");
    assert_eq!(answer.sources[0].url, "https://a.example/meccs");
    magnitude.verify().await;
    searxng.verify().await;
}
