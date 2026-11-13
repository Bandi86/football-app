//! S2b-1 Task 3 — storage identity repository-k integrációs tesztjei.
//!
//! Minden interfészfüggvényt fed: source_id_by_code, list_sources, active_ref,
//! crosswalk (mindkét irány), reviewed alias + context(), entity join + context(),
//! all_team_entities, review upsert/get/list, decide, manual ref, entity_exists,
//! entity_context, audit, list_aliases. A magyar nyelvű szövegeket visszaolvassuk.

use football_domain::identity::IdentityContext;
use football_storage::repositories::identity as repo;
use football_storage::test_support::test_pool;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Seed segédek (minimális FK-lánc, a canonicalize mintáján)
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

#[allow(clippy::too_many_arguments)]
async fn seed_ref(
    pool: &PgPool,
    source_id: Uuid,
    entity_kind: &str,
    source_entity_id: &str,
    entity_id: Uuid,
    method: &str,
    status: &str,
) {
    sqlx::query(
        "INSERT INTO identity.source_entity_ref
         (id, source_id, entity_kind, source_entity_id, entity_id, method, status)
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(entity_kind)
    .bind(source_entity_id)
    .bind(entity_id)
    .bind(method)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
}

#[allow(clippy::too_many_arguments)]
async fn seed_alias(
    pool: &PgPool,
    entity_id: Uuid,
    alias: &str,
    lang: &str,
    country_code: Option<&str>,
    gender: Option<&str>,
    age_group: Option<&str>,
    branch: Option<&str>,
    status: &str,
) {
    sqlx::query(
        "INSERT INTO identity.alias
         (id, entity_id, alias, lang, country_code, gender, age_group, branch, status, method)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'fixture_seed')",
    )
    .bind(Uuid::now_v7())
    .bind(entity_id)
    .bind(alias)
    .bind(lang)
    .bind(country_code)
    .bind(gender)
    .bind(age_group)
    .bind(branch)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_crosswalk(
    pool: &PgPool,
    left_source: &str,
    left_external_id: &str,
    right_source: &str,
    right_external_id: &str,
    rung: &str,
    status: &str,
) {
    sqlx::query(
        "INSERT INTO identity.crosswalk
         (id, left_source, left_external_id, right_source, right_external_id,
          rung, method, status, evidence)
         VALUES ($1,$2,$3,$4,$5,$6,'import',$7,'teszt')",
    )
    .bind(Uuid::now_v7())
    .bind(left_source)
    .bind(left_external_id)
    .bind(right_source)
    .bind(right_external_id)
    .bind(rung)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
}

fn new_review(
    source_id: Option<Uuid>,
    source_entity_id: &str,
    proposed_name: &str,
) -> repo::NewReview {
    repo::NewReview {
        source_id,
        entity_kind: "team".into(),
        source_entity_id: source_entity_id.into(),
        candidate_entity_ids: json!([]),
        reason: "no_exact_match".into(),
        proposed_name: proposed_name.into(),
        context: json!({}),
    }
}

// ---------------------------------------------------------------------------
// Review upsert / dedup
// ---------------------------------------------------------------------------

#[tokio::test]
async fn pending_review_upsert_dedup() {
    let pool = test_pool().await;
    let r = repo::NewReview {
        source_id: None,
        entity_kind: "team".into(),
        source_entity_id: "x".into(),
        candidate_entity_ids: json!([]),
        reason: "no_exact_match".into(),
        proposed_name: "X".into(),
        context: json!({}),
    };
    let id1 = repo::upsert_pending_review(&pool, &r).await.unwrap();
    let id2 = repo::upsert_pending_review(&pool, &r).await.unwrap();
    assert_eq!(id1, id2); // ugyanaz a pending sor frissül
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.review_queue")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn upsert_updates_pending_fields_and_reopens_after_decision() {
    let pool = test_pool().await;
    let mut r = new_review(None, "x", "Demó FC");
    r.candidate_entity_ids = json!(["első"]);
    r.context = json!({"country_code": "HU"});
    let id1 = repo::upsert_pending_review(&pool, &r).await.unwrap();

    // Ugyanarra a kulcsra frissül: a mezők átíródnak, az id marad.
    r.proposed_name = "Demó FC II.".into();
    r.candidate_entity_ids = json!(["második"]);
    r.context = json!({"gender": "male"});
    let id2 = repo::upsert_pending_review(&pool, &r).await.unwrap();
    assert_eq!(id1, id2);
    let row = repo::get_review(&pool, id1).await.unwrap().unwrap();
    assert_eq!(row.proposed_name, "Demó FC II.");
    assert_eq!(row.candidate_entity_ids, json!(["második"]));
    assert_eq!(row.context, json!({"gender": "male"}));

    // Döntés után a partial unique index már nem fogja: új pending sor nyílik.
    assert_eq!(
        repo::decide_review(&pool, id1, "approved", "teszt", None)
            .await
            .unwrap(),
        1
    );
    let id3 = repo::upsert_pending_review(&pool, &r).await.unwrap();
    assert_ne!(id1, id3);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.review_queue")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 2);
}

// ---------------------------------------------------------------------------
// Decide + manual ref (brief 2. teszt)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn decide_only_pending_and_manual_ref_upsert() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "demo").await;
    let entity_id = seed_team(&pool, "Demó FC", None, "male", "senior", "senior").await;

    let r = new_review(Some(source_id), "x", "Demó FC");
    let queue_id = repo::upsert_pending_review(&pool, &r).await.unwrap();
    assert_eq!(
        repo::decide_review(&pool, queue_id, "approved", "test", None)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        repo::decide_review(&pool, queue_id, "approved", "test", None)
            .await
            .unwrap(),
        0
    );
    repo::insert_manual_ref(&pool, source_id, "team", "x", entity_id)
        .await
        .unwrap();
    repo::insert_manual_ref(&pool, source_id, "team", "x", entity_id)
        .await
        .unwrap();
    let (eid, method): (Uuid, String) = sqlx::query_as(
        "SELECT entity_id, method FROM identity.source_entity_ref
         WHERE source_id=$1 AND entity_kind='team' AND source_entity_id='x' AND status='active'",
    )
    .bind(source_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(eid, entity_id);
    assert_eq!(method, "manual_review");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.source_entity_ref")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn decide_review_writes_decision_fields_and_hungarian_note() {
    let pool = test_pool().await;
    let r = new_review(None, "x", "Demó FC");
    let id = repo::upsert_pending_review(&pool, &r).await.unwrap();

    // Nem létező sor döntése 0 érintett sor.
    assert_eq!(
        repo::decide_review(&pool, Uuid::now_v7(), "approved", "teszt", None)
            .await
            .unwrap(),
        0
    );

    let note = "Kézi ellenőrzés után jóváhagyva";
    assert_eq!(
        repo::decide_review(&pool, id, "approved", "ember", Some(note))
            .await
            .unwrap(),
        1
    );
    let (status, decided_by, decision_note): (String, String, String) = sqlx::query_as(
        "SELECT status, decided_by, decision_note FROM identity.review_queue WHERE id=$1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "approved");
    assert_eq!(decided_by, "ember");
    assert_eq!(decision_note, note); // magyar szöveg visszaolvasva
    let decided_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT decided_at FROM identity.review_queue WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(decided_at.is_some());
}

// ---------------------------------------------------------------------------
// source_id_by_code / list_sources / active_ref
// ---------------------------------------------------------------------------

#[tokio::test]
async fn source_id_by_code_found_and_missing() {
    let pool = test_pool().await;
    let id = seed_source(&pool, "demo-synthetic").await;
    assert_eq!(
        repo::source_id_by_code(&pool, "demo-synthetic")
            .await
            .unwrap(),
        Some(id)
    );
    assert_eq!(
        repo::source_id_by_code(&pool, "nincs-ilyen").await.unwrap(),
        None
    );
}

#[tokio::test]
async fn list_sources_returns_id_code_pairs_ordered_by_code() {
    let pool = test_pool().await;
    // Üres tábla → üres lista.
    assert!(repo::list_sources(&pool).await.unwrap().is_empty());

    let b = seed_source(&pool, "masik").await;
    let a = seed_source(&pool, "elso").await;
    let rows = repo::list_sources(&pool).await.unwrap();
    assert_eq!(
        rows,
        vec![(a, "elso".to_string()), (b, "masik".to_string())]
    );
}

#[tokio::test]
async fn active_ref_returns_only_active() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "demo").await;
    let entity_id = seed_team(&pool, "Demó FC", None, "male", "senior", "senior").await;

    // Nincs ref → None.
    assert_eq!(
        repo::active_ref(&pool, source_id, "team", "x")
            .await
            .unwrap(),
        None
    );

    seed_ref(
        &pool,
        source_id,
        "team",
        "x",
        entity_id,
        "fixture_seed",
        "active",
    )
    .await;
    assert_eq!(
        repo::active_ref(&pool, source_id, "team", "x")
            .await
            .unwrap(),
        Some((entity_id, "fixture_seed".to_string()))
    );

    // Superseded ref nem aktív.
    seed_ref(
        &pool,
        source_id,
        "team",
        "y",
        entity_id,
        "fixture_seed",
        "superseded",
    )
    .await;
    assert_eq!(
        repo::active_ref(&pool, source_id, "team", "y")
            .await
            .unwrap(),
        None
    );
}

