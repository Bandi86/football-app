//! A `GET /v1/player/{id}` válasz kontraktja a közös fixture-hez.

use football_api::routes::player::PlayerResponse;

#[test]
fn player_response_round_trips_the_shared_fixture() {
    let fixture = include_str!("../../../../tests/fixtures/contract/player-response.json");
    let expected: serde_json::Value = serde_json::from_str(fixture).unwrap();
    let parsed: PlayerResponse = serde_json::from_str(fixture).unwrap();
    assert_eq!(parsed.player.name, "Jobson");
    assert_eq!(parsed.spells.len(), 1);
    assert_eq!(parsed.appearances.len(), 1);
    assert_eq!(parsed.stats.len(), 2);
    assert_eq!(parsed.incidents.len(), 2);
    let actual = serde_json::to_value(parsed).unwrap();
    assert_eq!(
        actual, expected,
        "a Rust szerializáció megegyezik a fixture-tel"
    );
}
