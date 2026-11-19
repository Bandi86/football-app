//! S26 — fixture-szintű rekonciliáció tesztjei: azonos verseny+szezon+
//! csapat-pár+UTC-nap, eltérő kickoff. Auto-merge csak azonos orientációnál
//! és egyező (vagy egyik oldalon hiányzó) eredménynél.

use chrono::{TimeZone, Utc};
use football_storage::object_store::StoredArtifact;
use football_storage::repositories::dedupe::{
    duplicate_fixture_groups, is_same_fixture, merge_duplicate, same_fixture_identity,
};
use football_storage::repositories::worker::{RawCapture, persist_raw};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

/// Két kanonikus meccs ugyanazzal a verseny/szezon/csapat-párral és UTC-nappal,
/// eltérő kickoffkal. Visszatérés: (comp_id, season_id, home, away, id_a, id_b).
async fn seed_fixture_pair(
    pool: &PgPool,
    kickoff_a: chrono::DateTime<Utc>,
    kickoff_b: chrono::DateTime<Utc>,
    result_a: Option<(i16, i16)>,
    result_b: Option<(i16, i16)>,
    mirrored: bool,
) -> (Uuid, Uuid, Uuid, Uuid, Uuid, Uuid) {
    let country_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'NO','Norway')")
        .bind(country_id)
        .execute(pool)
        .await
        .unwrap();
    let entity = Uuid::now_v7();
    let comp = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','Eliteserien')")
        .bind(entity)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO core.competition (id, entity_id, country_id, name) VALUES ($1,$2,$3,'Eliteserien')")
        .bind(comp)
        .bind(entity)
        .bind(country_id)
        .execute(pool)
        .await
        .unwrap();
    let season = Uuid::now_v7();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2015')")
        .bind(season)
        .bind(comp)
        .execute(pool)
        .await
        .unwrap();
    let mut teams = Vec::new();
    for name in ["Molde", "Tromsø"] {
        let team = Uuid::now_v7();
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(team)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO core.team (id, entity_id, country_id, name, gender) VALUES ($1,$1,$2,$3,'male')")
            .bind(team)
            .bind(country_id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        teams.push(team);
    }
    let (home, away) = (teams[0], teams[1]);

    // Provenance-lánc: forrás → artifact/capture → observation.
    let artifact = StoredArtifact {
        sha256: "b".repeat(64),
        storage_key: "00/00/s26-test.json.zst".into(),
        byte_size: 2,
        compression: "zstd",
    };
    let now = Utc::now();
    let capture = RawCapture {
        endpoint_key: "e",
        request_url: "http://example.test/",
        request_key: "s26-test",
        requested_at: now,
        received_at: now,
        http_status: 200,
        adapter_version: "1.0",
        parser_version: "1.0",
    };
    let (source_id, capture_id) = persist_raw(
        pool,
        "s26-test",
        "S26 Test",
        &artifact,
        &capture,
        "application/json",
    )
    .await
    .unwrap();

    let mut ids = Vec::new();
    for (i, (kickoff, result)) in [(kickoff_a, result_a), (kickoff_b, result_b)]
        .into_iter()
        .enumerate()
    {
        let obs = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO stage.observation
             (id, source_id, capture_id, entity_kind, aspect, natural_key,
              source_record_key, observed_at, normalized_payload, schema_version,
              quality_flags, content_hash)
             VALUES ($1,$2,$3,'match','match_core',$4,$4,now(),'{}'::jsonb,'1.0','{}',$5)",
        )
        .bind(obs)
        .bind(source_id)
        .bind(capture_id)
        .bind(format!("s26-key-{i}"))
        .bind(format!("s26-hash-{i}"))
        .execute(pool)
        .await
        .unwrap();
        let m = Uuid::now_v7();
        let (h, a) = if mirrored && i == 1 {
            (away, home)
        } else {
            (home, away)
        };
        sqlx::query(
            "INSERT INTO core.match
             (id, competition_id, season_id, home_team_id, away_team_id,
              kickoff_at, competition_timezone, status, kickoff_precision)
             VALUES ($1,$2,$3,$4,$5,$6,'UTC','finished','exact')",
        )
        .bind(m)
        .bind(comp)
        .bind(season)
        .bind(h)
        .bind(a)
        .bind(kickoff)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO fact.provenance
             (subject_kind, subject_id, field_key, observation_id, is_selected, policy_version, valid_from)
             VALUES ('match',$1,'kickoff_at',$2,true,'verification-1.1',now())",
        )
        .bind(m)
        .bind(obs)
        .execute(pool)
        .await
        .unwrap();
        if let Some((hf, af)) = result {
            sqlx::query(
                "INSERT INTO core.match_result (match_id, home_ft, away_ft) VALUES ($1,$2,$3)",
            )
            .bind(m)
            .bind(hf)
            .bind(af)
            .execute(pool)
            .await
            .unwrap();
        }
        ids.push(m);
    }
    (comp, season, home, away, ids[0], ids[1])
}

