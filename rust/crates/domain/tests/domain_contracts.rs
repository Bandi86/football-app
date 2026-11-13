use chrono::{TimeZone, Utc};
use football_domain::{
    Finding, MatchResultSnapshot, MatchSnapshot, MatchStatus, PublishState, Severity,
    VerificationReport, VerificationStatus, evaluate_publish_state, result_consistency_findings,
};

fn snapshot(status: MatchStatus, result: Option<MatchResultSnapshot>) -> MatchSnapshot {
    MatchSnapshot {
        id: uuid::Uuid::nil(),
        home_team_id: uuid::Uuid::nil(),
        away_team_id: uuid::Uuid::nil(),
        kickoff_at: Utc.with_ymd_and_hms(2026, 11, 9, 18, 0, 0).unwrap(),
        status,
        result,
    }
}

#[test]
fn scheduled_match_without_score_is_valid() {
    assert!(result_consistency_findings(&snapshot(MatchStatus::Scheduled, None)).is_empty());
}

#[test]
fn finished_match_without_score_is_blocked() {
    let findings = result_consistency_findings(&snapshot(MatchStatus::Finished, None));
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_key, "MATCH_RESULT_REQUIRED");
    assert_eq!(findings[0].severity, Severity::Blocker);
}

#[test]
fn half_time_score_cannot_exceed_full_time_score() {
    let result = MatchResultSnapshot {
        home_ft: Some(1),
        away_ft: Some(1),
        home_ht: Some(2),
        away_ht: Some(0),
        home_et: None,
        away_et: None,
        home_pens: None,
        away_pens: None,
    };
    let findings = result_consistency_findings(&snapshot(MatchStatus::Finished, Some(result)));
    assert!(findings.iter().any(|f| f.rule_key == "HT_GREATER_THAN_FT"));
}

#[test]
fn blocker_report_withholds_publication() {
    let report = VerificationReport {
        findings: vec![Finding::blocker(
            "TEAM_IDENTITY_UNRESOLVED",
            "home_team_id",
            "A hazai csapat identitása nem megerősített.",
        )],
    };
    assert_eq!(report.status(), VerificationStatus::Blocked);
    assert_eq!(evaluate_publish_state(&report), PublishState::Withheld);
}

#[test]
fn warning_only_report_is_provisional() {
    let report = VerificationReport {
        findings: vec![Finding::warning(
            "TRUST_LEVEL",
            "status",
            "A meccsállapot csak egy forrásból származik.",
        )],
    };
    assert_eq!(report.status(), VerificationStatus::Provisional);
    assert_eq!(
        evaluate_publish_state(&report),
        PublishState::ProvisionalPublish
    );
    assert!(!PublishState::ProvisionalPublish.default_visible());
}

#[test]
fn empty_report_is_verified_and_visible() {
    let report = VerificationReport { findings: vec![] };
    assert_eq!(report.status(), VerificationStatus::Verified);
    assert_eq!(
        evaluate_publish_state(&report),
        PublishState::VerifiedPublish
    );
    assert!(PublishState::VerifiedPublish.default_visible());
}
