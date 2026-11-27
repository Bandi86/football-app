//! Regressziós teszt a `season-stats-import` szelethez (S-szelet A):
//! valódi CLI-folyamat a teszt-DB-n. exact season_id/team_id párosítás,
//! numeric-only, source_id=research, captured_at=match_insight.captured_at,
//! ON CONFLICT DO NOTHING, dry-run. `--test-threads=1` kötelező.

use std::process::Command;

use football_storage::test_support::{seed_min_match, test_pool};
use uuid::Uuid;

fn run_cli(args: &[&str]) -> (String, bool) {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL kötelező az integrációs teszthez");
    let output = Command::new(env!("CARGO_BIN_EXE_footballctl"))
        .args(args)
        .env("DATABASE_URL", &database_url)
        .env(
            "DATA_DIR",
            std::env::temp_dir().join(format!("footballctl-season-stats-{}", Uuid::now_v7())),
        )
        .output()
        .expect("a footballctl futtatása");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        output.status.success(),
    )
}

#[tokio::test]
async fn season_stats_import_dry_run_apply_and_idempotent() {
    let pool = test_pool().await;
    let (match_id, _seed_source) = seed_min_match(&pool).await;

    let research_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ops.source (id, code, display_name) VALUES ($1,'research','Research')",
    )
    .bind(research_id)
    .execute(&pool)
    .await
    .expect("research source");

    let (season_id, home_team_id, away_team_id): (Uuid, Uuid, Uuid) =
        sqlx::query_as("SELECT season_id, home_team_id, away_team_id FROM core.match WHERE id=$1")
            .bind(match_id)
            .fetch_one(&pool)
            .await
            .expect("match row");

    let captured_at = chrono::DateTime::parse_from_rfc3339("2026-11-14T19:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);

    let season_stats = serde_json::json!({
        "home": {
            "wins": 3, "losses": 1, "points": 9, "goals_for": 7,
            "clean_sheets": null,            // nem kitalált 0
            "possession_pct": "55.0",        // szöveg -> nem szám
            "group": "NB I", "source": "fotmob_table", "rank_label": "1/12",
            "matches": 4
        },
        "away": { "wins": 1, "losses": 3, "points": 3 }
    });

    sqlx::query(
        "INSERT INTO fact.match_insight (match_id, source_id, captured_at, season_stats)
         VALUES ($1,$2,$3,$4)",
    )
    .bind(match_id)
    .bind(research_id)
    .bind(captured_at)
    .bind(&season_stats)
    .execute(&pool)
    .await
    .expect("match_insight");

    // --- dry-run: nem ír ---
    let (dry_out, dry_ok) = run_cli(&["season-stats-import"]);
    assert!(dry_ok, "{dry_out}");
    assert!(dry_out.contains("dry-run"), "{dry_out}");
    assert!(dry_out.contains("8 jelölt sor"), "{dry_out}");
    assert!(dry_out.contains("2 nem számszerű"), "{dry_out}");
    let count_dry: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.season_team_stats")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count_dry, 0, "dry-run nem írhat");

    // --- apply ---
    let (apply_out, apply_ok) = run_cli(&["season-stats-import", "--apply"]);
    assert!(apply_ok, "{apply_out}");
    assert!(apply_out.contains("8 beszúrt"), "{apply_out}");
    assert!(apply_out.contains("backup"), "{apply_out}");

    let rows: Vec<(
        Uuid,
        Uuid,
        Uuid,
        String,
        chrono::DateTime<chrono::Utc>,
        String,
    )> = sqlx::query_as(
        "SELECT season_id, team_id, source_id, stat_key, captured_at, value::text
             FROM fact.season_team_stats ORDER BY team_id, stat_key",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 8);
    assert!(rows.iter().all(|r| r.0 == season_id));
    assert!(rows.iter().all(|r| r.2 == research_id));
    assert!(rows.iter().all(|r| r.4 == captured_at));
    assert!(
        rows.iter()
            .any(|r| r.1 == home_team_id && r.3 == "wins" && r.5 == "3")
    );
    assert!(
        rows.iter()
            .any(|r| r.1 == away_team_id && r.3 == "points" && r.5 == "3")
    );
    assert!(
        !rows.iter().any(|r| r.3 == "clean_sheets"),
        "nem kitalált 0"
    );

    // match_team_stats / season_player_stats érintetlen
    let mt: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.match_team_stats")
        .fetch_one(&pool)
        .await
        .unwrap();
    let sp: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.season_player_stats")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(mt, 0);
    assert_eq!(sp, 0);

    // --- idempotens: ON CONFLICT DO NOTHING ---
    let (_again_out, again_ok) = run_cli(&["season-stats-import", "--apply"]);
    assert!(again_ok);
    let count_again: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.season_team_stats")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count_again, 8);
}
