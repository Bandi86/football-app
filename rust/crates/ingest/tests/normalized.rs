use football_ingest::adapter::Observation;
use football_ingest::fixture::FixtureSource;
use football_ingest::normalized::NormalizedDay;

#[test]
fn fixture_source_converts_losslessly() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");
    let fixture: FixtureSource = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let day = NormalizedDay::from(&fixture);
    assert_eq!(day.competition.name, fixture.competition.name);
    assert_eq!(day.competition.id, Some(fixture.competition.id));
    assert_eq!(day.season.id, Some(fixture.season.id));
    assert_eq!(day.teams.len(), fixture.teams.len());
    assert_eq!(day.matches.len(), fixture.matches.len());
    assert_eq!(day.matches[0].id, Some(fixture.matches[0].id));
    assert_eq!(
        day.matches[0].home_source_entity_id,
        fixture.matches[0].home_source_entity_id
    );
    let quote_count: usize = day
        .matches
        .iter()
        .filter_map(|m| m.odds.as_ref())
        .map(|o| o.quotes.len())
        .sum();
    let odds_matches = day.matches.iter().filter(|m| m.odds.is_some()).count();
    assert_eq!(
        quote_count,
        7 * odds_matches,
        "minden odds-meccs 7 quote-ot ad"
    );
}

#[test]
fn observations_build_the_same_day_as_the_fixture() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");
    let fixture: FixtureSource = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let observations = football_ingest::fixture::build_observations(&fixture).unwrap();
    let from_obs = NormalizedDay::from_observations(&observations).unwrap();
    let direct = NormalizedDay::from(&fixture);

    assert_eq!(from_obs.matches.len(), direct.matches.len());
    assert_eq!(from_obs.teams.len(), direct.teams.len());
    assert_eq!(from_obs.matches[0].id, direct.matches[0].id);
    assert_eq!(from_obs.matches[0].kickoff_at, direct.matches[0].kickoff_at);
    assert_eq!(from_obs.competition.name, direct.competition.name);
    assert_eq!(from_obs.season.id, direct.season.id);
    assert_eq!(from_obs.source_code, direct.source_code);

    let quote_count = |day: &NormalizedDay| -> usize {
        day.matches
            .iter()
            .filter_map(|m| m.odds.as_ref())
            .map(|o| o.quotes.len())
            .sum()
    };
    assert_eq!(quote_count(&from_obs), quote_count(&direct));
}

fn match_core(comp: &str, country: &str, key: &str, home: &str, away: &str) -> Observation {
    Observation {
        entity_kind: "match".into(),
        aspect: "match_core".into(),
        natural_key: key.into(),
        source_record_key: key.into(),
        observed_at: chrono::Utc::now(),
        payload: serde_json::json!({
            "canonical_match_id": null,
            "source_home_id": home,
            "source_away_id": away,
            "kickoff_at": "2026-11-16T15:00:00Z",
            "status": "scheduled",
            "result": null,
            "round_label": null,
            "competition_timezone": "UTC",
            "competition": {
                "id": null, "name": comp, "country_code": country,
                "country_name": country, "tier": 1, "priority": 50
            },
            "season": { "id": null, "label": "2026/27" },
            "source_code": "teszt",
            "captured_at": "2026-11-16T09:00:00Z"
        }),
    }
}

fn team(id: &str, name: &str) -> Observation {
    Observation {
        entity_kind: "team".into(),
        aspect: "team_identity".into(),
        natural_key: id.into(),
        source_record_key: id.into(),
        observed_at: chrono::Utc::now(),
        payload: serde_json::json!({
            "id": null, "source_entity_id": id, "name": name,
            "country_code": "HU", "gender": "male", "age_group": "senior",
            "branch": "senior"
        }),
    }
}

#[test]
fn grouping_splits_a_day_by_competition() {
    let observations = vec![
        team("a1", "A1"),
        team("a2", "A2"),
        team("b1", "B1"),
        team("b2", "B2"),
        match_core("Liga A", "HU", "m-a", "a1", "a2"),
        match_core("Liga B", "DE", "m-b", "b1", "b2"),
    ];
    let days = NormalizedDay::grouped_from_observations(&observations).unwrap();
    assert_eq!(days.len(), 2, "két verseny → két nap");
    assert_eq!(days[0].competition.name, "Liga A");
    assert_eq!(days[0].matches.len(), 1);
    assert_eq!(days[0].teams.len(), 2, "csak a saját csapatok");
    assert!(days[0].teams.iter().all(|t| t.name.starts_with('A')));
    assert_eq!(days[1].competition.name, "Liga B");
    assert_eq!(days[1].teams.len(), 2);
    assert!(days[1].teams.iter().all(|t| t.name.starts_with('B')));
}

#[test]
fn grouping_rejects_missing_match_meta() {
    let observations = vec![team("a1", "A1")];
    assert!(NormalizedDay::grouped_from_observations(&observations).is_err());
}
