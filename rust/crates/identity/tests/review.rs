//! S2b-1 Task 5 — a review-életciklus integrációs tesztjei.
//!
//! approve: kézi manual_review ref + approved queue + audit egy tranzakcióban,
//! utána a linker Exact(ManualReview)-t ad. Hibaágak: NotFound, AlreadyDecided,
//! InvalidEntity, InvalidInput, ContextConflict — hiba esetén nem íródik ref/audit.
//! reject: végleges + auditált, újra reject → AlreadyDecided, a rejected sor
//! megmarad, új evidence nélkül is új pending sor nyílik ugyanarra a párra.
//! A magyar nyelvű szövegeket visszaolvassuk.

use football_domain::identity::{IdentityContext, Resolution, ResolveMethod};
use football_identity::{IdentityError, resolve, review};
use football_storage::repositories::identity as repo;
use football_storage::test_support::test_pool;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Seed segédek (a linker.rs teszt mintáján)
// ---------------------------------------------------------------------------

async fn seed_source(pool: &PgPool, code: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,$2,'Demó')")
        .bind(id)
        .bind(code)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn seed_country(pool: &PgPool, code: &str, name: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,$2,$3)")
        .bind(id)
        .bind(code)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn seed_team(
    pool: &PgPool,
    name: &str,
    country_id: Option<Uuid>,
    gender: &str,
    age_group: &str,
    branch: &str,
) -> Uuid {
    let entity_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
        .bind(entity_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(Uuid::now_v7())
    .bind(entity_id)
    .bind(country_id)
    .bind(name)
    .bind(gender)
    .bind(age_group)
    .bind(branch)
    .execute(pool)
    .await
    .unwrap();
    entity_id
}

fn ctx_hu() -> IdentityContext {
    IdentityContext {
        country_code: Some("HU".into()),
        ..Default::default()
    }
}

fn ctx_en() -> IdentityContext {
    IdentityContext {
        country_code: Some("EN".into()),
        ..Default::default()
    }
}

/// Pending review-sort nyit a linkerrel (no_exact_match): a név szándékosan
/// nem egyezik egyetlen seedelt entitással sem.
async fn pending_queue(pool: &PgPool, source_id: Uuid, ext: &str, ctx: &IdentityContext) -> Uuid {
    match resolve(
        pool,
        source_id,
        "demo-synthetic",
        "team",
        ext,
        "Ismeretlen Klub",
        ctx,
    )
    .await
    .unwrap()
    {
        Resolution::NotFound { queue_id } => queue_id,
        other => panic!("NotFound helyett: {other:?}"),
    }
}

async fn queue_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM identity.review_queue")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn audit_count(pool: &PgPool, queue_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM ops.audit_event WHERE subject_id = $1")
        .bind(queue_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn ref_count(pool: &PgPool, source_id: Uuid, ext: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM identity.source_entity_ref
         WHERE source_id = $1 AND entity_kind = 'team' AND source_entity_id = $2",
    )
    .bind(source_id)
    .bind(ext)
    .fetch_one(pool)
    .await
    .unwrap()
}

// ---------------------------------------------------------------------------
// approve: ref + queue + audit, majd Exact(ManualReview)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn approve_writes_ref_and_audit_then_resolves_exact() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "demo-synthetic").await;
    let entity_id = seed_team(&pool, "Demó FC", None, "male", "senior", "senior").await;
    let queue_id = pending_queue(&pool, source_id, "rev-1", &ctx_hu()).await;

    review::approve(
        &pool,
        queue_id,
        entity_id,
        "cli",
        Some("Kézi egyezés megerősítve"),
    )
    .await
    .unwrap();

    // Aktív manual_review ref íródott.
    let (ref_entity, method, status): (Uuid, String, String) = sqlx::query_as(
        "SELECT entity_id, method, status FROM identity.source_entity_ref
         WHERE source_id = $1 AND entity_kind = 'team' AND source_entity_id = 'rev-1'",
    )
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ref_entity, entity_id);
    assert_eq!(method, "manual_review");
    assert_eq!(status, "active");

    // A queue-sor approved, a döntő és a magyar jegyzet visszaolvasva.
    let row = repo::get_review(&pool, queue_id).await.unwrap().unwrap();
    assert_eq!(row.status, "approved");
    let (decided_by, decision_note): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT decided_by, decision_note FROM identity.review_queue WHERE id = $1")
            .bind(queue_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(decided_by.as_deref(), Some("cli"));
    assert_eq!(decision_note.as_deref(), Some("Kézi egyezés megerősítve"));

    // Pontosan 1 audit sor, a döntés körülményeivel.
    assert_eq!(audit_count(&pool, queue_id).await, 1);
    let (actor, action, subject_kind, detail): (String, String, String, serde_json::Value) =
        sqlx::query_as(
            "SELECT actor, action, subject_kind, detail FROM ops.audit_event
             WHERE subject_id = $1",
        )
        .bind(queue_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(actor, "cli");
    assert_eq!(action, "identity_approve");
    assert_eq!(subject_kind, "review_queue");
    assert_eq!(detail["queue_id"], json!(queue_id));
    assert_eq!(detail["entity_id"], json!(entity_id));
    assert_eq!(detail["ok"], json!("no_exact_match")); // a queue reason-je (spec §3.2)
    assert_eq!(detail["note"], json!("Kézi egyezés megerősítve"));

    // A linker most már Exact(ManualReview)-t ad, új pending sor nélkül.
    let res = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "rev-1",
        "Ismeretlen Klub",
        &ctx_hu(),
    )
    .await
    .unwrap();
    match res {
        Resolution::Exact {
            entity_id: e,
            method,
        } => {
            assert_eq!(e, entity_id);
            assert_eq!(method, ResolveMethod::ManualReview);
        }
        other => panic!("Exact(ManualReview) helyett: {other:?}"),
    }
    assert_eq!(queue_count(&pool).await, 1);
}