// ---------------------------------------------------------------------------
// Crosswalk mindkét irány
// ---------------------------------------------------------------------------

#[tokio::test]
async fn active_crosswalk_both_directions_and_status_filter() {
    let pool = test_pool().await;
    seed_crosswalk(&pool, "demo", "1", "other", "2", "strong", "active").await;
    seed_crosswalk(&pool, "demo", "1", "third", "3", "weak", "superseded").await;

    // left → right
    let fwd = repo::active_crosswalk(&pool, "demo", "1").await.unwrap();
    assert_eq!(fwd.len(), 1);
    assert_eq!(fwd[0].right_source, "other");
    assert_eq!(fwd[0].right_external_id, "2");
    assert_eq!(fwd[0].rung, "strong");

    // right → left (a bal oldal kerül a right mezőkbe)
    let rev = repo::active_crosswalk(&pool, "other", "2").await.unwrap();
    assert_eq!(rev.len(), 1);
    assert_eq!(rev[0].right_source, "demo");
    assert_eq!(rev[0].right_external_id, "1");
    assert_eq!(rev[0].rung, "strong");

    // Superseded irány nem jelenik meg; ismeretlen kulcs üres.
    assert!(
        repo::active_crosswalk(&pool, "third", "3")
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        repo::active_crosswalk(&pool, "demo", "999")
            .await
            .unwrap()
            .is_empty()
    );
}

