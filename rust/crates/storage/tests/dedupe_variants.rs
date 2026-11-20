//! S37c — név-variáns duplikátumok: felismerés és merge.
//!
//! A valós eset: ugyanaz a meccs két forrásból, de a csapatnév forrásonként
//! eltér ("SV Darmstadt 98" / "Darmstadt"), ezért két külön kanonikus csapat
//! és két külön kanonikus meccs keletkezik. A szigorú `duplicate_groups`
//! (azonos csapat-azonosítók) ezeket nem látja — a variáns-keresés igen.

use chrono::{TimeZone, Utc};
use football_storage::repositories::dedupe::{
    is_variant_same_match, merge_duplicate, variant_duplicate_groups,
};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

struct Seeded {
    survivor: Uuid,
    duplicate: Uuid,
    source_id: Uuid,
}

async fn seed(pool: &PgPool) -> Seeded {
    let country_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'DE','Germany')")
        .bind(country_id)
        .execute(pool)
        .await
        .unwrap();

    let comp_entity = Uuid::now_v7();
    let competition_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition',$2)")
        .bind(comp_entity)
        .bind("2. Bundesliga")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name)
         VALUES ($1,$2,$3,'2. Bundesliga')",
    )
    .bind(competition_id)
    .bind(comp_entity)
    .bind(country_id)
    .execute(pool)
    .await
    .unwrap();
    let season_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season_id)
        .bind(competition_id)
        .execute(pool)
        .await
        .unwrap();

    let mut team_ids = Vec::new();
    for name in [
        "SV Darmstadt 98",
        "1. FC Heidenheim",
        "Darmstadt",
        "Heidenheim",
    ] {
        let entity = Uuid::now_v7();
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(entity)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO core.team (id, entity_id, country_id, name, gender)
             VALUES ($1,$1,$2,$3,'male')",
        )
        .bind(entity)
        .bind(country_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
        team_ids.push(entity);
    }

    let kickoff = Utc.with_ymd_and_hms(2026, 11, 20, 17, 30, 0).unwrap();
    let survivor = Uuid::now_v7();
    let duplicate = Uuid::now_v7();
    for (id, home, away) in [
        (survivor, team_ids[0], team_ids[1]),
        (duplicate, team_ids[2], team_ids[3]),
    ] {
        sqlx::query(
            "INSERT INTO core.match
             (id, competition_id, season_id, round_label, home_team_id, away_team_id,
              kickoff_at, competition_timezone, status)
             VALUES ($1,$2,$3,'Round 12',$4,$5,$6,'Europe/Berlin','scheduled')",
        )
        .bind(id)
        .bind(competition_id)
        .bind(season_id)
        .bind(home)
        .bind(away)
        .bind(kickoff)
        .execute(pool)
        .await
        .unwrap();
    }

    let source_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ops.source (id, code, display_name) VALUES ($1,'dedupe-src','Dedupe Source')",
    )
    .bind(source_id)
    .execute(pool)
    .await
    .unwrap();

    // Az áldozatnál odds és meccs-szintű forrás-híd: a merge-nek át kell vinni.
    sqlx::query(
        "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count)
         VALUES ($1,$2,now(),3)",
    )
    .bind(duplicate)
    .bind(source_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO fact.odds_quote (match_id, source_id, bookmaker, market, outcome, line, phase, price)
         VALUES ($1,$2,'bookie','1X2','home',0,'opening',2.10)",
    )
    .bind(duplicate)
    .bind(source_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO identity.source_entity_ref
         (id, source_id, entity_kind, source_entity_id, entity_id, method, confidence)
         VALUES ($1,$2,'match','src-match-1',$3,'exact_external_id',1.000)",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(duplicate)
    .execute(pool)
    .await
    .unwrap();

    Seeded {
        survivor,
        duplicate,
        source_id,
    }
}

#[tokio::test]
async fn variant_groups_find_the_name_variant_pair() {
    let pool = test_pool().await;
    let seeded = seed(&pool).await;
    let date = chrono::NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();

    let groups = variant_duplicate_groups(&pool, date).await.unwrap();
    assert_eq!(groups.len(), 1, "egy variáns-csoport");
    assert_eq!(groups[0].matches.len(), 2, "a pár mindkét sora bent van");
    let names: Vec<&str> = groups[0]
        .matches
        .iter()
        .map(|m| m.home_name.as_str())
        .collect();
    assert!(names.contains(&"SV Darmstadt 98") && names.contains(&"Darmstadt"));

    assert!(
        is_variant_same_match(&pool, seeded.survivor, seeded.duplicate, false)
            .await
            .unwrap(),
        "a név-variáns pár merge-elhető"
    );
}

#[tokio::test]
async fn merge_variant_duplicate_moves_odds_and_match_ref() {
    let pool = test_pool().await;
    let seeded = seed(&pool).await;
    let date = chrono::NaiveDate::from_ymd_opt(2026, 11, 20).unwrap();
    let groups = variant_duplicate_groups(&pool, date).await.unwrap();
    assert_eq!(groups.len(), 1);

    merge_duplicate(
        &pool,
        seeded.survivor,
        seeded.duplicate,
        "S37c teszt (név-variáns)",
    )
    .await
    .unwrap();

    let matches: i64 = sqlx::query_scalar("SELECT count(*) FROM core.match")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(matches, 1, "az áldozat törölve");

    let quotes: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.odds_quote WHERE match_id=$1")
        .bind(seeded.survivor)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(quotes, 1, "az odds a túlélőhöz került");

    let (kind, entity_id): (String, Uuid) = sqlx::query_as(
        "SELECT entity_kind, entity_id FROM identity.source_entity_ref
          WHERE source_id=$1 AND source_entity_id='src-match-1'",
    )
    .bind(seeded.source_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kind, "match");
    assert_eq!(entity_id, seeded.survivor, "a meccs-híd a túlélőre mutat");

    let archived: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.dedupe_archive WHERE duplicate_id=$1 AND payload ? 'source_ref'",
    )
    .bind(seeded.duplicate)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(archived, 1, "az archívum tartalmazza a forrás-hidat is");
}
