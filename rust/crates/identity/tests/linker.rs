//! S2b-1 Task 4 — a fail-closed linker integrációs tesztjei.
//!
//! Mind a 6 feloldási út + garanciák: egzakt ref (manual_review mapping,
//! superseded kizárás), crosswalk strong/medium → Exact / weak → nem Exact,
//! reviewed alias kontextus-szűrővel, egzakt név egyértelmű vs. többértelmű,
//! fuzzy soha nem Exact + kontextus-szűrő + küszöb, NotFound queue + pending
//! dedup, DB-hiba fail-closed. A magyar nyelvű szövegeket visszaolvassuk.

use football_domain::identity::{IdentityContext, Resolution, ResolveMethod};
use football_identity::{IdentityError, resolve};
use football_storage::repositories::identity as repo;
use football_storage::test_support::test_pool;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Seed segédek (az identity_repos.rs mintáján)
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
    source_entity_id: &str,
    entity_id: Uuid,
    method: &str,
    status: &str,
) {
    sqlx::query(
        "INSERT INTO identity.source_entity_ref
         (id, source_id, entity_kind, source_entity_id, entity_id, method, status)
         VALUES ($1,$2,'team',$3,$4,$5,$6)",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
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
    status: &str,
) {
    sqlx::query(
        "INSERT INTO identity.alias
         (id, entity_id, alias, lang, country_code, status, method)
         VALUES ($1,$2,$3,$4,$5,$6,'fixture_seed')",
    )
    .bind(Uuid::now_v7())
    .bind(entity_id)
    .bind(alias)
    .bind(lang)
    .bind(country_code)
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
) {
    sqlx::query(
        "INSERT INTO identity.crosswalk
         (id, left_source, left_external_id, right_source, right_external_id,
          rung, method, status, evidence)
         VALUES ($1,$2,$3,$4,$5,$6,'import','active','teszt')",
    )
    .bind(Uuid::now_v7())
    .bind(left_source)
    .bind(left_external_id)
    .bind(right_source)
    .bind(right_external_id)
    .bind(rung)
    .execute(pool)
    .await
    .unwrap();
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

async fn queue_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM identity.review_queue")
        .fetch_one(pool)
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// 1. út: egzakt külső ID
// ---------------------------------------------------------------------------

#[tokio::test]
async fn exact_external_id_resolves() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "demo-synthetic").await;
    let entity_id = seed_team(&pool, "Demó FC", None, "male", "senior", "senior").await;
    seed_ref(
        &pool,
        source_id,
        "ext-1",
        entity_id,
        "fixture_seed",
        "active",
    )
    .await;

    // A ref a névtől függetlenül felold (a proposed_name szándékosan eltér).
    let res = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "ext-1",
        "Eltérő Név",
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
            assert_eq!(method, ResolveMethod::ExactExternalId);
        }
        other => panic!("Exact(ExactExternalId) helyett: {other:?}"),
    }

    // manual_review ref → ManualReview módszer.
    seed_ref(
        &pool,
        source_id,
        "ext-2",
        entity_id,
        "manual_review",
        "active",
    )
    .await;
    let res = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "ext-2",
        "Demó FC",
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

    // Egzakt feloldás nem nyit review-sort.
    assert_eq!(queue_count(&pool).await, 0);

    // Superseded ref nem old fel: ismeretlen névvel NotFound + queue.
    seed_ref(
        &pool,
        source_id,
        "ext-sup",
        entity_id,
        "fixture_seed",
        "superseded",
    )
    .await;
    let res = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "ext-sup",
        "Ismeretlen Név",
        &ctx_hu(),
    )
    .await
    .unwrap();
    assert!(matches!(res, Resolution::NotFound { .. }));
    assert_eq!(queue_count(&pool).await, 1);
}

