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

pub fn default_rules() -> Vec<Box<dyn VerificationRule>> {
    vec![
        Box::new(ExactIdentityRule),
        Box::new(DuplicateNaturalKeyRule),
        Box::new(KickoffValidRule),
        Box::new(ResultConsistencyRule),
        Box::new(MaterialConflictRule),
        Box::new(TrustLevelRule),
    ]
}
