//! Fail-closed linker: forrás-entitás feloldása kanonikus entitásra.
//!
//! A spec §3.1 feloldási sorrendje:
//! 1. egzakt külső ID (aktív ref) → Exact,
//! 2. crosswalk-lánc (csak strong/medium; weak teljesen kihagyva) → Exact,
//! 3. reviewed alias kontextus-szűrővel, pontosan 1 entitás → Exact,
//! 4. egzakt foldolt név kontextus-szűrővel, `entity_kind`-ra szűrve,
//!    pontosan 1 → Exact. Női kontextusban a csapat-ág a jelölő nélküli névhez
//!    marker-normalizált egzakt kulcsokat is kipróbál ("…" → "…w"/"…women"),
//!    mert a katalógus a női csapatokat jelölővel tárolja,
//! 5. fuzzy jelöltek (token_overlap ≥ FUZZY_THRESHOLD, kontextus- és
//!    fajtaszűrve, soha nem Exact) → Candidates,
//! 6. NotFound.
//!
//! A 4-5. lépés a kért entitás-fajta katalógusán dolgozik (nem fixen a
//! csapatokon): a csapat-ág viselkedése változatlan, a játékvezető-ág saját
//! katalógust lát. A játékvezető-ágon a 4. lépés csak TELJES név egyezésénél
//! linkel automatikusan; a rövid alak (vezetéknév + kezdőbetű) az AGENTS.md §6
//! szerint csak review-jelölt lehet (lásd `is_full_person_name`).
//!
//! A Candidates és a NotFound mindig pending review-sort nyit (upsert), és
//! visszaadja annak queue_id-ját. DB-hiba `IdentityError::Db` — a hívó
//! fail-closed módon kezelje.

use std::collections::HashSet;

