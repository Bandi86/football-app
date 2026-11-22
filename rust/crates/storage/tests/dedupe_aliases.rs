//! S96 — cross-source csapatnév-változat duplikátumok: a finder bizonyíték-
//! alapon 1 csoportot ad, az apply a csapat-entitást és a fixture-t is mergeli.

use chrono::{TimeZone, Utc};
use football_storage::object_store::StoredArtifact;
use football_storage::repositories::dedupe::{
    AliasTeamSide, apply_cross_source_alias_group, find_cross_source_alias_duplicates,
};
use football_storage::repositories::worker::{RawCapture, persist_raw};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

struct Seed {
    anchor_team: Uuid,
    survivor_match: Uuid,
    duplicate_match: Uuid,
    survivor_entity: Uuid,
    duplicate_entity: Uuid,
}

/// Két meccs AZONOS kickoffen, versenyen, státusszal és eredménnyel; a hazai
/// oldalon két eltérő csapat-entitás (`name_a` / `name_b`), a vendég oldalon
/// közös horgony-entitás. Provenance csak a túlélő jelölt meccsen van.
async fn seed_alias_pair(pool: &PgPool, name_a: &str, name_b: &str, result_b: (i16, i16)) -> Seed {
    let country = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Hungary')")
        .bind(country)
        .execute(pool)
        .await
        .unwrap();
    let comp_entity = Uuid::now_v7();
    let comp = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','Liga A')",
    )
    .bind(comp_entity)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name) VALUES ($1,$2,$3,'Liga A')",
    )
    .bind(comp)
    .bind(comp_entity)
    .bind(country)
    .execute(pool)
    .await
    .unwrap();
    let season = Uuid::now_v7();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season)
        .bind(comp)
        .execute(pool)
        .await
        .unwrap();

    let mut teams = Vec::new();
    for name in [name_a, name_b, "Fortuna Köln"] {
        let entity = Uuid::now_v7();
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(entity)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO core.team
             (id, entity_id, country_id, name, gender, age_group, branch)
             VALUES ($1,$1,$2,$3,'male','senior','senior')",
        )
        .bind(entity)
        .bind(country)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
        teams.push(entity);
    }
    let (variant_a, variant_b, anchor) = (teams[0], teams[1], teams[2]);

    let kickoff = Utc.with_ymd_and_hms(2026, 11, 21, 18, 0, 0).unwrap();
    let survivor_match = Uuid::now_v7();
    let duplicate_match = Uuid::now_v7();
    for (id, home, result) in [
        (survivor_match, variant_a, (2i16, 1i16)),
        (duplicate_match, variant_b, result_b),
    ] {
        sqlx::query(
            "INSERT INTO core.match
             (id, competition_id, season_id, round_label, home_team_id, away_team_id,
              kickoff_at, competition_timezone, status)
             VALUES ($1,$2,$3,'Round 6',$4,$5,$6,'UTC','finished')",
        )
        .bind(id)
        .bind(comp)
        .bind(season)
        .bind(home)
        .bind(anchor)
        .bind(kickoff)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO core.match_result (match_id, home_ft, away_ft) VALUES ($1,$2,$3)")
            .bind(id)
            .bind(result.0)
            .bind(result.1)
            .execute(pool)
            .await
            .unwrap();
    }

    // Provenance-lánc (FK): source → artifact/capture → observation → provenance.
    let artifact = StoredArtifact {
        sha256: "b".repeat(64),
        storage_key: "00/00/dedupe-aliases-test.json.zst".into(),
        byte_size: 2,
        compression: "zstd",
    };
    let now = Utc::now();
    let capture = RawCapture {
        endpoint_key: "e",
        request_url: "http://example.test/",
        request_key: "dedupe-aliases-test",
        requested_at: now,
        received_at: now,
        http_status: 200,
        adapter_version: "1.0",
        parser_version: "1.0",
    };
    let (source_id, capture_id) = persist_raw(
        pool,
        "dedupe-aliases-test",
        "Dedupe Aliases Test",
        &artifact,
        &capture,
        "application/json",
    )
    .await
    .unwrap();
    let observation = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO stage.observation
         (id, source_id, capture_id, entity_kind, aspect, natural_key,
          source_record_key, observed_at, normalized_payload, schema_version,
          quality_flags, content_hash)
         VALUES ($1,$2,$3,'match','match_core','k','k',now(),'{}'::jsonb,'1.0','{}','hash-alias')",
    )
    .bind(observation)
    .bind(source_id)
    .bind(capture_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO fact.provenance
         (subject_kind, subject_id, field_key, observation_id, is_selected,
          policy_version, valid_from)
         VALUES ('match',$1,'score',$2,true,'1.0',now())",
    )
    .bind(survivor_match)
    .bind(observation)
    .execute(pool)
    .await
    .unwrap();

    Seed {
        anchor_team: anchor,
        survivor_match,
        duplicate_match,
        survivor_entity: variant_a,
        duplicate_entity: variant_b,
    }
}

