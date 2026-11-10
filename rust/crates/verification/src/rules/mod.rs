use chrono::{Datelike, Utc};
use football_domain::{Finding, Severity, result_consistency_findings};

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
        let first = &c.source_values[0];
        let conflicting = c.source_values.iter().skip(1).any(|other| {
            other.home_team_id != first.home_team_id
                || other.away_team_id != first.away_team_id
                || other.kickoff_at != first.kickoff_at
                || other.status != first.status
                || other.home_ft != first.home_ft
                || other.away_ft != first.away_ft
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

pub struct SingleSourceWarningRule;
impl VerificationRule for SingleSourceWarningRule {
    fn key(&self) -> &'static str {
        "SINGLE_SOURCE"
    }
    fn evaluate(&self, c: &VerificationContext<'_>) -> Vec<Finding> {
        if c.source_values.len() == 1 {
            vec![Finding {
                rule_key: "SINGLE_SOURCE",
                severity: Severity::Warning,
                field: "match",
                message: "A meccs csak egy forrásból megerősített.".into(),
            }]
        } else {
            vec![]
        }
    }
}

pub fn default_rules() -> Vec<Box<dyn VerificationRule>> {
    vec![
        Box::new(ExactIdentityRule),
        Box::new(DuplicateNaturalKeyRule),
        Box::new(KickoffValidRule),
        Box::new(ResultConsistencyRule),
        Box::new(MaterialConflictRule),
        Box::new(SingleSourceWarningRule),
    ]
}
