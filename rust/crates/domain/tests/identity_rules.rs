use football_domain::identity::{
    Candidate, FUZZY_THRESHOLD, IdentityContext, Resolution, ResolveMethod, context_matches,
    fold_name, normalize_name, token_overlap,
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
