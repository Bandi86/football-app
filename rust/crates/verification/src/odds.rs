use football_domain::trust::{OddsTrustState, OddsValues, SourceOdds, group_by_provider};

/// Az odds-értékek bizalmi állapota a független provider-csoportok egyezése alapján.
///
/// Két azonos providerű forrás nem független megerősítés; csoportonként az
/// első (source_code szerint rendezett) forrás képviseli az értékeket.
pub fn evaluate_odds_trust(odds_values: &[SourceOdds], tolerance: f64) -> OddsTrustState {
    if odds_values.is_empty() {
        return OddsTrustState::NoOdds;
    }
    let groups = group_by_provider(odds_values, |s| s.provider.as_deref());
    if groups.len() < 2 {
        return OddsTrustState::Provisional;
    }
    let reps: Vec<&OddsValues> = groups
        .iter()
        .map(|g| {
            let mut sorted: Vec<&&SourceOdds> = g.iter().collect();
            sorted.sort_by(|a, b| a.source_code.cmp(&b.source_code));
            &sorted[0].odds
        })
        .collect();
    let first = reps[0];
    if reps.iter().all(|r| first.matches_within(r, tolerance)) {
        OddsTrustState::Verified
    } else {
        OddsTrustState::Provisional
    }
}
