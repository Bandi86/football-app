use thiserror::Error;

#[derive(Debug, Error)]
#[error("ismeretlen db-érték: {0}")]
pub struct DbValueError(pub String);

#[derive(Debug, Error)]
pub enum FetchError {
    #[error("transport hiba: {0}")]
    Transport(String),
    #[error("a kért állomány nem található: {0}")]
    NotFound(String),
    #[error("időtúllépés a forrás felé")]
    Timeout,
    #[error("méretplafon túllépve: {0} bájt")]
    Oversize(u64),
    #[error("HTTP-státusz: {0}")]
    HttpStatus(u16),
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("decoy payload: {0}")]
    DecoyDetected(String),
    #[error("dátum-guard eltérés: kért {requested}, kapott {got}")]
    DateGuardMismatch { requested: String, got: String },
    #[error("séma-drift: {0}")]
    SchemaDrift(String),
    #[error("parse hiba: {0}")]
    Parse(String),
}
