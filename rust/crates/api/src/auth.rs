use axum::http::HeaderMap;

use crate::error::AppError;

#[derive(Debug, Clone, Copy)]
pub enum Scope {
    Read,
    Ops,
}

pub fn authorize(
    headers: &HeaderMap,
    scope: Scope,
    read_token: &str,
    ops_token: &str,
) -> Result<(), AppError> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(AppError::unauthorized)?;
    let token = value
        .strip_prefix("Bearer ")
        .ok_or_else(AppError::unauthorized)?;

    let allowed = match scope {
        Scope::Read => token == read_token || token == ops_token,
        Scope::Ops => token == ops_token,
    };

    if allowed {
        Ok(())
    } else {
        Err(AppError::unauthorized())
    }
}
