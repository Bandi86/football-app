use chrono::{TimeZone, Utc};
use football_domain::trust::{OddsTrustState, OddsValues, SourceOdds, SourceTrust, TrustClass};
use football_domain::{MatchResultSnapshot, MatchSnapshot, MatchStatus, Severity};
use football_verification::engine::{SourceMaterialValues, VerificationContext, VerificationRule};
use football_verification::odds::evaluate_odds_trust;
use football_verification::rules::TrustLevelRule;
use uuid::Uuid;

fn snapshot() -> MatchSnapshot {
    MatchSnapshot {
        id: Uuid::now_v7(),
        home_team_id: Uuid::now_v7(),
        away_team_id: Uuid::now_v7(),
        kickoff_at: Utc.with_ymd_and_hms(2026, 11, 9, 18, 0, 0).unwrap(),
        status: MatchStatus::Finished,
        result: Some(MatchResultSnapshot {
            home_ft: Some(2),
            away_ft: Some(1),
            home_ht: Some(1),
            away_ht: Some(0),
            home_et: None,
            away_et: None,
            home_pens: None,
            away_pens: None,
        }),
    }
}

fn source_values(codes: &[&str]) -> Vec<SourceMaterialValues> {
    codes
        .iter()
        .map(|code| SourceMaterialValues {
            source_code: (*code).to_string(),
            home_team_id: Uuid::now_v7(),
            away_team_id: Uuid::now_v7(),
            kickoff_at: Utc.with_ymd_and_hms(2026, 11, 9, 18, 0, 0).unwrap(),
            status: MatchStatus::Finished,
            home_ft: Some(2),
            away_ft: Some(1),
        })
        .collect()
}

fn trust(code: &str, domain: &str, class: TrustClass, provider: Option<&str>) -> SourceTrust {
    SourceTrust {
        source_code: code.to_string(),
        domain: domain.to_string(),
        trust_class: class,
        provider: provider.map(str::to_string),
    }
}

fn ctx<'a>(
    snapshot: &'a MatchSnapshot,
    values: &'a [SourceMaterialValues],
    source_trust: &'a [SourceTrust],
) -> VerificationContext<'a> {
    VerificationContext {
        snapshot,
        home_identity_exact: true,
        away_identity_exact: true,
        duplicate_match_count: 0,
        source_values: values,
        source_trust,
        odds_values: &[],
        odds_tolerance: 0.0,
    }
}

fn trust_findings(context: &VerificationContext<'_>) -> Vec<football_domain::Finding> {
    TrustLevelRule
        .evaluate(context)
        .into_iter()
        .filter(|f| f.rule_key == "TRUST_LEVEL")
        .collect()
}

#[test]
fn high_alone_no_finding() {
    let snap = snapshot();
    let values = source_values(&["footballdata"]);
    let trusts = vec![
        trust("footballdata", "fixture/kickoff", TrustClass::High, None),
        trust("footballdata", "result", TrustClass::High, None),
    ];
    let findings = trust_findings(&ctx(&snap, &values, &trusts));
    assert!(findings.is_empty());
}

#[test]
fn medium_one_group_warning() {
    let snap = snapshot();
    let values = source_values(&["source-a"]);
    let trusts = vec![
        trust("source-a", "fixture/kickoff", TrustClass::High, None),
        trust("source-a", "result", TrustClass::Medium, None),
    ];
    let findings = trust_findings(&ctx(&snap, &values, &trusts));
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].field, "result");
    assert_eq!(findings[0].severity, Severity::Warning);
    assert_eq!(
        findings[0].message,
        "Egy független forrás (közepes bizalom) — provisional."
    );
}

#[test]
fn medium_two_independent_no_finding() {
    let snap = snapshot();
    let values = source_values(&["source-a", "source-b"]);
    let trusts = vec![
        trust("source-a", "result", TrustClass::Medium, None),
        trust("source-b", "result", TrustClass::Medium, None),
        trust("source-a", "fixture/kickoff", TrustClass::Medium, None),
        trust("source-b", "fixture/kickoff", TrustClass::Medium, None),
    ];
    let findings = trust_findings(&ctx(&snap, &values, &trusts));
    assert!(findings.is_empty());
}

