//! Auditált bíró-alias seeding (S-szelet B), dry-run alapból.
//!
//! Cél: `identity.alias` sorok (entity_kind='referee') kizárólag
//! **evidence-alapú pontos (foldolt) egyezésre egy entitásra**; nincs fuzzy
//! auto-merge, nincs kitalált egyezés; a `fact.match_official` (research
//! megjelenítés) és a `core.match.referee_entity_id` érintetlen.
//!
//! Forrásnév: `fact.match_official.name` (role='referee', source=`research`,
//! trim, nem üres). Foldolt kulcs: `football_domain::identity::fold_name`.
//!
//! Evidence-követelmény (seed csak ha MIND teljesül):
//! 1. EVIDENCE: legalább egy olyan meccs, amelyen ez a (foldolt) név szerepel
//!    ÉS a meccs már `referee_entity_id` = E-re van linkelve (match-szintű,
//!    forrásközi kereszt-evidencia, nem név-alapú találgatás);
//! 2. EGYEDISÉG: a kulcshoz tartozó evidence-entitás halmaza pontosan {E}
//!    (egy entitás; >1 -> ambiguous, nem seed);
//! 3. NINCS KONFLIKTUS: nincs MÁS (nem merged) referee-entitás, amelynek
//!    canonical foldolt neve VAGY reviewed aliasa már = k; és E canonical
//!    foldolt neve sem = k (akkor a linker már egzakt-linkelné; nincs mit seed);
//! 4. nincs duplikált meglévő alias ugyanerre az entitásra (ON CONFLICT
//!    DO NOTHING + megelőző szűrés).
//!
//! Nem-megfelelő eset: ambiguous (evidence-set >1 / konfliktus) ->
//! **pending review** (multiple_candidates), no-match -> **pending review**
//! (no_exact_match); a meccs `referee_entity_id` marad NULL; research megjelenítés
//! változatlan. `--apply`-nál reviewed alias + pending review beszúrás;
//! visszaállás: a beszúrt alias/review sorok törlése (semmi mást nem ír).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{Context, Result};
use football_domain::identity::fold_name;
use sqlx::PgPool;
use uuid::Uuid;

const SOURCE_CODE: &str = "research";
const REVIEWED_BY: &str = "footballctl referee-alias-seed";
const ALIAS_LANG: &str = "und";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RefereeAliasReport {
    pub applied: bool,
    /// Fordított forrásnevek (foldolt kulcs).
    pub keys_total: usize,
    /// Bizonyíték (match-level link) nélküli kulcs.
    pub no_match: usize,
    /// Ambiguous kulcs (evidence-set >1 vagy konfliktus).
    pub ambiguous: usize,
    /// Már egzakt-linkelhető (nincs mit seed).
    pub already_exact: usize,
    /// Jelölt alias seed (evidence + egyediség + nincs konfliktus).
    pub seedable: usize,
    /// Ténylegesen beszúrt reviewed alias (apply).
    pub inserted_aliases: usize,
    /// Beszúrt pending review sor (apply).
    pub inserted_reviews: usize,
}

/// Osztályozott kulcs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyVerdict {
    /// Kitalált/bizonyíték nélküli.
    NoMatch,
    /// >1 entitás vagy konfliktus.
    Ambiguous { candidates: Vec<Uuid> },
    /// Már egzakt-linkelhető, nincs mit seed.
    AlreadyExact { entity_id: Uuid },
    /// Evidence + egyediség + nincs konfliktus -> seed.
    Seedable { entity_id: Uuid },
}

/// Tiszta osztályozás egy kulcshoz. `evidence` = entitások, amikre a
/// match-level evidence mutat; `conflicting` = más entitásra foldolt match;
/// `target_already_matchable` = a cél-entitás már egzakt-linkelhető erre a
/// kulcsra (canonical fold == k VAGY reviewed alias == k); `conflicting_other`
/// = más entitás már erre a kulcsra foldol.
pub fn classify_key(
    key: &str,
    evidence_entities: &BTreeSet<Uuid>,
    conflicting_other_entities: &BTreeSet<Uuid>,
    target_already_matchable: bool,
) -> KeyVerdict {
    if key.is_empty() || evidence_entities.is_empty() {
        return KeyVerdict::NoMatch;
    }
    // konfliktus: más entitás is már erre a kulcsra foldol
    if !conflicting_other_entities.is_empty() {
        let mut all: Vec<Uuid> = evidence_entities
            .iter()
            .chain(conflicting_other_entities.iter())
            .copied()
            .collect();
        all.sort();
        all.dedup();
        return KeyVerdict::Ambiguous { candidates: all };
    }
    if evidence_entities.len() != 1 {
        return KeyVerdict::Ambiguous {
            candidates: evidence_entities.iter().copied().collect(),
        };
    }
    let entity_id = *evidence_entities.iter().next().expect("len==1");
    if target_already_matchable {
        return KeyVerdict::AlreadyExact { entity_id };
    }
    KeyVerdict::Seedable { entity_id }
}

