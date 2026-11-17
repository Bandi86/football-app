//! Verseny-név kanonizálás (S21) — fix szótár, naming-stability szabály.
//!
//! A források (főleg a Soccerway lokalizált renderjei) ugyanarra a versenyre
//! több címet is adhatnak (HU/EN, „Play Offs"/„Playoffs", ország-suffix).
//! A verseny-entitás neve része a natural key-nek, ezért minden változat új
//! entitást és duplikált meccseket szülne. Itt egy **fix tábla** képezi a
//! megfigyelt címeket a dev-ben már meccset tartó kanonikus névre.
//!
//! Szabályok:
//! - a kulcs ékezet/kisbetű-független (`fold_name`), a címeket fordítás után
//!   is felismeri (pl. „Kings Cup - Thaiföld" → „Kings Cup (Thailand)");
//! - ismeretlen cím változatlanul megy tovább (nincs találgatás/fuzzy);
//! - új bejegyzést csak tesztelt, szándékos adatváltozásként adunk hozzá.

use football_domain::identity::fold_name;

use crate::normalized::NormalizedCompetition;

#[derive(Debug, Clone, Copy)]
struct Entry {
    /// Megfigyelt cím foldolt alakja.
    observed: &'static str,
    name: &'static str,
    country: Option<(&'static str, &'static str)>,
    tier: i16,
    priority: i32,
}

