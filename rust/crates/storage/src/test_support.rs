use sqlx::{PgPool, postgres::PgPoolOptions};

pub async fn test_pool() -> PgPool {
    let url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL kötelező az integrációs tesztekhez");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("tesztadatbázis kapcsolat");

    sqlx::migrate!("../../../db/migrations")
        .run(&pool)
        .await
        .expect("teszt migrációk");

    sqlx::query(
        "TRUNCATE ops.publish_gate, fact.verification, fact.conflict, fact.provenance,
         identity.source_entity_ref, core.match_result, core.match, core.team,
         core.season, core.competition, core.country, core.entity,
         stage.observation, raw.capture, raw.artifact, ops.source,
         ops.validation_finding, ops.validation_run, ops.validation_rule,
         ops.audit_event RESTART IDENTITY CASCADE",
    )
    .execute(&pool)
    .await
    .expect("tesztadatok törlése");

    sqlx::query(
        "INSERT INTO ops.validation_rule
         (id, rule_key, version, default_severity, active, description)
         VALUES
         (gen_random_uuid(), 'SINGLE_SOURCE', '1.0', 'warning', true, 'Egy forrás'),
         (gen_random_uuid(), 'KICKOFF_VALID', '1.0', 'blocker', true, 'Kickoff'),
         (gen_random_uuid(), 'EXACT_IDENTITY', '1.0', 'blocker', true, 'Identitás'),
         (gen_random_uuid(), 'DUPLICATE_NATURAL_KEY', '1.0', 'blocker', true, 'Duplikátum'),
         (gen_random_uuid(), 'MATCH_RESULT_CONSISTENT', '1.0', 'blocker', true, 'Eredmény'),
         (gen_random_uuid(), 'MATERIAL_SOURCE_CONFLICT', '1.0', 'blocker', true, 'Konfliktus')
         ON CONFLICT (rule_key) DO NOTHING",
    )
    .execute(&pool)
    .await
    .expect("szabályok");

    pool
}