use football_domain::identity::{
    Candidate, FUZZY_THRESHOLD, IdentityContext, Resolution, ResolveMethod, context_matches,
    fold_keep_separators, fold_name, token_overlap,
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
            && let Some((entity_id, _)) =
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

    // S1: az exact kulcs a foldolt név (ékezet + írásjel + whitespace nélkül);
    // a fuzzy jelöltképzés továbbra is a normalize_name/token_overlap marad.
    let folded = fold_name(proposed_name);

    // 3. review-olt alias + kontextus (pontosan 1 különböző entitás)
    let aliases = repo::reviewed_aliases_by_name(pool, &folded)
        .await
        .map_err(db)?;
    let alias_entities: HashSet<Uuid> = aliases
        .iter()
        .filter(|a| context_matches(ctx, &a.context()))
        .map(|a| a.entity_id)
        .collect();
    if alias_entities.len() == 1
        && let Some(&entity_id) = alias_entities.iter().next()
    {
        return Ok(Resolution::Exact {
            entity_id,
            method: ResolveMethod::ReviewedAlias,
        });
    }

    // 4. egzakt foldolt név + kontextus, a kért fajta katalógusán
    // (pontosan 1).
    //
    // Referee-ág: csak TELJES név egyezése linkelhet automatikusan. A rövid
    // alak (pl. "M Oliver", "Fyfe, I. M.") egy másik játékvezetőre is
    // illeszkedhet, ezért az AGENTS.md §6 tiltja az automatikus fuzzy
    // entitás-összevonást — ilyenkor a találat csak review-jelölt lesz.
    let mut entities = repo::entities_by_normalized_name(pool, entity_kind, &folded)
        .await
        .map_err(db)?;
    // Női csapatnevek: a tipplap jelölő nélkül küldi a nevet ("Granada"), a
    // kanonikus katalógus viszont jelölővel tárolja ("Granada W", "Granada (W)",
    // "Granada Women"). A `fold_name` minden elválasztót és szóközt eldob, ezért
    // `granada` ≠ `granadaw`, és a 4. lépés nem talál. 2026-11-22-én 26 ilyen
    // pending team-sor volt (mind női csapat, gender=female/branch=women
    // kontextussal), a katalógusban pedig 17 markerelt női csapatnév.
    //
    // Csak a marker HOZZÁADÁSÁNAK irányában bővítjük a kulcsot, és csak női
    // kontextusban: ez nem fuzzy merge (AGENTS.md §6), hanem további EGZAKT
    // string-kulcs — `fold(proposed)+"w"` és `fold(proposed)+"women"` —, amit a
    // kontextus-egyezés és az egyértelműség (pontosan 1 különböző entitás) tesz
    // linkelhetővé. A férfi ág viselkedése változatlan (egyetlen kulcs).
    if entity_kind == "team" && is_women_context(ctx) {
        for marker in ["w", "women"] {
            let key = format!("{folded}{marker}");
            for entity in repo::entities_by_normalized_name(pool, entity_kind, &key)
                .await
                .map_err(db)?
            {
                if !entities
                    .iter()
                    .any(|existing| existing.entity_id == entity.entity_id)
                {
                    entities.push(entity);
                }
            }
        }
    }
    let name_hits: Vec<_> = entities
        .iter()
        .filter(|e| context_matches(ctx, &e.context()))
        .collect();
    let auto_link_exact = entity_kind != "referee" || is_full_person_name(proposed_name);
    if name_hits.len() == 1 && auto_link_exact {
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
    } else if let Some(hit) = name_hits.first().filter(|_| !auto_link_exact) {
        // Egzakt nevű, de nem linkelhető (rövid alakú) találat: a fajtaszűrt
        // jelöltlistán review-jelöltként jelenik meg, nem linkel.
        candidates.push(Candidate {
            entity_id: hit.entity_id,
            score: 1.0,
            reason: "fuzzy_candidate",
        });
    }
    for e in repo::entities_of_kind(pool, entity_kind)
        .await
        .map_err(db)?
    {
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

/// Igaz, ha a név TELJES személynévnek tekinthető: legalább két token, és
/// egyik token sem kezdőbetű (1 karakteres). A rövid alak — vezetéknév +
/// kezdőbetű, pl. "M Oliver", "Fyfe, I. M.", "P. A. Durkin" — tehát hamis,
/// csakúgy, mint az egytokenes (monogramm) név: ezeket konzervatívan nem
/// linkeljük automatikusan (AGENTS.md §6).
fn is_full_person_name(name: &str) -> bool {
    let folded = fold_keep_separators(name);
    let tokens: Vec<&str> = folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect();
    tokens.len() >= 2 && tokens.iter().all(|token| token.chars().count() >= 2)
}

/// Igaz, ha a kontextus női meccset/versenyt jelez: `gender = "female"` vagy
/// `branch = "women"` (a séma a `core.team.gender`-t 'male'/'female'/'unknown',
/// a `core.team.branch`-t 'senior'/'reserve'/'youth'/'women' értékkel ismeri).
/// Csak ebben az irányban bővítjük a név-kulcsot jelölővel (lásd a 4. lépés).
fn is_women_context(ctx: &IdentityContext) -> bool {
    ctx.gender.as_deref() == Some("female") || ctx.branch.as_deref() == Some("women")
}

#[cfg(test)]
mod tests {
    use super::{is_full_person_name, is_women_context};
    use football_domain::identity::IdentityContext;

    #[test]
    fn full_names_are_accepted() {
        assert!(is_full_person_name("Felix Zwayer"));
        assert!(is_full_person_name("Félix Zwayer"));
        assert!(is_full_person_name("Dr. Felix Zwayer"));
        assert!(is_full_person_name("van der Berg"));
    }

    #[test]
    fn short_forms_are_rejected() {
        assert!(!is_full_person_name("M. Oliver"));
        assert!(!is_full_person_name("M Oliver"));
        assert!(!is_full_person_name("A Beaton"));
        assert!(!is_full_person_name("Fyfe, I. M."));
        assert!(!is_full_person_name("Rowbotham, J"));
        assert!(!is_full_person_name("Zwayer"));
        assert!(!is_full_person_name(""));
    }

    #[test]
    fn women_context_is_detected_by_gender_or_branch() {
        let by_gender = IdentityContext {
            gender: Some("female".into()),
            ..Default::default()
        };
        let by_branch = IdentityContext {
            branch: Some("women".into()),
            ..Default::default()
        };
        let men = IdentityContext {
            gender: Some("male".into()),
            branch: Some("senior".into()),
            ..Default::default()
        };
        assert!(is_women_context(&by_gender));
        assert!(is_women_context(&by_branch));
        assert!(!is_women_context(&men));
        assert!(!is_women_context(&IdentityContext::default()));
    }
}