// ---------------------------------------------------------------------------
// approve hibaágak: NotFound / AlreadyDecided / InvalidEntity / InvalidInput /
// ContextConflict
// ---------------------------------------------------------------------------

#[tokio::test]
async fn approve_errors() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "demo-synthetic").await;
    let hu_id = seed_country(&pool, "HU", "Magyarország").await;
    let entity_id = seed_team(&pool, "Demó FC", Some(hu_id), "male", "senior", "senior").await;

    // Nem létező queue → NotFound (magyar üzenet visszaolvasva).
    let err = review::approve(&pool, Uuid::now_v7(), entity_id, "cli", None)
        .await
        .expect_err("nem létező queue nem hagyható jóvá");
    assert!(matches!(err, IdentityError::NotFound));
    assert_eq!(err.to_string(), "A kért review-sor nem található.");

    // Nem létező entitás → InvalidEntity; ref/audit nem íródik.
    let q1 = pending_queue(&pool, source_id, "err-1", &ctx_hu()).await;
    let err = review::approve(&pool, q1, Uuid::now_v7(), "cli", None)
        .await
        .expect_err("nem létező entitás nem köthető össze");
    assert!(matches!(err, IdentityError::InvalidEntity));
    assert_eq!(err.to_string(), "Érvénytelen entitásazonosító.");
    assert_eq!(ref_count(&pool, source_id, "err-1").await, 0);
    assert_eq!(audit_count(&pool, q1).await, 0);
    let row = repo::get_review(&pool, q1).await.unwrap().unwrap();
    assert_eq!(row.status, "pending");

    // Kontextus-ütközés: HU entitás vs EN queue-kontextus → ContextConflict.
    let q2 = pending_queue(&pool, source_id, "err-2", &ctx_en()).await;
    let err = review::approve(&pool, q2, entity_id, "cli", None)
        .await
        .expect_err("eltérő kontextus nem hagyható jóvá");
    assert!(matches!(err, IdentityError::ContextConflict));
    assert_eq!(
        err.to_string(),
        "A kontextus nem egyezik a jelölt entitás kontextusával."
    );
    assert_eq!(ref_count(&pool, source_id, "err-2").await, 0);
    assert_eq!(audit_count(&pool, q2).await, 0);

    // Már döntött sor → AlreadyDecided; második approve nem ír új auditot.
    let q3 = pending_queue(&pool, source_id, "err-3", &ctx_hu()).await;
    review::approve(&pool, q3, entity_id, "cli", None)
        .await
        .unwrap();
    let err = review::approve(&pool, q3, entity_id, "cli", None)
        .await
        .expect_err("már döntött sor nem dönthető újra");
    assert!(matches!(err, IdentityError::AlreadyDecided));
    assert_eq!(err.to_string(), "A felülvizsgálati sor már el lett döntve.");
    assert_eq!(audit_count(&pool, q3).await, 1);

    // Forrás nélküli queue: kézi ref nem írható (nincs source_id kulcs) →
    // InvalidInput, dokumentált fail-closed korlát.
    let q4 = repo::upsert_pending_review(
        &pool,
        &repo::NewReview {
            source_id: None,
            entity_kind: "team".into(),
            source_entity_id: "err-4".into(),
            candidate_entity_ids: json!([]),
            reason: "no_exact_match".into(),
            proposed_name: "Ismeretlen Klub".into(),
            context: json!({"country_code": null, "gender": null,
                "age_group": null, "branch": null}),
        },
    )
    .await
    .unwrap();
    let err = review::approve(&pool, q4, entity_id, "cli", None)
        .await
        .expect_err("forrás nélküli sorhoz nem írható kézi ref");
    assert!(matches!(err, IdentityError::InvalidInput(_)));
    assert_eq!(
        err.to_string(),
        "A review-sorhoz nem tartozik forrás, kézi döntés nem lehetséges."
    );
    assert_eq!(audit_count(&pool, q4).await, 0);
}

