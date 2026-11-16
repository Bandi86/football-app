use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{TimeZone, Utc};
use football_api::{AppState, router};
use football_storage::test_support::test_pool;
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn health_is_public() {
    let pool = test_pool().await;
    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn day_requires_read_token() {
    let pool = test_pool().await;
    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    });

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/day/2026-11-09")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let authorized = app
        .oneshot(
            Request::builder()
                .uri("/v1/day/2026-11-09")
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(authorized.status(), StatusCode::OK);
}

#[tokio::test]
async fn day_response_includes_odds_trust_state() {
    let pool = test_pool().await;
    let base = seed_day_base(&pool).await;
    let verified_match = seed_day_match(&pool, &base, 18).await;
    let no_odds_match = seed_day_match(&pool, &base, 19).await;

    let source_id = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,'demo','Demó')")
        .bind(source_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO fact.odds_snapshot
         (match_id, source_id, captured_at, market_count, trust_state)
         VALUES ($1,$2,now(),18,'verified')",
    )
    .bind(verified_match)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO fact.odds_quote
         (match_id, source_id, bookmaker, market, outcome, line, phase, price)
         VALUES ($1,$2,'default','1x2','home',0,'closing',2.2),
                ($1,$2,'default','1x2','draw',0,'closing',3.3),
                ($1,$2,'default','1x2','away',0,'closing',3.2)",
    )
    .bind(verified_match)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();

    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    });
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/day/2026-11-09")
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let matches = body["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 2);
    // A verified odds állapota megjelenik az API válaszban.
    assert_eq!(matches[0]["id"], verified_match.to_string());
    assert_eq!(matches[0]["odds_trust_state"], "verified");
    // Odds nélküli meccsnél a mező null, nem hiányzik és nem üres string.
    // A puszta null-assert nem bizonyítaná a kulcs jelenlétét (a serde_json
    // indexelés hiányzó kulcsra is Null), ezért a contains_key is kell.
    assert_eq!(matches[1]["id"], no_odds_match.to_string());
    assert!(
        matches[1]
            .as_object()
            .unwrap()
            .contains_key("odds_trust_state")
    );
    assert!(matches[1]["odds_trust_state"].is_null());
}

struct DayBase {
    competition_id: Uuid,
    season_id: Uuid,
    home_team_id: Uuid,
    away_team_id: Uuid,
}

async fn seed_day_base(pool: &PgPool) -> DayBase {
    let country_id = Uuid::now_v7();
    let competition_entity_id = Uuid::now_v7();
    let competition_id = Uuid::now_v7();
    let season_id = Uuid::now_v7();
    let home_entity_id = Uuid::now_v7();
    let home_team_id = Uuid::now_v7();
    let away_entity_id = Uuid::now_v7();
    let away_team_id = Uuid::now_v7();

    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Hungary')")
        .bind(country_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','NB I')",
    )
    .bind(competition_entity_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name)
         VALUES ($1,$2,$3,'NB I')",
    )
    .bind(competition_id)
    .bind(competition_entity_id)
    .bind(country_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season_id)
        .bind(competition_id)
        .execute(pool)
        .await
        .unwrap();

    for (entity_id, team_id, name) in [
        (home_entity_id, home_team_id, "Újpest"),
        (away_entity_id, away_team_id, "Győr"),
    ] {
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(entity_id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO core.team
             (id, entity_id, country_id, name, gender, age_group, branch)
             VALUES ($1,$2,$3,$4,'male','senior','senior')",
        )
        .bind(team_id)
        .bind(entity_id)
        .bind(country_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    }

    DayBase {
        competition_id,
        season_id,
        home_team_id,
        away_team_id,
    }
}

async fn seed_day_match(pool: &PgPool, base: &DayBase, hour: u32) -> Uuid {
    let match_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, round_label, home_team_id, away_team_id,
          kickoff_at, competition_timezone, status)
         VALUES ($1,$2,$3,'12',$4,$5,$6,'Europe/Budapest','scheduled')",
    )
    .bind(match_id)
    .bind(base.competition_id)
    .bind(base.season_id)
    .bind(base.home_team_id)
    .bind(base.away_team_id)
    .bind(Utc.with_ymd_and_hms(2026, 11, 9, hour, 0, 0).unwrap())
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO ops.publish_gate
         (id, match_id, publish_date, state, rule_set_version, blockers, evaluated_at)
         VALUES ($1,$2,'2026-11-09','verified_publish','verification-1.1','[]',now())",
    )
    .bind(Uuid::now_v7())
    .bind(match_id)
    .execute(pool)
    .await
    .unwrap();

    match_id
}

#[tokio::test]
async fn day_response_includes_refreshed_at_and_asset_urls() {
    let pool = test_pool().await;
    let base = seed_day_base(&pool).await;
    seed_day_match(&pool, &base, 21).await;
    let sha = "a".repeat(64);
    let key = format!("assets/ab/cd/{sha}.webp");
    sqlx::query(
        "INSERT INTO ops.asset
         (id, entity_kind, entity_id, kind, url, storage_key, source, licence_status, captured_at)
         VALUES ($1,'team',$2,'logo','file://x',$3,'test','unknown',now())",
    )
    .bind(Uuid::now_v7())
    .bind(base.home_team_id)
    .bind(&key)
    .execute(&pool)
    .await
    .unwrap();

    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    });
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/day/2026-11-09")
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value["refreshed_at"].is_string(), "refreshed_at: {value}");
    let m = &value["matches"][0];
    assert_eq!(m["home_logo"], format!("/v1/asset/{key}"));
    assert!(m["away_logo"].is_null());
    assert!(m["home_elo"].is_null());
}

