//! S42 integrációs teszt: `footballctl competitions repair-countries`.
//!
//! Valódi CLI-folyamatot indít a teszt-adatbázison (TEST_DATABASE_URL), ezért
//! `--test-threads=1` kötelező. A seed a storage `test_support::test_pool`
//! mintáját követi: ország → verseny → szezon → csapatok → forrás/megfigyelés →
//! kiválasztott provenance → meccsek.
//!
//! Forgatókönyv: a GB „Premier League" alatt két angol és egy kínai
//! megfigyelés van. Dry-run: 1 verseny-ország változás (GB → GB-ENG) és 1
//! meccs-átsorolás (CN / Chinese FA Cup), írás nélkül. Apply: az ENG ország-sor
//! GB-ENG-re kódolódik (név marad „Anglia"), a verseny GB-ENG alá kerül, a kínai
//! meccs a CN / Chinese FA Cup új versenyébe és szezonjába sorolódik, és három
//! `competition_country_repair` audit-sor keletkezik.

use std::process::Command;

use chrono::{DateTime, TimeZone, Utc};
use football_storage::test_support::test_pool;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

/// A CLI kimenete + a gyerek adatkönyvtára (takarítás a teszt végén).
struct CliRun {
    stdout: String,
    success: bool,
}

fn run_cli(args: &[&str]) -> CliRun {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL kötelező az integrációs teszthez");
    let data_dir = std::env::temp_dir().join(format!("footballctl-s42-{}", Uuid::now_v7()));
    let output = Command::new(env!("CARGO_BIN_EXE_footballctl"))
        .args(args)
        .env("DATABASE_URL", &database_url)
        .env("DATA_DIR", &data_dir)
        .output()
        .expect("a footballctl futtatása");
    let _ = std::fs::remove_dir_all(&data_dir);
    CliRun {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        success: output.status.success(),
    }
}

async fn seed_country(pool: &PgPool, code: &str, name: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,$2,$3)")
        .bind(id)
        .bind(code)
        .bind(name)
        .execute(pool)
        .await
        .expect("ország");
    id
}

async fn seed_entity(pool: &PgPool, kind: &str, name: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,$2,$3)")
        .bind(id)
        .bind(kind)
        .bind(name)
        .execute(pool)
        .await
        .expect("entitás");
    id
}

async fn seed_team(pool: &PgPool, name: &str, country_id: Uuid) -> Uuid {
    let entity_id = seed_entity(pool, "team", name).await;
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.team (id, entity_id, country_id, name, gender)
         VALUES ($1,$2,$3,$4,'male')",
    )
    .bind(id)
    .bind(entity_id)
    .bind(country_id)
    .bind(name)
    .execute(pool)
    .await
    .expect("csapat");
    id
}

struct Seed {
    competition_id: Uuid,
    chinese_match_id: Uuid,
    english_match_ids: Vec<Uuid>,
}

const KICKOFF: &str = "2026-11-20T15:00:00Z";

fn kickoff() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 20, 15, 0, 0).unwrap()
}

/// Seedel egy meccset a megadott versenybe/szezonba, és kiválasztott
/// kickoff-provenance-szal összeköt egy megfigyelést.
#[allow(clippy::too_many_arguments)]
async fn seed_match(
    pool: &PgPool,
    source_id: Uuid,
    capture_id: Uuid,
    competition_id: Uuid,
    season_id: Uuid,
    home: Uuid,
    away: Uuid,
    competition_name: &str,
    country_code: &str,
    country_name: &str,
) -> Uuid {
    let match_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, round_label, home_team_id, away_team_id,
          kickoff_at, competition_timezone, status)
         VALUES ($1,$2,$3,NULL,$4,$5,$6,'Europe/Budapest','scheduled')",
    )
    .bind(match_id)
    .bind(competition_id)
    .bind(season_id)
    .bind(home)
    .bind(away)
    .bind(kickoff())
    .execute(pool)
    .await
    .expect("meccs");
    let observation_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO stage.observation
         (id, source_id, capture_id, entity_kind, aspect, natural_key, source_record_key,
          observed_at, event_time, normalized_payload, schema_version, content_hash)
         VALUES ($1,$2,$3,'match','match',$4,$5,now(),$6,$7,'1',$8)",
    )
    .bind(observation_id)
    .bind(source_id)
    .bind(capture_id)
    .bind(match_id.to_string())
    .bind(format!("rec-{}", Uuid::now_v7()))
    .bind(kickoff())
    .bind(json!({
        "competition": {
            "name": competition_name,
            "country_code": country_code,
            "country_name": country_name,
            "tier": 1,
            "priority": 50,
        },
        "season": {"label": "2026/27"},
    }))
    .bind(format!("hash-{}", Uuid::now_v7()))
    .execute(pool)
    .await
    .expect("megfigyelés");
    sqlx::query(
        "INSERT INTO fact.provenance
         (subject_kind, subject_id, field_key, observation_id, is_selected,
          policy_version, valid_from)
         VALUES ('match',$1,'kickoff_at',$2,true,'1.0',now())",
    )
    .bind(match_id)
    .bind(observation_id)
    .execute(pool)
    .await
    .expect("provenance");
    match_id
}

