use chrono::{Datelike, Utc};
use football_domain::trust::TrustClass;
use football_domain::{Finding, result_consistency_findings};

use crate::engine::{VerificationContext, VerificationRule};

pub struct ExactIdentityRule;
impl VerificationRule for ExactIdentityRule {
    fn key(&self) -> &'static str {
        "EXACT_IDENTITY"
    }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        let mut findings = vec![];
        if !c.home_identity_exact {
            findings.push(Finding::blocker(
                "EXACT_IDENTITY",
                "home_team_id",
                "A hazai csapat identitása nem erősíthető meg pontos külső ID-val.",
            ));
        }
        if !c.away_identity_exact {
            findings.push(Finding::blocker(
                "EXACT_IDENTITY",
                "away_team_id",
                "A vendég csapat identitása nem erősíthető meg pontos külső ID-val.",
            ));
        }
        findings
    }
}

pub struct DuplicateNaturalKeyRule;
impl VerificationRule for DuplicateNaturalKeyRule {
    fn key(&self) -> &'static str {
        "DUPLICATE_NATURAL_KEY"
    }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        if c.duplicate_match_count > 0 {
            vec![Finding::blocker(
                "DUPLICATE_NATURAL_KEY",
                "match",
                "A meccs természetes kulcsa duplikált rekordot talál.",
            )]
        } else {
            vec![]
        }
    }
}

pub struct KickoffValidRule;
impl VerificationRule for KickoffValidRule {
    fn key(&self) -> &'static str {
        "KICKOFF_VALID"
    }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        let year = c.snapshot.kickoff_at.year();
        if !(1900..=2100).contains(&year) {
            vec![Finding::blocker(
                "KICKOFF_VALID",
                "kickoff_at",
                "A kickoff időpontja értelmetlen tartományba esik.",
            )]
        } else if c.snapshot.kickoff_at > Utc::now() + chrono::Duration::days(730) {
            vec![Finding::warning(
                "KICKOFF_VALID",
                "kickoff_at",
                "A kickoff több mint két évre a jövőben van.",
            )]
        } else {
            vec![]
        }
    }
}

pub struct ResultConsistencyRule;
impl VerificationRule for ResultConsistencyRule {
    fn key(&self) -> &'static str {
        "MATCH_RESULT_CONSISTENT"
    }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        result_consistency_findings(c.snapshot)
    }
}

pub struct MaterialConflictRule;
impl VerificationRule for MaterialConflictRule {
    fn key(&self) -> &'static str {
        "MATERIAL_SOURCE_CONFLICT"
    }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        if c.source_values.len() < 2 {
            return vec![];
        }
        // Feloldott konfliktus: a `match` kulcsú döntés a teljes anyagi
        // összevetést feloldja; mezőszintű döntés csak az adott mezőt maszkolja.
        let resolved =
            |field: &str| -> bool { c.resolved_fields.iter().any(|f| f == "match" || f == field) };
        let first = &c.source_values[0];
        // A források különböző időpontokban készült pillanatképek: a régebbi
        // forrás `scheduled`/üres eredménye nem anyagi ellentmondás, ha a
        // frissebb már eredményt közöl. Ezért csak a valóban összevethető
        // értékeket tekintjük konfliktusnak: azonos státusz mellett eltérő
        // eredmény, illetve a fixture-identitás (csapat, kickoff) eltérése.
        let conflicting = c.source_values.iter().skip(1).any(|other| {
            let both_have_result = first.home_ft.is_some()
                && first.away_ft.is_some()
                && other.home_ft.is_some()
                && other.away_ft.is_some();
            let result_differs = both_have_result
                && (other.home_ft != first.home_ft || other.away_ft != first.away_ft);
            let kickoff_off_by = (other.kickoff_at - first.kickoff_at).num_minutes().abs() > 120;
            (other.home_team_id != first.home_team_id && !resolved("home_team_id"))
                || (other.away_team_id != first.away_team_id && !resolved("away_team_id"))
                || (kickoff_off_by && !resolved("kickoff_at"))
                || (result_differs && !resolved("home_ft") && !resolved("away_ft"))
        });
        if conflicting {
            vec![Finding::blocker(
                "MATERIAL_SOURCE_CONFLICT",
                "match",
                "A források anyagi meccsmezőben eltérnek.",
            )]
        } else {
            vec![]
        }
    }
}