const ENTRIES: &[Entry] = &[
    // Nemzetközi selejtezők / kupák angol + magyar alakjai.
    Entry {
        observed: "afc asian cup u17 - qualification",
        name: "AFC Asian Cup U17 - Qualification",
        country: None,
        tier: 0,
        priority: 70,
    },
    Entry {
        observed: "afc u17 asian cup u17 - qualification",
        name: "AFC Asian Cup U17 - Qualification",
        country: None,
        tier: 0,
        priority: 70,
    },
    Entry {
        observed: "afc azsia-kupa u17 - selejtezo",
        name: "AFC Asian Cup U17 - Qualification",
        country: None,
        tier: 0,
        priority: 70,
    },
    Entry {
        observed: "africa cup of nations - qualification",
        name: "Africa Cup of Nations - Qualification",
        country: None,
        tier: 0,
        priority: 40,
    },
    Entry {
        observed: "afrikai nemzetek kupaja - selejtezo",
        name: "Africa Cup of Nations - Qualification",
        country: None,
        tier: 0,
        priority: 40,
    },
    Entry {
        observed: "africa cup of nations u23 - qualification",
        name: "Africa Cup of Nations U23 - Qualification",
        country: None,
        tier: 0,
        priority: 70,
    },
    Entry {
        observed: "africa cup of nations u23 - qualification - qualification",
        name: "Africa Cup of Nations U23 - Qualification",
        country: None,
        tier: 0,
        priority: 70,
    },
    Entry {
        observed: "afrikai nemzetek kupaja u23 - selejtezo - selejtezo",
        name: "Africa Cup of Nations U23 - Qualification",
        country: None,
        tier: 0,
        priority: 70,
    },
    Entry {
        observed: "concacaf nations league - league a - play offs",
        name: "CONCACAF Nations League - League A - Play Offs",
        country: None,
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "concacaf nations league - league a - playoffs",
        name: "CONCACAF Nations League - League A - Play Offs",
        country: None,
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "concacaf nemzetek ligaja - a-liga - play offs",
        name: "CONCACAF Nations League - League A - Play Offs",
        country: None,
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "concacaf nations league - play-in",
        name: "CONCACAF Nations League - Play-in",
        country: None,
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "concacaf nations league - league b",
        name: "CONCACAF Nations League - League B",
        country: None,
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "concacaf nemzetek ligaja - b-liga",
        name: "CONCACAF Nations League - League B",
        country: None,
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "uefa nations league - league a",
        name: "UEFA Nations League - League A",
        country: None,
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "uefa nations league - league b",
        name: "UEFA Nations League - League B",
        country: None,
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "nemzetek ligaja - a-liga",
        name: "UEFA Nations League - League A",
        country: None,
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "nemzetek ligaja - b-liga",
        name: "UEFA Nations League - League B",
        country: None,
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "euro u17 - qualification - first stage",
        name: "Euro U17 - Qualification - First stage",
        country: None,
        tier: 1,
        priority: 70,
    },
    Entry {
        observed: "u17-es eb - selejtezo - elso szakasz",
        name: "Euro U17 - Qualification - First stage",
        country: None,
        tier: 1,
        priority: 70,
    },
    Entry {
        observed: "euro u19 - qualification - round 2 - league a",
        name: "Euro U19 - Qualification - Round 2 - League A",
        country: None,
        tier: 1,
        priority: 70,
    },
    Entry {
        observed: "u19-es eb - selejtezo - 2. fordulo - a-liga",
        name: "Euro U19 - Qualification - Round 2 - League A",
        country: None,
        tier: 1,
        priority: 70,
    },
    Entry {
        observed: "euro u19 - qualification - round 2 - league b",
        name: "Euro U19 - Qualification - Round 2 - League B",
        country: None,
        tier: 1,
        priority: 70,
    },
    Entry {
        observed: "u19-es eb - selejtezo - 2. fordulo - b-liga",
        name: "Euro U19 - Qualification - Round 2 - League B",
        country: None,
        tier: 1,
        priority: 70,
    },
    Entry {
        observed: "euro u21 - qualification - promotion",
        name: "Euro U21 - Qualification - Promotion",
        country: None,
        tier: 1,
        priority: 70,
    },
    Entry {
        observed: "euro u21 - qualification - play-offs",
        name: "Euro U21 - Qualification - Promotion",
        country: None,
        tier: 1,
        priority: 70,
    },
    Entry {
        observed: "u21-es eb - selejtezo - osztalyozo",
        name: "Euro U21 - Qualification - Promotion",
        country: None,
        tier: 1,
        priority: 70,
    },
    // Hazai kupák / ligák helyi és angol alakjai.
    Entry {
        observed: "kings cup (thailand)",
        name: "Kings Cup (Thailand)",
        country: None,
        tier: 0,
        priority: 40,
    },
    Entry {
        observed: "kings cup - thaifold",
        name: "Kings Cup (Thailand)",
        country: None,
        tier: 0,
        priority: 40,
    },
    Entry {
        observed: "copa colombia - second stage",
        name: "Copa Colombia - Second stage",
        country: Some(("CO", "Colombia")),
        tier: 0,
        priority: 40,
    },
    Entry {
        observed: "copa colombia - masodik szakasz",
        name: "Copa Colombia - Second stage",
        country: Some(("CO", "Colombia")),
        tier: 0,
        priority: 40,
    },
    Entry {
        observed: "brazil kupa u20",
        name: "Brazil Kupa U20",
        country: Some(("BR", "Brazil")),
        tier: 0,
        priority: 70,
    },
    Entry {
        observed: "copa do brasil u20",
        name: "Brazil Kupa U20",
        country: Some(("BR", "Brazil")),
        tier: 0,
        priority: 70,
    },
    Entry {
        observed: "saff championship - play off",
        name: "SAFF Championship - Play Off",
        country: None,
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "saff championship - play offs",
        name: "SAFF Championship - Play Off",
        country: None,
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "usl championship - play offs",
        name: "USL Championship - Play Offs",
        country: Some(("US", "USA")),
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "usl championship - play off",
        name: "USL Championship - Play Offs",
        country: Some(("US", "USA")),
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "torneo federal - promotion - play offs",
        name: "Torneo Federal - Promotion - Play Offs",
        country: Some(("AR", "Argentina")),
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "torneo federal - osztalyozo - play offs",
        name: "Torneo Federal - Promotion - Play Offs",
        country: Some(("AR", "Argentina")),
        tier: 1,
        priority: 50,
    },
    Entry {
        observed: "liga mx women - apertura - play offs",
        name: "Liga MX Women - Apertura - Play Offs",
        country: Some(("MX", "Mexico")),
        tier: 1,
        priority: 60,
    },
    Entry {
        observed: "iwl women",
        name: "IWL Women",
        country: Some(("IN", "India")),
        tier: 1,
        priority: 60,
    },
    Entry {
        observed: "iwl - noi",
        name: "IWL Women",
        country: Some(("IN", "India")),
        tier: 1,
        priority: 60,
    },
    Entry {
        observed: "fa cup",
        name: "FA Cup",
        country: Some(("GB", "England")),
        tier: 0,
        priority: 13,
    },
    Entry {
        observed: "fa kupa",
        name: "FA Cup",
        country: Some(("GB", "England")),
        tier: 0,
        priority: 13,
    },
];

