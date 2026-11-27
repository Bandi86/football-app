//! Kézi meccsszerkesztés tartósítása (S80).
//!
//! A kanonikus út forrásvezérelt; ez a modul a **kivételes, emberi döntés**
//! útja: minden írás nyers artifact + capture + observation + provenance +
//! audit event révén visszakövethető. A publish gate-et a hívó frissítteti
//! (reverify), így az eredmény nem kerüli meg az ellenőrzési láncot.
//!
//! Egy meccs szerkesztése **egyetlen tranzakció**: a `core.match` sor zárása
//! (`FOR UPDATE`), a natural key ütközés ellenőrzése, a match update, a
//! match_result csere/törlés, valamint az observation, provenance és audit
//! írása együtt vagy sehogy sem történik meg.

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use football_domain::manual_edit::{KickoffPrecision, ValidatedManualEdit, ValidatedMatchResult};
use football_domain::match_state::MatchStatus;
use serde_json::json;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::object_store::{ObjectStore, StoredArtifact};

/// Egy meccs kézi szerkesztése: a kliens szándéka domainben már validálva.
/// A `note` kötelező: indoklás nélkül nincs kézi korrekció.
pub struct ManualMatchEditInput<'a> {
    pub match_id: Uuid,
    pub actor: &'a str,
    pub edit: &'a ValidatedManualEdit,
    /// Stabil kéréskulcs az idempotenciához. `Some` esetén a capture
    /// `request_key`-je `match-edit:<match_id>:<kulcs>`, és azonos kulcsú
    /// ismétlés a meglévő observation/audit azonosítókat adja vissza.
    /// `None` esetén a régi, időbélyeges kulcs marad (kompatibilitás).
    pub idempotency_key: Option<&'a str>,
    /// A frissített provenance-sor policy verziója.
    /// Régi út: [`MANUAL_POLICY_VERSION`], új út: `manual-full-1.0`.
    pub policy_version: &'a str,
}

#[derive(Debug, Clone)]
pub struct ManualResultOutcome {
    pub observation_id: Uuid,
    pub audit_id: Uuid,
    pub capture_id: Uuid,
    /// Igaz, ha a kérés kulcsa már rögzített szerkesztéshez tartozott,
    /// és a handler nem írt új observationt/auditot.
    pub reused: bool,
}