pub struct TrustLevelRule;
impl VerificationRule for TrustLevelRule {
    fn key(&self) -> &'static str {
        "TRUST_LEVEL"
    }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        let mut findings = vec![];
        for domain in ["fixture/kickoff", "result"] {
            let contributors: Vec<&str> = c
                .source_values
                .iter()
                .map(|s| s.source_code.as_str())
                .collect();
            if contributors.is_empty() {
                continue;
            }
            let mut has_high = false;
            let mut mediums: Vec<(&str, Option<&str>)> = vec![];
            for code in &contributors {
                let trust_row = c
                    .source_trust
                    .iter()
                    .find(|t| t.source_code == *code && t.domain == domain);
                let class = trust_row
                    .map(|t| t.trust_class)
                    .unwrap_or(TrustClass::Medium);
                match class {
                    TrustClass::High => has_high = true,
                    TrustClass::Medium => {
                        let provider = trust_row.and_then(|t| t.provider.as_deref());
                        mediums.push((code, provider));
                    }
                    TrustClass::DisplayOnly => {} // nem számít hozzájárulónak
                }
            }
            if has_high {
                continue;
            }
            // Csak display_only hozzájárulók → nem ront (spec §3.1/6).
            if mediums.is_empty() {
                continue;
            }
            // Provider-csoportok: None → önálló, Some(p) → azonos p egy csoport.
            let mut named: std::collections::HashSet<&str> = Default::default();
            let mut independent_count = 0usize;
            for (_, provider) in &mediums {
                match provider {
                    None => independent_count += 1,
                    Some(p) => {
                        if named.insert(p) {
                            independent_count += 1;
                        }
                    }
                }
            }
            if independent_count < 2 {
                findings.push(Finding::warning(
                    "TRUST_LEVEL",
                    domain,
                    "Egy független forrás (közepes bizalom) — provisional.",
                ));
            }
        }
        findings
    }
}

/// S41: domestic versenyben mindkét oldal külföldi csapat.
///
/// Csak akkor tüzel, ha a verseny országa valódi ország (nem NULL/INT/XX/EUR),
/// mindkét csapat ország-kódja ismert, és EGYIK sem egyezik a verseny
/// országával (normalizálva: GB-WLS/ENG → GB, HUN → HU). Az egyoldalú külföldi
/// csapat legitim (Monaco a Ligue 1-ben, walesi klubok az angol kupában).
pub struct ForeignTeamsDomesticCompetitionRule;
impl VerificationRule for ForeignTeamsDomesticCompetitionRule {
    fn key(&self) -> &'static str {
        "FOREIGN_TEAMS_DOMESTIC_COMPETITION"
    }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        let (Some(competition), Some(home), Some(away)) = (
            c.competition_country.as_deref(),
            c.home_team_country.as_deref(),
            c.away_team_country.as_deref(),
        ) else {
            return vec![];
        };
        let competition = domestic_country_key(competition);
        if competition.is_empty() || matches!(competition.as_str(), "INT" | "XX" | "EUR") {
            return vec![];
        }
        let home = domestic_country_key(home);
        let away = domestic_country_key(away);
        if home.is_empty() || away.is_empty() {
            return vec![];
        }
        if home == competition || away == competition {
            return vec![];
        }
        vec![Finding::blocker(
            "FOREIGN_TEAMS_DOMESTIC_COMPETITION",
            "match",
            "A verseny országa szerinti hazai bajnokságban mindkét csapat külföldi.",
        )]
    }
}

/// Belföldi összevetés kulcsa: a `normalize_country_code` (GB-* → GB) mellett a
/// brit hazai nemzet-kódokat is GB-ra hozza, mert egy angol/walesi bajnokság
/// szempontjából mindegyik hazai; az identitás-összehasonlításban viszont
/// szándékosan megkülönböztetjük őket, ezért ez helyi, szabály-specifikus kulcs.
fn domestic_country_key(code: &str) -> String {
    let normalized = football_domain::identity::normalize_country_code(code);
    match normalized.as_str() {
        "ENG" | "SCO" | "WAL" | "NIR" | "GBR" => "GB".to_string(),
        _ => normalized,
    }
}

pub struct SameTeamBothSidesRule;
impl VerificationRule for SameTeamBothSidesRule {
    fn key(&self) -> &'static str {
        "SAME_TEAM_BOTH_SIDES"
    }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        let (Some(home), Some(away)) = (c.home_team_name.as_deref(), c.away_team_name.as_deref())
        else {
            return vec![];
        };
        // Az életkori jelölést elhagyjuk: "Bahrain" és "Bahrain U17" ugyanaz a
        // csapat, csak a korosztály-jelölés nem oldódott fel.
        let (home_key, away_key) = (
            football_domain::identity::strip_age_markers(home),
            football_domain::identity::strip_age_markers(away),
        );
        if home_key.is_empty() || away_key.is_empty() || home_key != away_key {
            return vec![];
        }
        vec![Finding::blocker(
            "SAME_TEAM_BOTH_SIDES",
            "match",
            "A hazai és a vendég oldalon ugyanaz a csapat szerepel.",
        )]
    }
}

