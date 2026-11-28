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

    let lease_type: String = sqlx::query_scalar(
        "SELECT data_type FROM information_schema.columns
         WHERE table_schema='ops' AND table_name='job' AND column_name='lease_token'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(lease_type, "uuid");
    let lease_index: String = sqlx::query_scalar(
        "SELECT indexdef FROM pg_indexes
         WHERE schemaname='ops' AND tablename='job' AND indexname='idx_job_lease_token'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(lease_index.contains("CREATE UNIQUE INDEX"));
    assert!(lease_index.contains("lease_token IS NOT NULL"));
    let lease_constraint: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_constraint
         WHERE conname='job_lease_token_status_check' AND conrelid='ops.job'::regclass",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(lease_constraint, 1);

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

    let invalid_status = sqlx::query(
        "INSERT INTO ops.job (id, source_slug, domain, entity_key, event_type,
         time_window, due_at, status, lease_token)
         VALUES (gen_random_uuid(),'demo-synthetic','match','invalid-token',
         'discovery','2026-11-12',now(),'pending',gen_random_uuid())",
    )
    .execute(&pool)
    .await;
    assert!(invalid_status.is_err());

    sqlx::query(
        "INSERT INTO ops.job (id, source_slug, domain, entity_key, event_type,
         time_window, due_at, status, lease_token)
         VALUES (gen_random_uuid(),'demo-synthetic','match','token-a',
         'discovery','2026-11-12',now(),'leased','11111111-1111-1111-1111-111111111111')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let duplicate_token = sqlx::query(
        "INSERT INTO ops.job (id, source_slug, domain, entity_key, event_type,
         time_window, due_at, status, lease_token)
         VALUES (gen_random_uuid(),'demo-synthetic','match','token-b',
         'discovery','2026-11-12',now(),'leased','11111111-1111-1111-1111-111111111111')",
    )
    .execute(&pool)
    .await;
    assert!(duplicate_token.is_err());

    sqlx::query(
        "INSERT INTO ops.fetch_ledger (id, source_slug, request_key, attempted_at, outcome)
         VALUES (gen_random_uuid(),'demo-synthetic','reserved-check',now(),'reserved')",
    )
    .execute(&pool)
    .await
    .unwrap();
}
