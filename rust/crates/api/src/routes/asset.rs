//! Asset-blob kiszolgálás (S8a): a logó/zászló bájtok a helyi
//! objektumtárból, tartalom-címzett kulccsal (`assets/<aa>/<bb>/<sha>.<ext>`).
//! A kulcsot szigorúan validáljuk (nincs path-traversal, nincs tetszőleges
//! fájl).

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::AppState;
use crate::auth::{Scope, authorize};
use crate::error::{ApiError, AppError};

fn is_hex(value: &str, len: usize) -> bool {
    value.len() == len && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn content_type(key: &str) -> Option<&'static str> {
    let ext = key.rsplit('.').next()?;
    match ext {
        "webp" => Some("image/webp"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

/// Kulcs-forma: `assets/<2 hex>/<2 hex>/<64 hex>.<ext>`.
fn valid_key(key: &str) -> bool {
    let parts: Vec<&str> = key.split('/').collect();
    if parts.len() != 4 || parts[0] != "assets" {
        return false;
    }
    if !is_hex(parts[1], 2) || !is_hex(parts[2], 2) {
        return false;
    }
    let Some((sha, _ext)) = parts[3].split_once('.') else {
        return false;
    };
    is_hex(sha, 64) && content_type(parts[3]).is_some()
}

#[utoipa::path(
    get,
    path = "/v1/asset/{key}",
    tag = "asset",
    summary = "Tartalom-címzett asset-blob (logó, zászló)",
    params(("key" = String, Path, description = "assets/<aa>/<bb>/<sha256>.<ext>")),
    responses(
        (status = 200, description = "A blob bájtjai", content_type = "image/*"),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn asset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(key): Path<String>,
) -> Result<Response, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    if !valid_key(&key) {
        return Err(AppError::bad_request(
            "Érvénytelen asset-kulcs (assets/<aa>/<bb>/<sha>.<ext>).",
        ));
    }
    let path = state.data_dir.join("raw").join(&key);
    let bytes = tokio::fs::read(&path).await.map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            AppError::not_found("Az asset nem található.")
        } else {
            tracing::error!(error=%err, key, "asset olvasás");
            AppError::internal("Az asset olvasása sikertelen.")
        }
    })?;
    let content_type = content_type(&key).unwrap_or("application/octet-stream");
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "private, max-age=86400"),
        ],
        Body::from(bytes),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_validation_rejects_traversal_and_bad_shapes() {
        let sha = "a".repeat(64);
        assert!(valid_key(&format!("assets/ab/cd/{sha}.webp")));
        assert!(!valid_key("assets/ab/cd/../../etc/passwd"));
        assert!(!valid_key(&format!("raw/ab/cd/{sha}.webp")));
        assert!(!valid_key(&format!("assets/ab/cd/{sha}.exe")));
        assert!(!valid_key(&format!("assets/zz/cd/{sha}.webp")));
        assert!(!valid_key(&format!("assets/ab/cd/{}.webp", "a".repeat(63))));
    }
}
