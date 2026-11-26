//! S173/S4: az egységes `PUT /v1/match/{id}/edit` szerződése.
//!
//! A tesztek a közös teszt-adatbázist használják (`test_pool` TRUNCATE-t végez),
//! ezért egy statikus zár sorosítja őket — lásd `manual_result.rs`.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
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

struct Seed {
    competition_id: Uuid,
    season_id: Uuid,
    home_id: Uuid,
    away_id: Uuid,
}

async fn seed_base(pool: &sqlx::PgPool) -> Seed {
    let seed = Seed {
        competition_id: Uuid::now_v7(),
        season_id: Uuid::now_v7(),
        home_id: Uuid::now_v7(),
        away_id: Uuid::now_v7(),
    };
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'XX','Ismeretlen') ON CONFLICT DO NOTHING")
        .bind(Uuid::now_v7())
        .execute(pool)
        .await
        .unwrap();
    for (id, name) in [(seed.home_id, "Home FC"), (seed.away_id, "Away FC")] {
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
    .bind(seed.competition_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.competition (id, entity_id, name, tier, priority) VALUES ($1,$1,'Test Cup',0,30)")
        .bind(seed.competition_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(seed.season_id)
        .bind(seed.competition_id)
        .execute(pool)
        .await
        .unwrap();
    seed
}

async fn insert_match(pool: &sqlx::PgPool, seed: &Seed, kickoff: &str, status: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, home_team_id, away_team_id, kickoff_at,
          competition_timezone, status)
         VALUES ($1,$2,$3,$4,$5,$6::timestamptz,'UTC',$7)",
    )
    .bind(id)
    .bind(seed.competition_id)
    .bind(seed.season_id)
    .bind(seed.home_id)
    .bind(seed.away_id)
    .bind(kickoff)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
    id
}

