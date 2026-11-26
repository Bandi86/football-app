//! A `GET /v1/issues` válasz kontraktja a közös fixture-hez.
//! A Swift oldal ugyanezt a fájlt olvassa, amikor az Üzemeltetés nézet
//! DTO-jait beköti (S2) — mezőátnevezés itt bukik először.

use chrono::DateTime;
use football_api::routes::issues::IssuesResponse;
use football_storage::repositories::issue::IssueRow;
use serde_json::{Value, json};
use uuid::Uuid;

#[test]
fn issues_response_round_trips_the_shared_fixture() {
    let fixture = include_str!("../../../../tests/fixtures/contract/issues-response.json");
    let expected: Value = serde_json::from_str(fixture).unwrap();

    let parsed: IssuesResponse = serde_json::from_str(fixture).unwrap();
    assert_eq!(parsed.count, 1);
    assert_eq!(parsed.issues[0].code, "missing_odds");
    assert_eq!(parsed.issues[0].source, "daily-check");
    let actual = serde_json::to_value(&parsed).unwrap();
    assert_eq!(
        actual, expected,
        "a Rust szerializáció megegyezik a fixture-tel"
    );
}

#[test]
fn issue_row_serialization_matches_the_contract_shape() {
    let row = IssueRow {
        id: Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
        subject_kind: "match".to_owned(),
        subject_id: Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
        code: "missing_odds".to_owned(),
        severity: "warning".to_owned(),
        detail: json!({ "source": "research" }),
        first_seen_at: DateTime::parse_from_rfc3339("2026-11-26T08:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
        last_seen_at: DateTime::parse_from_rfc3339("2026-11-26T09:30:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
        resolved_at: None,
        source: "daily-check".to_owned(),
    };
    let response = IssuesResponse {
        issues: vec![row],
        count: 1,
    };
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/contract/issues-response.json"
    ))
    .unwrap();
    assert_eq!(serde_json::to_value(response).unwrap(), fixture);
}
