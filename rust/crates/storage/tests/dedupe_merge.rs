use chrono::{TimeZone, Utc};
use football_storage::object_store::StoredArtifact;
use football_storage::repositories::dedupe::{duplicate_groups, is_same_match, merge_duplicate};
use football_storage::repositories::worker::{RawCapture, persist_raw};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_pair(pool: &PgPool) -> (Uuid, Uuid) {
    let country_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Hungary')")
        .bind(country_id)
        .execute(pool)
        .await
        .unwrap();
    let season_id = Uuid::now_v7();
    let mut comp_ids = Vec::new();
    for name in ["Liga A", "Liga A - régi"] {
        let entity = Uuid::now_v7();
        let comp = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition',$2)",
        )
        .bind(entity)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO core.competition (id, entity_id, country_id, name) VALUES ($1,$2,$3,$4)",
        )
        .bind(comp)
        .bind(entity)
        .bind(country_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
        comp_ids.push(comp);
    }
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season_id)
        .bind(comp_ids[0])
        .execute(pool)
        .await
        .unwrap();
    let mut teams = Vec::new();
    for name in ["Újpest", "Győr"] {
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
        teams.push(entity);
    }
    // Provenance-hez valódi observation-lánc (FK): source → artifact/capture → observation.
    let artifact = StoredArtifact {
        sha256: "a".repeat(64),
        storage_key: "00/00/dedupe-test.json.zst".into(),
        byte_size: 2,
        compression: "zstd",
    };
    let now = Utc::now();
    let capture = RawCapture {
        endpoint_key: "e",
        request_url: "http://example.test/",
        request_key: "dedupe-test",
        requested_at: now,
        received_at: now,
        http_status: 200,
        adapter_version: "1.0",
        parser_version: "1.0",
    };
    let (source_id, capture_id) = persist_raw(
        pool,
        "dedupe-test",
        "Dedupe Test",
        &artifact,
        &capture,
        "application/json",
    )
    .await
    .unwrap();
    let observation = |hash: &str| {
        let id = Uuid::now_v7();
        let (pool, hash) = (pool.clone(), hash.to_string());
        async move {
            sqlx::query(
                "INSERT INTO stage.observation
                 (id, source_id, capture_id, entity_kind, aspect, natural_key,
                  source_record_key, observed_at, normalized_payload, schema_version,
                  quality_flags, content_hash)
                 VALUES ($1,$2,$3,'match','match_core','k','k',now(),'{}'::jsonb,'1.0','{}',$4)",
            )
            .bind(id)
            .bind(source_id)
            .bind(capture_id)
            .bind(hash)
            .execute(&pool)
            .await
            .unwrap();
            id
        }
    };
    let obs_a = observation("hash-a").await;
    let obs_b = observation("hash-b").await;
    let obs_c = observation("hash-c").await;

    let kickoff = Utc.with_ymd_and_hms(2026, 11, 17, 15, 0, 0).unwrap();
    let mut match_ids = Vec::new();
    for (idx, comp) in comp_ids.iter().enumerate() {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO core.match
             (id, competition_id, season_id, round_label, home_team_id, away_team_id,
              kickoff_at, competition_timezone, status)
             VALUES ($1,$2,$3,$4,$5,$6,$7,'UTC','finished')",
        )
        .bind(id)
        .bind(comp)
        .bind(season_id)
        .bind(if idx == 0 { "Round 6" } else { "League A" })
        .bind(teams[0])
        .bind(teams[1])
        .bind(kickoff)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO ops.publish_gate
             (id, match_id, publish_date, state, rule_set_version, blockers, evaluated_at)
             VALUES ($1,$2,'2026-11-17','verified_publish','verification-1.1','[]',now())",
        )
        .bind(Uuid::now_v7())
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO core.match_result (match_id, home_ft, away_ft) VALUES ($1,2,1)")
            .bind(id)
            .execute(pool)
            .await
            .unwrap();
        // provenance: közös 'score' mező + áldozatnál egyedi 'venue'
        for (field, obs) in if idx == 0 {
            vec![("score".to_string(), obs_a), ("venue".to_string(), obs_c)]
        } else {
            vec![("score".to_string(), obs_b)]
        } {
            sqlx::query(
                "INSERT INTO fact.provenance
                 (subject_kind, subject_id, field_key, observation_id, is_selected,
                  policy_version, valid_from)
                 VALUES ('match',$1,$2,$3,true,'1.0',now())",
            )
            .bind(id)
            .bind(field)
            .bind(obs)
            .execute(pool)
            .await
            .unwrap();
        }
        match_ids.push(id);
    }
    (match_ids[0], match_ids[1])
}

#[tokio::test]
async fn merge_moves_provenance_and_archives_duplicate() {
    let pool = test_pool().await;
    let (survivor, duplicate) = seed_pair(&pool).await;
    let date = chrono::NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();

    let groups = duplicate_groups(&pool, date).await.unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].matches.len(), 2);
    assert!(is_same_match(&pool, survivor, duplicate).await.unwrap());

    let audit_id = merge_duplicate(&pool, survivor, duplicate, "teszt merge")
        .await
        .unwrap();

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM core.match")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "az áldozat törölve");
    let survivor_round: String =
        sqlx::query_scalar("SELECT round_label FROM core.match WHERE id=$1")
            .bind(survivor)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(survivor_round, "Round 6", "a túlélő sora marad");

    let fields: Vec<String> = sqlx::query_scalar(
        "SELECT field_key FROM fact.provenance
          WHERE subject_kind='match' AND subject_id=$1 ORDER BY field_key",
    )
    .bind(survivor)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        fields,
        vec![
            "score".to_string(),
            "score".to_string(),
            "venue".to_string()
        ],
        "mindkét forrás score-provenance-a megmarad, a venue átkerül"
    );
    let selected: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fact.provenance
          WHERE subject_kind='match' AND subject_id=$1 AND is_selected",
    )
    .bind(survivor)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(selected, 2, "mezőnként egy selected (nincs ütközés)");

    let archived: String = sqlx::query_scalar(
        "SELECT payload->'match'->>'id' FROM ops.dedupe_archive WHERE duplicate_id=$1",
    )
    .bind(duplicate)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(archived, duplicate.to_string());
    let audit: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.audit_event WHERE id=$1")
        .bind(audit_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(audit, 1);
}
