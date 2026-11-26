//! Üzemeltetési hiányjelek és jegyzetek típusai, a HTTP-rétegtől függetlenül.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Info,
    Warning,
    Hard,
}

impl IssueSeverity {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Hard => "hard",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            "hard" => Some(Self::Hard),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum IssueSubjectKind {
    Match,
    Team,
    Competition,
}

impl IssueSubjectKind {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::Team => "team",
            Self::Competition => "competition",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "match" => Some(Self::Match),
            "team" => Some(Self::Team),
            "competition" => Some(Self::Competition),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum IssueCode {
    MissingLogo,
    MissingOdds,
    OpenPast,
    DuplicateSuspect,
    IdentityReview,
    StaleStatus,
    SourceConflict,
}

impl IssueCode {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::MissingLogo => "missing_logo",
            Self::MissingOdds => "missing_odds",
            Self::OpenPast => "open_past",
            Self::DuplicateSuspect => "duplicate_suspect",
            Self::IdentityReview => "identity_review",
            Self::StaleStatus => "stale_status",
            Self::SourceConflict => "source_conflict",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "missing_logo" => Some(Self::MissingLogo),
            "missing_odds" => Some(Self::MissingOdds),
            "open_past" => Some(Self::OpenPast),
            "duplicate_suspect" => Some(Self::DuplicateSuspect),
            "identity_review" => Some(Self::IdentityReview),
            "stale_status" => Some(Self::StaleStatus),
            "source_conflict" => Some(Self::SourceConflict),
            _ => None,
        }
    }
}

/// A jel forrása; a kötőjel miatt nincs serde-származtatás (DB-leképezés kell).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueSource {
    DailyCheck,
    Integrity,
    Manual,
    Agent,
}

impl IssueSource {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::DailyCheck => "daily-check",
            Self::Integrity => "integrity",
            Self::Manual => "manual",
            Self::Agent => "agent",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "daily-check" => Some(Self::DailyCheck),
            "integrity" => Some(Self::Integrity),
            "manual" => Some(Self::Manual),
            "agent" => Some(Self::Agent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    Info,
    Problem,
    Fix,
    Question,
    Resolved,
}

impl NoteKind {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Problem => "problem",
            Self::Fix => "fix",
            Self::Question => "question",
            Self::Resolved => "resolved",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "info" => Some(Self::Info),
            "problem" => Some(Self::Problem),
            "fix" => Some(Self::Fix),
            "question" => Some(Self::Question),
            "resolved" => Some(Self::Resolved),
            _ => None,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum NoteError {
    #[error("Az üzenet nem lehet üres.")]
    EmptyBody,
    #[error("A szerző nem lehet üres.")]
    EmptyAuthor,
    #[error("A szerző csak 'ui', 'agent:<név>' vagy 'import:<forrás>' lehet.")]
    InvalidAuthor,
}

/// A jegyzet szövege nem lehet csak whitespace — ezt a DB CHECK is őrzi.
pub fn validate_note_body(body: &str) -> Result<&str, NoteError> {
    if body.trim().is_empty() {
        Err(NoteError::EmptyBody)
    } else {
        Ok(body)
    }
}

/// A jegyzet szerzője a felhasználó, egy ügynök vagy egy import.
pub fn validate_note_author(author: &str) -> Result<&str, NoteError> {
    let author = author.trim();
    if author.is_empty() {
        return Err(NoteError::EmptyAuthor);
    }
    if author == "ui"
        || author
            .strip_prefix("agent:")
            .is_some_and(|name| !name.trim().is_empty())
        || author
            .strip_prefix("import:")
            .is_some_and(|source| !source.trim().is_empty())
    {
        Ok(author)
    } else {
        Err(NoteError::InvalidAuthor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_enums_round_trip_through_db_strings() {
        for severity in [
            IssueSeverity::Info,
            IssueSeverity::Warning,
            IssueSeverity::Hard,
        ] {
            assert_eq!(IssueSeverity::parse(severity.as_db_str()), Some(severity));
        }
        for subject in [
            IssueSubjectKind::Match,
            IssueSubjectKind::Team,
            IssueSubjectKind::Competition,
        ] {
            assert_eq!(IssueSubjectKind::parse(subject.as_db_str()), Some(subject));
        }
        for code in [
            IssueCode::MissingLogo,
            IssueCode::MissingOdds,
            IssueCode::OpenPast,
            IssueCode::DuplicateSuspect,
            IssueCode::IdentityReview,
            IssueCode::StaleStatus,
            IssueCode::SourceConflict,
        ] {
            assert_eq!(IssueCode::parse(code.as_db_str()), Some(code));
        }
        for source in [
            IssueSource::DailyCheck,
            IssueSource::Integrity,
            IssueSource::Manual,
            IssueSource::Agent,
        ] {
            assert_eq!(IssueSource::parse(source.as_db_str()), Some(source));
        }
        for kind in [
            NoteKind::Info,
            NoteKind::Problem,
            NoteKind::Fix,
            NoteKind::Question,
            NoteKind::Resolved,
        ] {
            assert_eq!(NoteKind::parse(kind.as_db_str()), Some(kind));
        }
    }

    #[test]
    fn unknown_db_strings_are_rejected() {
        assert_eq!(IssueSeverity::parse("blocker"), None);
        assert_eq!(IssueCode::parse("missing_result"), None);
        assert_eq!(NoteKind::parse("done"), None);
        assert_eq!(IssueSource::parse("daily_check"), None);
    }

    #[test]
    fn note_body_must_not_be_whitespace() {
        assert_eq!(validate_note_body("ok"), Ok("ok"));
        assert_eq!(validate_note_body("  \n "), Err(NoteError::EmptyBody));
    }

    #[test]
    fn note_author_accepts_ui_agent_and_import_only() {
        assert_eq!(validate_note_author(" ui "), Ok("ui"));
        assert_eq!(validate_note_author("agent:lead"), Ok("agent:lead"));
        assert_eq!(
            validate_note_author("import:research"),
            Ok("import:research")
        );
        assert_eq!(validate_note_author(""), Err(NoteError::EmptyAuthor));
        assert_eq!(
            validate_note_author("agent:"),
            Err(NoteError::InvalidAuthor)
        );
        assert_eq!(
            validate_note_author("valaki"),
            Err(NoteError::InvalidAuthor)
        );
    }
}
