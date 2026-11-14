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
