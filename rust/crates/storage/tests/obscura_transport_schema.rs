use football_storage::repositories::worker as worker_repo;
use football_storage::test_support::test_pool;

#[tokio::test]
async fn source_policy_allows_obscura_transport() {
    let pool = test_pool().await;
    // A 0007 migráció bővíti a transport CHECK-et: az 'obscura' beszúrható.
    let res = sqlx::query(
        "INSERT INTO ops.source_policy (source_slug, transport, mode, daily_budget,
         min_delay_ms, jitter_ms, breaker_threshold, breaker_cooldown_secs,
         adapter_version, parser_version)
         VALUES ('obscura-test','obscura','shadow',10,2000,0,3,60,'1.0','1.0')",
    )
    .execute(&pool)
    .await;
    assert!(res.is_ok(), "obscura transport beszúrható: {:?}", res.err());
}

#[tokio::test]
async fn source_policy_rejects_unknown_transport() {
    let pool = test_pool().await;
    let res = sqlx::query(
        "INSERT INTO ops.source_policy (source_slug, transport, mode, daily_budget,
         min_delay_ms, jitter_ms, breaker_threshold, breaker_cooldown_secs,
         adapter_version, parser_version)
         VALUES ('bad-transport','wormhole','shadow',10,2000,0,3,60,'1.0','1.0')",
    )
    .execute(&pool)
    .await;
    assert!(res.is_err(), "ismeretlen transport nem beszúrható");
}

#[tokio::test]
async fn has_enabled_transport_sees_only_enabled_obscura_policy() {
    let pool = test_pool().await;
    sqlx::query(
        "INSERT INTO ops.source_policy (source_slug, transport, mode, daily_budget,
         min_delay_ms, jitter_ms, breaker_threshold, breaker_cooldown_secs,
         adapter_version, parser_version)
         VALUES ('obscura-lazy','obscura','shadow',10,2000,0,3,60,'1.0','1.0')",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        !worker_repo::has_enabled_transport(&pool, "obscura")
            .await
            .unwrap(),
        "a tiltott (enabled=false) obscura policy nem indít szervert"
    );
    sqlx::query("UPDATE ops.source_policy SET enabled = true WHERE source_slug = 'obscura-lazy'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        worker_repo::has_enabled_transport(&pool, "obscura")
            .await
            .unwrap(),
        "az engedélyezett obscura policy látszik"
    );
    assert!(
        !worker_repo::has_enabled_transport(&pool, "rust_http")
            .await
            .unwrap(),
        "más transzport nem számít obscurának"
    );
}
