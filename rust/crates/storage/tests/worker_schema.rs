use football_storage::test_support::test_pool;

#[tokio::test]
async fn worker_tables_exist_and_constraints_hold() {
    let pool = test_pool().await;

    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_tables WHERE schemaname='ops'
         AND tablename IN ('source_policy','fetch_ledger','breaker_state','job','coverage_slot','robots_cache')",
    )
    .fetch_one(&pool).await.unwrap();
    assert_eq!(tables, 6);

    // CHECK: ismeretlen transport nem írható
    let bad = sqlx::query(
        "INSERT INTO ops.source_policy (source_slug, transport, mode, daily_budget,
         min_delay_ms, jitter_ms, breaker_threshold, breaker_cooldown_secs,
         adapter_version, parser_version)
         VALUES ('x','wormhole','shadow',10,0,0,3,60,'1.0','1.0')",
    )
    .execute(&pool)
    .await;
    assert!(bad.is_err());

    // job egyedi kulcs: duplikált generálás nem szúr be második sort.
    // A status explicite kell: a 0004 migráció NOT NULL, default nélkül.
    sqlx::query(
        "INSERT INTO ops.source_policy (source_slug, transport, mode, daily_budget,
         min_delay_ms, jitter_ms, breaker_threshold, breaker_cooldown_secs,
         adapter_version, parser_version)
         VALUES ('demo-synthetic','fixture','shadow',10,0,0,3,60,'1.0','1.0')",
    )
    .execute(&pool)
    .await
    .unwrap();
    for _ in 0..2 {
        sqlx::query(
            "INSERT INTO ops.job (id, source_slug, domain, entity_key, event_type, time_window, due_at, status)
             VALUES (gen_random_uuid(),'demo-synthetic','match','day','discovery','2026-11-12', now(), 'pending')
             ON CONFLICT (source_slug, domain, entity_key, event_type, time_window) DO NOTHING",
        )
        .execute(&pool).await.unwrap();
    }
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.job")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 1);
}
