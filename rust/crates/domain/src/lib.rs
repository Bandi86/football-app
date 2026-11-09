pub mod match_state;
pub mod verification;

pub use match_state::{
    MatchResultSnapshot, MatchSnapshot, MatchStatus, result_consistency_findings,
};
pub use verification::{
    Finding, PublishState, Severity, VerificationReport, VerificationStatus, evaluate_publish_state,
};