#[tokio::test]
async fn asset_route_serves_blob_and_rejects_bad_keys() {
    let pool = test_pool().await;
    let dir = tempfile::tempdir().unwrap();
    let sha = "b".repeat(64);
    let key = format!("assets/12/34/{sha}.webp");
    let path = dir.path().join("raw").join(&key);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"webp-bytes").unwrap();

    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: dir.path().to_path_buf(),
    });

    let ok = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/asset/{key}"))
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
    assert_eq!(
        ok.headers().get("content-type").unwrap().to_str().unwrap(),
        "image/webp"
    );
    let body = ok.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"webp-bytes");

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/asset/{key}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let bad = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/asset/assets/12/34/../../etc/passwd")
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);

    let missing_key = format!("assets/12/34/{}.webp", "c".repeat(64));
    let missing = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/asset/{missing_key}"))
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn ops_status_returns_counts_and_sources() {
    let pool = test_pool().await;
    sqlx::query(
        "INSERT INTO ops.source_policy
         (source_slug, transport, mode, daily_budget, min_delay_ms, jitter_ms,
          breaker_threshold, breaker_cooldown_secs, adapter_version, parser_version,
          domains, events, enabled, max_bytes)
         VALUES ('teszt-forras','rust_http','canary',10,0,0,3,60,'1.0','1.0',
                 '[\"x\"]'::jsonb,'[]'::jsonb,true,1048576)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ops.job (id, source_slug, domain, entity_key, event_type,
                              time_window, due_at, priority, status)
         VALUES ($1,'teszt-forras','x','all','discovery','2026-11-15',now(),10,'pending')",
    )
    .bind(Uuid::now_v7())
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO identity.review_queue
         (id, source_id, entity_kind, source_entity_id, reason, proposed_name, context)
         VALUES ($1, NULL, 'team', 'x', 'no_exact_match', 'X', '{}'::jsonb)",
    )
    .bind(Uuid::now_v7())
    .execute(&pool)
    .await
    .unwrap();

    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    });
    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/ops/status")
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/ops/status")
                .header("authorization", "Bearer ops-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["pending_jobs"], 1);
    assert_eq!(value["pending_reviews"], 1);
    let sources = value["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0]["source_slug"], "teszt-forras");
    assert_eq!(sources[0]["mode"], "canary");
    assert_eq!(sources[0]["enabled"], true);
    assert_eq!(sources[0]["breaker_state"], "closed");
}

#[tokio::test]
async fn match_detail_returns_verified_fields_and_gate() {
    let pool = test_pool().await;
    let base = seed_day_base(&pool).await;
    let match_id = seed_day_match(&pool, &base, 22).await;

    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    });

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/match/{match_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/match/{match_id}"))
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["home_name"], "Újpest");
    assert_eq!(value["away_name"], "Győr");
    assert_eq!(value["competition_name"], "NB I");
    assert_eq!(value["gate_state"], "verified_publish");
    assert_eq!(value["kickoff_precision"], "exact");
    assert!(value["season_label"].is_string());

    let missing = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/match/{}", Uuid::now_v7()))
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn competitions_list_and_detail_return_gated_matches() {
    let pool = test_pool().await;
    let base = seed_day_base(&pool).await;
    seed_day_match(&pool, &base, 20).await;
    seed_day_match(&pool, &base, 21).await;

    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    });

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/competitions?date=2026-11-09")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/competitions?date=2026-11-09")
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let bytes = list.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let rows = value.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], "NB I");
    assert_eq!(rows[0]["matches"], 2);

    let competition_id = rows[0]["competition_id"].as_str().unwrap();
    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/competition/{competition_id}?date=2026-11-09&window=7"
                ))
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    let bytes = detail.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["detail"]["name"], "NB I");
    assert_eq!(value["detail"]["season_label"], "2026/27");
    assert_eq!(value["detail"]["matches"], 2);
    assert_eq!(value["matches"].as_array().unwrap().len(), 2);
    assert!(
        value["teams"].as_array().unwrap().is_empty(),
        "nincs eredmény → nincs mérleg"
    );

    let missing = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/competition/{}?date=2026-11-09",
                    Uuid::now_v7()
                ))
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn teams_list_and_detail_return_recent_matches() {
    let pool = test_pool().await;
    let base = seed_day_base(&pool).await;
    seed_day_match(&pool, &base, 20).await;
    seed_day_match(&pool, &base, 21).await;

    let app = router(AppState {
        pool,
        read_token: "read-test".into(),
        ops_token: "ops-test".into(),
        data_dir: std::env::temp_dir(),
    });

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/teams?query=%C3%9Ajp&date=2026-11-09")
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let bytes = list.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let rows = value.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], "Újpest");
    assert_eq!(rows[0]["recent_matches"], 2);

    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/v1/team/{}?date=2026-11-09&window=7",
                    base.home_team_id
                ))
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);
    let bytes = detail.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["detail"]["name"], "Újpest");
    assert_eq!(value["detail"]["played"], 0, "nincs eredmény");
    assert_eq!(value["matches"].as_array().unwrap().len(), 2);

    let missing = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/team/{}?date=2026-11-09", Uuid::now_v7()))
                .header("authorization", "Bearer read-test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}