// ---------------------------------------------------------------------------
// Alias szűrés + context()
// ---------------------------------------------------------------------------

#[tokio::test]
async fn reviewed_aliases_by_name_filters_and_context() {
    let pool = test_pool().await;
    let entity_id = seed_team(&pool, "Demó FC", None, "male", "senior", "senior").await;

    // A UNIQUE (entity_id, alias, lang, ...) miatt a pending/rejected sorok
    // eltérő lang-ot kapnak, hogy ne ütközzenek a reviewed kulccsal.
    seed_alias(
        &pool,
        entity_id,
        "Demó FC",
        "hu",
        Some("HU"),
        Some("male"),
        Some("senior"),
        Some("senior"),
        "reviewed",
    )
    .await;
    seed_alias(
        &pool, entity_id, "Demó FC", "en", None, None, None, None, "pending",
    )
    .await;
    seed_alias(
        &pool, entity_id, "Demó FC", "de", None, None, None, None, "rejected",
    )
    .await;
    seed_alias(
        &pool,
        entity_id,
        "Más név",
        "hu",
        None,
        None,
        None,
        None,
        "reviewed",
    )
    .await;

    // lower(alias) = normalized, csak reviewed.
    let rows = repo::reviewed_aliases_by_name(&pool, "demó fc")
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].alias, "Demó FC");
    assert_eq!(rows[0].entity_id, entity_id);
    assert_eq!(rows[0].status, "reviewed");
    assert_eq!(
        rows[0].context(),
        IdentityContext {
            country_code: Some("HU".into()),
            gender: Some("male".into()),
            age_group: Some("senior".into()),
            branch: Some("senior".into()),
        }
    );

    // Nem egyező név üres.
    assert!(
        repo::reviewed_aliases_by_name(&pool, "nincs")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn list_aliases_filters_by_entity_and_returns_all() {
    let pool = test_pool().await;
    let a = seed_team(&pool, "Demó FC", None, "male", "senior", "senior").await;
    let b = seed_team(&pool, "Más FC", None, "male", "senior", "senior").await;
    seed_alias(
        &pool,
        a,
        "Demó FC",
        "hu",
        Some("HU"),
        None,
        None,
        None,
        "reviewed",
    )
    .await;
    seed_alias(&pool, a, "Demó", "hu", None, None, None, None, "pending").await;
    seed_alias(&pool, b, "Más FC", "hu", None, None, None, None, "reviewed").await;

    let only_a = repo::list_aliases(&pool, Some(a)).await.unwrap();
    assert_eq!(only_a.len(), 2);
    assert!(only_a.iter().all(|r| r.entity_id == a));

    let all = repo::list_aliases(&pool, None).await.unwrap();
    assert_eq!(all.len(), 3);

    assert!(
        repo::list_aliases(&pool, Some(Uuid::now_v7()))
            .await
            .unwrap()
            .is_empty()
    );
}

// ---------------------------------------------------------------------------
// Entity join + context() / all_team_entities
// ---------------------------------------------------------------------------

#[tokio::test]
async fn team_entities_by_normalized_name_join_and_context() {
    let pool = test_pool().await;
    let country_id = seed_country(&pool, "HU", "Magyarország").await;
    let entity_id = seed_team(
        &pool,
        "Demó FC",
        Some(country_id),
        "male",
        "senior",
        "senior",
    )
    .await;
    // Azonos normalizált név másik entitásként ne zavarjon: nem team kind.
    let other = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'player','Demó FC')",
    )
    .bind(other)
    .execute(&pool)
    .await
    .unwrap();

    let rows = repo::team_entities_by_normalized_name(&pool, "demó fc")
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].entity_id, entity_id);
    assert_eq!(rows[0].canonical_name, "Demó FC");
    assert_eq!(
        rows[0].context(),
        IdentityContext {
            country_code: Some("HU".into()),
            gender: Some("male".into()),
            age_group: Some("senior".into()),
            branch: Some("senior".into()),
        }
    );

    assert!(
        repo::team_entities_by_normalized_name(&pool, "nincs")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn all_team_entities_includes_countryless_teams() {
    let pool = test_pool().await;
    let country_id = seed_country(&pool, "HU", "Magyarország").await;
    let a = seed_team(
        &pool,
        "Demó FC",
        Some(country_id),
        "male",
        "senior",
        "senior",
    )
    .await;
    let b = seed_team(&pool, "Vidék FC", None, "female", "youth", "youth").await;

    let rows = repo::all_team_entities(&pool).await.unwrap();
    assert_eq!(rows.len(), 2);
    let by_id = |id: Uuid| rows.iter().find(|r| r.entity_id == id).unwrap();
    assert_eq!(by_id(a).country_code.as_deref(), Some("HU"));
    // Ország nélküli csapat is jelen van, country_code = None (LEFT JOIN).
    assert_eq!(by_id(b).country_code, None);
    assert_eq!(by_id(b).gender.as_deref(), Some("female"));
    assert_eq!(by_id(b).branch.as_deref(), Some("youth"));
}

// ---------------------------------------------------------------------------
// get_review / list_reviews
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_and_list_reviews_with_status_and_order() {
    let pool = test_pool().await;
    let older = repo::upsert_pending_review(&pool, &new_review(None, "a", "Régi FC"))
        .await
        .unwrap();
    let newer = repo::upsert_pending_review(&pool, &new_review(None, "b", "Új FC"))
        .await
        .unwrap();
    // Garantált created_at sorrend a DESC teszthez.
    sqlx::query(
        "UPDATE identity.review_queue SET created_at = now() - interval '1 hour' WHERE id=$1",
    )
    .bind(older)
    .execute(&pool)
    .await
    .unwrap();

    let got = repo::get_review(&pool, newer).await.unwrap().unwrap();
    assert_eq!(got.id, newer);
    assert_eq!(got.proposed_name, "Új FC"); // magyar szöveg visszaolvasva
    assert_eq!(got.status, "pending");
    assert_eq!(got.reason, "no_exact_match");
    assert!(
        repo::get_review(&pool, Uuid::now_v7())
            .await
            .unwrap()
            .is_none()
    );

    // created_at DESC: a newer az első.
    let all = repo::list_reviews(&pool, None).await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].id, newer);
    assert_eq!(all[1].id, older);

    // Státusz szűrő.
    repo::decide_review(&pool, older, "rejected", "teszt", Some("elutasítva"))
        .await
        .unwrap();
    let pending = repo::list_reviews(&pool, Some("pending")).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, newer);
    let rejected = repo::list_reviews(&pool, Some("rejected")).await.unwrap();
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0].id, older);
}

