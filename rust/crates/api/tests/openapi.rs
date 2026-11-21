//! Az OpenAPI-dokumentum szerződés-őre: a leírt útvonalak valóban léteznek a
//! routerben, minden `$ref` feloldható, és a nap-válasz sémája pontosan azt a
//! mezőkészletet írja le, amit a fixture is rögzít.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use football_api::openapi::ApiDoc;
use football_api::{AppState, router};
use football_storage::test_support::test_pool;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use utoipa::OpenApi;

fn document() -> Value {
    serde_json::to_value(ApiDoc::openapi()).expect("a dokumentum szerializálható")
}

fn schema_properties(doc: &Value, name: &str) -> Vec<String> {
    let props = doc["components"]["schemas"][name]["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("hiányzó séma: {name}"));
    let mut keys: Vec<String> = props.keys().cloned().collect();
    keys.sort();
    keys
}

fn contract_keys(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value
        .as_object()
        .expect("objektum")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

/// A dokumentált művelet (GET vagy PUT) és a hozzá tartozó minta-body.
fn sample_method(item: &Value) -> (&'static str, String) {
    if item.get("put").is_some() {
        (
            "PUT",
            r#"{"home_ft":1,"away_ft":0,"note":"openapi kontrakt-teszt"}"#.to_string(),
        )
    } else {
        ("GET", String::new())
    }
}

/// A dokumentumban leírt útvonalat kérésre alakítja: minta-paraméterek és a
/// kötelező query paraméterek pótlása.
fn sample_uri(path: &str, item: &Value) -> String {
    let mut uri = path
        .replace("{date}", "2026-11-20")
        .replace("{id}", "00000000-0000-0000-0000-000000000001")
        .replace("{key}", &format!("assets/ab/cd/{}.webp", "a".repeat(64)));
    let mut query = Vec::new();
    if let Some(params) = item["get"]["parameters"].as_array() {
        for param in params {
            if param["in"] == "query" && param["required"] == true {
                let name = param["name"].as_str().unwrap();
                query.push(format!("{name}=2026-11-20"));
            }
        }
    }
    if !query.is_empty() {
        uri.push('?');
        uri.push_str(&query.join("&"));
    }
    uri
}

#[test]
// S82: a meccs-részletező válasz `incidents` tömbje és annak elemsémája
// dokumentált, a kliens ezekre épít.
fn match_detail_documents_incidents() {
    let doc = document();
    assert!(
        schema_properties(&doc, "MatchDetailRow")
            .iter()
            .any(|key| key == "incidents"),
        "a MatchDetailRow sémája leírja az incidents mezőt"
    );
    assert_eq!(
        schema_properties(&doc, "MatchIncidentRow"),
        vec![
            "detail".to_string(),
            "incident_type".to_string(),
            "minute".to_string(),
            "player_name".to_string(),
            "related_player_name".to_string(),
            "score_after_away".to_string(),
            "score_after_home".to_string(),
            "side".to_string(),
        ],
        "az esemény-elemséma mezőkészlete"
    );
}

#[tokio::test]
async fn every_documented_path_is_routed() {
    let pool = test_pool().await;
    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    });
    // Az oracle önellenőrzése: egy be nem kötött útvonal nem teljesítheti a fenti feltételt.
    let unknown = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/nincs-ilyen")
                .header(header::AUTHORIZATION, "Bearer ops-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    assert!(
        !unknown
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .contains("application/json"),
        "az ismeretlen útvonal JSON-t adott vissza: az oracle nem tudna különbséget tenni"
    );

    let doc = document();
    let paths = doc["paths"].as_object().expect("paths objektum");

    for (path, item) in paths {
        let uri = sample_uri(path, item);
        let (method, body_text_sample) = sample_method(item);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(&uri)
                    .header(header::AUTHORIZATION, "Bearer ops-test")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body_text_sample))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_text = String::from_utf8_lossy(&body);

        assert_ne!(
            status,
            StatusCode::METHOD_NOT_ALLOWED,
            "a {path} dokumentált műveletét a router nem engedi"
        );
        // Az ismeretlen útvonalat az axum üres, JSON nélküli 404-gyel válaszolja meg.
        // A handler saját 404-je viszont ApiError JSON: az bizonyítja, hogy a route él.
        let routed = status != StatusCode::NOT_FOUND || content_type.contains("application/json");
        assert!(
            routed,
            "a dokumentált {path} ({uri}) nincs bekötve a routerbe: {status} {content_type} {body_text}"
        );
    }
}

#[test]
fn all_refs_resolve() {
    let doc = document();
    let schemas = doc["components"]["schemas"]
        .as_object()
        .expect("components.schemas");
    let mut stack = vec![doc.clone()];
    let mut checked = 0usize;
    while let Some(value) = stack.pop() {
        match value {
            Value::Object(map) => {
                if let Some(Value::String(reference)) = map.get("$ref") {
                    let name = reference
                        .strip_prefix("#/components/schemas/")
                        .unwrap_or_else(|| panic!("nem támogatott hivatkozás: {reference}"));
                    assert!(
                        schemas.contains_key(name),
                        "feloldhatatlan $ref: {reference}"
                    );
                    checked += 1;
                }
                stack.extend(map.values().cloned());
            }
            Value::Array(items) => stack.extend(items),
            _ => {}
        }
    }
    assert!(checked > 0, "a dokumentumban nincs egyetlen $ref sem");
}

#[test]
fn day_schema_matches_contract_fixture() {
    let doc = document();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/contract/day-response.json"
    ))
    .expect("a napi szerződés-fixture érvényes JSON");

    assert_eq!(
        schema_properties(&doc, "DayResponse"),
        contract_keys(&fixture),
        "a DayResponse séma és a fixture felső szintű mezői eltérnek"
    );

    let schema_match = schema_properties(&doc, "DayMatch");
    for (index, game) in fixture["matches"]
        .as_array()
        .expect("matches tömb")
        .iter()
        .enumerate()
    {
        assert_eq!(
            schema_match,
            contract_keys(game),
            "a DayMatch séma és a fixture {index}. meccsének mezői eltérnek"
        );
    }
}

#[tokio::test]
async fn openapi_json_is_served_without_token() {
    let pool = test_pool().await;
    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let doc: Value = serde_json::from_slice(&body).expect("a válasz JSON");
    assert_eq!(doc["paths"].as_object().unwrap().len(), 12);
    assert!(
        doc["info"]["title"]
            .as_str()
            .unwrap()
            .contains("football-app")
    );
}
