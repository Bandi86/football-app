use football_domain::identity::{
    Candidate, FUZZY_THRESHOLD, IdentityContext, Resolution, ResolveMethod, alias_country_code,
    context_matches, fold_name, normalize_country_code, normalize_name, token_overlap,
};
use uuid::Uuid;

#[test]
fn normalize_keeps_accents_collapses_whitespace() {
    assert_eq!(normalize_name("  Ferencvárosi   TC "), "ferencvárosi tc");
    assert_eq!(normalize_name("Újpest FC"), "újpest fc");
}

#[test]
fn context_filter_blocks_mismatch_allows_unknown() {
    let hu = IdentityContext {
        country_code: Some("HU".into()),
        gender: None,
        age_group: None,
        branch: None,
    };
    let en = IdentityContext {
        country_code: Some("EN".into()),
        gender: None,
        age_group: None,
        branch: None,
    };
    let unknown = IdentityContext {
        country_code: None,
        gender: None,
        age_group: None,
        branch: None,
    };
    assert!(!context_matches(&hu, &en));
    assert!(context_matches(&hu, &unknown));
    assert!(context_matches(&hu, &hu));
    let women = IdentityContext {
        country_code: Some("HU".into()),
        gender: Some("women".into()),
        age_group: None,
        branch: None,
    };
    // spec 3.1: csak akkor blokkol, ha mindkét oldal Some és eltér; (None, Some) → illeszkedik
    assert!(context_matches(&hu, &women));
    let men = IdentityContext {
        country_code: Some("HU".into()),
        gender: Some("men".into()),
        age_group: None,
        branch: None,
    };
    assert!(!context_matches(&men, &women));
}

#[test]
fn token_overlap_jaccard() {
    assert_eq!(
        token_overlap("Manchester United", "Manchester United FC"),
        2.0 / 3.0
    );
    assert_eq!(token_overlap("Újpest", "Ferencváros"), 0.0);
    assert_eq!(token_overlap("", ""), 0.0);
}

#[test]
fn fuzzy_threshold_is_candidate_only() {
    // a küszöb értéke rögzített; a linker soha nem ad Exact-et fuzzy-ból (linker tesztek)
    assert_eq!(FUZZY_THRESHOLD, 0.6);
    let c = Candidate {
        entity_id: Uuid::now_v7(),
        score: 0.9,
        reason: "fuzzy_candidate",
    };
    let r = Resolution::Candidates {
        candidates: vec![c],
        queue_id: Uuid::now_v7(),
    };
    assert!(matches!(r, Resolution::Candidates { .. }));
    assert_eq!(ResolveMethod::ManualReview.as_db_str(), "manual_review");
}

#[test]
fn gb_subdivision_country_codes_are_equivalent() {
    // A v1 import skót/walesi alrégió-kódot ad (GB-SCT/GB-WLS), a
    // footballdata GB-t — azonos csapatnév mellett ez nem lehet két entitás.
    let scot = IdentityContext {
        country_code: Some("GB-SCT".into()),
        gender: Some("male".into()),
        age_group: Some("senior".into()),
        branch: Some("senior".into()),
    };
    let gb = IdentityContext {
        country_code: Some("GB".into()),
        gender: Some("male".into()),
        age_group: Some("senior".into()),
        branch: Some("senior".into()),
    };
    assert!(context_matches(&scot, &gb));
    let wls = IdentityContext {
        country_code: Some("GB-WLS".into()),
        ..gb.clone()
    };
    assert!(context_matches(&wls, &gb));
    // Más ország továbbra sem egyezik.
    let de = IdentityContext {
        country_code: Some("DE".into()),
        ..gb
    };
    assert!(!context_matches(&scot, &de));
}

#[test]
fn fold_name_handles_turkish_diacritics() {
    assert_eq!(fold_name("Beşiktaş"), "besiktas");
    assert_eq!(fold_name("Gençlerbirliği"), "genclerbirligi");
    assert_eq!(fold_name("İstanbulspor"), "istanbulspor");
    assert_eq!(fold_name("Eskişehirspor"), "eskisehirspor");
}