// ---------------------------------------------------------------------------
// entity_exists / entity_context
// ---------------------------------------------------------------------------

#[tokio::test]
async fn entity_exists_and_context() {
    let pool = test_pool().await;
    let country_id = seed_country(&pool, "HU", "Magyarország").await;
    let entity_id = seed_team(
        &pool,
        "Demó FC",
        Some(country_id),
        "male",
        "senior",
        "senior",
    )
    .await;

    assert!(repo::entity_exists(&pool, entity_id).await.unwrap());
    assert!(!repo::entity_exists(&pool, Uuid::now_v7()).await.unwrap());

    assert_eq!(
        repo::entity_context(&pool, entity_id).await.unwrap(),
        Some(IdentityContext {
            country_code: Some("HU".into()),
            gender: Some("male".into()),
            age_group: Some("senior".into()),
            branch: Some("senior".into()),
        })
    );
    // Nem létező entitás → None.
    assert_eq!(
        repo::entity_context(&pool, Uuid::now_v7()).await.unwrap(),
        None
    );

    // Team sor nélküli entitás → None.
    let bare = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team','Csak entitás')",
    )
    .bind(bare)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(repo::entity_context(&pool, bare).await.unwrap(), None);

    // Ország nélküli csapat → country_code None.
    let countryless = seed_team(&pool, "Vidék FC", None, "female", "youth", "youth").await;
    let ctx = repo::entity_context(&pool, countryless)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ctx.country_code, None);
    assert_eq!(ctx.gender.as_deref(), Some("female"));
}