#[test]
fn same_provider_one_group_warning() {
    let snap = snapshot();
    let values = source_values(&["source-a", "source-b"]);
    let trusts = vec![
        trust("source-a", "fixture/kickoff", TrustClass::High, None),
        trust("source-b", "fixture/kickoff", TrustClass::High, None),
        trust("source-a", "result", TrustClass::Medium, Some("provider-x")),
        trust("source-b", "result", TrustClass::Medium, Some("provider-x")),
    ];
    let findings = trust_findings(&ctx(&snap, &values, &trusts));
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].field, "result");
}

#[test]
fn display_only_not_counted() {
    let snap = snapshot();
    let values = source_values(&["source-a", "source-b"]);
    let trusts = vec![
        trust("source-a", "fixture/kickoff", TrustClass::High, None),
        trust("source-b", "fixture/kickoff", TrustClass::High, None),
        trust("source-a", "result", TrustClass::Medium, None),
        trust("source-b", "result", TrustClass::DisplayOnly, None),
    ];
    let findings = trust_findings(&ctx(&snap, &values, &trusts));
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].field, "result");

    // Csak display_only hozzájárulók: nem ront (mediums üres → nincs finding).
    let only_display = vec![
        trust("source-a", "fixture/kickoff", TrustClass::DisplayOnly, None),
        trust("source-b", "fixture/kickoff", TrustClass::DisplayOnly, None),
        trust("source-a", "result", TrustClass::DisplayOnly, None),
        trust("source-b", "result", TrustClass::DisplayOnly, None),
    ];
    let findings = trust_findings(&ctx(&snap, &values, &only_display));
    assert!(findings.is_empty());
}

#[test]
fn missing_trust_defaults_medium() {
    let snap = snapshot();
    let values = source_values(&["source-a"]);
    let findings = trust_findings(&ctx(&snap, &values, &[]));
    assert_eq!(findings.len(), 2);
    assert!(findings.iter().all(|f| f.severity == Severity::Warning));
}

#[test]
fn both_domains_evaluated() {
    let snap = snapshot();
    let values = source_values(&["source-a"]);
    let trusts = vec![
        trust("source-a", "fixture/kickoff", TrustClass::Medium, None),
        trust("source-a", "result", TrustClass::Medium, None),
    ];
    let findings = trust_findings(&ctx(&snap, &values, &trusts));
    assert_eq!(findings.len(), 2);
    let fields: Vec<&str> = findings.iter().map(|f| f.field).collect();
    assert!(fields.contains(&"fixture/kickoff"));
    assert!(fields.contains(&"result"));
}

fn odds(home: f64) -> OddsValues {
    OddsValues {
        home: Some(home),
        draw: Some(3.4),
        away: Some(4.2),
        btts_yes: Some(1.9),
        btts_no: Some(1.9),
        over25: Some(2.0),
        under25: Some(1.8),
    }
}

fn source_odds(code: &str, provider: Option<&str>, home: f64) -> SourceOdds {
    SourceOdds {
        source_code: code.to_string(),
        provider: provider.map(str::to_string),
        odds: odds(home),
    }
}

#[test]
fn two_independent_matching_is_verified() {
    let values = vec![
        source_odds("source-a", None, 1.85),
        source_odds("source-b", None, 1.85),
    ];
    assert_eq!(evaluate_odds_trust(&values, 0.0), OddsTrustState::Verified);
}

#[test]
fn two_differing_is_provisional() {
    let values = vec![
        source_odds("source-a", None, 1.85),
        source_odds("source-b", None, 2.1),
    ];
    assert_eq!(
        evaluate_odds_trust(&values, 0.0),
        OddsTrustState::Provisional
    );
}

#[test]
fn single_source_is_provisional() {
    let values = vec![source_odds("source-a", None, 1.85)];
    assert_eq!(
        evaluate_odds_trust(&values, 0.0),
        OddsTrustState::Provisional
    );
}

#[test]
fn empty_is_no_odds() {
    assert_eq!(evaluate_odds_trust(&[], 0.0), OddsTrustState::NoOdds);
}

#[test]
fn tolerance_boundary() {
    // 1.85 vs 1.86: a különbség pontosan a tolerance határán van (mikro-epszilon).
    let on_boundary = vec![
        source_odds("source-a", None, 1.85),
        source_odds("source-b", None, 1.86),
    ];
    assert_eq!(
        evaluate_odds_trust(&on_boundary, 0.01),
        OddsTrustState::Verified
    );
    // A határon túl: provisional.
    let beyond = vec![
        source_odds("source-a", None, 1.85),
        source_odds("source-b", None, 1.87),
    ];
    assert_eq!(
        evaluate_odds_trust(&beyond, 0.01),
        OddsTrustState::Provisional
    );
}
