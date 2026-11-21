//! S81: verseny-merge tesztek (football_v2_test_comp).

use chrono::{TimeZone, Utc};
use football_storage::repositories::competition_merge::{
    CompetitionMergeOutcome, competition_merge_preview, merge_competition,
};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

struct Comp {
    entity: Uuid,
    competition: Uuid,
}

async fn seed_country(pool: &PgPool, code: &str, name: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,$2,$3)")
        .bind(id)
        .bind(code)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn seed_source(pool: &PgPool, code: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,$2,$2)")
        .bind(id)
        .bind(code)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn seed_competition(
    pool: &PgPool,
    country: Option<Uuid>,
    canonical: &str,
    name: &str,
) -> Comp {
    let entity = Uuid::now_v7();
    let competition = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition',$2)")
        .bind(entity)
        .bind(canonical)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name)
         VALUES ($1,$2,$3,$4)",
    )
    .bind(competition)
    .bind(entity)
    .bind(country)
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
    Comp {
        entity,
        competition,
    }
}

async fn seed_season(pool: &PgPool, competition: Uuid, label: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,$3)")
        .bind(id)
        .bind(competition)
        .bind(label)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn seed_team(pool: &PgPool, country: Uuid, name: &str) -> Uuid {
    let entity = Uuid::now_v7();
    let team = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
        .bind(entity)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$2,$3,$4,'male','senior','senior')",
    )
    .bind(team)
    .bind(entity)
    .bind(country)
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
    team
}