#[tokio::test]
async fn same_day_different_kickoff_equal_result_is_one_group_and_merges() {
    let pool = test_pool().await;
    let a = Utc.with_ymd_and_hms(2015, 9, 19, 12, 0, 0).unwrap();
    let b = Utc.with_ymd_and_hms(2015, 9, 19, 13, 0, 0).unwrap();
    let (_comp, _season, _h, _a, id_a, id_b) =
        seed_fixture_pair(&pool, a, b, Some((1, 1)), Some((1, 1)), false).await;

    let groups = duplicate_fixture_groups(&pool, None, None, None)
        .await
        .unwrap();
    assert_eq!(groups.len(), 1, "egy fixture-csoport");
    assert_eq!(groups[0].matches.len(), 2);
    assert!(is_same_fixture(&pool, id_a, id_b).await.unwrap());

    merge_duplicate(&pool, id_a, id_b, "S26 teszt")
        .await
        .unwrap();

    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM core.match")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(left, 1, "az áldozat törölve");
    // Archívum: a payload tartalmazza az áldozat kickoffját (mindkét idő megvan).
    let archived_kickoff: String = sqlx::query_scalar(
        "SELECT payload->'match'->>'kickoff_at' FROM ops.dedupe_archive WHERE duplicate_id=$1",
    )
    .bind(id_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        archived_kickoff.starts_with("2015-09-19"),
        "{archived_kickoff}"
    );
    // Provenance átmozgatva, árva nincs.
    let prov: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fact.provenance WHERE subject_kind='match' AND subject_id=$1",
    )
    .bind(id_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(prov, 2);
    let orphan: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fact.provenance p WHERE p.subject_kind='match'
          AND NOT EXISTS (SELECT 1 FROM core.match m WHERE m.id=p.subject_id)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(orphan, 0);
    // Audit.
    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.audit_event WHERE action='match_dedupe_merge' AND subject_id=$1",
    )
    .bind(id_a)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 1);
}

#[tokio::test]
async fn differing_results_are_refused() {
    let pool = test_pool().await;
    let a = Utc.with_ymd_and_hms(2015, 10, 4, 12, 0, 0).unwrap();
    let b = Utc.with_ymd_and_hms(2015, 10, 4, 15, 0, 0).unwrap();
    let (_c, _s, _h, _a, id_a, id_b) =
        seed_fixture_pair(&pool, a, b, Some((1, 1)), Some((4, 0)), false).await;
    assert!(!is_same_fixture(&pool, id_a, id_b).await.unwrap());
    // A csoport ettől még látszik (a CLI listázza, nem merge-eli).
    let groups = duplicate_fixture_groups(&pool, None, None, None)
        .await
        .unwrap();
    assert_eq!(groups.len(), 1);
}

