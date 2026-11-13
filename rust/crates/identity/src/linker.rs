//! Fail-closed linker: forrás-entitás feloldása kanonikus entitásra.
//!
//! A spec §3.1 feloldási sorrendje:
//! 1. egzakt külső ID (aktív ref) → Exact,
//! 2. crosswalk-lánc (csak strong/medium; weak teljesen kihagyva) → Exact,
//! 3. reviewed alias kontextus-szűrővel, pontosan 1 entitás → Exact,
//! 4. egzakt normalizált név kontextus-szűrővel, pontosan 1 → Exact,
//! 5. fuzzy jelöltek (token_overlap ≥ FUZZY_THRESHOLD, kontextus-szűrve,
//!    soha nem Exact) → Candidates,
//! 6. NotFound.
//!
//! A Candidates és a NotFound mindig pending review-sort nyit (upsert), és
//! visszaadja annak queue_id-ját. DB-hiba `IdentityError::Db` — a hívó
//! fail-closed módon kezelje.

use std::collections::HashSet;

use football_domain::identity::{
    Candidate, FUZZY_THRESHOLD, IdentityContext, Resolution, ResolveMethod, context_matches,
    normalize_name, token_overlap,
};
use football_storage::repositories::identity as repo;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::IdentityError;

/// Felold egy forrás-entitást a kanonikus entitás-térbe a fail-closed
/// sorrend szerint. A `source_slug` a crosswalk szöveges forrás-kulcsa.
pub async fn resolve(
    pool: &PgPool,
    source_id: Uuid,
    source_slug: &str,
    entity_kind: &str,
    source_entity_id: &str,
    proposed_name: &str,
    ctx: &IdentityContext,
) -> Result<Resolution, IdentityError> {
    // A `{:#}` alternate form megőrzi a sqlx gyökérokot is a hibaláncban
    // (a review.rs db_err-jével azonos formátum).
    let db = |e: anyhow::Error| IdentityError::Db(format!("{e:#}"));

    // 1. egzakt külső ID
    if let Some((entity_id, method)) =
        repo::active_ref(pool, source_id, entity_kind, source_entity_id)
            .await
            .map_err(db)?
    {
        let m = match method.as_str() {
            "manual_review" => ResolveMethod::ManualReview,
            _ => ResolveMethod::ExactExternalId,
        };
        return Ok(Resolution::Exact {
            entity_id,
            method: m,
        });
    }

    // 2. crosswalk-lánc (strong/medium; minden elem aktív). A weak rung
    // sem Exact-et, sem jelöltet nem ad.
    for cw in repo::active_crosswalk(pool, source_slug, source_entity_id)
        .await
        .map_err(db)?
    {
        if cw.rung == "weak" {
            continue;
        }
        if let Some(target_source) = repo::source_id_by_code(pool, &cw.right_source)
            .await
            .map_err(db)?
        {
            if let Some((entity_id, _)) =
                repo::active_ref(pool, target_source, entity_kind, &cw.right_external_id)
                    .await
                    .map_err(db)?
            {
                return Ok(Resolution::Exact {
                    entity_id,
                    method: ResolveMethod::CrosswalkChain,
                });
            }
        }
    }

    let normalized = normalize_name(proposed_name);

    // 3. review-olt alias + kontextus (pontosan 1 különböző entitás)
    let aliases = repo::reviewed_aliases_by_name(pool, &normalized)
        .await
        .map_err(db)?;
    let alias_entities: HashSet<Uuid> = aliases
        .iter()
        .filter(|a| context_matches(ctx, &a.context()))
        .map(|a| a.entity_id)
        .collect();
    if alias_entities.len() == 1 {
        if let Some(&entity_id) = alias_entities.iter().next() {
            return Ok(Resolution::Exact {
                entity_id,
                method: ResolveMethod::ReviewedAlias,
            });
        }
    }

    // 4. egzakt normalizált név + kontextus (pontosan 1)
    let entities = repo::team_entities_by_normalized_name(pool, &normalized)
        .await
        .map_err(db)?;
    let name_hits: Vec<_> = entities
        .iter()
        .filter(|e| context_matches(ctx, &e.context()))
        .collect();
    if name_hits.len() == 1 {
        return Ok(Resolution::Exact {
            entity_id: name_hits[0].entity_id,
            method: ResolveMethod::ExactName,
        });
    }

    // 5. fuzzy jelöltek (soha Exact) + többértelmű név jelöltjei
    let mut candidates: Vec<Candidate> = Vec::new();
    if name_hits.len() > 1 {
        for e in &name_hits {
            candidates.push(Candidate {
                entity_id: e.entity_id,
                score: 1.0,
                reason: "multiple_candidates",
            });
        }
    }
    for e in repo::all_team_entities(pool).await.map_err(db)? {
        if !context_matches(ctx, &e.context()) {
            continue;
        }
        let score = token_overlap(proposed_name, &e.canonical_name);
        if score >= FUZZY_THRESHOLD && !candidates.iter().any(|c| c.entity_id == e.entity_id) {
            candidates.push(Candidate {
                entity_id: e.entity_id,
                score,
                reason: "fuzzy_candidate",
            });
        }
    }

    // 6. Candidates / NotFound — mindkettő pending review-sort nyit.
    let reason = if !candidates.is_empty() {
        if name_hits.len() > 1 {
            "multiple_candidates"
        } else {
            "fuzzy_candidate"
        }
    } else {
        "no_exact_match"
    };
    let candidate_ids: Vec<Uuid> = candidates.iter().map(|c| c.entity_id).collect();
    let queue_id = repo::upsert_pending_review(
        pool,
        &repo::NewReview {
            source_id: Some(source_id),
            entity_kind: entity_kind.into(),
            source_entity_id: source_entity_id.into(),
            candidate_entity_ids: serde_json::to_value(&candidate_ids)
                .map_err(|e| IdentityError::Db(e.to_string()))?,
            reason: reason.into(),
            proposed_name: proposed_name.into(),
            // Az IdentityContext nem Serialize (a domain crate tiszta marad),
            // ezért a kontextus JSON-t itt állítjuk elő, mezőnként.
            context: serde_json::json!({
                "country_code": ctx.country_code,
                "gender": ctx.gender,
                "age_group": ctx.age_group,
                "branch": ctx.branch,
            }),
        },
    )
    .await
    .map_err(db)?;
    if candidates.is_empty() {
        Ok(Resolution::NotFound { queue_id })
    } else {
        Ok(Resolution::Candidates {
            candidates,
            queue_id,
        })
    }
}
