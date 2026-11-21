//! S70/S80: a kézi meccsszerkesztés API-szerződése, tartósítása és audit-nyoma.
//!
//! A tesztek ugyanazt a teszt-adatbázist használják, amelyet a `test_pool`
//! minden hívásnál kiürít, ezért a binárison belül egy statikus zár sorosítja
//! őket — így a párhuzamos tokio-tesztek nem törlik ki egymás seed adatait.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use chrono::NaiveDate;
use football_api::{AppState, router};
use football_storage::test_support::test_pool;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tower::ServiceExt;
use uuid::Uuid;

static DB_LOCK: Mutex<()> = Mutex::const_new(());

fn app_state(pool: sqlx::PgPool) -> AppState {
    AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    }
}

/// Verseny, szezon és csapatpár: a natural key azonos részéhez.
struct Seed {
    competition_id: Uuid,
    season_id: Uuid,
    home_id: Uuid,
    away_id: Uuid,
}

async fn seed_competition_and_teams(pool: &sqlx::PgPool) -> Seed {
    let competition_id = Uuid::now_v7();
    let season_id = Uuid::now_v7();
    let home_id = Uuid::now_v7();
    let away_id = Uuid::now_v7();

    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'XX','Ismeretlen') ON CONFLICT DO NOTHING")
        .bind(Uuid::now_v7())
        .execute(pool)
        .await
        .unwrap();
    for (id, name) in [(home_id, "Home FC"), (away_id, "Away FC")] {
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO core.team (id, entity_id, name, gender, age_group, branch) VALUES ($1,$1,$2,'male','senior','senior')")
            .bind(id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
    }
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','Test Cup')",
    )
    .bind(competition_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.competition (id, entity_id, name, tier, priority) VALUES ($1,$1,'Test Cup',0,30)")
        .bind(competition_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season_id)
        .bind(competition_id)
        .execute(pool)
        .await
        .unwrap();
    Seed {
        competition_id,
        season_id,
        home_id,
        away_id,
    }
}

/// Egy meccs a megadott kezdéssel (RFC 3339), a natural key többi része a seedből.
async fn insert_match(pool: &sqlx::PgPool, seed: &Seed, kickoff_at: &str) -> Uuid {
    let match_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, home_team_id, away_team_id, kickoff_at,
          competition_timezone, status)
         VALUES ($1,$2,$3,$4,$5,$6::timestamptz,'UTC','scheduled')",
    )
    .bind(match_id)
    .bind(seed.competition_id)
    .bind(seed.season_id)
    .bind(seed.home_id)
    .bind(seed.away_id)
    .bind(kickoff_at)
    .execute(pool)
    .await
    .unwrap();
    match_id
}

async fn seed_match(pool: &sqlx::PgPool) -> (Uuid, Uuid, Uuid) {
    let seed = seed_competition_and_teams(pool).await;
    let match_id = insert_match(pool, &seed, "2026-11-21T17:00:00Z").await;
    (match_id, seed.home_id, seed.away_id)
}

async fn put(app: &axum::Router, id: Uuid, token: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/match/{id}/manual-result"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let payload: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, payload)
}

