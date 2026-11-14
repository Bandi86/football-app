use football_identity::crosswalk_conflicts;
use football_storage::test_support::test_pool;
use uuid::Uuid;

#[tokio::test]
async fn conflicting_crosswalk_bindings_are_reported() {
    let pool = test_pool().await;
    for right in ["ext-a", "ext-b"] {
        sqlx::query(
            "INSERT INTO identity.crosswalk
             (id, left_source, left_external_id, right_source, right_external_id,
              rung, method, status, evidence)
             VALUES ($1,'left','L1','right',$2,'strong','import','active','teszt')",
        )
        .bind(Uuid::now_v7())
        .bind(right)
        .execute(&pool)
        .await
        .unwrap();
    }
    let conflicts = crosswalk_conflicts(&pool).await.unwrap();
    assert_eq!(conflicts.len(), 1, "egy ambiguous kötés");
    assert!(conflicts[0].conflicting_right_external_ids.len() >= 2);
}
