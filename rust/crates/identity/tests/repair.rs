//! S41 — identitás-javítás integrációs tesztjei (szintetikus seed).
//!
//! Lefedett viselkedés:
//! - `approve_create` a csapatnévből vezeti le az age_group/gender/branch
//!   jelölőt, ezért U17-es névnél nem hasznosítja újra a senior csapatot;
//!   a kontextussal való ellentmondás a review-auditban megjelenik;
//! - `rebuild_source_refs` dry-runban nem ír, apply-ban a legfrissebb
//!   `team_identity` observation szerinti csapatra állítja a rossz refet;
//! - `repair_soccerway_matches` a capture-scope-olt nevekből javítja a
//!   felcserélt meccset, duplikátumnál merge-el, feloldhatatlan esetben
//!   `withheld` publish_gate-et ír a budapesti naptári napra;
//! - minden írás auditálva van (`identity_repair_*`).
//!
//! A tesztek a `football_v2_test` adatbázist használják a
//! `football_storage::test_support::test_pool()` segéden át, ezért
//! `--test-threads=1` szükséges.

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use football_identity::repair::{rebuild_source_refs, repair_soccerway_matches};
use football_identity::review;
use football_storage::test_support::test_pool;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Seed segédek
// ---------------------------------------------------------------------------

async fn seed_source(pool: &PgPool, code: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,$2,'Teszt forrás')")
        .bind(id)
        .bind(code)
        .execute(pool)
        .await
        .unwrap();
    id
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

async fn seed_team(
    pool: &PgPool,
    name: &str,
    country_id: Option<Uuid>,
    gender: &str,
    age_group: &str,
    branch: &str,
) -> Uuid {
    let entity_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
        .bind(entity_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(entity_id)
    .bind(entity_id)
    .bind(country_id)
    .bind(name)
    .bind(gender)
    .bind(age_group)
    .bind(branch)
    .execute(pool)
    .await
    .unwrap();
    entity_id
}

async fn seed_ref(pool: &PgPool, source_id: Uuid, code: &str, entity_id: Uuid) {
    sqlx::query(
        "INSERT INTO identity.source_entity_ref
         (id, source_id, entity_kind, source_entity_id, entity_id, method,
          confidence, status, reviewed_by)
         VALUES ($1,$2,'team',$3,$4,'manual_review',1.0,'active','bootstrap')",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(code)
    .bind(entity_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_queue(
    pool: &PgPool,
    source_id: Uuid,
    ext: &str,
    proposed_name: &str,
    context: Value,
) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO identity.review_queue
         (id, source_id, entity_kind, source_entity_id, candidate_entity_ids,
          reason, proposed_name, context, status)
         VALUES ($1,$2,'team',$3,'[]','no_exact_match',$4,$5,'pending')",
    )
    .bind(id)
    .bind(source_id)
    .bind(ext)
    .bind(proposed_name)
    .bind(context)
    .execute(pool)
    .await
    .unwrap();
    id
}

/// Egy capture (artifact + capture) — a capture-scope-olt igazság alapja.
async fn seed_capture(pool: &PgPool, source_id: Uuid) -> Uuid {
    let artifact_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO raw.artifact
         (id, content_sha256, compression, byte_size, storage_key, media_type)
         VALUES ($1,$2,'none',0,$3,'application/json')",
    )
    .bind(artifact_id)
    .bind(format!("sha-{artifact_id}"))
    .bind(format!("key-{artifact_id}"))
    .execute(pool)
    .await
    .unwrap();

    let capture_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO raw.capture
         (id, source_id, artifact_id, endpoint_key, request_url, requested_at,
          received_at, http_status, adapter_version, parser_version, request_key)
         VALUES ($1,$2,$3,'matches','https://example.test/capture',$4,$4,200,
                 'test-adapter','test-parser',$5)",
    )
    .bind(capture_id)
    .bind(source_id)
    .bind(artifact_id)
    .bind(Utc::now())
    .bind(format!("req-{capture_id}"))
    .execute(pool)
    .await
    .unwrap();
    capture_id
}

