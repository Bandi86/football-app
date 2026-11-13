//! Review-életciklus: kézi jóváhagyás és elutasítás, tranzakcióban, auditálva.
//!
//! `approve`: a pending review-sort egy kanonikus entitáshoz köti — egy
//! tranzakcióban aktív `manual_review` ref-et ír, a sort `approved`-ra zárja
//! és append-only audit-eseményt rögzít. `reject`: a sort `rejected`-re zárja
//! (végleges), jegyzettel és audit-tal. Mindkét döntés csak `pending` soron
//! lehetséges; a döntés után ugyanarra a párra új resolve új pending sort nyit.
//!
//! Dokumentált korlát: forrás nélküli (source_id = NULL) review-sor nem
//! hagyható jóvá, mert a kézi ref kulcsához kell a forrás — ilyenkor
//! `InvalidInput` (fail-closed). A jegyzet nélküli elutasítás szintén
//! `InvalidInput` („A jegyzet nem lehet üres.”).

use football_domain::identity::{IdentityContext, context_matches};
use football_storage::repositories::identity as repo;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::IdentityError;

fn db_err(e: anyhow::Error) -> IdentityError {
    // A `{:#}` alternate form megőrzi a sqlx gyökérokot is a hibaláncban.
    IdentityError::Db(format!("{e:#}"))
}

/// A queue-sor jsonb kontextusa domain kontextus-típussá (a linker ugyanazt
/// a mezőkészletet írja); hiányzó/null mező → None.
fn context_from_json(v: &serde_json::Value) -> IdentityContext {
    let field = |k: &str| v.get(k).and_then(|x| x.as_str()).map(String::from);
    IdentityContext {
        country_code: field("country_code"),
        gender: field("gender"),
        age_group: field("age_group"),
        branch: field("branch"),
    }
}

/// Review-sor betöltése döntéshez: nincs → NotFound, nem pending →
/// AlreadyDecided.
async fn load_pending(pool: &PgPool, queue_id: Uuid) -> Result<repo::ReviewRow, IdentityError> {
    let row = repo::get_review(pool, queue_id)
        .await
        .map_err(db_err)?
        .ok_or(IdentityError::NotFound)?;
    if row.status != "pending" {
        return Err(IdentityError::AlreadyDecided);
    }
    Ok(row)
}

/// Pending review-sor jóváhagyása: a forrás-entitás a megadott kanonikus
/// entitáshoz kötődik. Ellenőrzések: létező entitás (InvalidEntity), egyező
/// kontextus (ContextConflict, csak mindkét oldalon jelen lévő mezőnél).
/// Egy tranzakció: kézi ref + approved döntés + audit; ha a döntés 0 sort
/// érint (közben eldőlt), AlreadyDecided és rollback.
pub async fn approve(
    pool: &PgPool,
    queue_id: Uuid,
    entity_id: Uuid,
    decided_by: &str,
    note: Option<&str>,
) -> Result<(), IdentityError> {
    let row = load_pending(pool, queue_id).await?;

    if !repo::entity_exists(pool, entity_id).await.map_err(db_err)? {
        return Err(IdentityError::InvalidEntity);
    }
    if let Some(entity_ctx) = repo::entity_context(pool, entity_id)
        .await
        .map_err(db_err)?
    {
        let queue_ctx = context_from_json(&row.context);
        if !context_matches(&entity_ctx, &queue_ctx) {
            return Err(IdentityError::ContextConflict);
        }
    }
    // Forrás nélküli sorhoz nincs kézi ref kulcs (lásd a modul dokumentációt).
    let source_id = row.source_id.ok_or(IdentityError::InvalidInput(
        "A review-sorhoz nem tartozik forrás, kézi döntés nem lehetséges.".into(),
    ))?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| IdentityError::Db(e.to_string()))?;
    repo::insert_manual_ref(
        &mut *tx,
        source_id,
        &row.entity_kind,
        &row.source_entity_id,
        entity_id,
    )
    .await
    .map_err(db_err)?;
    let decided = repo::decide_review(&mut *tx, queue_id, "approved", decided_by, note)
        .await
        .map_err(db_err)?;
    if decided == 0 {
        // Közben eldőlt: a tx dropja rollbackel, a ref sem marad meg.
        return Err(IdentityError::AlreadyDecided);
    }
    repo::record_identity_decision(
        &mut *tx,
        decided_by,
        "identity_approve",
        queue_id,
        json!({"queue_id": queue_id, "entity_id": entity_id, "note": note}),
    )
    .await
    .map_err(db_err)?;
    tx.commit()
        .await
        .map_err(|e| IdentityError::Db(e.to_string()))?;
    Ok(())
}

/// Pending review-sor elutasítása, kötelező jegyzettel (üres → InvalidInput).
/// Végleges: a rejected sor megmarad, újra nem dönthető (AlreadyDecided).
/// Egy tranzakció: rejected döntés + audit; 0 érintett sor → AlreadyDecided.
pub async fn reject(
    pool: &PgPool,
    queue_id: Uuid,
    decided_by: &str,
    note: &str,
) -> Result<(), IdentityError> {
    if note.trim().is_empty() {
        return Err(IdentityError::InvalidInput(
            "A jegyzet nem lehet üres.".into(),
        ));
    }
    load_pending(pool, queue_id).await?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| IdentityError::Db(e.to_string()))?;
    let decided = repo::decide_review(&mut *tx, queue_id, "rejected", decided_by, Some(note))
        .await
        .map_err(db_err)?;
    if decided == 0 {
        return Err(IdentityError::AlreadyDecided);
    }
    repo::record_identity_decision(
        &mut *tx,
        decided_by,
        "identity_reject",
        queue_id,
        json!({"queue_id": queue_id, "note": note}),
    )
    .await
    .map_err(db_err)?;
    tx.commit()
        .await
        .map_err(|e| IdentityError::Db(e.to_string()))?;
    Ok(())
}