#[tokio::test]
async fn alias_finder_and_apply_merge_entity_and_fixture() {
    let pool = test_pool().await;
    let seed = seed_alias_pair(&pool, "FC Cologne", "FC Koln", (2, 1)).await;
    let from = Utc
        .with_ymd_and_hms(2026, 11, 21, 0, 0, 0)
        .unwrap()
        .date_naive();

    // A szigorúbb küszöb (0.99) kizárja a valódi névváltozatot: a különbség
    // a foldolt nevekben van, nem az azonos horgony-oldalon.
    let strict = find_cross_source_alias_duplicates(&pool, from, from, 0.99)
        .await
        .unwrap();
    assert!(strict.is_empty(), "0.99 küszöb mellett nem lehet pár");

    let groups = find_cross_source_alias_duplicates(&pool, from, from, 0.55)
        .await
        .unwrap();
    assert_eq!(groups.len(), 1, "pontosan egy csoport");
    let group = &groups[0];
    assert_eq!(group.side, AliasTeamSide::Home);
    assert_eq!(group.survivor.match_id, seed.survivor_match);
    assert_eq!(group.duplicate.match_id, seed.duplicate_match);
    assert!(
        (0.55..0.99).contains(&group.similarity),
        "FC Cologne/FC Koln hasonlóság: {}",
        group.similarity
    );

    let report = apply_cross_source_alias_group(&pool, group).await.unwrap();
    assert_eq!(report.survivor_entity_id, seed.survivor_entity);
    assert_eq!(report.merged_entity_id, Some(seed.duplicate_entity));

    // Egy meccs maradt, a túlélő csapat-entitással; a horgony változatlan.
    // (A két sor azonos nem-NULL körcímkével is eltért a csapatban: a
    // fixture-merge előbb fut, ezért a csapat-merge nem ütközik az
    // uq_match_natural_key egyedi indexbe.)
    let match_count: i64 = sqlx::query_scalar("SELECT count(*) FROM core.match")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(match_count, 1);
    let (home, away): (Uuid, Uuid) =
        sqlx::query_as("SELECT home_team_id, away_team_id FROM core.match")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((home, away), (seed.survivor_entity, seed.anchor_team));

    // Az áldozat team-sora törölve, entitása merge-ölve.
    let duplicate_team_left: i64 = sqlx::query_scalar("SELECT count(*) FROM core.team WHERE id=$1")
        .bind(seed.duplicate_entity)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(duplicate_team_left, 0);
    let merged_into: Option<Uuid> =
        sqlx::query_scalar("SELECT merged_into_id FROM core.entity WHERE id=$1")
            .bind(seed.duplicate_entity)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(merged_into, Some(seed.survivor_entity));

    // A két névváltozat-csapat közül pontosan egy maradt (a horgony külön entitás).
    let variant_teams_left: i64 =
        sqlx::query_scalar("SELECT count(*) FROM core.team WHERE name IN ('FC Cologne','FC Koln')")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(variant_teams_left, 1);
    let surviving_name: String = sqlx::query_scalar("SELECT name FROM core.team WHERE id=$1")
        .bind(seed.survivor_entity)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(surviving_name, "FC Cologne");

    // Az áldozat neve reviewed aliasként rögzült a túlélőn.
    let alias: Option<String> = sqlx::query_scalar(
        "SELECT alias FROM identity.alias WHERE entity_id=$1 AND alias='FC Koln' AND status='reviewed'",
    )
    .bind(seed.survivor_entity)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(alias.as_deref(), Some("FC Koln"));

    // A bizonyíték-szöveg az entity-merge archívumban van.
    let note: String = sqlx::query_scalar(
        "SELECT note FROM ops.entity_merge_archive WHERE duplicate_entity_id=$1",
    )
    .bind(seed.duplicate_entity)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        note.contains("S96: cross-source névváltozat duplikátum (FC Cologne / FC Koln); azonos verseny+kickoff+eredmény"),
        "{note}"
    );

    // A fixture-merge archívuma és auditja is létrejött.
    let archive: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ops.dedupe_archive WHERE duplicate_id=$1")
            .bind(seed.duplicate_match)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(archive, 1);
    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.audit_event
         WHERE action='match_dedupe_merge' AND subject_kind='match' AND subject_id=$1
           AND detail->>'duplicate_id' = $2",
    )
    .bind(seed.survivor_match)
    .bind(seed.duplicate_match.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 1);

    // Újrafuttatva a finder már nem talál semmit (a merge-ölt entitás kiesik).
    let again = find_cross_source_alias_duplicates(&pool, from, from, 0.55)
        .await
        .unwrap();
    assert!(again.is_empty(), "apply után nincs újabb jelölt");
}

#[tokio::test]
async fn alias_finder_rejects_different_result() {
    let pool = test_pool().await;
    // Ugyanaz a kickoff/verseny/státusz/horgony, de eltérő eredmény: nem pár.
    let _seed = seed_alias_pair(&pool, "FC Cologne", "FC Koln", (0, 0)).await;
    let from = Utc
        .with_ymd_and_hms(2026, 11, 21, 0, 0, 0)
        .unwrap()
        .date_naive();
    let groups = find_cross_source_alias_duplicates(&pool, from, from, 0.55)
        .await
        .unwrap();
    assert!(groups.is_empty(), "eltérő eredmény nem bizonyíték");
}