/// Lezárt meccs eredménnyel: a kickoff-only szerkesztés nem érhet hozzá.
async fn seed_finished_match(pool: &sqlx::PgPool) -> Uuid {
    let seed = seed_base(pool).await;
    let id = insert_match(pool, &seed, "2026-11-21T17:00:00Z", "finished").await;
    sqlx::query(
        "INSERT INTO core.match_result
         (match_id, home_ft, away_ft, winner_method, validity, decided_at, reason, decided_by)
         VALUES ($1,1,0,'regular','valid',now(),'teszt','ft')",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
    id
}

/// PUT az új végpontra, opcionális tokennel.
async fn put(
    app: &axum::Router,
    id: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method("PUT")
        .uri(format!("/v1/match/{id}/edit"))
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = app
        .clone()
        .oneshot(
            builder
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

#[tokio::test]
async fn edit_requires_ops_token_and_preserves_untouched_fields() {
    let _guard = DB_LOCK.lock().await;
    let pool = test_pool().await;
    let match_id = seed_finished_match(&pool).await;
    let app = router(app_state(pool.clone()));
    let body = json!({
        "kickoff_at": "2026-11-22T17:00:00Z",
        "note": "eltolódott kezdés javítása",
        "idempotency_key": "kickoff-6775371"
    });

    let (read, _) = put(&app, &match_id.to_string(), Some("read-test"), body.clone()).await;
    assert_eq!(read, StatusCode::UNAUTHORIZED, "read tokennel nem írhat");
    let (none, _) = put(&app, &match_id.to_string(), None, body.clone()).await;
    assert_eq!(none, StatusCode::UNAUTHORIZED, "token nélkül nem írhat");

    let (status, response) = put(&app, &match_id.to_string(), Some("ops-test"), body).await;
    assert_eq!(status, StatusCode::OK, "ops tokennel írhat: {response}");
    assert_eq!(response["match_id"], match_id.to_string());
    assert!(response["observation_id"].is_string());
    assert!(response["audit_id"].is_string());
    assert_eq!(response["reused"], false);
    assert!(response["state"].is_string());

    // Régi értékek megőrzése: csak a kickoff változott.
    let (kickoff, status_db, precision): (String, String, String) = sqlx::query_as(
        "SELECT kickoff_at::text, status, kickoff_precision FROM core.match WHERE id=$1",
    )
    .bind(match_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kickoff, "2026-11-22 17:00:00+00");
    assert_eq!(status_db, "finished");
    assert_eq!(precision, "exact");
    let (home_ft, away_ft): (Option<i16>, Option<i16>) =
        sqlx::query_as("SELECT home_ft, away_ft FROM core.match_result WHERE match_id=$1")
            .bind(match_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (home_ft, away_ft),
        (Some(1), Some(0)),
        "eredmény érintetlen"
    );

    // Provenance és policy verzió: csak a ténylegesen változott mező.
    let fields: Vec<(String, String)> = sqlx::query_as(
        "SELECT field_key, policy_version FROM fact.provenance
         WHERE subject_kind='match' AND subject_id=$1 AND is_selected ORDER BY field_key",
    )
    .bind(match_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        fields.len(),
        1,
        "csak a kickoff_at provenance-a frissül: {fields:?}"
    );
    assert_eq!(fields[0].0, "kickoff_at");
    assert_eq!(fields[0].1, "manual-full-1.0");

    let (action, detail): (String, Value) = sqlx::query_as(
        "SELECT action, detail FROM ops.audit_event
         WHERE subject_kind='match' AND subject_id=$1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(match_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(action, "manual_match_edit");
    assert_eq!(detail["before"]["kickoff_at"], "2026-11-21T17:00:00+00:00");
    assert_eq!(detail["after"]["kickoff_at"], "2026-11-22T17:00:00+00:00");
    assert_eq!(detail["after"]["status"], "finished");
    assert_eq!(detail["note"], "eltolódott kezdés javítása");
    assert_eq!(detail["observation_id"], response["observation_id"]);

    let (missing_status, missing) = put(
        &app,
        &Uuid::now_v7().to_string(),
        Some("ops-test"),
        json!({"kickoff_at": "2026-11-22T17:00:00Z", "note": "nincs ilyen meccs"}),
    )
    .await;
    assert_eq!(missing_status, StatusCode::NOT_FOUND);
    assert_eq!(missing["code"], "NOT_FOUND");
    assert!(
        missing["message"]
            .as_str()
            .unwrap()
            .contains("nem található")
    );
}

#[tokio::test]
async fn edit_is_idempotent_by_explicit_key_and_canonical_payload() {
    let _guard = DB_LOCK.lock().await;
    let pool = test_pool().await;
    let seed = seed_base(&pool).await;
    let explicit_id = insert_match(&pool, &seed, "2026-11-21T17:00:00Z", "scheduled").await;
    let canonical_id = insert_match(&pool, &seed, "2026-11-22T17:00:00Z", "scheduled").await;
    let app = router(app_state(pool.clone()));

    let explicit = json!({
        "kickoff_at": "2026-11-25T17:00:00Z",
        "note": "azonos kulcsú javítás",
        "idempotency_key": "kickoff-6775372"
    });
    let (first_status, first) = put(
        &app,
        &explicit_id.to_string(),
        Some("ops-test"),
        explicit.clone(),
    )
    .await;
    let (second_status, second) =
        put(&app, &explicit_id.to_string(), Some("ops-test"), explicit).await;
    assert_eq!(first_status, StatusCode::OK, "{first}");
    assert_eq!(second_status, StatusCode::OK, "{second}");
    assert_eq!(first["reused"], false);
    assert_eq!(second["reused"], true, "azonos kulcs: reused");
    assert_eq!(second["observation_id"], first["observation_id"]);
    assert_eq!(second["audit_id"], first["audit_id"]);

    let audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.audit_event WHERE subject_id=$1 AND action='manual_match_edit'",
    )
    .bind(explicit_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audits, 1, "ismétlés nem ír új auditot");

    // Kulcs nélkül a whitelist kanonikus lenyomata adja a stabil kulcsot.
    let canonical = json!({"kickoff_at": "2026-11-26T17:00:00Z", "note": "kanonikus kulcs"});
    let (first_status, first) = put(
        &app,
        &canonical_id.to_string(),
        Some("ops-test"),
        canonical.clone(),
    )
    .await;
    let (second_status, second) =
        put(&app, &canonical_id.to_string(), Some("ops-test"), canonical).await;
    assert_eq!(first_status, StatusCode::OK, "{first}");
    assert_eq!(second_status, StatusCode::OK, "{second}");
    assert_eq!(first["reused"], false);
    assert_eq!(second["reused"], true, "kanonikus lenyomat: reused");
    assert_eq!(second["observation_id"], first["observation_id"]);
}

#[tokio::test]
async fn edit_validates_input_and_ignores_unknown_fields() {
    let _guard = DB_LOCK.lock().await;
    let pool = test_pool().await;
    let seed = seed_base(&pool).await;
    let match_id = insert_match(&pool, &seed, "2026-11-21T17:00:00Z", "scheduled").await;
    let app = router(app_state(pool));

    let (bad_id, response) = put(
        &app,
        "nem-uuid",
        Some("ops-test"),
        json!({"note": "rossz azonosító"}),
    )
    .await;
    assert_eq!(bad_id, StatusCode::BAD_REQUEST);
    assert!(
        response["message"]
            .as_str()
            .unwrap()
            .contains("Érvénytelen meccs-azonosító")
    );

    let cases = [
        (
            json!({"status": "ismeretlen", "note": "rossz státusz"}),
            "Ismeretlen státusz",
        ),
        (
            json!({"kickoff_at": "2026-11-22T17:00:00Z", "note": "  "}),
            "indoklás kötelező",
        ),
        (
            json!({"home_ft": -1, "away_ft": 0, "note": "negatív"}),
            "nem lehet negatív",
        ),
    ];
    for (body, expected) in cases {
        let (status, response) =
            put(&app, &match_id.to_string(), Some("ops-test"), body.clone()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body} -> {response}");
        assert!(
            response["message"].as_str().unwrap().contains(expected),
            "{expected} nincs a magyar hibaüzenetben: {response}"
        );
    }

    // A whitelistból kimaradó mező figyelmen kívül marad.
    let (status, response) = put(
        &app,
        &match_id.to_string(),
        Some("ops-test"),
        json!({"note": "ismeretlen mező", "titkos_mezo": "figyelmen kívül"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["reused"], false);
}

/// A régi út kompatibilitása: az S173 nem törli a `/manual-result` végpontot.
#[tokio::test]
async fn old_manual_result_endpoint_still_works() {
    let _guard = DB_LOCK.lock().await;
    let pool = test_pool().await;
    let seed = seed_base(&pool).await;
    let match_id = insert_match(&pool, &seed, "2026-11-21T17:00:00Z", "scheduled").await;
    let app = router(app_state(pool));
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/v1/match/{match_id}/manual-result"))
                .header(header::AUTHORIZATION, "Bearer ops-test")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"home_ft": 2, "away_ft": 1, "note": "régi út"}))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "a régi végpont él");
}