async fn seed_match(
    pool: &PgPool,
    competition: Uuid,
    season: Uuid,
    home: Uuid,
    away: Uuid,
    day: u32,
) -> Uuid {
    let id = Uuid::now_v7();
    let kickoff = Utc
        .with_ymd_and_hms(2026, 8, day, 18, 0, 0)
        .single()
        .unwrap();
    sqlx::query(
        "INSERT INTO core.match
             (id, competition_id, season_id, home_team_id, away_team_id, kickoff_at,
              competition_timezone, status)
         VALUES ($1,$2,$3,$4,$5,$6,'UTC','scheduled')",
    )
    .bind(id)
    .bind(competition)
    .bind(season)
    .bind(home)
    .bind(away)
    .bind(kickoff)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn seed_alias(pool: &PgPool, entity: Uuid, alias: &str, country_code: &str) {
    sqlx::query(
        "INSERT INTO identity.alias
             (id, entity_id, alias, lang, country_code, status, method)
         VALUES ($1,$2,$3,'und',$4,'reviewed','import')",
    )
    .bind(Uuid::now_v7())
    .bind(entity)
    .bind(alias)
    .bind(country_code)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_ref(pool: &PgPool, source: Uuid, entity: Uuid, external_id: &str) {
    sqlx::query(
        "INSERT INTO identity.source_entity_ref
             (id, source_id, entity_kind, source_entity_id, entity_id, method, confidence, status)
         VALUES ($1,$2,'competition',$3,$4,'exact_external_id',1.0,'active')",
    )
    .bind(Uuid::now_v7())
    .bind(source)
    .bind(external_id)
    .bind(entity)
    .execute(pool)
    .await
    .unwrap();
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

async fn archive_count(pool: &PgPool, survivor_entity: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM ops.entity_merge_archive WHERE survivor_entity_id=$1")
        .bind(survivor_entity)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// (b) A merge átviszi a meccseket/seasonöket/aliasokat/source-refeket, beállítja
/// a merged_into_id-t, archív+audit sort ír, a meccsszám összesen változatlan;
/// (d) az ismételt hívás idempotens (nulla írás).
#[tokio::test]
async fn merge_moves_relations_and_is_idempotent() {
    let pool = test_pool().await;
    let country = seed_country(&pool, "ES", "Spain").await;
    let source = seed_source(&pool, "test-comp-merge").await;

    let survivor = seed_competition(&pool, Some(country), "La Liga", "La Liga").await;
    let duplicate = seed_competition(&pool, Some(country), "LaLiga", "LaLiga").await;
    let survivor_season = seed_season(&pool, survivor.competition, "2026/27").await;
    let duplicate_season = seed_season(&pool, duplicate.competition, "2026/27").await;
    let duplicate_old_season = seed_season(&pool, duplicate.competition, "2025/26").await;
    let home = seed_team(&pool, country, "Real Madrid").await;
    let away = seed_team(&pool, country, "Barcelona").await;

    seed_match(&pool, survivor.competition, survivor_season, home, away, 1).await;
    seed_match(&pool, survivor.competition, survivor_season, home, away, 2).await;
    seed_match(
        &pool,
        duplicate.competition,
        duplicate_season,
        home,
        away,
        3,
    )
    .await;
    seed_match(
        &pool,
        duplicate.competition,
        duplicate_old_season,
        home,
        away,
        4,
    )
    .await;

    seed_alias(&pool, survivor.entity, "La Liga", "ES").await;
    seed_alias(&pool, duplicate.entity, "LaLiga", "ES").await;
    // Ütköző alias: merge után a survivor már meglévő sorára esne, eldobandó.
    seed_alias(&pool, duplicate.entity, "La Liga", "ES").await;
    seed_ref(&pool, source, survivor.entity, "ext-surv").await;
    seed_ref(&pool, source, duplicate.entity, "ext-dup").await;

    let preview = competition_merge_preview(&pool, survivor.entity, duplicate.entity, false)
        .await
        .unwrap();
    assert!(!preview.already_merged);
    assert_eq!(preview.matches_to_move, 2);
    assert_eq!(preview.seasons_moved, 1);
    assert_eq!(preview.seasons_merged, 1);
    assert_eq!(preview.aliases, 2);
    assert_eq!(preview.refs, 1);

    let outcome = merge_competition(
        &pool,
        survivor.entity,
        duplicate.entity,
        "S81 teszt merge",
        false,
    )
    .await
    .unwrap();
    let report = match outcome {
        CompetitionMergeOutcome::Merged(report) => report,
        CompetitionMergeOutcome::AlreadyMerged { .. } => panic!("az első merge nem futhatott le"),
    };
    assert_eq!(report.matches_moved, 2);
    assert_eq!(report.seasons_moved, 1);
    assert_eq!(report.seasons_merged, 1);
    assert_eq!(report.aliases_moved, 1);
    assert_eq!(report.aliases_dropped, 1);
    assert_eq!(report.refs_moved, 1);
    assert_eq!(report.duplicate_match_groups, 0);

    // Meccsszám összesen változatlan, a survivor alatt 4 meccs van.
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 4);
    assert_eq!(
        count(
            &pool,
            &format!(
                "SELECT count(*) FROM core.match WHERE competition_id='{}'",
                survivor.competition
            )
        )
        .await,
        4
    );

    // A beolvadó competition sor megmarad, az entitás merged_into_id-t kap.
    assert_eq!(
        count(
            &pool,
            &format!(
                "SELECT count(*) FROM core.competition WHERE id='{}'",
                duplicate.competition
            )
        )
        .await,
        1,
        "a merged competition sora nem törlődik"
    );
    let merged_into: Option<Uuid> =
        sqlx::query_scalar("SELECT merged_into_id FROM core.entity WHERE id=$1")
            .bind(duplicate.entity)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(merged_into, Some(survivor.entity));

    // Seasonök: a survivor alatt 2 label, a beolvadó alatt 0 sor.
    assert_eq!(
        count(
            &pool,
            &format!(
                "SELECT count(*) FROM core.season WHERE competition_id='{}'",
                survivor.competition
            )
        )
        .await,
        2
    );
    assert_eq!(
        count(
            &pool,
            &format!(
                "SELECT count(*) FROM core.season WHERE competition_id='{}'",
                duplicate.competition
            )
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM core.season WHERE label='2026/27'",
        )
        .await,
        1,
        "ütköző labelből egy sor marad"
    );

    // Aliasok és refek a survivoron.
    assert_eq!(
        count(
            &pool,
            &format!(
                "SELECT count(*) FROM identity.alias WHERE entity_id='{}'",
                survivor.entity
            )
        )
        .await,
        2
    );
    assert_eq!(
        count(
            &pool,
            &format!(
                "SELECT count(*) FROM identity.alias WHERE entity_id='{}'",
                duplicate.entity
            )
        )
        .await,
        0
    );
    assert_eq!(
        count(
            &pool,
            &format!(
                "SELECT count(*) FROM identity.source_entity_ref WHERE entity_id='{}'",
                survivor.entity
            )
        )
        .await,
        2
    );
    assert_eq!(
        count(
            &pool,
            &format!(
                "SELECT count(*) FROM identity.source_entity_ref WHERE entity_id='{}'",
                duplicate.entity
            )
        )
        .await,
        0
    );

    // Archívum + audit: előtte/utána counts payload.
    assert_eq!(archive_count(&pool, survivor.entity).await, 1);
    let before: String = sqlx::query_scalar(
        "SELECT payload->'counts_before'->>'duplicate_matches'
         FROM ops.entity_merge_archive WHERE survivor_entity_id=$1",
    )
    .bind(survivor.entity)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(before, "2");
    let after: String = sqlx::query_scalar(
        "SELECT payload->'counts_after'->>'matches'
         FROM ops.entity_merge_archive WHERE survivor_entity_id=$1",
    )
    .bind(survivor.entity)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after, "4");
    assert_eq!(
        count(
            &pool,
            &format!(
                "SELECT count(*) FROM ops.audit_event
                 WHERE action='competition_merge' AND subject_id='{}'",
                survivor.entity
            )
        )
        .await,
        1
    );

    // Idempotencia: újabb hívás nem ír, hanem jelzi.
    let again = merge_competition(
        &pool,
        survivor.entity,
        duplicate.entity,
        "S81 teszt merge",
        false,
    )
    .await
    .unwrap();
    assert!(matches!(
        again,
        CompetitionMergeOutcome::AlreadyMerged { .. }
    ));
    assert_eq!(archive_count(&pool, survivor.entity).await, 1);
    assert_eq!(
        count(
            &pool,
            &format!(
                "SELECT count(*) FROM ops.audit_event
                 WHERE action='competition_merge' AND subject_id='{}'",
                survivor.entity
            )
        )
        .await,
        1
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 4);
}

/// (c) Guard-esetek: tiszta hiba és nulla írás.
#[tokio::test]
async fn guards_error_without_writes() {
    let pool = test_pool().await;
    let country = seed_country(&pool, "ES", "Spain").await;
    let other_country = seed_country(&pool, "IT", "Italy").await;

    // Nem önmaga.
    let self_comp = seed_competition(&pool, Some(country), "Self Liga", "Self Liga").await;
    let error = merge_competition(&pool, self_comp.entity, self_comp.entity, "teszt", false)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("nem lehet ugyanaz"));
    assert_eq!(archive_count(&pool, self_comp.entity).await, 0);

    // Rossz kind: az egyik entitás csapat.
    let comp = seed_competition(&pool, Some(country), "Beta League", "Beta League").await;
    let team_entity = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team','Beta')")
        .bind(team_entity)
        .execute(&pool)
        .await
        .unwrap();
    let error = merge_competition(&pool, comp.entity, team_entity, "teszt", false)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("competition"));
    assert_eq!(archive_count(&pool, comp.entity).await, 0);
    let team_merged: Option<Uuid> =
        sqlx::query_scalar("SELECT merged_into_id FROM core.entity WHERE id=$1")
            .bind(team_entity)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(team_merged, None);

    // Eltérő fold.
    let gamma = seed_competition(&pool, Some(country), "Gamma Liga", "Gamma Liga").await;
    let delta = seed_competition(&pool, Some(country), "Delta Liga", "Delta Liga").await;
    let error = merge_competition(&pool, gamma.entity, delta.entity, "teszt", false)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("fold"));
    assert_eq!(archive_count(&pool, gamma.entity).await, 0);

    // Eltérő ország.
    let epsilon_es = seed_competition(&pool, Some(country), "Epsilon Liga", "Epsilon Liga").await;
    let epsilon_it =
        seed_competition(&pool, Some(other_country), "Epsilon Liga", "Epsilon Liga").await;
    let error = merge_competition(&pool, epsilon_es.entity, epsilon_it.entity, "teszt", false)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("ország"));
    assert_eq!(archive_count(&pool, epsilon_es.entity).await, 0);

    // A survivor kevesebb meccsel: alapból hiba, --force esetén lefut.
    let zeta = seed_competition(&pool, Some(country), "Zeta Liga", "Zeta Liga").await;
    let eta = seed_competition(&pool, Some(country), "Zeta Liga", "Zeta Liga").await;
    let zeta_season = seed_season(&pool, zeta.competition, "2026/27").await;
    let eta_season = seed_season(&pool, eta.competition, "2026/27").await;
    let home = seed_team(&pool, country, "Hazai").await;
    let away = seed_team(&pool, country, "Vendég").await;
    seed_match(&pool, zeta.competition, zeta_season, home, away, 1).await;
    for day in [2, 3] {
        seed_match(&pool, eta.competition, eta_season, home, away, day).await;
    }
    let error = merge_competition(&pool, zeta.entity, eta.entity, "teszt", false)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("kevesebb meccse"));
    assert_eq!(archive_count(&pool, zeta.entity).await, 0);
    let eta_merged: Option<Uuid> =
        sqlx::query_scalar("SELECT merged_into_id FROM core.entity WHERE id=$1")
            .bind(eta.entity)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(eta_merged, None);
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 3);

    let forced = merge_competition(&pool, zeta.entity, eta.entity, "force teszt", true)
        .await
        .unwrap();
    assert!(matches!(forced, CompetitionMergeOutcome::Merged(_)));
    assert_eq!(archive_count(&pool, zeta.entity).await, 1);
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 3);
    assert_eq!(
        count(
            &pool,
            &format!(
                "SELECT count(*) FROM core.match WHERE competition_id='{}'",
                zeta.competition
            )
        )
        .await,
        3
    );
}

