use football_storage::test_support::test_pool;

#[tokio::test]
async fn identity_tables_and_constraints() {
    let pool = test_pool().await;
    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_tables WHERE schemaname='identity'
         AND tablename IN ('crosswalk','alias','review_queue')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tables, 3);

    // CHECK: ismeretlen rung nem írható
    let bad = sqlx::query(
        "INSERT INTO identity.crosswalk (id, left_source, left_external_id,
         right_source, right_external_id, rung, method, evidence)
         VALUES (gen_random_uuid(),'a','1','b','2','unknown','import','x')",
    )
    .execute(&pool)
    .await;
    assert!(bad.is_err());

    // részleges pending unique: ugyanarra a kulcsra csak egy pending
    sqlx::query(
        "INSERT INTO identity.review_queue (id, entity_kind, source_entity_id,
         candidate_entity_ids, reason, proposed_name, context)
         VALUES (gen_random_uuid(),'team','x','[]','no_exact_match','X','{}')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let dup = sqlx::query(
        "INSERT INTO identity.review_queue (id, entity_kind, source_entity_id,
         candidate_entity_ids, reason, proposed_name, context)
         VALUES (gen_random_uuid(),'team','x','[]','no_exact_match','X','{}')",
    )
    .execute(&pool)
    .await;
    assert!(dup.is_err());
}