// ---------------------------------------------------------------------------
// 2. út: crosswalk-lánc (strong/medium → Exact; weak → nem Exact)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn crosswalk_strong_chain_resolves_weak_only_candidates() {
    let pool = test_pool().await;
    let demo_id = seed_source(&pool, "demo-synthetic").await;
    let other_id = seed_source(&pool, "other-synthetic").await;
    let entity_id = seed_team(&pool, "Demó FC", None, "male", "senior", "senior").await;

    // strong: demo cw-strong ↔ other o-1, other-ref aktív → Exact(CrosswalkChain).
    seed_crosswalk(
        &pool,
        "demo-synthetic",
        "cw-strong",
        "other-synthetic",
        "o-1",
        "strong",
    )
    .await;
    seed_ref(&pool, other_id, "o-1", entity_id, "fixture_seed", "active").await;
    let res = resolve(
        &pool,
        demo_id,
        "demo-synthetic",
        "team",
        "cw-strong",
        "Ismeretlen Név",
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
            assert_eq!(method, ResolveMethod::CrosswalkChain);
        }
        other => panic!("Exact(CrosswalkChain) helyett: {other:?}"),
    }

    // medium: szintén Exact(CrosswalkChain).
    seed_crosswalk(
        &pool,
        "demo-synthetic",
        "cw-medium",
        "other-synthetic",
        "o-2",
        "medium",
    )
    .await;
    seed_ref(&pool, other_id, "o-2", entity_id, "fixture_seed", "active").await;
    let res = resolve(
        &pool,
        demo_id,
        "demo-synthetic",
        "team",
        "cw-medium",
        "Ismeretlen Név",
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
            assert_eq!(method, ResolveMethod::CrosswalkChain);
        }
        other => panic!("Exact(CrosswalkChain) medium helyett: {other:?}"),
    }

    // weak: a lánc nem ad Exact-et, a jelölt-listába sem kerül → NotFound,
    // a queue reason no_exact_match.
    seed_crosswalk(
        &pool,
        "demo-synthetic",
        "cw-weak",
        "other-synthetic",
        "o-3",
        "weak",
    )
    .await;
    seed_ref(&pool, other_id, "o-3", entity_id, "fixture_seed", "active").await;
    let res = resolve(
        &pool,
        demo_id,
        "demo-synthetic",
        "team",
        "cw-weak",
        "Ismeretlen Név",
        &ctx_hu(),
    )
    .await
    .unwrap();
    match res {
        Resolution::NotFound { queue_id } => {
            let row = repo::get_review(&pool, queue_id).await.unwrap().unwrap();
            assert_eq!(row.reason, "no_exact_match");
            assert_eq!(row.status, "pending");
            assert_eq!(row.candidate_entity_ids, json!([]));
        }
        other => panic!("weak crosswalk nem lehet Exact/jelölt: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 3. út: reviewed alias + kontextus
// ---------------------------------------------------------------------------

#[tokio::test]
async fn reviewed_alias_with_context_resolves() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "demo-synthetic").await;
    let hu_id = seed_country(&pool, "HU", "Magyarország").await;
    // A kanonikus név szándékosan más: csak az alias oldhat fel.
    let entity_id = seed_team(
        &pool,
        "Demó Sportklub",
        Some(hu_id),
        "male",
        "senior",
        "senior",
    )
    .await;
    seed_alias(&pool, entity_id, "Demó FC", "hu", Some("HU"), "reviewed").await;

    // HU kontextus → Exact(ReviewedAlias).
    let res = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "alias-1",
        "Demó FC",
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
            assert_eq!(method, ResolveMethod::ReviewedAlias);
        }
        other => panic!("Exact(ReviewedAlias) helyett: {other:?}"),
    }

    // EN kontextus: a HU alias kiszűrődik → nem Exact.
    let res = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "alias-2",
        "Demó FC",
        &ctx_en(),
    )
    .await
    .unwrap();
    assert!(
        matches!(res, Resolution::NotFound { .. }),
        "EN kontextusban nem lehet Exact: {res:?}"
    );

    // Pending alias EN kontextussal sem old fel (csak reviewed számít).
    let other_entity = seed_team(&pool, "Második FC", None, "male", "senior", "senior").await;
    seed_alias(&pool, other_entity, "Demó FC", "en", Some("EN"), "pending").await;
    let res = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "alias-3",
        "Demó FC",
        &ctx_en(),
    )
    .await
    .unwrap();
    assert!(
        matches!(res, Resolution::NotFound { .. }),
        "pending alias nem oldhat fel: {res:?}"
    );
}

