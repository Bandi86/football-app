//! A három meccs-szintű mikro-végpont válaszának közös kontraktja.
//! Ugyanazokat a fixture-eket a Swift DTO-k is dekódolják (S4).

use football_api::routes::match_micro::{LineupsResponse, PlayerStatsResponse, ShotsResponse};

#[test]
fn lineups_response_round_trips_the_shared_fixture() {
    let fixture = include_str!("../../../../tests/fixtures/contract/match-lineups-response.json");
    let expected: serde_json::Value = serde_json::from_str(fixture).unwrap();
    let parsed: LineupsResponse = serde_json::from_str(fixture).unwrap();
    assert_eq!(parsed.lineups.len(), 2);
    assert_eq!(
        parsed.lineups[0].player_name.as_deref(),
        Some("Kapus Játékos")
    );
    assert_eq!(parsed.lineups[1].player_entity_id, None);
    assert_eq!(serde_json::to_value(parsed).unwrap(), expected);
}

#[test]
fn player_stats_response_round_trips_the_shared_fixture() {
    let fixture =
        include_str!("../../../../tests/fixtures/contract/match-player-stats-response.json");
    let expected: serde_json::Value = serde_json::from_str(fixture).unwrap();
    let parsed: PlayerStatsResponse = serde_json::from_str(fixture).unwrap();
    assert_eq!(parsed.player_stats.len(), 2);
    assert_eq!(parsed.player_stats[1].stat_key, "Goals");
    assert_eq!(serde_json::to_value(parsed).unwrap(), expected);
}

#[test]
fn shots_response_round_trips_the_shared_fixture() {
    let fixture = include_str!("../../../../tests/fixtures/contract/match-shots-response.json");
    let expected: serde_json::Value = serde_json::from_str(fixture).unwrap();
    let parsed: ShotsResponse = serde_json::from_str(fixture).unwrap();
    assert_eq!(parsed.shots.len(), 2);
    assert_eq!(parsed.shots[0].xg, Some(0.18));
    assert_eq!(parsed.shots[1].player_name, None);
    assert_eq!(serde_json::to_value(parsed).unwrap(), expected);
}