// ---------------------------------------------------------------------------
// reject: végleges + auditált; új resolve új pending sort nyit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn reject_is_final_and_audited() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "demo-synthetic").await;
    let _other = seed_team(&pool, "Távoli FC", None, "male", "senior", "senior").await;
    let queue_id = pending_queue(&pool, source_id, "rej-1", &ctx_hu()).await;

    // Üres (vagy csak whitespace) jegyzet → InvalidInput, a sor pending marad.
    for note in ["", "   "] {
        let err = review::reject(&pool, queue_id, "cli", note)
            .await
            .expect_err("jegyzet nélkül nem utasítható el");
        assert!(matches!(err, IdentityError::InvalidInput(_)));
        assert_eq!(err.to_string(), "A jegyzet nem lehet üres.");
    }
    let row = repo::get_review(&pool, queue_id).await.unwrap().unwrap();
    assert_eq!(row.status, "pending");
    assert_eq!(audit_count(&pool, queue_id).await, 0);

    // Elutasítás jegyzettel → rejected + 1 audit sor, magyar jegyzet visszaolvasva.
    review::reject(&pool, queue_id, "cli", "Nem egyező klub")
        .await
        .unwrap();
    let row = repo::get_review(&pool, queue_id).await.unwrap().unwrap();
    assert_eq!(row.status, "rejected");
    let (decided_by, decision_note): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT decided_by, decision_note FROM identity.review_queue WHERE id = $1")
            .bind(queue_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(decided_by.as_deref(), Some("cli"));
    assert_eq!(decision_note.as_deref(), Some("Nem egyező klub"));
    assert_eq!(audit_count(&pool, queue_id).await, 1);
    let (action, detail): (String, serde_json::Value) =
        sqlx::query_as("SELECT action, detail FROM ops.audit_event WHERE subject_id = $1")
            .bind(queue_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(action, "identity_reject");
    assert_eq!(detail["queue_id"], json!(queue_id));
    assert_eq!(detail["ok"], json!("no_exact_match")); // a queue reason-je (spec §3.2)
    assert_eq!(detail["note"], json!("Nem egyező klub"));

    // Újra reject → AlreadyDecided, nem ír második auditot.
    let err = review::reject(&pool, queue_id, "cli", "Újabb jegyzet")
        .await
        .expect_err("végleges döntés nem ismételhető");
    assert!(matches!(err, IdentityError::AlreadyDecided));
    assert_eq!(err.to_string(), "A felülvizsgálati sor már el lett döntve.");
    assert_eq!(audit_count(&pool, queue_id).await, 1);

    // Nem létező queue → NotFound.
    let err = review::reject(&pool, Uuid::now_v7(), "cli", "jegyzet")
        .await
        .expect_err("nem létező queue nem utasítható el");
    assert!(matches!(err, IdentityError::NotFound));

    // Ugyanarra a párra új resolve: a rejected sor megmarad, ÚJ pending nyílik.
    let res = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "rej-1",
        "Ismeretlen Klub",
        &ctx_hu(),
    )
    .await
    .unwrap();
    match res {
        Resolution::NotFound { queue_id: q2 } => {
            assert_ne!(q2, queue_id, "új pending sornak új id kell");
            let old = repo::get_review(&pool, queue_id).await.unwrap().unwrap();
            assert_eq!(old.status, "rejected");
            let new = repo::get_review(&pool, q2).await.unwrap().unwrap();
            assert_eq!(new.status, "pending");
        }
        other => panic!("NotFound helyett: {other:?}"),
    }
    assert_eq!(queue_count(&pool).await, 2);
}

#[tokio::test]
async fn approve_with_mismatched_entity_kind_is_rejected() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "demo-synthetic").await;
    let queue_id = pending_queue(&pool, source_id, "rev-kind", &ctx_hu()).await;
    // A queue-sor team kind; a választott entitás player kind.
    let player_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'player','Teszt Játékos')",
    )
    .bind(player_id)
    .execute(&pool)
    .await
    .unwrap();

    let err = review::approve(&pool, queue_id, player_id, "cli", None)
        .await
        .expect_err("kind-eltérés hibát kell adjon");
    assert!(
        matches!(err, IdentityError::KindMismatch(_)),
        "ehelyett: {err:?}"
    );
    // A queue-sor pending marad, ref nem íródik.
    let status: String = sqlx::query_scalar("SELECT status FROM identity.review_queue WHERE id=$1")
        .bind(queue_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "pending", "az elutasított approve nem dönt");
    assert_eq!(ref_count(&pool, source_id, "rev-kind").await, 0);
}
