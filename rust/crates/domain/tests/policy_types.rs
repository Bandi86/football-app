use football_domain::policy::{
    BreakerState, CoverageState, EventTemplate, FetchOutcome, JobStatus, SourceMode, SourcePolicy,
    Transport,
};

fn demo_policy() -> SourcePolicy {
    SourcePolicy {
        source_slug: "demo-synthetic".into(),
        transport: Transport::Fixture,
        mode: SourceMode::Shadow,
        daily_budget: 10,
        min_delay_ms: 2000,
        jitter_ms: 500,
        breaker_threshold: 3,
        breaker_cooldown_secs: 300,
        adapter_version: "1.0".into(),
        parser_version: "1.0".into(),
        domains: vec!["match".into()],
        events: vec![EventTemplate {
            event_type: "discovery".into(),
            offset_minutes: None,
            interval_minutes: Some(1440),
        }],
        enabled: true,
        max_bytes: 5_242_880,
    }
}

#[test]
fn policy_serde_roundtrip() {
    let policy = demo_policy();
    let json = serde_json::to_string(&policy).unwrap();
    let back: SourcePolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(back.source_slug, "demo-synthetic");
    assert_eq!(back.events[0].interval_minutes, Some(1440));
}

#[test]
fn db_string_roundtrip() {
    macro_rules! check_roundtrip {
        ($t:ty, $($variant:ident => $s:literal),+ $(,)?) => {
            $(
                assert_eq!(<$t>::$variant.as_db_str(), $s);
                assert_eq!(<$t>::from_db_str($s).unwrap(), <$t>::$variant);
            )+
        };
    }

    check_roundtrip!(Transport, RustHttp => "rust_http", Fixture => "fixture");
    check_roundtrip!(SourceMode, Shadow => "shadow", Canary => "canary", Live => "live");
    check_roundtrip!(BreakerState, Closed => "closed", Open => "open", HalfOpen => "half_open");
    check_roundtrip!(
        CoverageState,
        Pending => "pending",
        Fetching => "fetching",
        Done => "done",
        NoData => "no_data",
        Blocked => "blocked",
        Failed => "failed",
        Stale => "stale",
        Quarantined => "quarantined",
        Skipped => "skipped"
    );
    check_roundtrip!(
        FetchOutcome,
        Success => "success",
        HttpError => "http_error",
        Timeout => "timeout",
        Oversize => "oversize",
        RobotsBlocked => "robots_blocked",
        QuotaBlocked => "quota_blocked",
        BreakerOpen => "breaker_open",
        TransportError => "transport_error",
        ParseError => "parse_error"
    );
    check_roundtrip!(
        JobStatus,
        Pending => "pending",
        Leased => "leased",
        Done => "done",
        Failed => "failed",
        Cancelled => "cancelled"
    );

    let err = CoverageState::from_db_str("ismeretlen").unwrap_err();
    assert_eq!(err.0, "ismeretlen");
    assert_eq!(err.to_string(), "ismeretlen db-érték: ismeretlen");
}

#[test]
fn transport_obscura_db_string() {
    assert_eq!(Transport::Obscura.as_db_str(), "obscura");
    assert_eq!(
        Transport::from_db_str("obscura").unwrap(),
        Transport::Obscura
    );
}
