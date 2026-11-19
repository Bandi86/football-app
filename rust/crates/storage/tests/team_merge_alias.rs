//! S26e — alias-variáns csapat-merge tesztjei: eltérő nevek, azonos kontextus,
//! v1-alias bizonyítékkal. A reviewed alias rögzítődik, a merge tranzakciós.

use chrono::Utc;
use football_storage::repositories::identity_merge::{
    merge_team, merge_team_preview, merge_team_via_alias,
};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

struct Seed {
    survivor_entity: Uuid,
    duplicate_entity: Uuid,
    survivor_team: Uuid,
    duplicate_team: Uuid,
}

/// Két csapat eltérő névvel, azonos országgal/nemmel/korosztállyal/ággal,
/// egy-egy meccsel.
async fn seed_variant_pair(pool: &PgPool, name_a: &str, name_b: &str) -> Seed {
    let country = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'GB','England')")
        .bind(country)
        .execute(pool)
        .await
        .unwrap();
    let comp_entity = Uuid::now_v7();
    let comp = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','Premier League')")
        .bind(comp_entity)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO core.competition (id, entity_id, country_id, name) VALUES ($1,$2,$3,'Premier League')")
        .bind(comp)
        .bind(comp_entity)
        .bind(country)
        .execute(pool)
        .await
        .unwrap();
    let season = Uuid::now_v7();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2015/16')")
        .bind(season)
        .bind(comp)
        .execute(pool)
        .await
        .unwrap();
    let mut entities = Vec::new();
    let mut teams = Vec::new();
    for name in [name_a, name_b] {
        let entity = Uuid::now_v7();
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(entity)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch) VALUES ($1,$1,$2,$3,'male','senior','senior')")
            .bind(entity)
            .bind(country)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        entities.push(entity);
        teams.push(entity);
    }
    // Egy-egy meccs (különböző ellenféllel, hogy ne legyen meccs-duplikátum).
    for (i, team) in teams.iter().enumerate() {
        let opponent = Uuid::now_v7();
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(opponent)
            .bind(format!("Opponent {i}"))
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO core.team (id, entity_id, country_id, name, gender) VALUES ($1,$1,$2,$3,'male')")
            .bind(opponent)
            .bind(country)
            .bind(format!("Opponent {i}"))
            .execute(pool)
            .await
            .unwrap();
        let m = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO core.match
             (id, competition_id, season_id, home_team_id, away_team_id, kickoff_at, competition_timezone, status)
             VALUES ($1,$2,$3,$4,$5,$6,'UTC','finished')",
        )
        .bind(m)
        .bind(comp)
        .bind(season)
        .bind(team)
        .bind(opponent)
        .bind(Utc::now())
        .execute(pool)
        .await
        .unwrap();
    }
    Seed {
        survivor_entity: entities[0],
        duplicate_entity: entities[1],
        survivor_team: teams[0],
        duplicate_team: teams[1],
    }
}

#[tokio::test]
async fn strict_merge_refuses_different_names() {
    let pool = test_pool().await;
    let seed = seed_variant_pair(&pool, "Manchester City", "Man City").await;
    let err = merge_team(&pool, seed.survivor_entity, seed.duplicate_entity, "teszt")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("eltérő név"), "{err}");
}

#[tokio::test]
async fn alias_variant_merge_moves_matches_and_records_reviewed_alias() {
    let pool = test_pool().await;
    let seed = seed_variant_pair(&pool, "Manchester City", "Man City").await;

    // Előnézet alias-úton (eltérő nevek engedélyezve).
    let preview = merge_team_preview(&pool, seed.survivor_entity, seed.duplicate_entity, false)
        .await
        .unwrap();
    assert_eq!(preview.matches_moved, 1);

    let report = merge_team_via_alias(
        &pool,
        seed.survivor_entity,
        seed.duplicate_entity,
        "v1 team_aliases: 'Man City' -> 'Manchester City'",
    )
    .await
    .unwrap();
    assert_eq!(report.matches_moved, 1);

    // A meccs a túlélő csapatára mutat.
    let home: Uuid = sqlx::query_scalar("SELECT home_team_id FROM core.match LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(home, seed.survivor_team);
    // Az áldozat team-sor törölve, entitás merge-ölt.
    let team_left: i64 = sqlx::query_scalar("SELECT count(*) FROM core.team WHERE id=$1")
        .bind(seed.duplicate_team)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(team_left, 0);
    let merged_into: Option<Uuid> =
        sqlx::query_scalar("SELECT merged_into_id FROM core.entity WHERE id=$1")
            .bind(seed.duplicate_entity)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(merged_into, Some(seed.survivor_entity));
    // Reviewed alias rögzült a túlélőn, az áldozat nevével.
    let (alias, status, method): (String, String, String) = sqlx::query_as(
        "SELECT alias, status, method FROM identity.alias WHERE entity_id=$1 AND alias='Man City'",
    )
    .bind(seed.survivor_entity)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (alias.as_str(), status.as_str(), method.as_str()),
        ("Man City", "reviewed", "import")
    );
    // Archívum + audit, bizonyítékkal.
    let note: String = sqlx::query_scalar(
        "SELECT note FROM ops.entity_merge_archive WHERE duplicate_entity_id=$1",
    )
    .bind(seed.duplicate_entity)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(note.contains("v1 team_aliases"), "{note}");
    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.audit_event WHERE action='team_merge' AND subject_id=$1",
    )
    .bind(seed.survivor_entity)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 1);
}

#[tokio::test]
async fn alias_variant_merge_requires_evidence() {
    let pool = test_pool().await;
    let seed = seed_variant_pair(&pool, "Manchester City", "Man City").await;
    let err = merge_team_via_alias(&pool, seed.survivor_entity, seed.duplicate_entity, "  ")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("bizonyíték"), "{err}");
}

#[tokio::test]
async fn alias_variant_merge_still_refuses_context_mismatch() {
    let pool = test_pool().await;
    let seed = seed_variant_pair(&pool, "Manchester City", "Man City").await;
    // Az áldozat országát NULL-ra állítjuk → kontextus-eltérés.
    sqlx::query("UPDATE core.team SET country_id=NULL WHERE id=$1")
        .bind(seed.duplicate_team)
        .execute(&pool)
        .await
        .unwrap();
    let err = merge_team_via_alias(
        &pool,
        seed.survivor_entity,
        seed.duplicate_entity,
        "v1 team_aliases: 'Man City' -> 'Manchester City'",
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("eltérő kontextus"), "{err}");
}

#[tokio::test]
async fn gb_subdivision_country_is_equivalent_to_gb() {
    let pool = test_pool().await;
    let seed = seed_variant_pair(&pool, "Swansea City", "Swansea").await;
    // A túlélő GB-WLS (v1 alrégió-kód), az áldozat GB (footballdata).
    let wls = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'GB-WLS','Wales')")
        .bind(wls)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE core.team SET country_id=$1 WHERE id=$2")
        .bind(wls)
        .bind(seed.survivor_team)
        .execute(&pool)
        .await
        .unwrap();
    let report = merge_team_via_alias(
        &pool,
        seed.survivor_entity,
        seed.duplicate_entity,
        "v1 team_aliases: 'Swansea' -> 'Swansea City'",
    )
    .await
    .unwrap();
    assert_eq!(report.matches_moved, 1);
}
