use football_domain::identity::{
    Candidate, FUZZY_THRESHOLD, IdentityContext, Resolution, ResolveMethod, context_matches,
    normalize_name, token_overlap,
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