struct EntityRow {
    id: Uuid,
    canonical_fold: String,
}

async fn research_source_id(pool: &PgPool) -> Result<Uuid> {
    sqlx::query_scalar("SELECT id FROM ops.source WHERE code=$1")
        .bind(SOURCE_CODE)
        .fetch_one(pool)
        .await
        .context("research forrás hiányzik")
}

fn target_already_matchable(
    target: Uuid,
    key: &str,
    canonical_index: &HashMap<String, BTreeSet<Uuid>>,
    alias_index: &HashMap<String, BTreeSet<Uuid>>,
) -> bool {
    canonical_index
        .get(key)
        .map(|s| s.contains(&target))
        .unwrap_or(false)
        || alias_index
            .get(key)
            .map(|s| s.contains(&target))
            .unwrap_or(false)
}

pub async fn run(pool: &PgPool, apply: bool) -> Result<RefereeAliasReport> {
    let source_id = research_source_id(pool).await?;

    // referee katalógus (nem merged)
    let entities: Vec<EntityRow> = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, identity.fold_name(canonical_name)
         FROM core.entity
         WHERE kind='referee' AND merged_into_id IS NULL",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(id, canonical_fold)| EntityRow { id, canonical_fold })
    .collect();
    let canonical_index: HashMap<String, BTreeSet<Uuid>> = {
        let mut m: HashMap<String, BTreeSet<Uuid>> = HashMap::new();
        for e in &entities {
            if !e.canonical_fold.is_empty() {
                m.entry(e.canonical_fold.clone()).or_default().insert(e.id);
            }
        }
        m
    };

    // reviewed alias -> entitás(ek), foldolt
    let alias_rows: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT identity.fold_name(a.alias), a.entity_id
         FROM identity.alias a JOIN core.entity e ON e.id=a.entity_id
         WHERE e.kind='referee' AND e.merged_into_id IS NULL AND a.status='reviewed'",
    )
    .fetch_all(pool)
    .await?;
    let mut alias_index: HashMap<String, BTreeSet<Uuid>> = HashMap::new();
    for (k, id) in alias_rows {
        if !k.is_empty() {
            alias_index.entry(k).or_default().insert(id);
        }
    }

    // research referee nevek + match-level evidence
    let rows: Vec<(String, Option<Uuid>)> = sqlx::query_as(
        "SELECT r.name, m.referee_entity_id
         FROM fact.match_official r JOIN core.match m ON m.id=r.match_id
         WHERE r.role='referee' AND r.source_id=$1 AND btrim(r.name) <> ''",
    )
    .bind(source_id)
    .fetch_all(pool)
    .await?;

    // kulcs -> reprezentatív név (trim) + evidence-entitások
    let mut rep_name: BTreeMap<String, String> = BTreeMap::new();
    let mut evidence: BTreeMap<String, BTreeSet<Uuid>> = BTreeMap::new();
    for (name, eid) in rows {
        let trimmed = name.trim().to_string();
        let key = fold_name(&trimmed);
        if key.is_empty() {
            continue;
        }
        rep_name
            .entry(key.clone())
            .or_insert_with(|| trimmed.clone());
        if let Some(eid) = eid {
            // csak nem-merged referee-entitásra
            if entities.iter().any(|e| e.id == eid) {
                evidence.entry(key.clone()).or_default().insert(eid);
            }
        }
    }

    // az összes kulcs (a no-match is látszódjon)
    let mut keys: BTreeSet<String> = evidence.keys().cloned().collect();
    keys.extend(rep_name.keys().cloned());

    let mut report = RefereeAliasReport {
        applied: apply,
        keys_total: keys.len(),
        ..Default::default()
    };

    let mut seeds: Vec<(Uuid, String)> = Vec::new();
    let mut pending: Vec<(String, &'static str, Vec<Uuid>, String)> = Vec::new();

    for key in &keys {
        let ev = evidence.get(key).cloned().unwrap_or_default();
        // konfliktus: más entitás már erre a kulcsra foldol (canonical/alias)
        let mut conflicting: BTreeSet<Uuid> = BTreeSet::new();
        if let Some(set) = canonical_index.get(key) {
            conflicting.extend(set.iter().copied());
        }
        if let Some(set) = alias_index.get(key) {
            conflicting.extend(set.iter().copied());
        }
        // a cél-entitás canonical/alias == key ne számítson konfliktusnak
        // önmagával
        let target = ev.iter().next().copied();
        if let Some(t) = target {
            conflicting.remove(&t);
        }
        let proposed_name = rep_name.get(key).cloned().unwrap_or_default();
        let target_already_matchable = target
            .map(|t| target_already_matchable(t, key, &canonical_index, &alias_index))
            .unwrap_or(false);

        let verdict = classify_key(key, &ev, &conflicting, target_already_matchable);
        match verdict {
            KeyVerdict::NoMatch => {
                report.no_match += 1;
                pending.push((key.clone(), "no_exact_match", Vec::new(), proposed_name));
            }
            KeyVerdict::Ambiguous { candidates } => {
                report.ambiguous += 1;
                pending.push((
                    key.clone(),
                    "multiple_candidates",
                    candidates,
                    proposed_name,
                ));
            }
            KeyVerdict::AlreadyExact { .. } => {
                report.already_exact += 1;
            }
            KeyVerdict::Seedable { entity_id } => {
                report.seedable += 1;
                seeds.push((entity_id, proposed_name.clone()));
            }
        }
    }

    if apply {
        let mut tx = pool.begin().await?;
        for (entity_id, alias) in &seeds {
            let res = sqlx::query(
                "INSERT INTO identity.alias
                   (id, entity_id, alias, lang, status, method, reviewed_by, reviewed_at)
                 VALUES ($1,$2,$3,$4,'reviewed','import',$5,now())
                 ON CONFLICT DO NOTHING",
            )
            .bind(Uuid::now_v7())
            .bind(entity_id)
            .bind(alias)
            .bind(ALIAS_LANG)
            .bind(REVIEWED_BY)
            .execute(&mut *tx)
            .await?;
            report.inserted_aliases += res.rows_affected() as usize;
        }
        for (key, reason, candidates, proposed_name) in &pending {
            let res = sqlx::query(
                "INSERT INTO identity.review_queue
                   (id, source_id, entity_kind, source_entity_id, candidate_entity_ids,
                    reason, proposed_name, status)
                 VALUES ($1,$2,'referee',$3,$4::jsonb,$5,$6,'pending')
                 ON CONFLICT DO NOTHING",
            )
            .bind(Uuid::now_v7())
            .bind(source_id)
            .bind(key)
            .bind(serde_json::to_string(candidates)?)
            .bind(reason)
            .bind(proposed_name)
            .execute(&mut *tx)
            .await?;
            report.inserted_reviews += res.rows_affected() as usize;
        }
        tx.commit().await?;
    }

    Ok(report)
}

