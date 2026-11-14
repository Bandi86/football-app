//! S2b-1 identitás-réteg: fail-closed linker és tipizált hibák.
//!
//! A `linker::resolve` a spec §3.1 sorrendjében old fel forrás-entitásokat
//! kanonikus entitásokra; minden nem egzakt kimenet pending review-sort nyit.

pub mod crosswalk;
pub mod error;
pub mod linker;
pub mod review;

pub use crosswalk::{CrosswalkConflict, crosswalk_conflicts};
pub use error::IdentityError;
pub use linker::resolve;