#[tokio::test]
async fn mirrored_orientation_is_refused_by_the_guard() {
    let pool = test_pool().await;
    let a = Utc.with_ymd_and_hms(2022, 10, 1, 12, 0, 0).unwrap();
    let b = Utc.with_ymd_and_hms(2022, 10, 1, 13, 0, 0).unwrap();
    // Tükrözött sor, orientáció-helyesen egyező eredménnyel (0-1 / 1-0).
    let (_c, _s, _h, _a, id_a, id_b) =
        seed_fixture_pair(&pool, a, b, Some((0, 1)), Some((1, 0)), true).await;
    assert!(
        !is_same_fixture(&pool, id_a, id_b).await.unwrap(),
        "tükrözött pár: kézi döntés kell"
    );
}

#[tokio::test]
async fn missing_result_on_one_side_is_mergeable() {
    let pool = test_pool().await;
    let a = Utc.with_ymd_and_hms(2016, 5, 1, 12, 0, 0).unwrap();
    let b = Utc.with_ymd_and_hms(2016, 5, 1, 14, 0, 0).unwrap();
    let (_c, _s, _h, _a, id_a, id_b) =
        seed_fixture_pair(&pool, a, b, Some((2, 0)), None, false).await;
    assert!(is_same_fixture(&pool, id_a, id_b).await.unwrap());
    merge_duplicate(&pool, id_a, id_b, "S26 teszt")
        .await
        .unwrap();
    // Az áldozat eredménye nem volt; a túlélőé megmaradt.
    let (hf, af): (i16, i16) =
        sqlx::query_as("SELECT home_ft, away_ft FROM core.match_result WHERE match_id=$1")
            .bind(id_a)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((hf, af), (2, 0));
}

#[tokio::test]
async fn different_utc_date_is_not_a_fixture_group() {
    let pool = test_pool().await;
    let a = Utc.with_ymd_and_hms(2015, 9, 19, 22, 0, 0).unwrap();
    let b = Utc.with_ymd_and_hms(2015, 9, 20, 1, 0, 0).unwrap();
    let (_c, _s, _h, _a, _id_a, _id_b) =
        seed_fixture_pair(&pool, a, b, Some((1, 1)), Some((1, 1)), false).await;
    let groups = duplicate_fixture_groups(&pool, None, None, None)
        .await
        .unwrap();
    assert!(groups.is_empty(), "más UTC-nap: nem fixture-csoport");
}

#[tokio::test]
async fn date_and_competition_filters_apply() {
    let pool = test_pool().await;
    let a = Utc.with_ymd_and_hms(2015, 9, 19, 12, 0, 0).unwrap();
    let b = Utc.with_ymd_and_hms(2015, 9, 19, 13, 0, 0).unwrap();
    seed_fixture_pair(&pool, a, b, Some((1, 1)), Some((1, 1)), false).await;
    let hit = duplicate_fixture_groups(
        &pool,
        Some(a.date_naive()),
        Some(a.date_naive()),
        Some("Eliteserien"),
    )
    .await
    .unwrap();
    assert_eq!(hit.len(), 1);
    let miss = duplicate_fixture_groups(&pool, None, None, Some("Premier League"))
        .await
        .unwrap();
    assert!(miss.is_empty());
    let miss_date = duplicate_fixture_groups(
        &pool,
        Some(
            Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0)
                .unwrap()
                .date_naive(),
        ),
        None,
        None,
    )
    .await
    .unwrap();
    assert!(miss_date.is_empty());
}

#[tokio::test]
async fn reviewed_identity_match_ignores_result_disagreement() {
    let pool = test_pool().await;
    let a = Utc.with_ymd_and_hms(2015, 10, 4, 12, 0, 0).unwrap();
    let b = Utc.with_ymd_and_hms(2015, 10, 4, 15, 0, 0).unwrap();
    let (_c, _s, _h, _a, id_a, id_b) =
        seed_fixture_pair(&pool, a, b, Some((1, 1)), Some((4, 0)), false).await;
    // A normál őr elutasítja az eltérő eredményű párt...
    assert!(!is_same_fixture(&pool, id_a, id_b).await.unwrap());
    // ...a felülvizsgált azonosság-őr viszont elfogadja (a CLI --review-note
    // útja, ahol a túlélő eredményét külső bizonyíték igazolta).
    assert!(same_fixture_identity(&pool, id_a, id_b).await.unwrap());
}