// ---- Tesztek ---------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u8) -> Uuid {
        Uuid::from_bytes([n; 16])
    }

    fn set(items: &[Uuid]) -> BTreeSet<Uuid> {
        items.iter().copied().collect()
    }

    #[test]
    fn no_match_when_no_evidence_or_empty_key() {
        assert_eq!(
            classify_key("", &set(&[id(1)]), &set(&[]), false),
            KeyVerdict::NoMatch
        );
        assert_eq!(
            classify_key("x", &set(&[]), &set(&[]), false),
            KeyVerdict::NoMatch
        );
    }

    #[test]
    fn seedable_requires_single_entity_no_conflict() {
        assert_eq!(
            classify_key("k", &set(&[id(1)]), &set(&[]), false),
            KeyVerdict::Seedable { entity_id: id(1) }
        );
    }

    #[test]
    fn ambiguous_when_multiple_evidence_entities() {
        match classify_key("k", &set(&[id(1), id(2)]), &set(&[]), false) {
            KeyVerdict::Ambiguous { candidates } => assert_eq!(candidates.len(), 2),
            other => panic!("ambiguous várt, kapott {other:?}"),
        }
    }

    #[test]
    fn ambiguous_when_conflicting_other_entity() {
        // pontosan egy evidence, DE más entitás is már erre a kulcsra foldol
        match classify_key("k", &set(&[id(1)]), &set(&[id(2)]), false) {
            KeyVerdict::Ambiguous { candidates } => {
                assert!(candidates.contains(&id(1)) && candidates.contains(&id(2)));
            }
            other => panic!("ambiguous várt, kapott {other:?}"),
        }
    }

    #[test]
    fn already_exact_when_canonical_already_folds_to_key() {
        assert_eq!(
            classify_key("k", &set(&[id(1)]), &set(&[]), true),
            KeyVerdict::AlreadyExact { entity_id: id(1) }
        );
    }
}