/// A tartósítás tipizált hibái: a HTTP-réteg ezekből képez státuszkódot.
#[derive(Debug, thiserror::Error)]
pub enum ManualEditError {
    #[error("nincs ilyen meccs: {0}")]
    MatchNotFound(Uuid),
    /// A natural key más meccsre illeszkedik: 409, a válaszban a másik id.
    #[error("a natural key másik meccsre illeszkedik: {existing_match_id}")]
    NaturalKeyConflict { existing_match_id: Uuid },
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

const SOURCE_CODE: &str = "manual-ui";
/// A régi `/manual-result` út policy verziója; az új `/edit` út
/// `manual-full-1.0`-t használ (a handler adja meg).
pub const MANUAL_POLICY_VERSION: &str = "manual-2.0";
const ADAPTER_VERSION: &str = "manual-2.0";
const ENDPOINT_KEY: &str = "manual-match-edit";

/// A meccs aktuális, auditálható állapota a szerkesztés előtt.
#[derive(Debug, Clone, FromRow)]
struct MatchEditState {
    competition_id: Uuid,
    season_id: Uuid,
    round_label: Option<String>,
    home_team_id: Uuid,
    away_team_id: Uuid,
    kickoff_at: DateTime<Utc>,
    kickoff_precision: String,
    status: String,
}

/// A `core.match_result` aktuális állapota az audit before/afterhez.
#[derive(Debug, Clone, FromRow)]
struct MatchResultState {
    home_ft: Option<i16>,
    away_ft: Option<i16>,
    home_ht: Option<i16>,
    away_ht: Option<i16>,
    home_et: Option<i16>,
    away_et: Option<i16>,
    home_pens: Option<i16>,
    away_pens: Option<i16>,
    decided_by: Option<String>,
    winner_method: Option<String>,
    validity: String,
}

/// Egy meccs kézi szerkesztése. A hiányzó eredmény-mező `NULL` a cserélt
/// sorban; ha a kérés egyáltalán nem hoz eredmény-mezőt, a meglévő
/// `core.match_result` sor érintetlen marad.
pub async fn apply_manual_match_edit(
    pool: &PgPool,
    store: &ObjectStore,
    input: ManualMatchEditInput<'_>,
) -> std::result::Result<ManualResultOutcome, ManualEditError> {
    apply_manual_match_edit_inner(pool, store, input)
        .await
        .map_err(|error| match error.downcast::<ManualEditError>() {
            Ok(error) => error,
            Err(error) => ManualEditError::Other(error),
        })
}

async fn apply_manual_match_edit_inner(
    pool: &PgPool,
    store: &ObjectStore,
    input: ManualMatchEditInput<'_>,
) -> Result<ManualResultOutcome> {
    let captured_at = Utc::now();
    let edit = input.edit;

    let mut tx = pool.begin().await?;

    let current = sqlx::query_as::<_, MatchEditState>(
        "SELECT competition_id, season_id, round_label, home_team_id, away_team_id,
                kickoff_at, kickoff_precision, status
         FROM core.match WHERE id=$1 FOR UPDATE",
    )
    .bind(input.match_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| anyhow::Error::new(ManualEditError::MatchNotFound(input.match_id)))?;

    // A meccs sor zárása sorosítja azonos kulcsú párhuzamos kéréseket: a második
    // már látja az első által COMMIT-elt capture-t, így nem ír új bizonyítékot.
    let stable_request_key = input
        .idempotency_key
        .map(|key| format!("match-edit:{}:{}", input.match_id, key));
    if let Some(ref request_key) = stable_request_key {
        let previous_capture: Option<Uuid> = sqlx::query_scalar(
            "SELECT c.id FROM raw.capture c
              WHERE c.source_id=(SELECT id FROM ops.source WHERE code=$1)
                AND c.request_key=$2
              ORDER BY c.requested_at DESC, c.id DESC
              LIMIT 1",
        )
        .bind(SOURCE_CODE)
        .bind(request_key)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(capture_id) = previous_capture {
            let observation_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM stage.observation WHERE capture_id=$1 ORDER BY observed_at DESC LIMIT 1",
            )
            .bind(capture_id)
            .fetch_optional(&mut *tx)
            .await?;
            let audit_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM ops.audit_event
                 WHERE subject_kind='match' AND subject_id=$1 AND detail->>'observation_id'=$2
                 ORDER BY created_at DESC LIMIT 1",
            )
            .bind(input.match_id)
            .bind(observation_id.map(|id| id.to_string()))
            .fetch_optional(&mut *tx)
            .await?;
            match (observation_id, audit_id) {
                (Some(observation_id), Some(audit_id)) => {
                    return Ok(ManualResultOutcome {
                        observation_id,
                        audit_id,
                        capture_id,
                        reused: true,
                    });
                }
                _ => {
                    return Err(anyhow!(
                        "meglévő szerkesztési kéréshez hiányzik az observation vagy az audit: {request_key}"
                    ));
                }
            }
        }
    }

    let current_status = MatchStatus::parse(&current.status).with_context(|| {
        format!(
            "ismeretlen core.match.status érték a(z) {} meccsnél: {}",
            input.match_id, current.status
        )
    })?;
    let new_status = edit.status.unwrap_or(current_status);
    let new_kickoff_at = edit.kickoff_at.unwrap_or(current.kickoff_at);
    let new_precision = edit
        .kickoff_precision
        .unwrap_or_else(|| {
            KickoffPrecision::parse(&current.kickoff_precision).unwrap_or(KickoffPrecision::Exact)
        })
        .as_str();

    // Natural key: (competition_id, season_id, round_label, home_team_id,
    // away_team_id, kickoff_at) egy másik meccsre nem mutathat.
    let conflict: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM core.match
         WHERE competition_id=$1 AND season_id=$2
           AND round_label IS NOT DISTINCT FROM $3
           AND home_team_id=$4 AND away_team_id=$5
           AND kickoff_at=$6 AND id<>$7",
    )
    .bind(current.competition_id)
    .bind(current.season_id)
    .bind(&current.round_label)
    .bind(current.home_team_id)
    .bind(current.away_team_id)
    .bind(new_kickoff_at)
    .bind(input.match_id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(existing_match_id) = conflict {
        return Err(anyhow::Error::new(ManualEditError::NaturalKeyConflict {
            existing_match_id,
        }));
    }

    let current_result = sqlx::query_as::<_, MatchResultState>(
        "SELECT home_ft, away_ft, home_ht, away_ht, home_et, away_et,
                home_pens, away_pens, decided_by, winner_method, validity
         FROM core.match_result WHERE match_id=$1",
    )
    .bind(input.match_id)
    .fetch_optional(&mut *tx)
    .await?;

    let kickoff_changed = edit
        .kickoff_at
        .is_some_and(|value| value != current.kickoff_at);
    let precision_changed = edit
        .kickoff_precision
        .is_some_and(|value| value.as_str() != current.kickoff_precision);
    let status_changed = edit.status.is_some_and(|value| value != current_status);
    let result_replaced = edit.result.is_some();
    let result_cleared = edit.clear_result;
    let result_touched = result_replaced || result_cleared;

    // Csak a ténylegesen változott oszlopokat írjuk, hogy a `updated_at` és az
    // audit valóban változást tükrözzön.
    if kickoff_changed || precision_changed || status_changed {
        if kickoff_changed {
            sqlx::query("UPDATE core.match SET kickoff_at=$2, updated_at=now() WHERE id=$1")
                .bind(input.match_id)
                .bind(new_kickoff_at)
                .execute(&mut *tx)
                .await?;
        }
        if precision_changed {
            sqlx::query("UPDATE core.match SET kickoff_precision=$2, updated_at=now() WHERE id=$1")
                .bind(input.match_id)
                .bind(new_precision)
                .execute(&mut *tx)
                .await?;
        }
        if status_changed {
            sqlx::query("UPDATE core.match SET status=$2, updated_at=now() WHERE id=$1")
                .bind(input.match_id)
                .bind(new_status.as_db_str())
                .execute(&mut *tx)
                .await?;
        }
    }

    if let Some(result) = edit.result.as_ref() {
        upsert_match_result(&mut tx, input.match_id, result, captured_at, &edit.note).await?;
    } else if result_cleared {
        sqlx::query("DELETE FROM core.match_result WHERE match_id=$1")
            .bind(input.match_id)
            .execute(&mut *tx)
            .await?;
    }

    let after_result_json = edit.result.as_ref().map(result_json).or_else(|| {
        if result_cleared {
            None
        } else {
            current_result.as_ref().map(result_state_json)
        }
    });

    let payload = json!({
        "source_code": SOURCE_CODE,
        "captured_at": captured_at,
        "canonical_match_id": input.match_id,
        "source_home_id": format!("manual:{}", current.home_team_id),
        "source_away_id": format!("manual:{}", current.away_team_id),
        "kickoff_at": new_kickoff_at,
        "kickoff_precision": new_precision,
        "status": new_status.as_db_str(),
        "result": after_result_json,
        "note": edit.note,
    });
    let bytes = serde_json::to_vec(&payload)?;
    let artifact = store.put_json(&bytes)?;

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

    let request_key = stable_request_key.unwrap_or_else(|| {
        format!(
            "manual-edit:{}:{}",
            input.match_id,
            captured_at.to_rfc3339()
        )
    });
    let capture_id: Uuid = sqlx::query_scalar(
        "INSERT INTO raw.capture
         (id, source_id, artifact_id, endpoint_key, request_url, requested_at,
          received_at, http_status, adapter_version, parser_version, request_key)
         VALUES ($1,$2,$3,$4,'manual://ui',$5,$6,200,$7,$8,$9)
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(artifact_id)
    .bind(ENDPOINT_KEY)
    .bind(captured_at)
    .bind(captured_at)
    .bind(ADAPTER_VERSION)
    .bind(ADAPTER_VERSION)
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
         VALUES ($1,$2,$3,'match','match_core',$4,$5,$6,$7,$8,$9)",
    )
    .bind(observation_id)
    .bind(source_id)
    .bind(capture_id)
    .bind(&source_record_key)
    .bind(&source_record_key)
    .bind(captured_at)
    .bind(&payload)
    .bind(ADAPTER_VERSION)
    .bind(&artifact.sha256)
    .execute(&mut *tx)
    .await?;

    // Provenance csak a megváltozott mezőkre: a régi sor jelölése megszűnik,
    // az új a mostani observationre mutat.
    let mut changed_fields: Vec<&'static str> = Vec::new();
    if kickoff_changed {
        changed_fields.push("kickoff_at");
    }
    if precision_changed {
        changed_fields.push("kickoff_precision");
    }
    if status_changed {
        changed_fields.push("status");
    }
    if result_touched {
        changed_fields.push("result");
    }
    if result_replaced {
        changed_fields.push("decided_by");
    }
    for field in changed_fields {
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
             VALUES ('match',$1,$2,$3,true,$4,$5)",
        )
        .bind(input.match_id)
        .bind(field)
        .bind(observation_id)
        .bind(input.policy_version)
        .bind(captured_at)
        .execute(&mut *tx)
        .await?;
    }

    let audit_id = Uuid::now_v7();
    let before = json!({
        "kickoff_at": current.kickoff_at.to_rfc3339(),
        "kickoff_precision": current.kickoff_precision,
        "status": current.status,
        "result": current_result.as_ref().map(result_state_json),
    });
    let after = json!({
        "kickoff_at": new_kickoff_at.to_rfc3339(),
        "kickoff_precision": new_precision,
        "status": new_status.as_db_str(),
        "result": after_result_json,
    });
    sqlx::query(
        "INSERT INTO ops.audit_event (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1,$2,'manual_match_edit','match',$3,$4)",
    )
    .bind(audit_id)
    .bind(input.actor)
    .bind(input.match_id)
    .bind(json!({
        "before": before,
        "after": after,
        "note": edit.note,
        "observation_id": observation_id,
        "request_key": request_key,
    }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(ManualResultOutcome {
        observation_id,
        audit_id,
        capture_id,
        reused: false,
    })
}

async fn upsert_match_result(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    match_id: Uuid,
    result: &ValidatedMatchResult,
    decided_at: DateTime<Utc>,
    reason: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO core.match_result
         (match_id, home_ft, away_ft, home_ht, away_ht, home_et, away_et,
          home_pens, away_pens, winner_method, validity, decided_at, reason, decided_by)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
         ON CONFLICT (match_id) DO UPDATE SET
           home_ft=EXCLUDED.home_ft, away_ft=EXCLUDED.away_ft,
           home_ht=EXCLUDED.home_ht, away_ht=EXCLUDED.away_ht,
           home_et=EXCLUDED.home_et, away_et=EXCLUDED.away_et,
           home_pens=EXCLUDED.home_pens, away_pens=EXCLUDED.away_pens,
           winner_method=EXCLUDED.winner_method, validity=EXCLUDED.validity,
           decided_at=EXCLUDED.decided_at, reason=EXCLUDED.reason,
           decided_by=EXCLUDED.decided_by",
    )
    .bind(match_id)
    .bind(result.home_ft)
    .bind(result.away_ft)
    .bind(result.home_ht)
    .bind(result.away_ht)
    .bind(result.home_et)
    .bind(result.away_et)
    .bind(result.home_pens)
    .bind(result.away_pens)
    .bind(result.winner_method.as_str())
    .bind(result.validity.as_str())
    .bind(decided_at)
    .bind(reason)
    .bind(result.decided_by.as_str())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn result_json(result: &ValidatedMatchResult) -> serde_json::Value {
    json!({
        "home_ft": result.home_ft,
        "away_ft": result.away_ft,
        "home_ht": result.home_ht,
        "away_ht": result.away_ht,
        "home_et": result.home_et,
        "away_et": result.away_et,
        "home_pens": result.home_pens,
        "away_pens": result.away_pens,
        "decided_by": result.decided_by.as_str(),
        "winner_method": result.winner_method.as_str(),
        "validity": result.validity.as_str(),
    })
}

fn result_state_json(state: &MatchResultState) -> serde_json::Value {
    json!({
        "home_ft": state.home_ft,
        "away_ft": state.away_ft,
        "home_ht": state.home_ht,
        "away_ht": state.away_ht,
        "home_et": state.home_et,
        "away_et": state.away_et,
        "home_pens": state.home_pens,
        "away_pens": state.away_pens,
        "decided_by": state.decided_by,
        "winner_method": state.winner_method,
        "validity": state.validity,
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
