pub mod error;
pub mod identity;
pub mod manual_edit;
pub mod match_state;
pub mod policy;
pub mod trust;
pub mod verification;

pub use manual_edit::{
    DecidedBy, KickoffPrecision, ManualEditError, ManualMatchEdit, ManualResultEdit,
    ResultValidity, ValidatedManualEdit, ValidatedMatchResult, WinnerMethod, validate_manual_edit,
};
pub use match_state::{
    MatchResultSnapshot, MatchSnapshot, MatchStatus, final_score, result_consistency_findings,
};
pub use verification::{
    Finding, PublishState, Severity, VerificationReport, VerificationStatus, evaluate_publish_state,
};