/// S81: alias nélküli páros merge-e után a túlélő entitásra aliasként
/// felkerül mindkét versenynév és mindkét kanonikus entitás-név, hogy a
/// következő ingest ne hozza létre újra a beolvadó versenyt.
#[tokio::test]
async fn merge_backfills_aliases_for_future_lookup() {
    let pool = test_pool().await;
    let country = seed_country(&pool, "CL", "Chile").await;
    let survivor = seed_competition(&pool, Some(country), "La Liga", "Liga de Primera").await;
    let duplicate = seed_competition(&pool, Some(country), "La Liga", "La Liga").await;

    let outcome = merge_competition(
        &pool,
        survivor.entity,
        duplicate.entity,
        "S81 alias backfill teszt",
        true,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, CompetitionMergeOutcome::Merged(_)));

    let aliases: Vec<String> =
        sqlx::query_scalar("SELECT alias FROM identity.alias WHERE entity_id = $1 ORDER BY alias")
            .bind(survivor.entity)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        aliases,
        vec!["La Liga".to_string(), "Liga de Primera".to_string()],
        "a két versenynév aliasként a túlélőn van"
    );

    let mut conn = pool.acquire().await.unwrap();
    let resolved =
        football_storage::repositories::competition_identity::find_or_create_competition(
            &mut conn,
            &football_storage::repositories::competition_identity::CompetitionIdentity {
                id_hint: None,
                name: "La Liga",
                country_id: Some(country),
                country_code: Some("CL"),
                tier: None,
                priority: None,
                source_id: None,
                source_entity_id: None,
            },
        )
        .await
        .unwrap();
    assert!(
        !resolved.created,
        "a merge utáni lookup nem hozhat létre új versenyt"
    );
    assert_eq!(
        resolved.competition_id, survivor.competition,
        "a túlélő versenyre oldódik fel"
    );
}