#[test]
fn fold_name_extended_transliteration_and_separator_stripping() {
    // S1: a fold az ékezet-hajtást a teljes karaktertáblával végzi…
    assert_eq!(fold_name("ß"), "s");
    assert_eq!(fold_name("Æsir"), "asir");
    assert_eq!(fold_name("Œuvre"), "ouvre");
    assert_eq!(fold_name("Ðor"), "dor");
    assert_eq!(fold_name("Þór"), "tor");
    assert_eq!(fold_name("ħ"), "h");
    assert_eq!(fold_name("ŧ"), "t");
    assert_eq!(fold_name("Bodø/Glimt"), "bodoglimt");
    assert_eq!(fold_name("ŁKS Łódź"), "lkslodz");
    // …majd elhagyja a szóközt és a leggyakoribb írásjeleket.
    assert_eq!(fold_name("Queen's Park"), "queenspark");
    assert_eq!(fold_name("K. Lierse S.K."), "kliersesk");
    assert_eq!(fold_name("AS FAN"), "asfan");
    assert_eq!(fold_name("  Ferencvárosi   TC "), "ferencvarositc");
    assert_eq!(fold_name("Ostia Mare"), fold_name("Ostiamare"));
    assert_eq!(
        fold_name("Harland & Wolff Welders F.C."),
        "harlandwolffweldersfc"
    );
    // Nem-latin betűket nem dob el (nincs [^a-z0-9] szűrés).
    assert_eq!(fold_name("ЦСКА Москва"), "цскамосква");
    assert_eq!(fold_name("FC東京"), "fc東京");
    // Idempotens: a foldolt kulcs újra foldolva ugyanaz.
    assert_eq!(fold_name(&fold_name("Bodø/Glimt")), "bodoglimt");
}

#[test]
fn neutral_country_codes_act_as_wildcards() {
    let fr = IdentityContext {
        country_code: Some("FR".into()),
        gender: Some("male".into()),
        age_group: Some("senior".into()),
        branch: Some("senior".into()),
    };
    for neutral in ["XX", "INT", "UNK", "ZZ", "", " "] {
        let other = IdentityContext {
            country_code: Some(neutral.into()),
            ..fr.clone()
        };
        assert!(context_matches(&fr, &other), "semleges kód: {neutral:?}");
    }
    // A hiányzó ország eddig is wildcard volt.
    let unknown = IdentityContext {
        country_code: None,
        ..fr.clone()
    };
    assert!(context_matches(&fr, &unknown));
    // Két különböző valódi ország továbbra sem egyezik.
    let de = IdentityContext {
        country_code: Some("DE".into()),
        ..fr.clone()
    };
    assert!(!context_matches(&fr, &de));
}

#[test]
fn alias_country_code_is_canonical_or_null() {
    assert_eq!(alias_country_code(Some("FRA")), Some("FR".into()));
    assert_eq!(alias_country_code(Some("fr")), Some("FR".into()));
    // Semleges/üres kód nem valódi ország → NULL, nem nyers INT/XX.
    assert_eq!(alias_country_code(Some("INT")), None);
    assert_eq!(alias_country_code(Some("XX")), None);
    assert_eq!(alias_country_code(Some("UNK")), None);
    assert_eq!(alias_country_code(Some("ZZ")), None);
    assert_eq!(alias_country_code(Some("")), None);
    assert_eq!(alias_country_code(None), None);
}