async fn seed(pool: &PgPool) -> Seed {
    let gb = seed_country(pool, "GB", "Egyesült Királyság").await;
    let eng = seed_country(pool, "ENG", "Anglia").await;
    let cn = seed_country(pool, "CN", "Kína").await;

    let source_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ops.source (id, code, display_name) VALUES ($1,'soccerway','Soccerway')",
    )
    .bind(source_id)
    .execute(pool)
    .await
    .expect("forrás");
    let artifact_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO raw.artifact (id, content_sha256, compression, byte_size, storage_key, media_type)
         VALUES ($1,$2,'none',1,$3,'application/json')",
    )
    .bind(artifact_id)
    .bind(format!("sha-{}", Uuid::now_v7()))
    .bind(format!("key-{}", Uuid::now_v7()))
    .execute(pool)
    .await
    .expect("artifact");
    let capture_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO raw.capture
         (id, source_id, artifact_id, endpoint_key, request_url, requested_at, received_at,
          http_status, adapter_version, parser_version, request_key)
         VALUES ($1,$2,$3,'matches','https://example.invalid',now(),now(),200,'1','1',$4)",
    )
    .bind(capture_id)
    .bind(source_id)
    .bind(artifact_id)
    .bind(format!("req-{}", Uuid::now_v7()))
    .execute(pool)
    .await
    .expect("capture");

    let competition_entity_id = seed_entity(pool, "competition", "Premier League").await;
    let competition_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name, tier, priority)
         VALUES ($1,$2,$3,'Premier League',1,50)",
    )
    .bind(competition_id)
    .bind(competition_entity_id)
    .bind(gb)
    .execute(pool)
    .await
    .expect("verseny");
    let season_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season_id)
        .bind(competition_id)
        .execute(pool)
        .await
        .expect("szezon");

    let mut english_match_ids = Vec::new();
    for (home, away) in [("Arsenal", "Chelsea"), ("Liverpool", "Everton")] {
        let home_id = seed_team(pool, home, eng).await;
        let away_id = seed_team(pool, away, eng).await;
        english_match_ids.push(
            seed_match(
                pool,
                source_id,
                capture_id,
                competition_id,
                season_id,
                home_id,
                away_id,
                "Premier League",
                "GB",
                "England",
            )
            .await,
        );
    }
    let chinese_match_id = seed_match(
        pool,
        source_id,
        capture_id,
        competition_id,
        season_id,
        seed_team(pool, "Beijing Guoan", cn).await,
        seed_team(pool, "Shanghai Port", cn).await,
        "FA Cup",
        "CN",
        "China",
    )
    .await;

    Seed {
        competition_id,
        chinese_match_id,
        english_match_ids,
    }
}

async fn competition_country_code(pool: &PgPool, competition_id: Uuid) -> Option<String> {
    sqlx::query_scalar(
        "SELECT cc.code FROM core.competition c
           LEFT JOIN core.country cc ON cc.id = c.country_id
          WHERE c.id = $1",
    )
    .bind(competition_id)
    .fetch_one(pool)
    .await
    .expect("verseny országa")
}

async fn match_competition(pool: &PgPool, match_id: Uuid) -> (Uuid, Uuid) {
    sqlx::query_as("SELECT competition_id, season_id FROM core.match WHERE id = $1")
        .bind(match_id)
        .fetch_one(pool)
        .await
        .expect("meccs versenye")
}

