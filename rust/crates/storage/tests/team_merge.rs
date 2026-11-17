use chrono::{TimeZone, Utc};
use football_storage::repositories::identity_merge::{
    duplicate_team_groups, merge_team, merge_team_preview,
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

async fn seed(pool: &PgPool) -> Seed {
    let country = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'CN','China')")
        .bind(country)
        .execute(pool)
        .await
        .unwrap();
    let comp_entity = Uuid::now_v7();
    let competition = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','Friendly International')")
        .bind(comp_entity)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO core.competition (id, entity_id, name) VALUES ($1,$2,'Friendly International')")
        .bind(competition)
        .bind(comp_entity)
        .execute(pool)
        .await
        .unwrap();
    let season = Uuid::now_v7();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season)
        .bind(competition)
        .execute(pool)
        .await
        .unwrap();

    // Két azonos nevű/kontextusú entitás + egy közös ellenfél.
    let mut twins = Vec::new();
    for name in ["China", "China"] {
        let entity = Uuid::now_v7();
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
        .bind(Uuid::now_v7())
        .bind(entity)
        .bind(country)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
        twins.push(entity);
    }
    let opponent_entity = Uuid::now_v7();
    let opponent_team = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team','Bahrain')")
        .bind(opponent_entity)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$2,$3,'Bahrain','male','senior','senior')",
    )
    .bind(opponent_team)
    .bind(opponent_entity)
    .bind(country)
    .execute(pool)
    .await
    .unwrap();

    let team_of = |entity: Uuid| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM core.team WHERE entity_id = $1")
                .bind(entity)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let survivor_team = team_of(twins[0]).await;
    let duplicate_team = team_of(twins[1]).await;

    // Ugyanaz a meccs mindkét ikernél → a merge után duplikált meccs keletkezik.
    let kickoff = Utc.with_ymd_and_hms(2026, 11, 17, 13, 0, 0).unwrap();
    for team in [survivor_team, duplicate_team] {
        sqlx::query(
            "INSERT INTO core.match
             (id, competition_id, season_id, home_team_id, away_team_id, kickoff_at,
              competition_timezone, status)
             VALUES ($1,$2,$3,$4,$5,$6,'UTC','scheduled')",
        )
        .bind(Uuid::now_v7())
        .bind(competition)
        .bind(season)
        .bind(team)
        .bind(opponent_team)
        .bind(kickoff)
        .execute(pool)
        .await
        .unwrap();
    }

    // Aliasok: közös (eldobandó) + egyedi (átmozgatandó) az áldozaton.
    for (entity, alias) in [(twins[0], "China"), (twins[1], "China"), (twins[1], "Kína")] {
        sqlx::query(
            "INSERT INTO identity.alias
             (id, entity_id, alias, lang, country_code, status, method)
             VALUES ($1,$2,$3,'hu','CN','reviewed','fixture_seed')",
        )
        .bind(Uuid::now_v7())
        .bind(entity)
        .bind(alias)
        .execute(pool)
        .await
        .unwrap();
    }

    // Forrás-ref az áldozaton.
    let source = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,'tipplap','Tipplap')")
        .bind(source)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO identity.source_entity_ref
         (id, source_id, entity_kind, source_entity_id, entity_id, method)
         VALUES ($1,$2,'team','china',$3,'manual_review')",
    )
    .bind(Uuid::now_v7())
    .bind(source)
    .bind(twins[1])
    .execute(pool)
    .await
    .unwrap();

    // Tabella: ugyanarra a szezonra mindkettőnél (konfliktus → eldobás).
    for (team, position) in [(survivor_team, 1i16), (duplicate_team, 2i16)] {
        sqlx::query(
            "INSERT INTO core.standing
             (season_id, team_id, position, source_id, captured_at)
             VALUES ($1,$2,$3,$4,now())",
        )
        .bind(season)
        .bind(team)
        .bind(position)
        .bind(source)
        .execute(pool)
        .await
        .unwrap();
    }

    Seed {
        survivor_entity: twins[0],
        duplicate_entity: twins[1],
        survivor_team,
        duplicate_team,
    }
}

#[tokio::test]
async fn merge_team_moves_children_and_archives_duplicate() {
    let pool = test_pool().await;
    let seed = seed(&pool).await;

    let groups = duplicate_team_groups(&pool).await.unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].teams.len(), 2);

    let preview = merge_team_preview(&pool, seed.survivor_entity, seed.duplicate_entity)
        .await
        .unwrap();
    assert_eq!(preview.matches_moved, 1);
    assert_eq!(preview.potential_duplicate_matches, 1);
    assert_eq!(preview.aliases, 2);
    assert_eq!(preview.refs, 1);

    let report = merge_team(
        &pool,
        seed.survivor_entity,
        seed.duplicate_entity,
        "teszt merge",
    )
    .await
    .unwrap();
    assert_eq!(report.matches_moved, 1);
    assert_eq!(report.aliases_moved, 1, "a Kína alias átmozgatva");
    assert_eq!(report.aliases_dropped, 1, "a közös China alias eldobva");
    assert_eq!(report.refs_moved, 1);
    assert_eq!(report.standings_moved, 0);
    assert_eq!(report.standings_dropped, 1);
    assert_eq!(report.duplicate_matches, 1);

    // A meccsek a túlélő team-re mutatnak; az áldozat team-sor törölve.
    let on_survivor: i64 =
        sqlx::query_scalar("SELECT count(*) FROM core.match WHERE home_team_id = $1")
            .bind(seed.survivor_team)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(on_survivor, 2);
    let dup_team_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM core.team WHERE id = $1")
        .bind(seed.duplicate_team)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(dup_team_rows, 0);

    // Az áldozat entitás lenyomat-megjelölése és az archívum.
    let merged_into: Option<Uuid> =
        sqlx::query_scalar("SELECT merged_into_id FROM core.entity WHERE id = $1")
            .bind(seed.duplicate_entity)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(merged_into, Some(seed.survivor_entity));
    let (archived, note): (i64, Option<String>) = sqlx::query_as(
        "SELECT count(*), max(note) FROM ops.entity_merge_archive WHERE duplicate_entity_id = $1",
    )
    .bind(seed.duplicate_entity)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(archived, 1);
    assert_eq!(note.as_deref(), Some("teszt merge"));

    // Alias és ref a túlélőnél.
    let aliases: i64 =
        sqlx::query_scalar("SELECT count(*) FROM identity.alias WHERE entity_id = $1")
            .bind(seed.survivor_entity)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(aliases, 2);
    let ref_entity: Uuid = sqlx::query_scalar(
        "SELECT entity_id FROM identity.source_entity_ref WHERE source_entity_id='china'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ref_entity, seed.survivor_entity);

    // Audit-sor.
    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.audit_event WHERE action='team_merge' AND subject_id=$1",
    )
    .bind(seed.survivor_entity)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 1);

    // Második merge: az áldozat már merge-ölt → hiba (fail-closed).
    let err = merge_team(
        &pool,
        seed.survivor_entity,
        seed.duplicate_entity,
        "ismételt",
    )
    .await
    .unwrap_err();
    assert!(format!("{err:#}").contains("már merge-ölt"), "{err:#}");
}
