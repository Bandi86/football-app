use football_domain::trust::{
    ODDS_TOLERANCE_DEFAULT, OddsValues, SourceOdds, TrustClass, group_by_provider,
};

fn odds(home: f64) -> OddsValues {
    OddsValues {
        home,
        draw: 3.4,
        away: 4.2,
        btts_yes: 1.72,
        btts_no: 2.05,
        over25: 2.1,
        under25: 1.75,
    }
}

#[test]
fn trust_class_db_strings() {
    assert_eq!(TrustClass::High.as_db_str(), "high");
    assert_eq!(TrustClass::Medium.as_db_str(), "medium");
    assert_eq!(TrustClass::DisplayOnly.as_db_str(), "display_only");
    assert_eq!(TrustClass::from_db_str("high").unwrap(), TrustClass::High);
    assert!(TrustClass::from_db_str("unknown").is_err());
}

#[test]
fn odds_matching_tolerance_boundary() {
    let a = odds(1.85);
    assert!(a.matches_within(&odds(1.85), 0.0));
    assert!(!a.matches_within(&odds(1.86), 0.0));
    assert!(a.matches_within(&odds(1.86), 0.01)); // pontosan a tolerance
    assert!(!a.matches_within(&odds(1.87), 0.01));
    assert_eq!(ODDS_TOLERANCE_DEFAULT, 0.0);
}

#[test]
fn provider_grouping_null_is_independent() {
    let items = vec![
        SourceOdds {
            source_code: "a".into(),
            provider: None,
            odds: odds(1.0),
        },
        SourceOdds {
            source_code: "b".into(),
            provider: None,
            odds: odds(1.0),
        },
        SourceOdds {
            source_code: "c".into(),
            provider: Some("p".into()),
            odds: odds(1.0),
        },
        SourceOdds {
            source_code: "d".into(),
            provider: Some("p".into()),
            odds: odds(1.0),
        },
    ];
    let groups = group_by_provider(&items, |s| s.provider.as_deref());
    assert_eq!(groups.len(), 3); // a, b, {c,d}
    assert_eq!(groups[2].len(), 2);
}