#[allow(clippy::too_many_arguments)]
async fn seed_team_obs(
    pool: &PgPool,
    source_id: Uuid,
    capture_id: Uuid,
    code: &str,
    name: &str,
    country_code: &str,
    gender: &str,
    age_group: &str,
    branch: &str,
    observed_at: DateTime<Utc>,
) -> Uuid {
    let id = Uuid::now_v7();
    let payload = json!({
        "source_entity_id": code,
        "name": name,
        "country_code": country_code,
        "gender": gender,
        "age_group": age_group,
        "branch": branch,
    });
    sqlx::query(
        "INSERT INTO stage.observation
         (id, source_id, capture_id, entity_kind, aspect, natural_key, source_record_key,
          observed_at, normalized_payload, schema_version, quality_flags, content_hash)
         VALUES ($1,$2,$3,'team','team_identity',$4,$4,$5,$6,'1.0','{}',$7)",
    )
    .bind(id)
    .bind(source_id)
    .bind(capture_id)
    .bind(code)
    .bind(observed_at)
    .bind(&payload)
    .bind(format!("hash-{id}"))
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn seed_match_obs(
    pool: &PgPool,
    source_id: Uuid,
    capture_id: Uuid,
    record_key: &str,
    home_code: &str,
    away_code: &str,
    kickoff_at: DateTime<Utc>,
) -> Uuid {
    let id = Uuid::now_v7();
    let payload = json!({
        "source_home_id": home_code,
        "source_away_id": away_code,
        "kickoff_at": kickoff_at.to_rfc3339(),
        "status": "scheduled",
        "competition_timezone": "Europe/Budapest",
    });
    sqlx::query(
        "INSERT INTO stage.observation
         (id, source_id, capture_id, entity_kind, aspect, natural_key, source_record_key,
          observed_at, normalized_payload, schema_version, quality_flags, content_hash)
         VALUES ($1,$2,$3,'match','match_core',$4,$4,$5,$6,'1.0','{}',$7)",
    )
    .bind(id)
    .bind(source_id)
    .bind(capture_id)
    .bind(record_key)
    .bind(kickoff_at)
    .bind(&payload)
    .bind(format!("hash-{id}"))
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn seed_competition(pool: &PgPool, country_id: Option<Uuid>) -> (Uuid, Uuid) {
    let competition_entity_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','Teszt Kupa')",
    )
    .bind(competition_entity_id)
    .execute(pool)
    .await
    .unwrap();
    let competition_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name)
         VALUES ($1,$2,$3,'Teszt Kupa')",
    )
    .bind(competition_id)
    .bind(competition_entity_id)
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
    (competition_id, season_id)
}

async fn seed_match(
    pool: &PgPool,
    competition_id: Uuid,
    season_id: Uuid,
    home_team_id: Uuid,
    away_team_id: Uuid,
    kickoff_at: DateTime<Utc>,
) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, round_label, home_team_id, away_team_id,
          kickoff_at, competition_timezone, status)
         VALUES ($1,$2,$3,NULL,$4,$5,$6,'Europe/Budapest','scheduled')",
    )
    .bind(id)
    .bind(competition_id)
    .bind(season_id)
    .bind(home_team_id)
    .bind(away_team_id)
    .bind(kickoff_at)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn select_provenance(pool: &PgPool, match_id: Uuid, observation_id: Uuid, field_key: &str) {
    sqlx::query(
        "INSERT INTO fact.provenance
         (subject_kind, subject_id, field_key, observation_id, is_selected,
          policy_version, valid_from)
         VALUES ('match',$1,$2,$3,true,'test-1.0',now())",
    )
    .bind(match_id)
    .bind(field_key)
    .bind(observation_id)
    .execute(pool)
    .await
    .unwrap();
}

// ---------------------------------------------------------------------------
// Olvasó segédek az állításokhoz
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct TeamSnapshot {
    canonical_name: String,
    name: String,
    gender: String,
    age_group: String,
    branch: String,
    country_code: Option<String>,
}

async fn team_snapshot(pool: &PgPool, entity_id: Uuid) -> TeamSnapshot {
    let row: (String, String, String, String, String, Option<String>) = sqlx::query_as(
        "SELECT e.canonical_name, t.name, t.gender, t.age_group, t.branch, c.code
           FROM core.entity e
           JOIN core.team t ON t.entity_id = e.id
           LEFT JOIN core.country c ON c.id = t.country_id
          WHERE e.id = $1",
    )
    .bind(entity_id)
    .fetch_one(pool)
    .await
    .unwrap();
    TeamSnapshot {
        canonical_name: row.0,
        name: row.1,
        gender: row.2,
        age_group: row.3,
        branch: row.4,
        country_code: row.5,
    }
}