/// A megfigyelt verseny-meta kanonizált változata (ismeretlen cím → változatlan).
pub fn canonicalize(competition: &NormalizedCompetition) -> NormalizedCompetition {
    let key = fold_name(&competition.name);
    let Some(entry) = ENTRIES.iter().find(|e| e.observed == key) else {
        return competition.clone();
    };
    let (country_code, country_name) = match entry.country {
        Some((code, name)) => (code.to_string(), name.to_string()),
        None => (
            competition.country_code.clone(),
            competition.country_name.clone(),
        ),
    };
    NormalizedCompetition {
        id: competition.id,
        name: entry.name.to_string(),
        country_code,
        country_name,
        tier: entry.tier,
        priority: entry.priority,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comp(name: &str, country: &str, country_name: &str) -> NormalizedCompetition {
        NormalizedCompetition {
            id: None,
            name: name.into(),
            country_code: country.into(),
            country_name: country_name.into(),
            tier: 1,
            priority: 50,
        }
    }

    #[test]
    fn localised_titles_map_to_canonical_english() {
        let hu = comp("Kings Cup - Thaiföld", "", "");
        let out = canonicalize(&hu);
        assert_eq!(out.name, "Kings Cup (Thailand)");
        assert_eq!(out.tier, 0);
        assert_eq!(out.priority, 40);

        let hu = comp("Afrikai Nemzetek Kupája - Selejtező", "", "");
        assert_eq!(
            canonicalize(&hu).name,
            "Africa Cup of Nations - Qualification"
        );

        let hu = comp("U19-es EB - Selejtező - 2. forduló - B-Liga", "", "");
        assert_eq!(
            canonicalize(&hu).name,
            "Euro U19 - Qualification - Round 2 - League B"
        );
    }

    #[test]
    fn country_is_filled_from_the_dictionary() {
        let no_country = comp("Copa Colombia - Második szakasz", "", "");
        let out = canonicalize(&no_country);
        assert_eq!(out.name, "Copa Colombia - Second stage");
        assert_eq!(out.country_code, "CO");
        assert_eq!(out.country_name, "Colombia");
    }

    #[test]
    fn english_variants_merge_into_one_name() {
        let a = canonicalize(&comp(
            "CONCACAF Nations League - League A - Playoffs",
            "",
            "",
        ));
        let b = canonicalize(&comp(
            "CONCACAF Nations League - League A - Play Offs",
            "",
            "",
        ));
        assert_eq!(a.name, b.name);
        let a = canonicalize(&comp("SAFF Championship - Play Offs", "", ""));
        let b = canonicalize(&comp("SAFF Championship - Play Off", "", ""));
        assert_eq!(a.name, b.name);
    }

    #[test]
    fn unknown_titles_pass_through_unchanged() {
        let unknown = comp("Vysshaya Liga", "TJ", "Tajikistan");
        let out = canonicalize(&unknown);
        assert_eq!(out.name, unknown.name);
        assert_eq!(out.country_code, "TJ");
        assert_eq!(out.tier, 1);
        assert_eq!(out.priority, 50);
    }
}
