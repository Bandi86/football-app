//! `POST /v1/asset/upload` — logó feltöltése lokális gépről (S185/S8).
//!
//! Csak WebP/PNG, legfeljebb 2 MB. A fájl a tartalom-címzett objektumtárba
//! (`data_dir/raw/assets/<aa>/<bb>/<sha>.<ext>`) kerül, az `ops.asset` sor és
//! az audit-esemény tranzakción kívül, de idempotensen. A felhasználó nem
//! adhat meg URL-t és nem írhat más entitáshoz, mint amelyiket a UI küld.

use axum::{
    Json,
    extract::{Multipart, State},
    http::HeaderMap,
};
use chrono::Utc;
use football_storage::repositories::asset_upload::{UploadedAsset, audit, entity_exists, record};
use serde::Serialize;
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{Scope, authorize},
    error::{ApiError, AppError},
};

const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Serialize, ToSchema)]
pub struct UploadResponse {
    pub storage_key: String,
    pub url: String,
    pub entity_kind: String,
    pub entity_id: Uuid,
    pub mime_type: String,
}

fn image_format(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some(("png", "image/png"))
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(("webp", "image/webp"))
    } else {
        None
    }
}

#[utoipa::path(
    post,
    path = "/v1/asset/upload",
    tag = "asset",
    summary = "Logó feltöltése (WebP/PNG, legfeljebb 2 MB)",
    responses(
        (status = 200, body = UploadResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 404, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn upload_asset(
    State(state): State<AppState>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<Json<UploadResponse>, AppError> {
    authorize(&headers, Scope::Ops, &state.read_token, &state.ops_token)?;

    let mut entity_kind = String::new();
    let mut entity_id = String::new();
    let mut original_filename = String::new();
    let mut bytes: Option<Vec<u8>> = None;
    let mut multipart = multipart;
    while let Some(field) = multipart.next_field().await.map_err(|err| {
        tracing::warn!(error=%err, "feltöltési mező olvasása");
        AppError::bad_request("Érvénytelen feltöltési űrlap.")
    })? {
        match field.name().unwrap_or("") {
            "entity_kind" => entity_kind = field.text().await.unwrap_or_default(),
            "entity_id" => entity_id = field.text().await.unwrap_or_default(),
            "filename" => original_filename = field.text().await.unwrap_or_default(),
            "file" => {
                let data = field.bytes().await.map_err(|err| {
                    tracing::warn!(error=%err, "feltöltött fájl olvasása");
                    AppError::bad_request("A feltöltött fájl nem olvasható.")
                })?;
                if data.len() > MAX_BYTES {
                    return Err(AppError::bad_request("A fájl legfeljebb 2 MB lehet."));
                }
                bytes = Some(data.to_vec());
            }
            _ => {}
        }
    }

    if entity_kind != "team" && entity_kind != "competition" {
        return Err(AppError::bad_request(
            "Csak csapat vagy verseny logója tölthető fel.",
        ));
    }
    let entity = Uuid::parse_str(&entity_id)
        .map_err(|_| AppError::bad_request("Érvénytelen entitás-azonosító."))?;
    if !entity_exists(&state.pool, &entity_kind, entity)
        .await
        .map_err(|err| {
            tracing::error!(error=%err, "feltöltési cél ellenőrzése");
            AppError::internal("A feltöltési cél ellenőrzése sikertelen.")
        })?
    {
        return Err(AppError::not_found("A megadott entitás nem található."));
    }
    let bytes = bytes.ok_or_else(|| AppError::bad_request("Hiányzó fájlmező (file)."))?;
    let (extension, mime_type) = image_format(&bytes)
        .ok_or_else(|| AppError::bad_request("Csak PNG vagy WebP kép engedélyezett."))?;

    let digest = hex::encode(Sha256::digest(&bytes));
    let storage_key = format!(
        "assets/{}/{}/{}.{}",
        &digest[0..2],
        &digest[2..4],
        digest,
        extension
    );
    let path = state.data_dir.join("raw").join(&storage_key);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|err| {
            tracing::error!(error=%err, "asset könyvtár létrehozása");
            AppError::internal("Az asset tárolója nem hozható létre.")
        })?;
    }
    if tokio::fs::metadata(&path).await.is_err() {
        tokio::fs::write(&path, &bytes).await.map_err(|err| {
            tracing::error!(error=%err, "asset írása");
            AppError::internal("A fájl tárolása sikertelen.")
        })?;
    }

    let captured_at = Utc::now();
    let asset = UploadedAsset {
        entity_kind: &entity_kind,
        entity_id: entity,
        storage_key: &storage_key,
        mime_type,
        original_filename: if original_filename.is_empty() {
            "logo"
        } else {
            &original_filename
        },
    };
    record(&state.pool, &asset, captured_at)
        .await
        .map_err(|err| {
            tracing::error!(error=%err, "asset nyilvántartás");
            AppError::internal("A feltöltés nyilvántartása sikertelen.")
        })?;
    audit(&state.pool, &entity_kind, entity, &storage_key)
        .await
        .map_err(|err| {
            tracing::error!(error=%err, "asset audit");
            AppError::internal("A feltöltés auditálása sikertelen.")
        })?;

    Ok(Json(UploadResponse {
        storage_key: storage_key.clone(),
        url: format!("/v1/asset/{storage_key}"),
        entity_kind,
        entity_id: entity,
        mime_type: mime_type.to_string(),
    }))
}