async fn active_ref(pool: &PgPool, source_id: Uuid, code: &str) -> Option<Uuid> {
    sqlx::query_scalar(
        "SELECT entity_id FROM identity.source_entity_ref
          WHERE source_id = $1 AND entity_kind = 'team'
            AND source_entity_id = $2 AND status = 'active'",
    )
    .bind(source_id)
    .bind(code)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn entity_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM core.entity WHERE kind = 'team'")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn audit_count(pool: &PgPool, action: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM ops.audit_event WHERE action = $1")
        .bind(action)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn audit_detail(pool: &PgPool, action: &str) -> Value {
    let detail: Option<Value> =
        sqlx::query_scalar("SELECT detail FROM ops.audit_event WHERE action = $1 LIMIT 1")
            .bind(action)
            .fetch_optional(pool)
            .await
            .unwrap();
    detail.unwrap_or(Value::Null)
}

async fn match_teams(pool: &PgPool, match_id: Uuid) -> Option<(Uuid, Uuid)> {
    sqlx::query_as("SELECT home_team_id, away_team_id FROM core.match WHERE id = $1")
        .bind(match_id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn match_exists(pool: &PgPool, match_id: Uuid) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT count(*) FROM core.match WHERE id = $1")
        .bind(match_id)
        .fetch_one(pool)
        .await
        .unwrap()
        == 1
}

async fn gate_row(pool: &PgPool, match_id: Uuid) -> Option<(String, String, Value, NaiveDate)> {
    sqlx::query_as(
        "SELECT state, rule_set_version, blockers, publish_date
           FROM ops.publish_gate WHERE match_id = $1",
    )
    .bind(match_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

fn day(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

fn kickoff(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, 0)
        .unwrap()
}

// ---------------------------------------------------------------------------
// 1) approve_create: a névből levezetett kontextus
// ---------------------------------------------------------------------------

#[tokio::test]
async fn approve_create_makes_new_youth_team_instead_of_reusing_senior() {
    let pool = test_pool().await;
    let source = seed_source(&pool, "soccerway").await;
    let bahrain = seed_country(&pool, "BH", "Bahrain").await;
    let senior = seed_team(&pool, "Bahrain", Some(bahrain), "male", "senior", "senior").await;

    // A review-sor kontextusából hiányzik az age_group (ez okozta az eredeti
    // hibát); a neve viszont hordozza az U17 jelölőt.
    let queue = seed_queue(
        &pool,
        source,
        "YBHnufXf",
        "Bahrain U17",
        json!({"country_code": "BH", "gender": "male"}),
    )
    .await;

    let created = review::approve_create(&pool, queue, "teszt", Some("S41 teszt"))
        .await
        .unwrap();

    assert_ne!(
        created, senior,
        "a senior csapatot nem szabad újrahasznosítani"
    );
    let team = team_snapshot(&pool, created).await;
    assert_eq!(team.canonical_name, "Bahrain U17");
    assert_eq!(team.name, "Bahrain U17");
    assert_eq!(team.gender, "male");
    assert_eq!(
        team.age_group, "youth",
        "a névből levezetett korosztály nyer"
    );
    assert_eq!(team.branch, "youth");
    assert_eq!(team.country_code.as_deref(), Some("BH"));

    assert_eq!(active_ref(&pool, source, "YBHnufXf").await, Some(created));
    // A senior csapat érintetlen.
    let untouched = team_snapshot(&pool, senior).await;
    assert_eq!(untouched.canonical_name, "Bahrain");
    assert_eq!(untouched.age_group, "senior");
    assert_eq!(audit_count(&pool, "identity_approve_create").await, 1);
}

#[tokio::test]
async fn approve_create_name_marker_overrides_conflicting_review_context_and_audits_it() {
    let pool = test_pool().await;
    let source = seed_source(&pool, "soccerway").await;
    let bahrain = seed_country(&pool, "BH", "Bahrain").await;
    // Korábbi, hibásan senior kontextussal létrehozott U17 entitás.
    let legacy = seed_team(
        &pool,
        "Bahrain U17",
        Some(bahrain),
        "male",
        "senior",
        "senior",
    )
    .await;

    // A review-sor kontextusa seniornak mondja a csapatot — a név nyer.
    let queue = seed_queue(
        &pool,
        source,
        "YBHnufXf",
        "Bahrain U17",
        json!({
            "country_code": "BH",
            "gender": "male",
            "age_group": "senior",
            "branch": "senior",
        }),
    )
    .await;

    let created = review::approve_create(&pool, queue, "teszt", Some("S41 konfliktus"))
        .await
        .unwrap();
    assert_ne!(
        created, legacy,
        "a senior kontextusú U17 sort nem hasznosítjuk újra"
    );

    let team = team_snapshot(&pool, created).await;
    assert_eq!(team.age_group, "youth");
    assert_eq!(team.branch, "youth");

    let detail = audit_detail(&pool, "identity_approve_create").await;
    let adjustments = detail["context_adjustments"]
        .as_array()
        .expect("context_adjustments tömb");
    assert!(
        adjustments.iter().any(|note| note
            .as_str()
            .unwrap_or("")
            .contains("age_group: senior → youth")),
        "az auditban meg kell jelennie a korosztály-korrekciónak: {detail}"
    );
    assert!(
        adjustments.iter().any(|note| note
            .as_str()
            .unwrap_or("")
            .contains("branch: senior → youth")),
        "az auditban meg kell jelennie a branch-korrekciónak: {detail}"
    );
}

// ---------------------------------------------------------------------------
// 2) rebuild_source_refs
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rebuild_dry_run_writes_nothing_then_apply_relinks_to_latest_observation() {
    let pool = test_pool().await;
    let source = seed_source(&pool, "soccerway").await;
    let capture = seed_capture(&pool, source).await;
    let bahrain = seed_country(&pool, "BH", "Bahrain").await;
    let morocco = seed_country(&pool, "MA", "Morocco").await;
    let niger = seed_country(&pool, "NE", "Niger").await;
    let jordan = seed_country(&pool, "JO", "Jordan").await;

    let senior = seed_team(&pool, "Bahrain", Some(bahrain), "male", "senior", "senior").await;
    let u17 = seed_team(
        &pool,
        "Bahrain U17",
        Some(bahrain),
        "male",
        "youth",
        "youth",
    )
    .await;
    let morocco_team = seed_team(&pool, "Morocco", Some(morocco), "male", "senior", "senior").await;
    let niger_team = seed_team(&pool, "Niger", Some(niger), "male", "senior", "senior").await;
    let jordan_team = seed_team(&pool, "Jordan", Some(jordan), "male", "senior", "senior").await;

    // A négy kód: régen rossz ref, újabb observation a helyes névvel.
    seed_ref(&pool, source, "YBHnufXf", senior).await;
    seed_ref(&pool, source, "IDKYO3R8", morocco_team).await;
    seed_ref(&pool, source, "NOOBS01", jordan_team).await;
    seed_ref(&pool, source, "JO1", jordan_team).await;

    // Ugyanarra a kódra két observation: a frissebb "Bahrain U17" nyer.
    seed_team_obs(
        &pool,
        source,
        capture,
        "YBHnufXf",
        "Bahrain",
        "BH",
        "male",
        "senior",
        "senior",
        kickoff(2026, 11, 17, 10, 0),
    )
    .await;
    seed_team_obs(
        &pool,
        source,
        capture,
        "YBHnufXf",
        "Bahrain U17",
        "BH",
        "male",
        "youth",
        "youth",
        kickoff(2026, 11, 20, 10, 0),
    )
    .await;
    seed_team_obs(
        &pool,
        source,
        capture,
        "IDKYO3R8",
        "Niger",
        "NE",
        "male",
        "senior",
        "senior",
        kickoff(2026, 11, 18, 10, 0),
    )
    .await;
    seed_team_obs(
        &pool,
        source,
        capture,
        "JO1",
        "Jordan",
        "JO",
        "male",
        "senior",
        "senior",
        kickoff(2026, 11, 18, 10, 0),
    )
    .await;

    let entities_before = entity_count(&pool).await;

    // --- dry-run: semmi nem íródik ---
    let dry = rebuild_source_refs(&pool, "soccerway", true).await.unwrap();
    assert_eq!(dry.source, "soccerway");
    assert_eq!(dry.codes_seen, 4);
    assert_eq!(dry.relinked, 2);
    assert_eq!(dry.unchanged, 1);
    assert_eq!(dry.no_obs, 1);
    assert_eq!(dry.teams_created, 0);
    assert_eq!(dry.ambiguous, 0);
    assert_eq!(dry.changes.len(), 2);
    assert!(dry.changes.iter().any(|(code, old, new)| code == "YBHnufXf"
        && old.as_deref() == Some("Bahrain")
        && new == "Bahrain U17"));
    assert!(dry.changes.iter().any(|(code, old, new)| code == "IDKYO3R8"
        && old.as_deref() == Some("Morocco")
        && new == "Niger"));

    assert_eq!(active_ref(&pool, source, "YBHnufXf").await, Some(senior));
    assert_eq!(
        active_ref(&pool, source, "IDKYO3R8").await,
        Some(morocco_team)
    );
    assert_eq!(entity_count(&pool).await, entities_before);
    assert_eq!(audit_count(&pool, "identity_repair_source_ref").await, 0);

    // --- apply: a ref a legfrissebb observation csapatára áll ---
    let applied = rebuild_source_refs(&pool, "soccerway", false)
        .await
        .unwrap();
    assert_eq!(applied.codes_seen, 4);
    assert_eq!(applied.relinked, 2);
    assert_eq!(applied.unchanged, 1);
    assert_eq!(applied.no_obs, 1);
    assert_eq!(applied.teams_created, 0);

    assert_eq!(active_ref(&pool, source, "YBHnufXf").await, Some(u17));
    assert_eq!(
        active_ref(&pool, source, "IDKYO3R8").await,
        Some(niger_team)
    );
    // A no_obs kód refje békén marad.
    assert_eq!(
        active_ref(&pool, source, "NOOBS01").await,
        Some(jordan_team)
    );
    assert_eq!(active_ref(&pool, source, "JO1").await, Some(jordan_team));
    assert_eq!(audit_count(&pool, "identity_repair_source_ref").await, 2);

    let detail = audit_detail(&pool, "identity_repair_source_ref").await;
    assert!(detail["observation_id"].as_str().is_some());
    assert_eq!(detail["source"], "soccerway");

    // --- ismételt apply: idempotens ---
    let again = rebuild_source_refs(&pool, "soccerway", false)
        .await
        .unwrap();
    assert_eq!(again.relinked, 0);
    assert_eq!(again.unchanged, 3);
    assert_eq!(again.no_obs, 1);
}

#[tokio::test]
async fn rebuild_creates_missing_team_with_derived_context_and_audits_every_write() {
    let pool = test_pool().await;
    let source = seed_source(&pool, "soccerway").await;
    let capture = seed_capture(&pool, source).await;
    seed_team_obs(
        &pool,
        source,
        capture,
        "NEWU17",
        "Singapore U17",
        "SG",
        "male",
        "youth",
        "youth",
        kickoff(2026, 11, 21, 9, 0),
    )
    .await;

    let dry = rebuild_source_refs(&pool, "soccerway", true).await.unwrap();
    assert_eq!(dry.teams_created, 1);
    assert_eq!(dry.relinked, 1);
    assert_eq!(dry.changes[0].0, "NEWU17");
    assert_eq!(dry.changes[0].1, None);
    assert_eq!(dry.changes[0].2, "Singapore U17");
    assert_eq!(entity_count(&pool).await, 0);

    let applied = rebuild_source_refs(&pool, "soccerway", false)
        .await
        .unwrap();
    assert_eq!(applied.teams_created, 1);
    assert_eq!(applied.relinked, 1);

    let entity_id = active_ref(&pool, source, "NEWU17")
        .await
        .expect("az új csapatra mutató aktív ref");
    let team = team_snapshot(&pool, entity_id).await;
    assert_eq!(team.canonical_name, "Singapore U17");
    assert_eq!(team.gender, "male");
    assert_eq!(team.age_group, "youth");
    assert_eq!(team.branch, "youth");
    assert_eq!(team.country_code.as_deref(), Some("SG"));

    // Minden írás auditálva: csapat-létrehozás + ref-átállítás.
    assert_eq!(audit_count(&pool, "identity_repair_team_create").await, 1);
    assert_eq!(audit_count(&pool, "identity_repair_source_ref").await, 1);
    let created = audit_detail(&pool, "identity_repair_team_create").await;
    assert_eq!(created["age_group"], "youth");
}

// ---------------------------------------------------------------------------
// 3) repair_soccerway_matches — felcserélt meccs
// ---------------------------------------------------------------------------

async fn swapped_match_fixture(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid, Uuid) {
    let source = seed_source(pool, "soccerway").await;
    let capture = seed_capture(pool, source).await;
    let hungary = seed_country(pool, "HU", "Hungary").await;
    let home = seed_team(pool, "Home FC", Some(hungary), "male", "senior", "senior").await;
    let away = seed_team(pool, "Away FC", Some(hungary), "male", "senior", "senior").await;
    let (competition, season) = seed_competition(pool, Some(hungary)).await;
    let kickoff_at = kickoff(2026, 11, 20, 18, 0);

    // A kanonikus meccs felcserélt csapatokkal jött létre.
    let match_id = seed_match(pool, competition, season, away, home, kickoff_at).await;
    let match_obs = seed_match_obs(
        pool,
        source,
        capture,
        "soccerway-1",
        "HOMECODE",
        "AWAYCODE",
        kickoff_at,
    )
    .await;
    seed_team_obs(
        pool, source, capture, "HOMECODE", "Home FC", "HU", "male", "senior", "senior", kickoff_at,
    )
    .await;
    seed_team_obs(
        pool, source, capture, "AWAYCODE", "Away FC", "HU", "male", "senior", "senior", kickoff_at,
    )
    .await;
    select_provenance(pool, match_id, match_obs, "home_team_id").await;

    (match_id, home, away, competition, season)
}

#[tokio::test]
async fn repair_dry_run_reports_but_writes_nothing() {
    let pool = test_pool().await;
    let (match_id, home, away, _, _) = swapped_match_fixture(&pool).await;
    let before = match_teams(&pool, match_id).await;

    let report = repair_soccerway_matches(&pool, day(2026, 11, 20), day(2026, 11, 21), true, true)
        .await
        .unwrap();
    assert_eq!(report.matches_scanned, 1);
    assert_eq!(report.repaired, 1);
    assert_eq!(report.merged, 0);
    assert_eq!(report.unresolved, 0);
    assert_eq!(report.withheld, 0);

    // A dry-run sem a meccset, sem a gate-et nem írhatja.
    assert_eq!(match_teams(&pool, match_id).await, before);
    assert_eq!(match_teams(&pool, match_id).await, Some((away, home)));
    assert_eq!(audit_count(&pool, "identity_repair_match").await, 0);
    assert_eq!(audit_count(&pool, "match_dedupe_merge").await, 0);
    assert!(gate_row(&pool, match_id).await.is_none());
    let _ = (home, away);
}

#[tokio::test]
async fn repair_swapped_match_uses_capture_scope_observations_and_audits() {
    let pool = test_pool().await;
    let (match_id, home, away, _, _) = swapped_match_fixture(&pool).await;

    let report = repair_soccerway_matches(&pool, day(2026, 11, 20), day(2026, 11, 21), false, true)
        .await
        .unwrap();
    assert_eq!(report.matches_scanned, 1);
    assert_eq!(report.repaired, 1);
    assert_eq!(report.merged, 0);
    assert_eq!(report.unresolved, 0);
    assert_eq!(report.withheld, 0);
    assert_eq!(report.errors, Vec::new());

    assert_eq!(match_teams(&pool, match_id).await, Some((home, away)));
    assert_eq!(audit_count(&pool, "identity_repair_match").await, 1);
    let detail = audit_detail(&pool, "identity_repair_match").await;
    assert_eq!(detail["match_id"], match_id.to_string());
    assert_eq!(detail["new_home"], home.to_string());
    assert_eq!(detail["new_away"], away.to_string());
    assert!(detail["observation_id"].as_str().is_some());

    // Ismételt futás: már helyes, nem ír.
    let again = repair_soccerway_matches(&pool, day(2026, 11, 20), day(2026, 11, 21), false, true)
        .await
        .unwrap();
    assert_eq!(again.already_correct, 1);
    assert_eq!(again.repaired, 0);
    assert_eq!(audit_count(&pool, "identity_repair_match").await, 1);
}

#[tokio::test]
async fn repair_merges_duplicate_match_instead_of_violating_unique_key() {
    let pool = test_pool().await;
    let (wrong_id, home, away, competition, season) = swapped_match_fixture(&pool).await;

    // Ugyanaz a természetes kulcs, de a capture szerinti (helyes)
    // orientációval már létezik egy sor.
    let kickoff_at = kickoff(2026, 11, 20, 18, 0);
    let correct_id = seed_match(&pool, competition, season, home, away, kickoff_at).await;

    let report = repair_soccerway_matches(&pool, day(2026, 11, 20), day(2026, 11, 21), false, true)
        .await
        .unwrap();
    assert_eq!(report.matches_scanned, 1);
    assert_eq!(report.merged, 1);
    assert_eq!(report.repaired, 0);
    assert_eq!(report.errors, Vec::new());

    assert!(
        !match_exists(&pool, wrong_id).await,
        "a rossz sor a merge után nem maradhat"
    );
    assert!(match_exists(&pool, correct_id).await);
    assert_eq!(
        match_teams(&pool, correct_id).await,
        Some((home, away)),
        "a capture szerinti orientáció marad"
    );

    let archived: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ops.dedupe_archive WHERE duplicate_id = $1")
            .bind(wrong_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(archived, 1, "a duplikátum archívumba kerül");

    assert_eq!(audit_count(&pool, "identity_repair_match").await, 1);
    let detail = audit_detail(&pool, "identity_repair_match").await;
    assert_eq!(detail["merged_into"], correct_id.to_string());
}

// ---------------------------------------------------------------------------
// 4) Feloldhatatlan meccs → withheld
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unresolved_match_is_withheld_on_budapest_publish_date() {
    let pool = test_pool().await;
    let source = seed_source(&pool, "soccerway").await;
    let capture = seed_capture(&pool, source).await;
    let hungary = seed_country(&pool, "HU", "Hungary").await;
    let home = seed_team(&pool, "Home FC", Some(hungary), "male", "senior", "senior").await;
    let away = seed_team(&pool, "Away FC", Some(hungary), "male", "senior", "senior").await;
    let (competition, season) = seed_competition(&pool, Some(hungary)).await;

    // UTC 23:30 → Budapesten már másnap (CET, +1): 2026-11-21.
    let kickoff_at = kickoff(2026, 11, 20, 23, 30);
    let match_id = seed_match(&pool, competition, season, home, away, kickoff_at).await;
    let match_obs = seed_match_obs(
        &pool,
        source,
        capture,
        "soccerway-2",
        "HOMECODE",
        "AWAYCODE",
        kickoff_at,
    )
    .await;
    select_provenance(&pool, match_id, match_obs, "home_team_id").await;
    // Szándékosan nincs team_identity observation → feloldhatatlan.

    let dry = repair_soccerway_matches(&pool, day(2026, 11, 20), day(2026, 11, 21), true, true)
        .await
        .unwrap();
    assert_eq!(dry.matches_scanned, 1);
    assert_eq!(dry.unresolved, 1);
    assert_eq!(dry.withheld, 0);
    assert!(dry.repaired == 0);
    assert!(gate_row(&pool, match_id).await.is_none(), "dry-run nem ír");

    let applied =
        repair_soccerway_matches(&pool, day(2026, 11, 20), day(2026, 11, 21), false, true)
            .await
            .unwrap();
    assert_eq!(applied.unresolved, 1);
    assert_eq!(applied.withheld, 1);

    let (state, rule_set_version, blockers, publish_date) =
        gate_row(&pool, match_id).await.expect("withheld gate-sor");
    assert_eq!(state, "withheld");
    assert_eq!(rule_set_version, "verification-1.2");
    assert_eq!(publish_date, day(2026, 11, 21));
    let blocker = &blockers[0];
    assert_eq!(blocker["rule"], "IDENTITY_UNRESOLVED");
    let message = blocker["message"].as_str().unwrap_or("");
    assert!(
        message.contains("nem oldható fel"),
        "magyar blokkoló üzenet: {message}"
    );

    assert_eq!(audit_count(&pool, "identity_repair_match").await, 1);
    let detail = audit_detail(&pool, "identity_repair_match").await;
    assert_eq!(detail["state"], "withheld");
    assert_eq!(detail["rule"], "IDENTITY_UNRESOLVED");

    // Érvénytelen tartomány tipizált hibát ad.
    let invalid =
        repair_soccerway_matches(&pool, day(2026, 11, 21), day(2026, 11, 20), true, false).await;
    match invalid {
        Err(football_identity::IdentityError::InvalidInput(message)) => {
            assert!(message.contains("nem lehet későbbi"));
        }
        other => panic!("InvalidInput helyett: {other:?}"),
    }
}