/// S84: cross-source névváltozat (pl. "Turkish Super Lig" vs "Süper Lig")
/// csak --force-szal merge-elhető, és a merge után a beolvadó neve aliasként
/// a túlélőn marad, hogy a következő ingest ne hozza létre újra.
#[tokio::test]
async fn force_merges_cross_source_name_variant_and_keeps_alias() {
    let pool = test_pool().await;
    let country = seed_country(&pool, "TR", "Turkey").await;
    let survivor = seed_competition(&pool, Some(country), "Süper Lig", "Süper Lig").await;
    let duplicate = seed_competition(
        &pool,
        Some(country),
        "Turkish Super Lig",
        "Turkish Super Lig",
    )
    .await;

    let error = merge_competition(&pool, survivor.entity, duplicate.entity, "teszt", false)
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("fold-olt neve eltér"),
        "force nélkül a névváltozat merge-nek hibáznia kell: {error}"
    );

    let outcome = merge_competition(
        &pool,
        survivor.entity,
        duplicate.entity,
        "S84 cross-source teszt",
        true,
    )
    .await
    .unwrap();
    assert!(matches!(outcome, CompetitionMergeOutcome::Merged(_)));

    let aliases: Vec<String> =
        sqlx::query_scalar("SELECT alias FROM identity.alias WHERE entity_id = $1 ORDER BY alias")
            .bind(survivor.entity)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        aliases,
        vec!["Süper Lig".to_string(), "Turkish Super Lig".to_string()]
    );
}
