//! Felhasználói logófeltöltés nyilvántartása (S185/S8).
//!
//! A fájl maga a tartalom-címzett objektumtárba kerül (l. `routes::asset`),
//! itt csak az `ops.asset` sor és az audit-esemény keletkezik. A feltöltött
//! URL mindig a lokális asset-végpontra mutat — felhasználó nem adhat URL-t.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

/// A feltöltés adatai: a bájtokat és a kiszolgáló kulcsot az API réteg adja.
pub struct UploadedAsset<'a> {
    pub entity_kind: &'a str,
    pub entity_id: Uuid,
    pub storage_key: &'a str,
    pub mime_type: &'a str,
    pub original_filename: &'a str,
}

/// Idempotens: az azonos (entity, storage_key) sor nem duplikálódik.
pub async fn record<'e, E>(
    executor: E,
    asset: &UploadedAsset<'_>,
    captured_at: DateTime<Utc>,
) -> Result<Uuid>
where
    E: Executor<'e, Database = Postgres> + Copy,
{
    let id = Uuid::now_v7();
    let url = format!("/v1/asset/{}", asset.storage_key);
    sqlx::query(
        "INSERT INTO ops.asset
            (id, entity_kind, entity_id, kind, url, storage_key, source,
             licence, licence_status, captured_at, origin, uploaded_by,
             original_filename, mime_type)
         VALUES ($1,$2,$3,'logo',$4,$5,'ui-upload','user-upload','cleared',$6,
                 'user_upload','ui',$7,$8)
         ON CONFLICT (entity_kind, entity_id, kind, url) DO UPDATE SET
             storage_key=EXCLUDED.storage_key, mime_type=EXCLUDED.mime_type,
             original_filename=EXCLUDED.original_filename, captured_at=EXCLUDED.captured_at
         RETURNING id",
    )
    .bind(id)
    .bind(asset.entity_kind)
    .bind(asset.entity_id)
    .bind(&url)
    .bind(asset.storage_key)
    .bind(captured_at)
    .bind(asset.original_filename)
    .bind(asset.mime_type)
    .fetch_one(executor)
    .await
    .context("feltöltött asset rögzítése")?;
    Ok(id)
}

/// A feltöltés auditálása (ki, mikor, melyik entitás, melyik kulcs).
pub async fn audit<'e, E>(
    executor: E,
    entity_kind: &str,
    entity_id: Uuid,
    storage_key: &str,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres> + Copy,
{
    let detail = serde_json::json!({ "storage_key": storage_key, "origin": "user_upload" });
    sqlx::query(
        "INSERT INTO ops.audit_event (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1,'ui','asset_uploaded',$2,$3,$4)",
    )
    .bind(Uuid::now_v7())
    .bind(entity_kind)
    .bind(entity_id)
    .bind(detail)
    .execute(executor)
    .await
    .context("asset feltöltés auditálása")?;
    Ok(())
}

/// A feltöltési cél entitás létezik-e (fail-closed, nem kitalált ID).
pub async fn entity_exists(pool: &PgPool, entity_kind: &str, entity_id: Uuid) -> Result<bool> {
    let table = match entity_kind {
        "team" => "core.team",
        "competition" => "core.competition",
        _ => return Ok(false),
    };
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = $1)");
    sqlx::query_scalar::<_, bool>(&sql)
        .bind(entity_id)
        .fetch_one(pool)
        .await
        .context("feltöltési cél ellenőrzése")
}