#[test]
fn iso3_country_codes_are_equivalent_to_iso2() {
    // S37a: a v1-import ISO3 kódot hoz (GER/ESP/ITA/FRA/HUN), az új források
    // ISO2-t (DE/ES/IT/FR/HU). A core.country mindkettőt tartalmazza, ezért a
    // kontextus-egyeztetés nem bukhat el a kód alakján.
    let pairs = [
        ("GER", "DE"),
        ("ESP", "ES"),
        ("ITA", "IT"),
        ("FRA", "FR"),
        ("HUN", "HU"),
        ("NED", "NL"),
        ("POR", "PT"),
    ];
    for (iso3, iso2) in pairs {
        let a = IdentityContext {
            country_code: Some(iso3.into()),
            gender: Some("male".into()),
            age_group: Some("senior".into()),
            branch: Some("senior".into()),
        };
        let b = IdentityContext {
            country_code: Some(iso2.into()),
            ..a.clone()
        };
        assert!(context_matches(&a, &b), "{iso3} ≠ {iso2}");
        assert_eq!(normalize_country_code(iso3), iso2);
        // kisbetűs/tárolt alak is ugyanaz
        assert_eq!(normalize_country_code(&iso3.to_lowercase()), iso2);
    }
    // Az ENG szándékosan nem GB: külön ország-rekord a v2-ben.
    assert_eq!(normalize_country_code("ENG"), "ENG");
    assert!(!context_matches(
        &IdentityContext {
            country_code: Some("ENG".into()),
            ..IdentityContext::default()
        },
        &IdentityContext {
            country_code: Some("GB".into()),
            ..IdentityContext::default()
        }
    ));
    // Ismeretlen kód változatlan marad.
    assert_eq!(normalize_country_code("zz"), "ZZ");
}

#[test]
fn club_name_variants_are_likely_the_same_team() {
    use football_domain::identity::names_likely_same;
    // Szuffix-különbség.
    assert!(names_likely_same("Újpest", "Újpest FC"));
    assert!(names_likely_same("SV Darmstadt 98", "Darmstadt"));
    // Monogram.
    assert!(names_likely_same("TNS", "The New Saints"));
    // Elgépelés/nyelvi alak.
    assert!(names_likely_same("Dinamo Tbilisi", "Dinamo Tbiliszi"));
    // Néma különbség.
    assert!(names_likely_same("Bodö/Glimt", "Bodø / Glimt"));
    // Negatív esetek: nem szabad összemosni.
    assert!(!names_likely_same("Manchester United", "Manchester City"));
    assert!(!names_likely_same("Újpest", "Győr"));
    assert!(!names_likely_same("FC", "SC"));
    // A meccs-párosítás a csapatnevek teljes párosán megy: a részleges
    // egyezés (csak az egyik oldal) nem elég.
    assert!(!names_likely_same("Real Madrid", "Real Sociedad"));
}

#[test]
fn strict_name_equality_rejects_prefix_lookalikes_but_accepts_real_variants() {
    use football_domain::identity::names_strictly_same;
    // Valódi variánsok (S37c auto-merge).
    assert!(names_strictly_same("SV Darmstadt 98", "Darmstadt"));
    assert!(names_strictly_same("Llandudno FC", "Llandudno"));
    assert!(names_strictly_same(
        "Briton Ferry Llansawel",
        "Briton Ferry"
    ));
    assert!(names_strictly_same("The New Saints", "TNS"));
    assert!(names_strictly_same("Dinamo Tbilisi", "Dinamo Tbiliszi"));
    assert!(names_strictly_same("Germany U17", "Germany"));
    // Nem egyező csapatnevek (a lazább szabály tévesen egyezne).
    assert!(!names_strictly_same("Dominica", "Dominican Republic"));
    assert!(!names_strictly_same("Niger", "Nigeria"));
    assert!(!names_strictly_same("Manchester United", "Manchester City"));
    // Kurált átírás-variánsok (S79): csak a felsorolt tokenek.
    assert!(names_strictly_same("Enköping", "Enkoeping"));
    assert!(names_strictly_same("Göteborg", "Goeteborg"));
    assert!(names_strictly_same("Köln", "Koeln"));
    assert!(!names_strictly_same("Koeman", "Koman"));
    assert!(!names_strictly_same("Újpest", "Győr"));
}
