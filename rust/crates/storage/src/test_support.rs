use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

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
        "TRUNCATE fact.odds_quote, fact.match_stats, fact.odds_snapshot, core.standing,
         ops.publish_gate, fact.verification, fact.conflict, fact.provenance,
         identity.review_queue, identity.alias, identity.crosswalk,
         identity.source_entity_ref, core.match_result, core.match, core.team,
         core.season, core.competition, core.country, core.entity,
         stage.observation, raw.capture, raw.artifact, ops.source_trust, ops.source,
         ops.validation_finding, ops.validation_run, ops.validation_rule,
         ops.robots_cache, ops.coverage_slot, ops.job, ops.breaker_state,
         ops.fetch_ledger, ops.source_policy,
         ops.audit_event RESTART IDENTITY CASCADE",
    )
    .execute(&pool)
    .await
    .expect("tesztadatok törlése");

    sqlx::query(
        "INSERT INTO ops.validation_rule
         (id, rule_key, version, default_severity, active, description)
         VALUES
         (gen_random_uuid(), 'TRUST_LEVEL', '1.0', 'warning', true, 'Bizalmi szint'),
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

/// Minimál érvényes lánc a sématesztekhez: ország → verseny → szezon →
/// 2 csapat → 1 source → 1 meccs. Visszaadja (match_id, source_id).
pub async fn seed_min_match(pool: &PgPool) -> (Uuid, Uuid) {
    let country_id = Uuid::now_v7();
    let competition_entity_id = Uuid::now_v7();
    let competition_id = Uuid::now_v7();
    let season_id = Uuid::now_v7();
    let home_entity_id = Uuid::now_v7();
    let home_team_id = Uuid::now_v7();
    let away_entity_id = Uuid::now_v7();
    let away_team_id = Uuid::now_v7();
    let source_id = Uuid::now_v7();
    let match_id = Uuid::now_v7();

    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Hungary')")
        .bind(country_id)
        .execute(pool)
        .await
        .expect("country");
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','NB I')",
    )
    .bind(competition_entity_id)
    .execute(pool)
    .await
    .expect("competition entity");
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name)
         VALUES ($1,$2,$3,'NB I')",
    )
    .bind(competition_id)
    .bind(competition_entity_id)
    .bind(country_id)
    .execute(pool)
    .await
    .expect("competition");
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season_id)
        .bind(competition_id)
        .execute(pool)
        .await
        .expect("season");
    for (entity_id, team_id, name) in [
        (home_entity_id, home_team_id, "Újpest"),
        (away_entity_id, away_team_id, "Győr"),
    ] {
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(entity_id)
            .bind(name)
            .execute(pool)
            .await
            .expect("team entity");
        sqlx::query(
            "INSERT INTO core.team
             (id, entity_id, country_id, name, gender, age_group, branch)
             VALUES ($1,$2,$3,$4,'male','senior','senior')",
        )
        .bind(team_id)
        .bind(entity_id)
        .bind(country_id)
        .bind(name)
        .execute(pool)
        .await
        .expect("team");
    }
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,'seed','Seed')")
        .bind(source_id)
        .execute(pool)
        .await
        .expect("source");
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, home_team_id, away_team_id, kickoff_at,
          competition_timezone, status)
         VALUES ($1,$2,$3,$4,$5,'2026-11-14T18:00:00Z','Europe/Budapest','scheduled')",
    )
    .bind(match_id)
    .bind(competition_id)
    .bind(season_id)
    .bind(home_team_id)
    .bind(away_team_id)
    .execute(pool)
    .await
    .expect("match");
    (match_id, source_id)
}