#[tokio::test]
async fn dry_run_reports_and_apply_repairs_in_one_transaction() {
    let pool = test_pool().await;
    let seed = seed(&pool).await;
    let args = [
        "competitions",
        "repair-countries",
        "--from",
        "2026-11-20",
        "--to",
        "2026-11-20",
    ];

    // 1) Dry-run: riport, de semmi írás.
    let dry = run_cli(&args);
    assert!(dry.success, "dry-run nem futott le");
    assert!(
        dry.stdout.contains("verseny-ország változna: 1"),
        "várt riport-sor hiányzik:\n{}",
        dry.stdout
    );
    assert!(
        dry.stdout.contains("átsorolódó meccs: 1"),
        "várt átsorolás-szám hiányzik:\n{}",
        dry.stdout
    );
    assert!(
        dry.stdout.contains("Chinese FA Cup"),
        "cél-verseny hiányzik a riportból"
    );
    assert!(
        dry.stdout.contains("nem történt írás"),
        "a dry-run nem jelzi az írás hiányát"
    );
    assert_eq!(
        competition_country_code(&pool, seed.competition_id)
            .await
            .as_deref(),
        Some("GB"),
        "a dry-run nem írhat"
    );
    assert_eq!(
        match_competition(&pool, seed.chinese_match_id).await.0,
        seed.competition_id,
        "a dry-run nem sorolhat át"
    );

    // 2) Apply: egy tranzakcióban ír.
    let applied = run_cli(&[args.as_slice(), &["--apply"]].concat());
    assert!(applied.success, "apply nem futott le:\n{}", applied.stdout);

    // Az ENG ország-sor GB-ENG-re kódolódott, a neve maradt.
    let eng: (String, String) =
        sqlx::query_as("SELECT code, name FROM core.country WHERE code = 'GB-ENG'")
            .fetch_one(&pool)
            .await
            .expect("GB-ENG ország-sor");
    assert_eq!(eng.0, "GB-ENG");
    assert_eq!(eng.1, "Anglia", "a meglévő név marad");
    assert_eq!(
        competition_country_code(&pool, seed.competition_id)
            .await
            .as_deref(),
        Some("GB-ENG")
    );

    // A kínai meccs a CN / Chinese FA Cup új versenyébe és szezonjába került.
    let (target_competition, target_season) = match_competition(&pool, seed.chinese_match_id).await;
    assert_ne!(target_competition, seed.competition_id);
    assert_ne!(target_season, competition_id_for_season(&pool).await);
    let (country, name): (String, String) = sqlx::query_as(
        "SELECT cc.code, c.name FROM core.competition c
           JOIN core.country cc ON cc.id = c.country_id WHERE c.id = $1",
    )
    .bind(target_competition)
    .fetch_one(&pool)
    .await
    .expect("cél-verseny");
    assert_eq!(country, "CN");
    assert_eq!(name, "Chinese FA Cup");
    let label: String = sqlx::query_scalar("SELECT label FROM core.season WHERE id = $1")
        .bind(target_season)
        .fetch_one(&pool)
        .await
        .expect("cél-szezon");
    assert_eq!(label, "2026/27", "a szezon címkéje marad");

    // Az angol meccsek a helyükön maradtak.
    for match_id in &seed.english_match_ids {
        assert_eq!(
            match_competition(&pool, *match_id).await.0,
            seed.competition_id,
            "az angol meccs nem sorolódhat át"
        );
    }

    // Audit: ország-átkódolás + verseny + meccs-átsorolás.
    let audits: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT subject_kind, subject_id FROM ops.audit_event
          WHERE action = 'competition_country_repair' AND actor = 'cli'",
    )
    .fetch_all(&pool)
    .await
    .expect("audit");
    assert_eq!(audits.len(), 3, "várt audit-sorok: {audits:?}");
    assert!(
        audits
            .iter()
            .any(|(kind, id)| kind == "match" && *id == seed.chinese_match_id),
        "a meccs-audit hiányzik"
    );
    assert!(
        audits
            .iter()
            .any(|(kind, id)| kind == "competition" && *id == seed.competition_id),
        "a verseny-audit hiányzik"
    );
    assert!(
        audits.iter().any(|(kind, _)| kind == "country"),
        "az ország-átkódolás auditja hiányzik"
    );
}

/// A verseny eredeti szezonja (a meccs átállítása után is létezik).
async fn competition_id_for_season(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        "SELECT s.id FROM core.season s
           JOIN core.competition c ON c.id = s.competition_id
          WHERE c.name = 'Premier League' LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("eredeti szezon")
}

const _: () = {
    // A KICKOFF konstans dokumentációs célú; a kickoff() a mérvadó.
    let _ = KICKOFF;
};