async fn result_row(
    pool: &sqlx::PgPool,
    match_id: Uuid,
) -> Option<(
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<String>,
    Option<String>,
    String,
)> {
    sqlx::query_as(
        "SELECT home_ft, away_ft, home_ht, away_ht, home_et, away_et,
                home_pens, away_pens, decided_by, winner_method, validity
         FROM core.match_result WHERE match_id=$1",
    )
    .bind(match_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

fn date(raw: &str) -> NaiveDate {
    raw.parse().expect("érvényes dátum")
}

#[tokio::test]
async fn manual_result_requires_ops_token_writes_provenance_and_audit() {
    let _guard = DB_LOCK.lock().await;
    let pool = test_pool().await;
    let (match_id, _, _) = seed_match(&pool).await;
    let app = router(app_state(pool.clone()));
    // A régi kliens ugyanezt a három mezőt küldi: hátrafelé kompatibilis kell legyen.
    let body = json!({"home_ft": 2, "away_ft": 1, "note": "kézi javítás a tesztben"});

    let (unauth, _) = put(&app, match_id, "read-test", body.clone()).await;
    assert_eq!(unauth, StatusCode::UNAUTHORIZED, "read tokennel nem írhat");

    let (status, response) = put(&app, match_id, "ops-test", body).await;
    assert_eq!(status, StatusCode::OK, "ops tokennel írhat: {response}");
    assert_eq!(response["match_id"], match_id.to_string());
    assert!(response["observation_id"].is_string());
    assert!(response["audit_id"].is_string());

    let row = result_row(&pool, match_id)
        .await
        .expect("a régi body FT eredményt ír");
    assert_eq!((row.0, row.1), (Some(2), Some(1)));
    assert_eq!(row.8.as_deref(), Some("ft"));
    assert_eq!(
        row.9.as_deref(),
        Some("regular"),
        "winner_method a decided_by-ból"
    );
    assert_eq!(row.10, "valid");
    assert_eq!(
        (row.2, row.3, row.4, row.5, row.6, row.7),
        (None, None, None, None, None, None)
    );

    let (status_db, precision_db): (String, String) =
        sqlx::query_as("SELECT status, kickoff_precision FROM core.match WHERE id=$1")
            .bind(match_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status_db, "finished");
    assert_eq!(precision_db, "exact");

    let provenance: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fact.provenance
         WHERE subject_kind='match' AND subject_id=$1 AND is_selected
           AND field_key IN ('result','status','decided_by')",
    )
    .bind(match_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        provenance, 3,
        "eredmény, státusz és decided_by provenance-t kap"
    );

    let (action, detail): (String, Value) = sqlx::query_as(
        "SELECT action, detail FROM ops.audit_event
         WHERE subject_kind='match' AND subject_id=$1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(match_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(action, "manual_match_edit");
    assert_eq!(detail["before"]["status"], "scheduled");
    assert_eq!(detail["after"]["status"], "finished");
    assert!(detail["before"]["result"].is_null());
    assert_eq!(detail["after"]["result"]["home_ft"], 2);
    assert_eq!(detail["note"], "kézi javítás a tesztben");
    assert!(detail["observation_id"].is_string());
}

#[tokio::test]
async fn full_edit_stores_extra_time_result_and_audits_before_after() {
    let _guard = DB_LOCK.lock().await;
    let pool = test_pool().await;
    let (match_id, _, _) = seed_match(&pool).await;
    let app = router(app_state(pool.clone()));

    let body = json!({
        "status": "finished",
        "home_ft": 1, "away_ft": 1,
        "home_ht": 1, "away_ht": 0,
        "home_et": 2, "away_et": 1,
        "decided_by": "et",
        "note": "hosszabbításban dőlt el"
    });
    let (status, response) = put(&app, match_id, "ops-test", body).await;
    assert_eq!(status, StatusCode::OK, "teljes szerkesztés: {response}");

    let row = result_row(&pool, match_id).await.expect("eredmény sor");
    assert_eq!((row.0, row.1), (Some(1), Some(1)));
    assert_eq!((row.2, row.3), (Some(1), Some(0)));
    assert_eq!((row.4, row.5), (Some(2), Some(1)));
    assert_eq!(row.8.as_deref(), Some("et"));
    assert_eq!(row.9.as_deref(), Some("extra_time"));

    let status_db: String = sqlx::query_scalar("SELECT status FROM core.match WHERE id=$1")
        .bind(match_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status_db, "finished");

    // Csak a megváltozott mezők kapnak új provenance-t, a régi jelölés megszűnik.
    let selected: Vec<String> = sqlx::query_scalar(
        "SELECT field_key FROM fact.provenance
         WHERE subject_kind='match' AND subject_id=$1 AND is_selected ORDER BY field_key",
    )
    .bind(match_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(selected, vec!["decided_by", "result", "status"]);
    let deselected: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fact.provenance
         WHERE subject_kind='match' AND subject_id=$1 AND NOT is_selected",
    )
    .bind(match_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        deselected, 0,
        "első szerkesztésnél nincs korábbi selected sor"
    );

    let detail: Value = sqlx::query_scalar(
        "SELECT detail FROM ops.audit_event
         WHERE subject_kind='match' AND subject_id=$1 AND action='manual_match_edit'
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(match_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(detail["before"]["status"], "scheduled");
    assert_eq!(detail["after"]["status"], "finished");
    assert_eq!(detail["after"]["result"]["home_et"], 2);
    assert_eq!(detail["after"]["result"]["decided_by"], "et");
    assert_eq!(detail["after"]["result"]["winner_method"], "extra_time");

    let payload: Value =
        sqlx::query_scalar("SELECT normalized_payload FROM stage.observation WHERE id=$1::uuid")
            .bind(response["observation_id"].as_str().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(payload["status"], "finished");
    assert_eq!(payload["result"]["decided_by"], "et");
    assert_eq!(payload["kickoff_precision"], "exact");
    assert_eq!(payload["note"], "hosszabbításban dőlt el");
}

#[tokio::test]
async fn penalties_are_stored_with_penalties_winner_method() {
    let _guard = DB_LOCK.lock().await;
    let pool = test_pool().await;
    let (match_id, _, _) = seed_match(&pool).await;
    let app = router(app_state(pool.clone()));

    let body = json!({
        "status": "finished",
        "home_ft": 1, "away_ft": 1,
        "home_et": 2, "away_et": 2,
        "home_pens": 4, "away_pens": 3,
        "decided_by": "pens",
        "note": "tizenegyesekkel"
    });
    let (status, response) = put(&app, match_id, "ops-test", body).await;
    assert_eq!(status, StatusCode::OK, "tizenegyes eredmény: {response}");

    let row = result_row(&pool, match_id).await.expect("eredmény sor");
    assert_eq!((row.6, row.7), (Some(4), Some(3)));
    assert_eq!(row.8.as_deref(), Some("pens"));
    assert_eq!(row.9.as_deref(), Some("penalties"));
}

#[tokio::test]
async fn kickoff_move_writes_new_publish_date_and_deletes_stale_gate_row() {
    let _guard = DB_LOCK.lock().await;
    let pool = test_pool().await;
    let (match_id, _, _) = seed_match(&pool).await;
    // A seedelt meccs kezdése 2026-11-21T17:00:00Z, a helyi (budapesti) dátum 11-21.
    sqlx::query(
        "INSERT INTO ops.publish_gate
         (id, match_id, publish_date, state, rule_set_version, blockers, evaluated_at)
         VALUES ($1,$2,$3,'withheld','test', '[]'::jsonb, now())",
    )
    .bind(Uuid::now_v7())
    .bind(match_id)
    .bind(date("2026-11-21"))
    .execute(&pool)
    .await
    .unwrap();
    let app = router(app_state(pool.clone()));

    let body = json!({
        "kickoff_at": "2026-11-22T17:00:00Z",
        "kickoff_precision": "exact",
        "note": "áthelyezve a következő napra"
    });
    let (status, response) = put(&app, match_id, "ops-test", body).await;
    assert_eq!(status, StatusCode::OK, "kezdés áthelyezése: {response}");

    let kickoff_db: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT kickoff_at FROM core.match WHERE id=$1")
            .bind(match_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(kickoff_db.to_rfc3339(), "2026-11-22T17:00:00+00:00");

    let dates: Vec<NaiveDate> = sqlx::query_scalar(
        "SELECT publish_date FROM ops.publish_gate WHERE match_id=$1 ORDER BY publish_date",
    )
    .bind(match_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        dates,
        vec![date("2026-11-22")],
        "a régi, elavult gate sor törlődik, csak az új helyi dátum marad"
    );

    // Az eredményhez nem nyúltunk: csak kickoff/precision változott.
    assert!(
        result_row(&pool, match_id).await.is_none(),
        "csak kickoff szerkesztésnél a match_result érintetlen (itt nem is volt)"
    );
}

#[tokio::test]
async fn canceled_status_without_result_fields_deletes_result() {
    let _guard = DB_LOCK.lock().await;
    let pool = test_pool().await;
    let (match_id, _, _) = seed_match(&pool).await;
    let app = router(app_state(pool.clone()));

    let (first, first_response) = put(
        &app,
        match_id,
        "ops-test",
        json!({"home_ft": 3, "away_ft": 0, "note": "először eredmény"}),
    )
    .await;
    assert_eq!(first, StatusCode::OK, "{first_response}");
    assert!(result_row(&pool, match_id).await.is_some());

    let (status, response) = put(
        &app,
        match_id,
        "ops-test",
        json!({"status": "canceled", "note": "a meccs elmaradt"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "lemondás: {response}");
    assert!(
        result_row(&pool, match_id).await.is_none(),
        "canceled + nincs eredmény-mező -> a match_result törlődik"
    );
    let status_db: String = sqlx::query_scalar("SELECT status FROM core.match WHERE id=$1")
        .bind(match_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status_db, "canceled");

    // A törölt eredmény provenance-a sem maradhat selected állapotban.
    let selected_result: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fact.provenance
         WHERE subject_kind='match' AND subject_id=$1 AND field_key='result' AND is_selected",
    )
    .bind(match_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        selected_result, 1,
        "csak a legutóbbi observation a selected"
    );
    let deselected_result: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fact.provenance
         WHERE subject_kind='match' AND subject_id=$1 AND field_key='result' AND NOT is_selected",
    )
    .bind(match_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        deselected_result, 1,
        "a korábbi eredmény provenance-a lekerül"
    );

    let detail: Value = sqlx::query_scalar(
        "SELECT detail FROM ops.audit_event
         WHERE subject_kind='match' AND subject_id=$1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(match_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(detail["before"]["result"]["home_ft"], 3);
    assert!(detail["after"]["result"].is_null());
    assert_eq!(detail["after"]["status"], "canceled");
}

#[tokio::test]
async fn natural_key_collision_is_conflict_with_existing_match_id() {
    let _guard = DB_LOCK.lock().await;
    let pool = test_pool().await;
    let seed = seed_competition_and_teams(&pool).await;
    let first = insert_match(&pool, &seed, "2026-11-21T17:00:00Z").await;
    let second = insert_match(&pool, &seed, "2026-11-28T17:00:00Z").await;
    let app = router(app_state(pool.clone()));

    let body = json!({
        "kickoff_at": "2026-11-28T17:00:00Z",
        "kickoff_precision": "exact",
        "note": "ütköző kezdés"
    });
    let (status, response) = put(&app, first, "ops-test", body).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "natural key ütközés: {response}"
    );
    assert_eq!(response["code"], "CONFLICT");
    assert_eq!(
        response["conflict_match_id"],
        second.to_string(),
        "a válasz megadja a másik meccs azonosítóját"
    );

    // Az ütközés nem írt semmit: a kezdés és az audit érintetlen.
    let kickoff_db: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT kickoff_at FROM core.match WHERE id=$1")
            .bind(first)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(kickoff_db.to_rfc3339(), "2026-11-21T17:00:00+00:00");
    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.audit_event WHERE subject_id=$1 AND action='manual_match_edit'",
    )
    .bind(first)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 0);
}

#[tokio::test]
async fn manual_result_validates_input_and_missing_match() {
    let _guard = DB_LOCK.lock().await;
    let pool = test_pool().await;
    let (match_id, _, _) = seed_match(&pool).await;
    let app = router(app_state(pool));

    let cases = [
        (
            json!({"home_ft": -1, "away_ft": 0, "note": "negatív"}),
            "nem lehet negatív",
        ),
        (
            json!({"home_ft": 1, "away_ft": 0, "note": "  "}),
            "indoklás kötelező",
        ),
        (
            json!({"home_ft": 1, "away_ft": 0, "home_ht": 2, "away_ht": 0, "note": "HT>FT"}),
            "nem lehet nagyobb",
        ),
        (
            json!({"home_ft": 1, "away_ft": 1, "home_pens": 4, "away_pens": 3, "note": "pens ET nélkül"}),
            "Tizenegyespárbaj csak hosszabbítás",
        ),
        (
            json!({"status": "scheduled", "home_ft": 1, "away_ft": 0, "note": "scheduled + eredmény"}),
            "nem tartozhat eredmény",
        ),
        (
            json!({"kickoff_at": "2026-11-21T17:00:00Z", "kickoff_precision": "date_only", "note": "date_only rossz idővel"}),
            "12:00:00Z",
        ),
    ];
    for (body, expected) in cases {
        let (status, response) = put(&app, match_id, "ops-test", body.clone()).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "elvárt 400: {body} -> {response}"
        );
        let message = response["message"].as_str().unwrap_or_default();
        assert!(
            message.contains(expected),
            "a hibaüzenet mezőnkénti magyarázatot ad: {message}"
        );
    }

    let (missing, response) = put(
        &app,
        Uuid::now_v7(),
        "ops-test",
        json!({"home_ft": 1, "away_ft": 0, "note": "nincs ilyen meccs"}),
    )
    .await;
    assert_eq!(missing, StatusCode::NOT_FOUND);
    assert_eq!(response["code"], "NOT_FOUND");
}
