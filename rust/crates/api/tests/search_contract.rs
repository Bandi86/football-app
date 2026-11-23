//! A `GET /v1/search` válasz kontraktja a közös fixture-hez.

use football_api::routes::search::SearchResponse;

#[test]
fn search_response_round_trips_the_shared_fixture() {
    let fixture = include_str!("../../../../tests/fixtures/contract/search-response.json");
    let expected: serde_json::Value = serde_json::from_str(fixture).unwrap();
    let parsed: SearchResponse = serde_json::from_str(fixture).unwrap();
    assert_eq!(parsed.query, "ujpest");
    assert_eq!(parsed.results.len(), 3);
    let actual = serde_json::to_value(parsed).unwrap();
    assert_eq!(
        actual, expected,
        "a Rust szerializáció megegyezik a fixture-tel"
    );
}
