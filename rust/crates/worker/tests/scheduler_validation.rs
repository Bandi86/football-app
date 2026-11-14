use chrono::Utc;
use football_domain::policy::{EventTemplate, SourceMode, SourcePolicy, Transport};
use football_storage::repositories::worker as repo;
use football_storage::test_support::test_pool;
use football_worker::scheduler;

fn policy_with(events: Vec<EventTemplate>) -> SourcePolicy {
    SourcePolicy {
        source_slug: "demo-synthetic".into(),
        transport: Transport::Fixture,
        mode: SourceMode::Shadow,
        daily_budget: 10,
        min_delay_ms: 0,
        jitter_ms: 0,
        breaker_threshold: 3,
        breaker_cooldown_secs: 60,
        adapter_version: "1.0".into(),
        parser_version: "1.0".into(),
        domains: vec!["match".into()],
        events,
        enabled: true,
        max_bytes: 5_242_880,
    }
}

#[tokio::test]
async fn unknown_event_type_generates_no_job() {
    let pool = test_pool().await;
    let policy = policy_with(vec![EventTemplate {
        event_type: "mystery".into(),
        offset_minutes: None,
        interval_minutes: Some(1440),
    }]);
    repo::upsert_policy(&pool, &policy).await.unwrap();
    let inserted = scheduler::generate_jobs(&pool, &[policy], Utc::now())
        .await
        .unwrap();
    assert_eq!(inserted, 0, "ismeretlen event_type -> nincs job");
}

#[tokio::test]
async fn invalid_interval_generates_no_job() {
    let pool = test_pool().await;
    let policy = policy_with(vec![EventTemplate {
        event_type: "discovery".into(),
        offset_minutes: None,
        interval_minutes: Some(60),
    }]);
    repo::upsert_policy(&pool, &policy).await.unwrap();
    let inserted = scheduler::generate_jobs(&pool, &[policy], Utc::now())
        .await
        .unwrap();
    assert_eq!(inserted, 0, "a 60 perces interval nem engedett");
}

#[tokio::test]
async fn daily_interval_generates_one_job() {
    let pool = test_pool().await;
    let policy = policy_with(vec![EventTemplate {
        event_type: "discovery".into(),
        offset_minutes: None,
        interval_minutes: Some(1440),
    }]);
    repo::upsert_policy(&pool, &policy).await.unwrap();
    let inserted = scheduler::generate_jobs(&pool, &[policy], Utc::now())
        .await
        .unwrap();
    assert_eq!(inserted, 1);
}
