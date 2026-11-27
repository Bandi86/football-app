//! S168 — hiányjelek, jegyzetek és meccs-történet repository-tesztei.
//! A `test_pool()` minden híváskor törli a közös tesztadatbázist, ezért a
//! teljes workspace-t `--test-threads=1`-gyel kell futtatni.

use chrono::NaiveDate;
use football_domain::ops::{IssueCode, IssueSeverity, IssueSource, IssueSubjectKind, NoteKind};
use football_storage::repositories::detect;
use football_storage::repositories::history;
use football_storage::repositories::issue::{self, IssueFilter, IssueScope, NewIssue};
use football_storage::repositories::note::{self, NotePatch};
use football_storage::test_support::{seed_min_match, test_pool};
use serde_json::json;
use uuid::Uuid;

fn match_issue(match_id: Uuid, code: IssueCode, severity: IssueSeverity) -> NewIssue {
    NewIssue {
        subject_kind: IssueSubjectKind::Match,
        subject_id: match_id,
        code,
        severity,
        source: IssueSource::Integrity,
        detail: json!({ "checked": true }),
    }
}

#[tokio::test]
async fn upsert_keeps_first_seen_and_refreshes_last_seen() {
    let pool = test_pool().await;
    let (match_id, _) = seed_min_match(&pool).await;

    let first_id = issue::upsert_seen(
        &pool,
        &match_issue(match_id, IssueCode::MissingOdds, IssueSeverity::Warning),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE ops.issue SET first_seen_at = now() - interval '1 hour',
                               last_seen_at = now() - interval '1 hour'
          WHERE id = $1",
    )
    .bind(first_id)
    .execute(&pool)
    .await
    .unwrap();

    let mut again = match_issue(match_id, IssueCode::MissingOdds, IssueSeverity::Hard);
    again.detail = json!({ "checked": true, "count": 3 });
    let second_id = issue::upsert_seen(&pool, &again).await.unwrap();

    assert_eq!(first_id, second_id, "a nyitott jel egy sor marad");
    let rows = issue::list(&pool, &IssueFilter::default()).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].first_seen_at < rows[0].last_seen_at);
    assert_eq!(rows[0].severity, "hard");
    assert_eq!(rows[0].detail["count"], 3);
}

#[tokio::test]
async fn resolved_issue_is_kept_but_a_new_open_one_is_created() {
    let pool = test_pool().await;
    let (match_id, _) = seed_min_match(&pool).await;

    let first = issue::upsert_seen(
        &pool,
        &match_issue(match_id, IssueCode::OpenPast, IssueSeverity::Info),
    )
    .await
    .unwrap();
    assert!(issue::resolve(&pool, first).await.unwrap());
    assert!(
        !issue::resolve(&pool, first).await.unwrap(),
        "újra lezárás nem ír"
    );
    let second = issue::upsert_seen(
        &pool,
        &match_issue(match_id, IssueCode::OpenPast, IssueSeverity::Info),
    )
    .await
    .unwrap();
    assert_ne!(first, second);

    let all = issue::list(&pool, &IssueFilter::default()).await.unwrap();
    assert_eq!(all.len(), 2, "a megoldott sor az idővonal miatt megmarad");
    let open = issue::list(
        &pool,
        &IssueFilter {
            open_only: true,
            ..IssueFilter::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].id, second);
}

