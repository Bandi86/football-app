//! Kézi, felhasználó által a kliensben felvitt eredmény tartósítása (S70).
//!
//! A kanonikus út forrásvezérelt; ez a modul a **kivételes, emberi döntés**
//! útja: minden írás nyers artifact + capture + observation + provenance +
//! audit event révén visszakövethető. A publish gate-et a hívó frissítteti
//! (reverify), így az eredmény nem kerüli meg az ellenőrzési láncot.

use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::object_store::{ObjectStore, StoredArtifact};

/// A kliens által felvitt eredmény. A `note` kötelező: indoklás nélkül nincs
/// kézi korrekció (ugyanaz az elv, mint a konfliktus-feloldásnál).
pub struct ManualResultInput<'a> {
    pub match_id: Uuid,
    pub home_ft: i16,
    pub away_ft: i16,
    pub note: &'a str,
    pub actor: &'a str,
}

#[derive(Debug, Clone)]
pub struct ManualResultOutcome {
    pub observation_id: Uuid,
    pub audit_id: Uuid,
    pub capture_id: Uuid,
}

const SOURCE_CODE: &str = "manual-ui";

/// Egy meccs eredményének kézi felvétele. Csak `finished` státuszú, FT
/// eredményt ír — a pótló/allékás eredmények (ET, tizenegyes) a kanonikus útra
/// tartoznak, a UI ezt az egyszerű esetet szolgálja ki.
pub async fn upsert_manual_result(
    pool: &PgPool,
    store: &ObjectStore,
    input: ManualResultInput<'_>,
) -> Result<ManualResultOutcome> {
    if input.home_ft < 0 || input.away_ft < 0 {
        bail!("a gólszám nem lehet negatív");
    }
    if input.note.trim().is_empty() {
        bail!("a kézi eredményhez indoklás kötelező");
    }

    let row = sqlx::query_as::<_, (Uuid, Uuid, chrono::DateTime<Utc>)>(
        "SELECT home_team_id, away_team_id, kickoff_at FROM core.match WHERE id=$1",
    )
    .bind(input.match_id)
    .fetch_optional(pool)
    .await?
    .with_context(|| format!("nincs ilyen meccs: {}", input.match_id))?;
    let (home_id, away_id, kickoff_at) = row;

    let captured_at = Utc::now();
    let payload = json!({
        "source_code": SOURCE_CODE,
        "captured_at": captured_at,
        "canonical_match_id": input.match_id,
        "source_home_id": format!("manual:{home_id}"),
        "source_away_id": format!("manual:{away_id}"),
        "kickoff_at": kickoff_at,
        "status": "finished",
        "result": {
            "home_ft": input.home_ft,
            "away_ft": input.away_ft,
            "home_ht": null,
            "away_ht": null,
            "home_et": null,
            "away_et": null,
            "home_pens": null,
            "away_pens": null,
        },
        "note": input.note,
    });
    let bytes = serde_json::to_vec(&payload)?;
    let artifact = store.put_json(&bytes)?;

    let mut tx = pool.begin().await?;

    let source_id: Uuid = sqlx::query_scalar(
        "INSERT INTO ops.source (id, code, display_name)
         VALUES ($1,$2,'Kézi bevitel (kliens)')
         ON CONFLICT (code) DO UPDATE SET display_name=EXCLUDED.display_name
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(SOURCE_CODE)
    .fetch_one(&mut *tx)
    .await?;

    let artifact_id = upsert_artifact(&mut tx, &artifact).await?;

    let request_key = format!("manual:{}:{}", input.match_id, captured_at.to_rfc3339());
    sqlx::query(
        "INSERT INTO raw.capture
         (id, source_id, artifact_id, endpoint_key, request_url, requested_at,
          received_at, http_status, adapter_version, parser_version, request_key)
         VALUES ($1,$2,$3,'manual-result','manual://ui', $4,$5,200,'manual-1.0','manual-1.0',$6)",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(artifact_id)
    .bind(captured_at)
    .bind(captured_at)
    .bind(&request_key)
    .execute(&mut *tx)
    .await?;
    let capture_id: Uuid =
        sqlx::query_scalar("SELECT id FROM raw.capture WHERE source_id=$1 AND request_key=$2")
            .bind(source_id)
            .bind(&request_key)
            .fetch_one(&mut *tx)
            .await?;

    let observation_id = Uuid::now_v7();
    let source_record_key = format!("manual:{}", input.match_id);
    sqlx::query(
        "INSERT INTO stage.observation
         (id, source_id, capture_id, entity_kind, aspect, natural_key,
          source_record_key, observed_at, normalized_payload, schema_version,
          content_hash)
         VALUES ($1,$2,$3,'match','match_core',$4,$5,$6,$7,'1.0',$8)",
    )
    .bind(observation_id)
    .bind(source_id)
    .bind(capture_id)
    .bind(&source_record_key)
    .bind(&source_record_key)
    .bind(captured_at)
    .bind(&payload)
    .bind(&artifact.sha256)
    .execute(&mut *tx)
    .await?;

    // Ugyanazon mező régi selected sorai nem maradhatnak versenyben.
    for field in ["result", "status"] {
        sqlx::query(
            "UPDATE fact.provenance SET is_selected=false
             WHERE subject_kind='match' AND subject_id=$1 AND field_key=$2 AND is_selected",
        )
        .bind(input.match_id)
        .bind(field)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO fact.provenance
             (subject_kind, subject_id, field_key, observation_id, is_selected,
              policy_version, valid_from)
             VALUES ('match',$1,$2,$3,true,'manual-1.0',$4)",
        )
        .bind(input.match_id)
        .bind(field)
        .bind(observation_id)
        .bind(captured_at)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        "INSERT INTO core.match_result
         (match_id, home_ft, away_ft, home_ht, away_ht, home_et, away_et,
          home_pens, away_pens, validity, decided_at, reason, decided_by)
         VALUES ($1,$2,$3,NULL,NULL,NULL,NULL,NULL,NULL,'valid',$4,$5,'ft')
         ON CONFLICT (match_id) DO UPDATE SET
           home_ft=EXCLUDED.home_ft, away_ft=EXCLUDED.away_ft,
           home_ht=NULL, away_ht=NULL, home_et=NULL, away_et=NULL,
           home_pens=NULL, away_pens=NULL, validity='valid',
           decided_at=EXCLUDED.decided_at, reason=EXCLUDED.reason,
           decided_by='ft'",
    )
    .bind(input.match_id)
    .bind(input.home_ft)
    .bind(input.away_ft)
    .bind(captured_at)
    .bind(input.note)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE core.match SET status='finished', updated_at=now() WHERE id=$1")
        .bind(input.match_id)
        .execute(&mut *tx)
        .await?;

    let audit_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ops.audit_event (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1,$2,'manual_result_entry','match',$3,$4)",
    )
    .bind(audit_id)
    .bind(input.actor)
    .bind(input.match_id)
    .bind(json!({
        "home_ft": input.home_ft,
        "away_ft": input.away_ft,
        "note": input.note,
        "source": SOURCE_CODE,
        "observation_id": observation_id,
    }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(ManualResultOutcome {
        observation_id,
        audit_id,
        capture_id,
    })
}

async fn upsert_artifact(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    artifact: &StoredArtifact,
) -> Result<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO raw.artifact
         (id, content_sha256, compression, byte_size, storage_key, media_type)
         VALUES ($1,$2,$3,$4,$5,'application/json')
         ON CONFLICT (content_sha256) DO UPDATE SET content_sha256=EXCLUDED.content_sha256
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(&artifact.sha256)
    .bind(artifact.compression)
    .bind(artifact.byte_size as i64)
    .bind(&artifact.storage_key)
    .fetch_one(&mut **tx)
    .await?;
    Ok(id)
}
