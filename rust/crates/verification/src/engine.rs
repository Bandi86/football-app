use football_domain::trust::{SourceOdds, SourceTrust};
use football_domain::{Finding, MatchSnapshot, VerificationReport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMaterialValues {
    pub source_code: String,
    pub home_team_id: uuid::Uuid,
    pub away_team_id: uuid::Uuid,
    pub kickoff_at: chrono::DateTime<chrono::Utc>,
    pub status: football_domain::MatchStatus,
    pub home_ft: Option<i16>,
    pub away_ft: Option<i16>,
}

pub struct VerificationContext<'a> {
    pub snapshot: &'a MatchSnapshot,
    pub home_identity_exact: bool,
    pub away_identity_exact: bool,
    pub duplicate_match_count: i64,
    pub source_values: &'a [SourceMaterialValues],
    pub source_trust: &'a [SourceTrust],
    pub odds_values: &'a [SourceOdds],
    pub odds_tolerance: f64,
}

pub trait VerificationRule: Send + Sync {
    fn key(&self) -> &'static str;
    fn evaluate(&self, context: &VerificationContext<'_>) -> Vec<Finding>;
}

pub struct VerificationEngine;

impl VerificationEngine {
    pub fn run(
        rules: &[Box<dyn VerificationRule>],
        context: &VerificationContext<'_>,
    ) -> VerificationReport {
        let findings = rules
            .iter()
            .flat_map(|rule| rule.evaluate(context))
            .collect();
        VerificationReport { findings }
    }
}