// ---------------------------------------------------------------------------
// Audit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn record_identity_decision_inserts_audit_event() {
    let pool = test_pool().await;
    let queue_id = Uuid::now_v7();
    let detail =
        json!({"queue_id": queue_id, "entity": "Demó FC", "ok": true, "note": "Jóváhagyva"});
    repo::record_identity_decision(&pool, "ember", "identity_approve", queue_id, detail.clone())
        .await
        .unwrap();

    let (actor, action, subject_kind, subject_id, stored): (
        String,
        String,
        String,
        Uuid,
        serde_json::Value,
    ) = sqlx::query_as(
        "SELECT actor, action, subject_kind, subject_id, detail FROM ops.audit_event",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(actor, "ember");
    assert_eq!(action, "identity_approve");
    assert_eq!(subject_kind, "review_queue");
    assert_eq!(subject_id, queue_id);
    assert_eq!(stored, detail); // magyar szöveg visszaolvasva
    assert_eq!(stored["note"], "Jóváhagyva");
}

// ---------------------------------------------------------------------------
// Executor-generikus függvények tranzakcióban (Task 5 újrafelhasználás)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn executor_generic_functions_work_inside_transaction() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "demo").await;
    let entity_id = seed_team(&pool, "Demó FC", None, "male", "senior", "senior").await;
    let queue_id = repo::upsert_pending_review(&pool, &new_review(Some(source_id), "x", "Demó FC"))
        .await
        .unwrap();

    let mut tx = pool.begin().await.unwrap();
    repo::insert_manual_ref(&mut *tx, source_id, "team", "x", entity_id)
        .await
        .unwrap();
    let affected = repo::decide_review(
        &mut *tx,
        queue_id,
        "approved",
        "ember",
        Some("tranzakcióban"),
    )
    .await
    .unwrap();
    assert_eq!(affected, 1);
    repo::record_identity_decision(
        &mut *tx,
        "ember",
        "identity_approve",
        queue_id,
        json!({"ok": true}),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // A tranzakció után minden látható.
    assert_eq!(
        repo::active_ref(&pool, source_id, "team", "x")
            .await
            .unwrap(),
        Some((entity_id, "manual_review".to_string()))
    );
    assert_eq!(
        repo::get_review(&pool, queue_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "approved"
    );
    let audits: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.audit_event")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(audits, 1);
}
