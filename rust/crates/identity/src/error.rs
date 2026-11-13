//! Az identitás-réteg tipizált hibái, magyar nyelvű üzenetekkel.
//!
//! A `Db` ág kezelése fail-closed: a hívó (tick) naplóz és false flaggel
//! folytatja — bizonytalan identitás soha nem enged át.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("Adatbázis-hiba az identitás-feloldás közben: {0}")]
    Db(String),

    #[error("A kért review-sor nem található.")]
    NotFound,

    #[error("Több lehetséges egyezés; kézi döntés szükséges.")]
    AmbiguousDecision,

    #[error("A kontextus nem egyezik a jelölt entitás kontextusával.")]
    ContextConflict,

    #[error("A felülvizsgálati sor már el lett döntve.")]
    AlreadyDecided,

    #[error("Érvénytelen entitásazonosító.")]
    InvalidEntity,

    /// Bemeneti hiba dinamikus magyar üzenettel (pl. üres jegyzet, forrás
    /// nélküli review-sor); a szöveg pontosan a megjelenítendő üzenet.
    #[error("{0}")]
    InvalidInput(String),
}