// ---------------------------------------------------------------------------
// 4. út: egzakt normalizált név (egyértelmű vs. többértelmű)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn exact_name_unique_vs_ambiguous() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "demo-synthetic").await;
    let hu_id = seed_country(&pool, "HU", "Magyarország").await;
    let en_id = seed_country(&pool, "EN", "Anglia").await;
    let a = seed_team(&pool, "Demó FC", Some(hu_id), "male", "senior", "senior").await;

    // Egyetlen egyező entitás → Exact(ExactName); a normalizálás a dupla
    // szóközt is kezeli.
    let res = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "name-1",
        "Demó  FC",
        &ctx_hu(),
    )
    .await
    .unwrap();
    match res {
        Resolution::Exact {
            entity_id: e,
            method,
        } => {
            assert_eq!(e, a);
            assert_eq!(method, ResolveMethod::ExactName);
        }
        other => panic!("Exact(ExactName) helyett: {other:?}"),
    }
    assert_eq!(queue_count(&pool).await, 0);

    // Második azonos normalizált nevű HU entitás → Candidates + queue.
    let b = seed_team(&pool, "Demó FC", Some(hu_id), "male", "senior", "senior").await;
    let res = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "name-2",
        "Demó FC",
        &ctx_hu(),
    )
    .await
    .unwrap();
    match res {
        Resolution::Candidates {
            candidates,
            queue_id,
        } => {
            assert_eq!(candidates.len(), 2);
            let ids: Vec<Uuid> = candidates.iter().map(|c| c.entity_id).collect();
            assert!(ids.contains(&a) && ids.contains(&b));
            assert!(candidates.iter().all(|c| c.reason == "multiple_candidates"));
            assert!(candidates.iter().all(|c| c.score == 1.0));
            let row = repo::get_review(&pool, queue_id).await.unwrap().unwrap();
            assert_eq!(row.reason, "multiple_candidates");
            assert_eq!(row.status, "pending");
            assert_eq!(row.proposed_name, "Demó FC"); // magyar szöveg visszaolvasva
            let stored: Vec<Uuid> = serde_json::from_value(row.candidate_entity_ids).unwrap();
            assert_eq!(stored.len(), 2);
            assert!(stored.contains(&a) && stored.contains(&b));
        }
        other => panic!("Candidates(multiple_candidates) helyett: {other:?}"),
    }

    // A kontextus-szűrő egyértelműsít: EN kontextusban csak az EN entitás marad.
    let c = seed_team(&pool, "Demó FC", Some(en_id), "male", "senior", "senior").await;
    let res = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "name-3",
        "Demó FC",
        &ctx_en(),
    )
    .await
    .unwrap();
    match res {
        Resolution::Exact {
            entity_id: e,
            method,
        } => {
            assert_eq!(e, c);
            assert_eq!(method, ResolveMethod::ExactName);
        }
        other => panic!("kontextus-szűrt Exact(ExactName) helyett: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 5. út: fuzzy jelöltek (soha Exact) + kontextus-szűrő + küszöb
// ---------------------------------------------------------------------------

#[tokio::test]
async fn fuzzy_never_exact_and_queues() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "demo-synthetic").await;
    let hu_id = seed_country(&pool, "HU", "Magyarország").await;
    let en_id = seed_country(&pool, "EN", "Anglia").await;
    let hu_team = seed_team(&pool, "Demó FC", Some(hu_id), "male", "senior", "senior").await;
    // Azonos nevű EN entitás: a kontextus-szűrőnek ki kell zárnia.
    let _en_team = seed_team(&pool, "Demó FC", Some(en_id), "male", "senior", "senior").await;

    // 'Demó FC U19' vs 'Demó FC': token-átfedés 2/3 ≥ 0.6 → Candidates,
    // soha nem Exact.
    let res = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "fuzzy-1",
        "Demó FC U19",
        &ctx_hu(),
    )
    .await
    .unwrap();
    match res {
        Resolution::Candidates {
            candidates,
            queue_id,
        } => {
            assert_eq!(candidates.len(), 1, "EN jelölt nem kerülhet a listába");
            assert_eq!(candidates[0].entity_id, hu_team);
            assert_eq!(candidates[0].reason, "fuzzy_candidate");
            assert!((candidates[0].score - 2.0 / 3.0).abs() < 1e-9);
            let row = repo::get_review(&pool, queue_id).await.unwrap().unwrap();
            assert_eq!(row.reason, "fuzzy_candidate");
            assert_eq!(row.status, "pending");
            assert_eq!(row.proposed_name, "Demó FC U19");
            assert_eq!(row.candidate_entity_ids, json!([hu_team]));
        }
        other => panic!("fuzzy soha nem lehet Exact: {other:?}"),
    }

    // Küszöb alatt (2/4 = 0.5 < 0.6) nincs jelölt → NotFound.
    let res = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "fuzzy-2",
        "Demó FC U19 U21",
        &ctx_hu(),
    )
    .await
    .unwrap();
    assert!(
        matches!(res, Resolution::NotFound { .. }),
        "küszöb alatt nem lehet jelölt: {res:?}"
    );
}