pub fn default_rules() -> Vec<Box<dyn VerificationRule>> {
    vec![
        Box::new(ExactIdentityRule),
        Box::new(DuplicateNaturalKeyRule),
        Box::new(KickoffValidRule),
        Box::new(ResultConsistencyRule),
        Box::new(MaterialConflictRule),
        Box::new(TrustLevelRule),
        Box::new(SameTeamBothSidesRule),
        Box::new(ForeignTeamsDomesticCompetitionRule),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{SourceMaterialValues, VerificationEngine};
    use chrono::{TimeZone, Utc};
    use football_domain::{MatchSnapshot, MatchStatus, VerificationReport};

    fn snapshot() -> MatchSnapshot {
        MatchSnapshot {
            id: uuid::Uuid::nil(),
            home_team_id: uuid::Uuid::nil(),
            away_team_id: uuid::Uuid::nil(),
            kickoff_at: Utc.with_ymd_and_hms(2026, 11, 20, 17, 0, 0).unwrap(),
            status: MatchStatus::Scheduled,
            result: None,
        }
    }

    fn context<'a>(
        snapshot: &'a MatchSnapshot,
        values: &'a [SourceMaterialValues],
        trust: &'a [football_domain::trust::SourceTrust],
        odds: &'a [football_domain::trust::SourceOdds],
        resolved: &'a [String],
        home_name: Option<&str>,
        away_name: Option<&str>,
    ) -> VerificationContext<'a> {
        VerificationContext {
            snapshot,
            home_identity_exact: true,
            away_identity_exact: true,
            home_team_name: home_name.map(str::to_string),
            away_team_name: away_name.map(str::to_string),
            home_team_country: None,
            away_team_country: None,
            competition_country: None,
            duplicate_match_count: 0,
            source_values: values,
            source_trust: trust,
            odds_values: odds,
            odds_tolerance: 0.02,
            resolved_fields: resolved,
        }
    }

    fn evaluate_rule(home: &str, away: &str) -> VerificationReport {
        let snapshot = snapshot();
        let context = context(&snapshot, &[], &[], &[], &[], Some(home), Some(away));
        VerificationEngine::run(&[Box::new(SameTeamBothSidesRule)], &context)
    }

    #[test]
    fn same_team_both_sides_blocks_age_variants() {
        for away in ["Bahrain U17", "Bahrain U19"] {
            let report = evaluate_rule("Bahrain", away);
            assert!(
                report
                    .findings
                    .iter()
                    .any(|f| f.rule_key == "SAME_TEAM_BOTH_SIDES"),
                "Bahrain vs {away} blokkoló"
            );
        }
    }

    #[test]
    fn same_team_both_sides_blocks_identical_adult_name() {
        let report = evaluate_rule("Bahrain", "Bahrain");
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule_key == "SAME_TEAM_BOTH_SIDES")
        );
    }

    #[test]
    fn same_team_both_sides_does_not_fire_for_distinct_teams() {
        let report = evaluate_rule("Bahrain", "Kyrgyzstan U17");
        assert!(
            report
                .findings
                .iter()
                .all(|f| f.rule_key != "SAME_TEAM_BOTH_SIDES")
        );
    }

    #[test]
    fn same_team_both_sides_does_not_fire_without_names_or_on_empty_names() {
        let snapshot = snapshot();
        let no_names = context(&snapshot, &[], &[], &[], &[], None, None);
        assert!(
            VerificationEngine::run(&[Box::new(SameTeamBothSidesRule)], &no_names)
                .findings
                .is_empty()
        );
        let empty_names = context(&snapshot, &[], &[], &[], &[], Some(""), Some(""));
        assert!(
            VerificationEngine::run(&[Box::new(SameTeamBothSidesRule)], &empty_names)
                .findings
                .is_empty()
        );
    }

    fn evaluate_foreign_rule(
        home_country: Option<&str>,
        away_country: Option<&str>,
        competition_country: Option<&str>,
    ) -> VerificationReport {
        let snapshot = snapshot();
        let mut context = context(&snapshot, &[], &[], &[], &[], None, None);
        context.home_team_country = home_country.map(str::to_string);
        context.away_team_country = away_country.map(str::to_string);
        context.competition_country = competition_country.map(str::to_string);
        VerificationEngine::run(&[Box::new(ForeignTeamsDomesticCompetitionRule)], &context)
    }

    fn foreign_blocked(report: &VerificationReport) -> bool {
        report
            .findings
            .iter()
            .any(|f| f.rule_key == "FOREIGN_TEAMS_DOMESTIC_COMPETITION")
    }

    #[test]
    fn foreign_teams_domestic_competition_blocks_both_foreign_teams() {
        // Mért adathiba: "FA Cup [GB] CN/CN" és "Ligue 1 [CD] CG/CG".
        assert!(foreign_blocked(&evaluate_foreign_rule(
            Some("CN"),
            Some("CN"),
            Some("GB")
        )));
        assert!(foreign_blocked(&evaluate_foreign_rule(
            Some("CG"),
            Some("CG"),
            Some("CD")
        )));
    }

    #[test]
    fn foreign_teams_domestic_competition_allows_one_foreign_team() {
        // Monaco a Ligue 1-ben, walesi klubok az angol kupában.
        assert!(!foreign_blocked(&evaluate_foreign_rule(
            Some("MC"),
            Some("FR"),
            Some("FR")
        )));
        assert!(!foreign_blocked(&evaluate_foreign_rule(
            Some("GB-WLS"),
            Some("ENG"),
            Some("GB")
        )));
    }

    #[test]
    fn foreign_teams_domestic_competition_compares_normalized_codes() {
        // HUN → HU: a magyar bajnokság magyar csapata nem külföldi.
        assert!(!foreign_blocked(&evaluate_foreign_rule(
            Some("HUN"),
            Some("DEU"),
            Some("HU")
        )));
        assert!(foreign_blocked(&evaluate_foreign_rule(
            Some("HUN"),
            Some("DEU"),
            Some("RO")
        )));
    }

    #[test]
    fn foreign_teams_domestic_competition_treats_british_home_nations_as_domestic() {
        // A mért "Championship [GB] GB-WLS/ENG" sor normalizálás után nem tüzel,
        // és a WAL/ENG/GBR alakok is az angol verseny hazai oldalai.
        assert!(!foreign_blocked(&evaluate_foreign_rule(
            Some("WAL"),
            Some("ENG"),
            Some("GBR")
        )));
        assert!(!foreign_blocked(&evaluate_foreign_rule(
            Some("GB-SCT"),
            Some("NIR"),
            Some("GB")
        )));
    }

    #[test]
    fn foreign_teams_domestic_competition_ignores_international_competitions() {
        for competition in [None, Some("INT"), Some("XX"), Some("EUR")] {
            assert!(!foreign_blocked(&evaluate_foreign_rule(
                Some("CN"),
                Some("CN"),
                competition
            )));
        }
        assert!(!foreign_blocked(&evaluate_foreign_rule(
            None,
            None,
            Some("GB")
        )));
    }

    fn source_values(
        code: &str,
        status: MatchStatus,
        home_ft: Option<i16>,
        away_ft: Option<i16>,
    ) -> SourceMaterialValues {
        SourceMaterialValues {
            source_code: code.to_string(),
            home_team_id: uuid::Uuid::nil(),
            away_team_id: uuid::Uuid::nil(),
            kickoff_at: Utc.with_ymd_and_hms(2026, 11, 20, 17, 0, 0).unwrap(),
            status,
            home_ft,
            away_ft,
        }
    }

    #[test]
    fn stale_scheduled_source_does_not_conflict_with_a_newer_result() {
        // A régi pillanatkép szerint még nem kezdődött el, a frissebb szerint
        // már lejátszották: ez időbeli eltérés, nem anyagi konfliktus.
        let snapshot = snapshot();
        let values = vec![
            source_values("openfoot", MatchStatus::Scheduled, None, None),
            source_values("research", MatchStatus::Finished, Some(1), Some(2)),
        ];
        let report = VerificationEngine::run(
            &[Box::new(MaterialConflictRule)],
            &context(&snapshot, &values, &[], &[], &[], None, None),
        );
        assert!(
            !report
                .findings
                .iter()
                .any(|finding| finding.rule_key == "MATERIAL_SOURCE_CONFLICT")
        );
    }

    #[test]
    fn two_finished_sources_with_different_results_still_conflict() {
        let snapshot = snapshot();
        let values = vec![
            source_values("a", MatchStatus::Finished, Some(1), Some(2)),
            source_values("b", MatchStatus::Finished, Some(0), Some(0)),
        ];
        let report = VerificationEngine::run(
            &[Box::new(MaterialConflictRule)],
            &context(&snapshot, &values, &[], &[], &[], None, None),
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.rule_key == "MATERIAL_SOURCE_CONFLICT")
        );
    }
}
