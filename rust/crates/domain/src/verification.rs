use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Blocker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Unverified,
    Verified,
    Provisional,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishState {
    VerifiedPublish,
    ProvisionalPublish,
    Withheld,
    Quarantined,
}

impl PublishState {
    pub fn default_visible(self) -> bool {
        self == Self::VerifiedPublish
    }

    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::VerifiedPublish => "verified_publish",
            Self::ProvisionalPublish => "provisional_publish",
            Self::Withheld => "withheld",
            Self::Quarantined => "quarantined",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub rule_key: &'static str,
    pub severity: Severity,
    pub field: &'static str,
    pub message: String,
}

impl Finding {
    pub fn blocker(
        rule_key: &'static str,
        field: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            rule_key,
            severity: Severity::Blocker,
            field,
            message: message.into(),
        }
    }

    pub fn warning(
        rule_key: &'static str,
        field: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            rule_key,
            severity: Severity::Warning,
            field,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationReport {
    pub findings: Vec<Finding>,
}

impl VerificationReport {
    pub fn status(&self) -> VerificationStatus {
        if self
            .findings
            .iter()
            .any(|finding| finding.severity == Severity::Blocker)
        {
            VerificationStatus::Blocked
        } else if self
            .findings
            .iter()
            .any(|finding| finding.severity == Severity::Warning)
        {
            VerificationStatus::Provisional
        } else {
            VerificationStatus::Verified
        }
    }
}

pub fn evaluate_publish_state(report: &VerificationReport) -> PublishState {
    match report.status() {
        VerificationStatus::Blocked => PublishState::Withheld,
        VerificationStatus::Provisional => PublishState::ProvisionalPublish,
        VerificationStatus::Verified => PublishState::VerifiedPublish,
        VerificationStatus::Unverified => PublishState::Withheld,
    }
}