// ---------------------------------------------------------------------------
// 6. út: NotFound → queue + pending dedup
// ---------------------------------------------------------------------------

#[tokio::test]
async fn not_found_queues_and_pending_dedup() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "demo-synthetic").await;
    // Nem egyező nevű entitás: a fuzzy átfedés 0.
    let _other = seed_team(&pool, "Távoli FC", None, "male", "senior", "senior").await;

    let res = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "nf-1",
        "Ismeretlen Klub",
        &ctx_hu(),
    )
    .await
    .unwrap();
    let queue_id = match res {
        Resolution::NotFound { queue_id } => queue_id,
        other => panic!("NotFound helyett: {other:?}"),
    };
    let row = repo::get_review(&pool, queue_id).await.unwrap().unwrap();
    assert_eq!(row.status, "pending");
    assert_eq!(row.reason, "no_exact_match");
    assert_eq!(row.proposed_name, "Ismeretlen Klub"); // magyar szöveg visszaolvasva
    assert_eq!(row.candidate_entity_ids, json!([]));
    assert_eq!(
        row.context,
        json!({"country_code": "HU", "gender": null, "age_group": null, "branch": null})
    );
    assert_eq!(row.source_id, Some(source_id));

    // Második resolve ugyanarra a kulcsra: ugyanaz a pending queue_id.
    let res = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "nf-1",
        "Ismeretlen Klub",
        &ctx_hu(),
    )
    .await
    .unwrap();
    match res {
        Resolution::NotFound { queue_id: q2 } => assert_eq!(q2, queue_id),
        other => panic!("NotFound helyett: {other:?}"),
    }
    assert_eq!(queue_count(&pool).await, 1);
}

// ---------------------------------------------------------------------------
// Garanciák: DB-hiba fail-closed + magyar hibaüzenetek
// ---------------------------------------------------------------------------

#[tokio::test]
async fn db_error_is_fail_closed() {
    let pool = test_pool().await;
    let source_id = Uuid::now_v7();
    pool.close().await;

    let err = resolve(
        &pool,
        source_id,
        "demo-synthetic",
        "team",
        "x",
        "Demó FC",
        &ctx_hu(),
    )
    .await
    .expect_err("zárt pool esetén DB-hiba kell");
    match &err {
        IdentityError::Db(msg) => assert!(!msg.is_empty()),
        other => panic!("IdentityError::Db helyett: {other:?}"),
    }
    // A megjelenített hibaüzenet magyar nyelvű (visszaolvasva).
    assert!(
        err.to_string()
            .starts_with("Adatbázis-hiba az identitás-feloldás közben:"),
        "magyar üzenet: {err}"
    );
}

#[test]
fn hungarian_error_messages() {
    assert_eq!(
        IdentityError::Db("kapcsolat megszakadt".into()).to_string(),
        "Adatbázis-hiba az identitás-feloldás közben: kapcsolat megszakadt"
    );
    assert_eq!(
        IdentityError::NotFound.to_string(),
        "A kért review-sor nem található."
    );
    assert_eq!(
        IdentityError::AmbiguousDecision.to_string(),
        "Több lehetséges egyezés; kézi döntés szükséges."
    );
    assert_eq!(
        IdentityError::ContextConflict.to_string(),
        "A kontextus nem egyezik a jelölt entitás kontextusával."
    );
    assert_eq!(
        IdentityError::AlreadyDecided.to_string(),
        "A felülvizsgálati sor már el lett döntve."
    );
    assert_eq!(
        IdentityError::InvalidEntity.to_string(),
        "Érvénytelen entitásazonosító."
    );
}