#[tokio::test]
async fn list_filters_by_day_code_and_severity() {
    let pool = test_pool().await;
    let (match_id, _) = seed_min_match(&pool).await;
    let team_id: Uuid = sqlx::query_scalar("SELECT home_team_id FROM core.match WHERE id = $1")
        .bind(match_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    issue::upsert_seen(
        &pool,
        &match_issue(match_id, IssueCode::MissingOdds, IssueSeverity::Warning),
    )
    .await
    .unwrap();
    issue::upsert_seen(
        &pool,
        &match_issue(match_id, IssueCode::MissingLogo, IssueSeverity::Hard),
    )
    .await
    .unwrap();
    let team_issue = NewIssue {
        subject_kind: IssueSubjectKind::Team,
        subject_id: team_id,
        code: IssueCode::IdentityReview,
        severity: IssueSeverity::Info,
        source: IssueSource::Agent,
        detail: json!({}),
    };
    issue::upsert_seen(&pool, &team_issue).await.unwrap();

    let day = IssueFilter {
        scope: IssueScope::Day(NaiveDate::from_ymd_opt(2026, 11, 14).unwrap()),
        ..IssueFilter::default()
    };
    let rows = issue::list(&pool, &day).await.unwrap();
    assert_eq!(rows.len(), 2, "nap-szűrő csak meccs-tárgyú jeleket ad");

    let other_day = IssueFilter {
        scope: IssueScope::Day(NaiveDate::from_ymd_opt(2026, 11, 15).unwrap()),
        ..IssueFilter::default()
    };
    assert!(issue::list(&pool, &other_day).await.unwrap().is_empty());

    let hard = IssueFilter {
        severity: Some(IssueSeverity::Hard),
        ..IssueFilter::default()
    };
    let hard_rows = issue::list(&pool, &hard).await.unwrap();
    assert_eq!(hard_rows.len(), 1);
    assert_eq!(hard_rows[0].code, "missing_logo");

    let odds = IssueFilter {
        code: Some(IssueCode::MissingOdds),
        ..IssueFilter::default()
    };
    assert_eq!(issue::list(&pool, &odds).await.unwrap().len(), 1);
}

#[tokio::test]
async fn detector_reports_match_gaps_only_for_the_gate_day() {
    let pool = test_pool().await;
    let (match_id, _) = seed_min_match(&pool).await;
    sqlx::query(
        "INSERT INTO ops.publish_gate
          (id, match_id, publish_date, state, rule_set_version, evaluated_at)
         VALUES ($1, $2, '2026-11-14', 'provisional_publish', '1.0', now())",
    )
    .bind(Uuid::now_v7())
    .bind(match_id)
    .execute(&pool)
    .await
    .unwrap();

    let detected = detect::detect_day(&pool, NaiveDate::from_ymd_opt(2026, 11, 14).unwrap())
        .await
        .unwrap();
    assert!(
        detected
            .iter()
            .any(|d| d.subject_id == match_id && d.code == IssueCode::MissingOdds)
    );
    assert!(
        detected
            .iter()
            .any(|d| d.subject_id == match_id && d.code == IssueCode::MissingLogo)
    );
    assert!(
        detected
            .iter()
            .any(|d| d.subject_id == match_id && d.code == IssueCode::OpenPast)
    );
    assert!(
        detect::detect_day(&pool, NaiveDate::from_ymd_opt(2026, 11, 15).unwrap())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn detector_reports_missing_result_for_a_finished_match_without_score() {
    let pool = test_pool().await;
    let (match_id, _) = seed_min_match(&pool).await;
    // Szándékosan nincs core.match_result sor: a lezárt meccsnek nincs eredménye.
    sqlx::query("UPDATE core.match SET status = 'finished' WHERE id = $1")
        .bind(match_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO ops.publish_gate
          (id, match_id, publish_date, state, rule_set_version, evaluated_at)
         VALUES ($1, $2, '2026-11-14', 'provisional_publish', '1.0', now())",
    )
    .bind(Uuid::now_v7())
    .bind(match_id)
    .execute(&pool)
    .await
    .unwrap();

    let detected = detect::detect_day(&pool, NaiveDate::from_ymd_opt(2026, 11, 14).unwrap())
        .await
        .unwrap();
    assert!(
        detected
            .iter()
            .any(|d| d.subject_id == match_id && d.code == IssueCode::MissingResult),
        "a lezárt, eredmény nélküli meccs külön missing_result jelet kap"
    );
    assert!(
        !detected
            .iter()
            .any(|d| d.subject_id == match_id && d.code == IssueCode::StaleStatus),
        "a félrevezető régi kódot a detektor nem írja többé"
    );
    assert!(
        detect::detect_day(&pool, NaiveDate::from_ymd_opt(2026, 11, 15).unwrap())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn note_crud_round_trip() {
    let pool = test_pool().await;
    let (match_id, _) = seed_min_match(&pool).await;

    let created = note::insert(
        &pool,
        match_id,
        "agent:lead",
        "A forrás még nem ad odds-ot.",
        NoteKind::Problem,
    )
    .await
    .unwrap();
    assert_eq!(created.kind, "problem");
    assert_eq!(
        note::list_for_match(&pool, match_id).await.unwrap().len(),
        1
    );

    let updated = note::update(
        &pool,
        created.id,
        &NotePatch {
            body: Some("Az odds megérkezett.".to_owned()),
            kind: Some(NoteKind::Fix),
            resolved: Some(true),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(updated.body, "Az odds megérkezett.");
    assert_eq!(updated.kind, "fix");
    assert!(updated.resolved_at.is_some());
    assert_eq!(updated.author, "agent:lead", "a szerző nem változhat");

    let empty_patch = note::update(&pool, created.id, &NotePatch::default())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(empty_patch.updated_at, updated.updated_at);
}

#[tokio::test]
async fn history_merges_audit_events_and_selected_provenance() {
    let pool = test_pool().await;
    let (match_id, source_id) = seed_min_match(&pool).await;

    sqlx::query(
        "INSERT INTO ops.audit_event
           (id, actor, action, subject_kind, subject_id, detail, created_at)
         VALUES ($1, 'ui', 'manual_match_edit', 'match', $2, $3, '2026-11-20T10:00:00Z')",
    )
    .bind(Uuid::now_v7())
    .bind(match_id)
    .bind(json!({ "before": { "status": "scheduled" }, "after": { "status": "finished" } }))
    .execute(&pool)
    .await
    .unwrap();

    let artifact_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO raw.artifact (id, content_sha256, compression, byte_size, storage_key, media_type)
         VALUES ($1, $2, 'none', 10, $3, 'application/json')",
    )
    .bind(artifact_id)
    .bind(format!("sha-{match_id}"))
    .bind(format!("artifacts/{match_id}.json"))
    .execute(&pool)
    .await
    .unwrap();
    let capture_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO raw.capture
           (id, source_id, artifact_id, endpoint_key, request_url, requested_at,
            received_at, http_status, adapter_version, parser_version, request_key)
         VALUES ($1, $2, $3, 'test', 'http://127.0.0.1/test', now(), now(), 200, 't', 't', $4)",
    )
    .bind(capture_id)
    .bind(source_id)
    .bind(artifact_id)
    .bind(format!("capture-{match_id}"))
    .execute(&pool)
    .await
    .unwrap();
    let observation_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO stage.observation
           (id, source_id, capture_id, entity_kind, aspect, natural_key,
            source_record_key, observed_at, normalized_payload, schema_version, content_hash)
         VALUES ($1, $2, $3, 'match', 'match_core', $4, $5,
                 '2026-11-21T10:00:00Z', '{}'::jsonb, '1.0', $6)",
    )
    .bind(observation_id)
    .bind(source_id)
    .bind(capture_id)
    .bind(match_id.to_string())
    .bind(format!("obs-{match_id}"))
    .bind(format!("hash-{match_id}"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO fact.provenance
           (subject_kind, subject_id, field_key, observation_id, is_selected,
            policy_version, valid_from)
         VALUES ('match', $1, 'kickoff_at', $2, true, 'test-1.0', '2026-11-21T10:00:00Z')",
    )
    .bind(match_id)
    .bind(observation_id)
    .execute(&pool)
    .await
    .unwrap();

    let entries = history::list_for_match(&pool, match_id, 50).await.unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].origin, "provenance", "a frissebb elöl");
    assert_eq!(entries[0].field_key.as_deref(), Some("kickoff_at"));
    assert_eq!(entries[0].source.as_deref(), Some("seed"));
    assert_eq!(entries[1].origin, "audit");
    assert_eq!(entries[1].action, "manual_match_edit");
    assert_eq!(entries[1].before.as_ref().unwrap()["status"], "scheduled");
    assert_eq!(entries[1].after.as_ref().unwrap()["status"], "finished");

    assert!(
        history::list_for_match(&pool, match_id, 1)
            .await
            .unwrap()
            .len()
            == 1
    );
}
