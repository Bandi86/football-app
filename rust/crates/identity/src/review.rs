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
    let kind = repo::entity_kind(pool, entity_id)
        .await
        .map_err(db_err)?
        .ok_or(IdentityError::InvalidEntity)?;
    if kind != row.entity_kind {
        return Err(IdentityError::KindMismatch(format!(
            "a választott entitás kind-ja '{kind}', a review-sor elvárt kind-ja '{expected}'",
            expected = row.entity_kind
        )));
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
        json!({"queue_id": queue_id, "entity_id": entity_id, "ok": row.reason, "note": note}),
    )
    .await
    .map_err(db_err)?;
    tx.commit()
        .await
        .map_err(|e| IdentityError::Db(e.to_string()))?;
    Ok(())
}

/// Pending review-sor jóváhagyása ÚJ kanonikus entitással (élő forrás
/// bootstrap): kind-tudatos létrehozás — `team` → `core.entity` +
/// `core.team` (ország a sor kontextusából), más kind → csak `core.entity`
/// —, majd a meglévő approve-lánc (kézi ref + döntés + audit) egy
/// tranzakcióban. Visszaadja az új entitás azonosítóját.
pub async fn approve_create(
    pool: &PgPool,
    queue_id: Uuid,
    decided_by: &str,
    note: Option<&str>,
) -> Result<Uuid, IdentityError> {
    let row = load_pending(pool, queue_id).await?;
    let source_id = row.source_id.ok_or(IdentityError::InvalidInput(
        "A review-sorhoz nem tartozik forrás, kézi döntés nem lehetséges.".into(),
    ))?;
    let entity_id = Uuid::now_v7();

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| IdentityError::Db(e.to_string()))?;

    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,$2,$3)")
        .bind(entity_id)
        .bind(&row.entity_kind)
        .bind(&row.proposed_name)
        .execute(&mut *tx)
        .await
        .map_err(|e| IdentityError::Db(e.to_string()))?;

    if row.entity_kind == "team" {
        let ctx = context_from_json(&row.context);
        let country_code = ctx.country_code.as_deref().ok_or_else(|| {
            IdentityError::InvalidInput(
                "a review-sor kontextusában nincs ország; a csapat nem hozható létre".into(),
            )
        })?;
        // Ismeretlen országkódnál az entitás-létrehozás pótolja a kanonikus
        // sort (a megjelenítési név később finomítható).
        let country_id: Uuid = match sqlx::query_scalar("SELECT id FROM core.country WHERE code=$1")
            .bind(country_code)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| IdentityError::Db(e.to_string()))?
        {
            Some(id) => id,
            None => sqlx::query_scalar(
                "INSERT INTO core.country (id, code, name) VALUES ($1,$2,$2) RETURNING id",
            )
            .bind(Uuid::now_v7())
            .bind(country_code)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| IdentityError::Db(e.to_string()))?,
        };
        sqlx::query(
            "INSERT INTO core.team
             (id, entity_id, country_id, name, gender, age_group, branch)
             VALUES ($1,$1,$2,$3,$4,$5,$6)",
        )
        .bind(entity_id)
        .bind(country_id)
        .bind(&row.proposed_name)
        .bind(ctx.gender.as_deref().unwrap_or("male"))
        .bind(ctx.age_group.as_deref().unwrap_or("senior"))
        .bind(ctx.branch.as_deref().unwrap_or("senior"))
        .execute(&mut *tx)
        .await
        .map_err(|e| IdentityError::Db(e.to_string()))?;
    }

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
        return Err(IdentityError::AlreadyDecided);
    }
    repo::record_identity_decision(
        &mut *tx,
        decided_by,
        "identity_approve_create",
        queue_id,
        json!({"queue_id": queue_id, "entity_id": entity_id, "ok": row.reason, "note": note}),
    )
    .await
    .map_err(db_err)?;
    tx.commit()
        .await
        .map_err(|e| IdentityError::Db(e.to_string()))?;
    Ok(entity_id)
}

/// Az összes pending review-sor jóváhagyása új entitással (élő bootstrap
/// batch). Soronként a `approve_create` fut (külön tranzakció); a hibás sorok
/// kimaradnak és a végén hibalistában térnek vissza.
pub async fn approve_create_pending(
    pool: &PgPool,
    decided_by: &str,
) -> Result<(Vec<(Uuid, Uuid)>, Vec<(Uuid, String)>), IdentityError> {
    let rows = repo::list_reviews(pool, Some("pending"))
        .await
        .map_err(db_err)?;
    let mut approved = Vec::new();
    let mut failed = Vec::new();
    for row in rows {
        match approve_create(pool, row.id, decided_by, Some("bootstrap batch")).await {
            Ok(entity_id) => approved.push((row.id, entity_id)),
            Err(err) => failed.push((row.id, err.to_string())),
        }
    }
    Ok((approved, failed))
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
    let row = load_pending(pool, queue_id).await?;

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
        json!({"queue_id": queue_id, "ok": row.reason, "note": note}),
    )
    .await
    .map_err(db_err)?;
    tx.commit()
        .await
        .map_err(|e| IdentityError::Db(e.to_string()))?;
    Ok(())
}
