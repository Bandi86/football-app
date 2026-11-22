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
        observed: "primera division",
        name: "La Liga",
        country: Some(("ES", "Spain")),
        tier: 1,
        priority: 6,
    },
    Entry {
        observed: "la liga",
        name: "La Liga",
        country: Some(("ES", "Spain")),
        tier: 1,
        priority: 6,
    },
    Entry {
        observed: "la liga 2",
        name: "LaLiga 2",
        country: Some(("ES", "Spain")),
        tier: 2,
        priority: 12,
    },
    Entry {
        observed: "laliga 2",
        name: "LaLiga 2",
        country: Some(("ES", "Spain")),
        tier: 2,
        priority: 12,
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

/// S95: foldolt megfigyelt címek, amelyek több országban is ugyanígy
/// szerepelnek. Ezeknél a szótári átnevezés csak akkor fut le, ha a megfigyelt
/// ország hiányzik vagy egyezik a szótár országával — különben a megfigyelt
/// nevet hagyjuk meg, mert az adott ország saját, azonos nevű versenye.
const COUNTRY_SCOPED_OBSERVED_TITLES: &[&str] = &["laliga", "primeradivision"];

/// A megfigyelt verseny-meta kanonizált változata (ismeretlen cím → változatlan).
pub fn canonicalize(competition: &NormalizedCompetition) -> NormalizedCompetition {
    let key = fold_name(&competition.name);
    // S1: a fold_name most a szóközt és az írásjeleket is elhagyja, ezért a
    // szótár korábbi, szóközös `observed` kulcsait is foldolva hasonlítjuk
    // (a szótári literálok így változatlanul olvashatók maradnak).
    let Some(entry) = ENTRIES.iter().find(|e| fold_name(e.observed) == key) else {
        return competition.clone();
    };
    let country_compatible = match entry.country {
        Some((code, name)) => observed_country_compatible(
            &competition.country_code,
            &competition.country_name,
            code,
            name,
        ),
        None => true,
    };
    // S95: az általános, több országban is előforduló címek (homonimák) esetén
    // a szótári átnevezés ország-ütközésnél nem futhat le, különben egy másik
    // ország versenyét nevezné át (pl. venezuelai „Primera División" → spanyol
    // „La Liga"), és a kliens a spanyol bajnokság alá csoportosítaná.
    if !country_compatible && COUNTRY_SCOPED_OBSERVED_TITLES.contains(&key.as_str()) {
        return competition.clone();
    }
    let (country_code, country_name) = match entry.country {
        Some((code, name)) => {
            // S41: a szótári ország csak akkor írja felül a megfigyeltet, ha a
            // megfigyelt ország hiányzik, vagy (kód/név szerint) egyezik.
            // Különben a megfigyelt ország marad (pl. kínai FA Kupa → CN).
            if country_compatible {
                (code.to_string(), name.to_string())
            } else {
                (
                    competition.country_code.clone(),
                    competition.country_name.clone(),
                )
            }
        }
        None => (
            competition.country_code.clone(),
            competition.country_name.clone(),
        ),
    };
    NormalizedCompetition {
        id: competition.id,
        source_entity_id: competition.source_entity_id.clone(),
        name: entry.name.to_string(),
        country_code,
        country_name,
        tier: entry.tier,
        priority: entry.priority,
    }
}

/// A megfigyelt ország elfogadható-e a szótári országgal szemben?
///
/// Elfogadott, ha a megfigyelt ország üres, ha a normalizált kódok egyeznek,
/// vagy ha az ország-**név** ugyanarra az országra oldódik fel (pl. „England” /
/// „Wales” → GB). Ismeretlen, eltérő kódnál a megfigyelt ország marad.
fn observed_country_compatible(
    observed_code: &str,
    observed_name: &str,
    entry_code: &str,
    entry_name: &str,
) -> bool {
    if observed_code.trim().is_empty() && observed_name.trim().is_empty() {
        return true;
    }
    let observed = football_domain::identity::normalize_country_code(observed_code);
    let entry = football_domain::identity::normalize_country_code(entry_code);
    if observed.trim().is_empty() || observed == entry {
        return true;
    }
    if fold_name(observed_name) == fold_name(entry_name) {
        return true;
    }
    let Some((observed_resolved, _)) = resolve_country_name(observed_name) else {
        return false;
    };
    football_domain::identity::normalize_country_code(observed_resolved) == entry
}

/// Ország-név → (ISO, angol név): először az angol, majd a lokalizált tábla.
fn resolve_country_name(name: &str) -> Option<(&'static str, &'static str)> {
    crate::countries::by_english_name(name).or_else(|| crate::countries::by_localized_title(name))
}

/// Ország-specifikus verseny-név felülírás (S42).
///
/// A források néha egy másik ország hazai kupáját/ligáját a maguk általános
/// versenynevével adják meg: a kínai FA Kupát „FA Cup" néven (ami a GB
/// szótárral brit kupára oldódna), a szlovák kupát szintén „FA Cup"/„Cup"
/// alakban, a chilei/bolíviai első osztályt pedig „La Liga"/„Primera División"
/// néven (ami az ES spanyol ligára oldódna). Ilyenkor a megfigyelt nevet a
/// cél-ország specifikus nevére kell cserélni.
///
/// A globális szótár ([`canonicalize`]) változatlanul marad; ez a függvény csak
/// arra az esetre ad választ, amikor az ország önmagában eldönti a nevet.
/// `None` = nincs ország-specifikus szabály, jöhet a szótár.
pub fn country_specific_name(country_code: &str, observed_name: &str) -> Option<&'static str> {
    let country = football_domain::identity::canonical_country_code(country_code);
    if country.is_empty() {
        return None;
    }
    let folded = fold_name(observed_name);
    if folded.is_empty() {
        return None;
    }
    let mentions = |needles: &[&str]| {
        needles
            .iter()
            .any(|needle| folded.contains(&fold_name(needle)))
    };
    let is_cup = mentions(&["cup", "kupa"]);
    match country.as_str() {
        "CN" if is_cup => Some("Chinese FA Cup"),
        "SK" if is_cup => Some("Slovak Cup"),
        "CL" | "BO" if mentions(&["la liga", "primera division"]) => Some("Primera División"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comp(name: &str, country: &str, country_name: &str) -> NormalizedCompetition {
        NormalizedCompetition {
            id: None,
            source_entity_id: None,
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

    #[test]
    fn dictionary_country_does_not_override_a_conflicting_observed_country() {
        // Kínai FA Kupa: a soccerway megfigyelés CN, a szótár GB/England.
        let cn = canonicalize(&comp("FA Cup", "CN", "China"));
        assert_eq!(cn.name, "FA Cup", "a név kanonizálódik");
        assert_eq!(cn.country_code, "CN", "a megfigyelt ország marad");
        assert_eq!(cn.country_name, "China");
        assert_eq!(cn.priority, 13);

        // Bolíviai La Liga: BO marad, nem lesz ES/Spain.
        let bo = canonicalize(&comp("La Liga", "BO", "Bolivia"));
        assert_eq!(bo.name, "La Liga");
        assert_eq!(bo.country_code, "BO");
        assert_eq!(bo.country_name, "Bolivia");

        // Chilei La Liga: CHI megfigyelés nem lesz ES.
        let cl = canonicalize(&comp("La Liga", "CHI", "Chile"));
        assert_eq!(cl.country_code, "CHI");
        assert_eq!(cl.country_name, "Chile");
    }

    #[test]
    fn country_scoped_titles_are_not_renamed_for_other_countries() {
        // S95: a venezuelai Primera División nem a spanyol La Liga nevet kapja,
        // különben a kliens a spanyol bajnokság alá csoportosítaná.
        let ve = canonicalize(&comp("Primera División", "VEN", "Venezuela"));
        assert_eq!(ve.name, "Primera División");
        assert_eq!(ve.country_code, "VEN");
        assert_eq!(ve.priority, 50);

        // Ország nélküli megfigyelésnél marad a szótári kanonizálás.
        let es = canonicalize(&comp("Primera Division", "ES", "Spain"));
        assert_eq!(es.name, "La Liga");
        assert_eq!(es.priority, 6);

        // Costa Rica-i La Liga sem lesz ES.
        let cr = canonicalize(&comp("La Liga", "CR", "Costa Rica"));
        assert_eq!(cr.name, "La Liga");
        assert_eq!(cr.country_code, "CR");
    }

    #[test]
    fn dictionary_country_applies_when_observed_country_matches_or_is_missing() {
        let gb = canonicalize(&comp("FA Cup", "GB", "England"));
        assert_eq!(gb.name, "FA Cup");
        assert_eq!(gb.country_code, "GB");
        assert_eq!(gb.country_name, "England");

        // ISO3 alak ugyanarra az országra oldódik fel (GBR → GB).
        let gbr = canonicalize(&comp("FA Cup", "GBR", "England"));
        assert_eq!(gbr.country_code, "GB");
        assert_eq!(gbr.country_name, "England");

        // Ország-név alapján is elfogadjuk (Wales → GB).
        let wales = canonicalize(&comp("FA Cup", "", "Wales"));
        assert_eq!(wales.country_code, "GB");
        assert_eq!(wales.country_name, "England");

        let es = canonicalize(&comp("La Liga", "ES", "Spain"));
        assert_eq!(es.country_code, "ES");
        assert_eq!(es.country_name, "Spain");

        let missing = canonicalize(&comp("FA Cup", "", ""));
        assert_eq!(missing.country_code, "GB");
        assert_eq!(missing.country_name, "England");

        let missing_es = canonicalize(&comp("La Liga", "", ""));
        assert_eq!(missing_es.country_code, "ES");
        assert_eq!(missing_es.country_name, "Spain");
    }

    #[test]
    fn country_specific_cup_names() {
        // A kínai FA Kupát a források „FA Cup" néven adják: a szótár GB-ra
        // oldaná, az ország-specifikus név viszont CN-re mutat.
        assert_eq!(
            country_specific_name("CN", "FA Cup"),
            Some("Chinese FA Cup")
        );
        assert_eq!(
            country_specific_name("CN", "Chinese FA Cup"),
            Some("Chinese FA Cup")
        );
        assert_eq!(
            country_specific_name("CN", "Kínai Kupa"),
            Some("Chinese FA Cup")
        );
        // Szlovák kupa ugyanígy „FA Cup"/„Cup" alakban.
        assert_eq!(country_specific_name("SK", "FA Cup"), Some("Slovak Cup"));
        assert_eq!(
            country_specific_name("SK", "Slovnaft Cup"),
            Some("Slovak Cup")
        );
        assert_eq!(country_specific_name("SK", "2. liga"), None);
        // Ország-specifikus szabály nélkül a szótár dönt.
        assert_eq!(country_specific_name("GB", "FA Cup"), None);
        assert_eq!(country_specific_name("ES", "La Liga"), None);
        assert_eq!(country_specific_name("", "FA Cup"), None);
        assert_eq!(country_specific_name("CN", ""), None);
    }

    #[test]
    fn country_specific_primera_division_names() {
        for code in ["CL", "CHI"] {
            assert_eq!(
                country_specific_name(code, "La Liga"),
                Some("Primera División")
            );
            assert_eq!(
                country_specific_name(code, "Primera Division"),
                Some("Primera División")
            );
        }
        assert_eq!(
            country_specific_name("BO", "Primera Division"),
            Some("Primera División")
        );
        // Nem első osztály: nincs felülírás.
        assert_eq!(country_specific_name("CL", "Copa Chile"), None);
        assert_eq!(country_specific_name("BO", "Copa Bolivia"), None);
    }
}

#[cfg(test)]
mod spanish_probe {
    use super::*;
    #[test]
    fn la_liga_variants_map() {
        let c = NormalizedCompetition {
            id: None,
            source_entity_id: None,
            name: "La Liga 2".into(),
            country_code: "ES".into(),
            country_name: "Spain".into(),
            tier: 2,
            priority: 12,
        };
        assert_eq!(canonicalize(&c).name, "LaLiga 2");
        let c2 = NormalizedCompetition {
            name: "Primera División".into(),
            ..c.clone()
        };
        assert_eq!(canonicalize(&c2).name, "La Liga");
    }
}
