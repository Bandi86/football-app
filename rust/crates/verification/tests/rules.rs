use chrono::{TimeZone, Utc};
use football_domain::{MatchResultSnapshot, MatchSnapshot, MatchStatus, Severity};
use football_verification::engine::{VerificationContext, VerificationEngine};
use football_verification::rules::default_rules;
use uuid::Uuid;

fn valid_snapshot() -> MatchSnapshot {
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

#[test]
fn valid_match_has_no_blockers() {
    let snapshot = valid_snapshot();
    let context = VerificationContext {
        snapshot: &snapshot,
        home_identity_exact: true,
        away_identity_exact: true,
        duplicate_match_count: 0,
        source_values: &[],
    };
    let report = VerificationEngine::run(&default_rules(), &context);
    assert!(report.findings.is_empty());
}

#[test]
fn unresolved_team_identity_is_blocker() {
    let snapshot = valid_snapshot();
    let context = VerificationContext {
        snapshot: &snapshot,
        home_identity_exact: false,
        away_identity_exact: true,
        duplicate_match_count: 0,
        source_values: &[],
    };
    let report = VerificationEngine::run(&default_rules(), &context);
    assert!(
        report
            .findings
            .iter()
            .any(|f| { f.rule_key == "EXACT_IDENTITY" && f.severity == Severity::Blocker })
    );
}

#[test]
fn duplicate_natural_key_is_blocker() {
    let snapshot = valid_snapshot();
    let context = VerificationContext {
        snapshot: &snapshot,
        home_identity_exact: true,
        away_identity_exact: true,
        duplicate_match_count: 1,
        source_values: &[],
    };
    let report = VerificationEngine::run(&default_rules(), &context);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule_key == "DUPLICATE_NATURAL_KEY")
    );
}
