//! v1 crosswalk-seed: a v1 adatbázis csapat-nevei/aliasai → v2 reviewed
//! aliasok. Tiszta tervező: a v2 entitás-listát és a v1 neveket kapja, a
//! DB-írást a storage repo végzi.
//!
//! Illesztés: ékezet-független név-egyezés + kontextus (ország, nem), és
//! csak **egyértelmű** találat seedel (több jelölt → kihagyás, nem tippelés).
//! A linker kulcsa a lower(alias), ezért az ékezetes nevet a foldolt alakkal
//! együtt tároljuk (pl. „Ferencváros" + „ferencvaros").

use std::collections::{HashMap, HashSet};

use football_domain::identity::{AliasSeed, fold_name};
use football_storage::repositories::identity::EntityRow;

/// Egy v1 csapat a hozzá tartozó nevekkel (név + aliasok, nyelvvel).
#[derive(Debug, Clone)]
pub struct V1Team {
    pub names: Vec<(String, String)>,
    pub country_code: Option<String>,
    pub gender: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SeedStats {
    pub v1_teams: usize,
    pub matched: usize,
    pub ambiguous: usize,
    pub unmatched: usize,
    pub seeded_names: usize,
}

fn context_ok(v1: &V1Team, v2: &EntityRow) -> bool {
    let country_ok = match (&v1.country_code, &v2.country_code) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => true,
    };
    let gender_ok = match (&v1.gender, &v2.gender) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => true,
    };
    country_ok && gender_ok
}

/// v2 entitások és v1 csapatok → reviewed alias-seedek + statisztika.
/// Csak egyértelmű (pontosan egy) kontextus-egyező v2 entitásnál seedel.
pub fn plan_seeds(v2: &[EntityRow], v1: &[V1Team]) -> (Vec<AliasSeed>, SeedStats) {
    let mut index: HashMap<String, Vec<&EntityRow>> = HashMap::new();
    for row in v2 {
        index
            .entry(fold_name(&row.canonical_name))
            .or_default()
            .push(row);
    }

    let mut stats = SeedStats {
        v1_teams: v1.len(),
        ..Default::default()
    };
    let mut seeds: Vec<AliasSeed> = Vec::new();
    let mut seen: HashSet<(uuid::Uuid, String, String)> = HashSet::new();

    for team in v1 {
        let mut candidate_ids: HashSet<uuid::Uuid> = HashSet::new();
        let mut candidates: Vec<&EntityRow> = Vec::new();
        for (name, _) in &team.names {
            if let Some(rows) = index.get(&fold_name(name)) {
                for row in rows {
                    if context_ok(team, row) && candidate_ids.insert(row.entity_id) {
                        candidates.push(row);
                    }
                }
            }
        }
        match candidates.len() {
            0 => {
                stats.unmatched += 1;
                continue;
            }
            1 => {}
            _ => {
                stats.ambiguous += 1;
                continue;
            }
        }
        let target = candidates[0];
        stats.matched += 1;
        for (name, lang) in &team.names {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                continue;
            }
            let variants = [trimmed.to_string(), fold_name(trimmed)];
            for alias in variants {
                if !seen.insert((target.entity_id, alias.clone(), lang.clone())) {
                    continue;
                }
                seeds.push(AliasSeed {
                    entity_id: target.entity_id,
                    alias,
                    lang: lang.clone(),
                    country_code: target.country_code.clone(),
                    gender: target.gender.clone(),
                    age_group: target.age_group.clone(),
                    branch: target.branch.clone(),
                });
                stats.seeded_names += 1;
            }
        }
    }
    (seeds, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn entity(name: &str, country: Option<&str>, gender: Option<&str>) -> EntityRow {
        EntityRow {
            entity_id: Uuid::now_v7(),
            canonical_name: name.into(),
            country_code: country.map(String::from),
            gender: gender.map(String::from),
            age_group: Some("senior".into()),
            branch: Some("senior".into()),
        }
    }

    fn v1(names: &[(&str, &str)], country: Option<&str>, gender: Option<&str>) -> V1Team {
        V1Team {
            names: names
                .iter()
                .map(|(n, l)| (n.to_string(), l.to_string()))
                .collect(),
            country_code: country.map(String::from),
            gender: gender.map(String::from),
        }
    }

    #[test]
    fn seeds_accented_and_folded_aliases_for_unique_match() {
        let v2 = vec![entity("Ferencvaros", Some("HU"), Some("male"))];
        let teams = vec![v1(
            &[("Ferencváros", "hu"), ("Ferencvárosi TC", "hu")],
            Some("HU"),
            Some("male"),
        )];
        let (seeds, stats) = plan_seeds(&v2, &teams);
        assert_eq!(stats.matched, 1);
        let aliases: HashSet<&str> = seeds.iter().map(|s| s.alias.as_str()).collect();
        assert!(aliases.contains("Ferencváros"));
        assert!(aliases.contains("ferencvaros"));
        assert!(aliases.contains("Ferencvárosi TC"));
        assert!(aliases.contains("ferencvarosi tc"));
        assert_eq!(seeds[0].lang, "hu");
        assert_eq!(seeds[0].country_code.as_deref(), Some("HU"));
    }

    #[test]
    fn skips_ambiguous_and_unmatched() {
        let v2 = vec![
            entity("Arsenal", Some("GB"), Some("male")),
            entity("Arsenal", Some("UA"), Some("male")),
            entity("Lyon", Some("FR"), Some("male")),
        ];
        let teams = vec![
            // ország nélkül mindkét Arsenal jelölt → kihagyás
            v1(&[("Arsenal", "en")], None, Some("male")),
            // ország-egyezéssel egyértelmű
            v1(&[("Lyon", "en")], Some("FR"), Some("male")),
            v1(&[("Ismeretlen FC", "en")], None, None),
        ];
        let (seeds, stats) = plan_seeds(&v2, &teams);
        assert_eq!(stats.ambiguous, 1);
        assert_eq!(stats.matched, 1);
        assert_eq!(stats.unmatched, 1);
        assert!(seeds.iter().all(|s| s.alias != "Arsenal"));
        assert!(seeds.iter().any(|s| s.alias == "Lyon"));
    }

    #[test]
    fn gender_context_blocks_cross_gender_match() {
        let v2 = vec![entity("Ferencvaros", Some("HU"), Some("female"))];
        let teams = vec![v1(&[("Ferencváros", "hu")], Some("HU"), Some("male"))];
        let (seeds, stats) = plan_seeds(&v2, &teams);
        assert!(seeds.is_empty());
        assert_eq!(stats.unmatched, 1);
    }
}
