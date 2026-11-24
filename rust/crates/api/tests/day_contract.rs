//! S31c — OpenAPI↔Swift kontrakt: a `GET /v1/day/{date}` válasz kanonikus alakja.
//!
//! A `tests/fixtures/contract/day-response.json` a szerződés egyetlen forrása:
//! ezt a tesztet a Rust DTO-k szerializációjához méri, a Swift oldal ugyanezt a
//! fájlt dekódolja (`DayContractTests`). Egy mezőátnevezés vagy típusváltás
//! így valamelyik oldalon mindenképp elbukik — nem tud csendben elcsúszni.

use chrono::{DateTime, NaiveDate, Utc};
use football_api::routes::day::{DayMatch, DayResponse};
use serde_json::{Value, json};

fn timestamp(raw: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(raw)
        .expect("érvényes RFC 3339 időpont")
        .with_timezone(&Utc)
}

fn full_match() -> DayMatch {
    DayMatch {
        id: uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
        kickoff_at: timestamp("2026-11-20T18:00:00Z"),
        kickoff_precision: "exact".into(),
        status: "finished".into(),
        home: "Ferencváros".into(),
        away: "Újpest".into(),
        home_score: Some(2),
        away_score: Some(1),
        publish_state: "verified_publish".into(),
        blockers: json!([{
            "rule_key": "odds_freshness",
            "severity": "warning",
            "field": "odds",
            "message": "Az odds 40 perccel ezelőtti."
        }]),
        competition_id: uuid::Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
        competition_name: "NB I".into(),
        country_code: Some("HU".into()),
        country_name: Some("Magyarország".into()),
        tier: 1,
        priority: 1,
        round_label: Some("5. forduló".into()),
        home_ht: Some(1),
        away_ht: Some(0),
        home_et: None,
        away_et: None,
        home_pens: None,
        away_pens: None,
        home_position: Some(1),
        away_position: Some(4),
        odds: Some(json!({
            "home": 1.85,
            "draw": 3.4,
            "away": 4.2,
            "btts_yes": 1.72,
            "btts_no": 2.05,
            "over25": 1.9,
            "under25": 1.95
        })),
        market_count: Some(5),
        odds_source: Some("footballdata".into()),
        home_logo: Some("hu/ftc.png".into()),
        away_logo: Some("hu/ujpest.png".into()),
        competition_logo: Some("hu/nb1.png".into()),
        home_elo: Some(1650.5),
        away_elo: Some(1480.25),
        odds_captured_at: Some(timestamp("2026-11-20T17:20:00Z")),
        odds_trust_state: Some("trusted".into()),
    }
}

fn minimal_match() -> DayMatch {
    DayMatch {
        id: uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
        kickoff_at: timestamp("2026-11-20T20:15:00Z"),
        kickoff_precision: "date_only".into(),
        status: "scheduled".into(),
        home: "Debrecen".into(),
        away: "Paks".into(),
        home_score: None,
        away_score: None,
        publish_state: "unknown".into(),
        blockers: json!([]),
        competition_id: uuid::Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
        competition_name: "NB I".into(),
        country_code: Some("HU".into()),
        country_name: Some("Magyarország".into()),
        tier: 1,
        priority: 1,
        round_label: None,
        home_ht: None,
        away_ht: None,
        home_et: None,
        away_et: None,
        home_pens: None,
        away_pens: None,
        home_position: None,
        away_position: None,
        odds: None,
        market_count: None,
        odds_source: None,
        home_logo: None,
        away_logo: None,
        competition_logo: None,
        home_elo: None,
        away_elo: None,
        odds_captured_at: None,
        odds_trust_state: None,
    }
}

#[test]
fn day_response_matches_contract_fixture() {
    let response = DayResponse {
        date: NaiveDate::from_ymd_opt(2026, 11, 20).unwrap(),
        timezone: "Europe/Budapest",
        refreshed_at: Some(timestamp("2026-11-20T17:30:00Z")),
        next_refresh_at: Some(timestamp("2026-11-20T18:15:00Z")),
        gaps: football_api::routes::day::DayGaps {
            missing_kickoff: 1,
            open_past: 0,
            missing_result: 0,
            missing_team_logo: 1,
            missing_competition_logo: 2,
            missing_odds: 1,
            withheld: 0,
        },
        matches: vec![full_match(), minimal_match()],
    };

    let produced = serde_json::to_value(&response).expect("a DayResponse szerializálható");
    let contract: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/contract/day-response.json"
    ))
    .expect("a kontrakt-fixture érvényes JSON");

    assert_eq!(
        produced, contract,
        "a /v1/day válasz alakja eltért a kontrakt-fixture-től"
    );
}
