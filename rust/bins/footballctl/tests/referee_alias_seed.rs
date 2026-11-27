//! Regressziós teszt a `referee-alias-seed` szelethez (S-szelet B):
//! evidence-alapú pontos (foldolt) egyezés egy referee-entitásra, dry-run,
//! ambiguous -> pending review, no-match -> pending review, research
//! megjelenítés + core.match.referee_entity_id érintetlen, nincs fuzzy.
//! `--test-threads=1` kötelező (közös teszt-DB).

use std::process::Command;

use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

fn run_cli(args: &[&str]) -> (String, bool) {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .expect("TEST_DATABASE_URL kötelező az integrációs teszthez");
    let output = Command::new(env!("CARGO_BIN_EXE_footballctl"))
        .args(args)
        .env("DATABASE_URL", &database_url)
        .env(
            "DATA_DIR",
            std::env::temp_dir().join(format!("footballctl-referee-{}", Uuid::now_v7())),
        )
        .output()
        .expect("a footballctl futtatása");
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        output.status.success(),
    )
}

async fn seed_match(
    pool: &PgPool,
    source_id: Uuid,
    referee_entity_id: Option<Uuid>,
    official_name: &str,
    tag: &str,
) -> Uuid {
    // minimál ország/verseny/szezon/csapat lánc egyedi taggel
    let country = Uuid::now_v7();
    let comp_entity = Uuid::now_v7();
    let comp = Uuid::now_v7();
    let season = Uuid::now_v7();
    let home_ent = Uuid::now_v7();
    let home = Uuid::now_v7();
    let away_ent = Uuid::now_v7();
    let away = Uuid::now_v7();
    let match_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,$2,$3)")
        .bind(country)
        .bind(format!("C{tag}"))
        .bind(tag)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition',$2)")
        .bind(comp_entity)
        .bind(tag)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name) VALUES ($1,$2,$3,$4)",
    )
    .bind(comp)
    .bind(comp_entity)
    .bind(country)
    .bind(tag)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season)
        .bind(comp)
        .execute(pool)
        .await
        .unwrap();
    for (ent, team, name) in [
        (home_ent, home, format!("H{tag}")),
        (away_ent, away, format!("A{tag}")),
    ] {
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(ent)
            .bind(&name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch) VALUES ($1,$2,$3,$4,'male','senior','senior')")
            .bind(team).bind(ent).bind(country).bind(&name).execute(pool).await.unwrap();
    }
    sqlx::query(
        "INSERT INTO core.match (id, competition_id, season_id, home_team_id, away_team_id,
         kickoff_at, competition_timezone, status, referee_entity_id)
         VALUES ($1,$2,$3,$4,$5,'2026-11-14T18:00:00Z','UTC','scheduled',$6)",
    )
    .bind(match_id)
    .bind(comp)
    .bind(season)
    .bind(home)
    .bind(away)
    .bind(referee_entity_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO fact.match_official (match_id, source_id, role, name, captured_at)
         VALUES ($1,$2,'referee',$3,'2026-11-14T19:00:00Z')",
    )
    .bind(match_id)
    .bind(source_id)
    .bind(official_name)
    .execute(pool)
    .await
    .unwrap();
    match_id
}

#[tokio::test]
async fn referee_alias_seed_evidence_only_dry_run_apply() {
    let pool = test_pool().await;
    let research = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ops.source (id, code, display_name) VALUES ($1,'research','Research')",
    )
    .bind(research)
    .execute(&pool)
    .await
    .unwrap();

    // referee entitás, más elírású canonical név
    let referee = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'referee','Mohammed Al Hoaish')")
        .bind(referee).execute(&pool).await.unwrap();

    // evidence: linked meccs + research név "Mohammed Al-Huwaish"
    // (foldolt != canonical, DE match-level evidence -> seedable, nincs kitalált egyezés)
    seed_match(&pool, research, Some(referee), "Mohammed Al-Huwaish", "a").await;
    // no-match: research név csak NULL-linked meccsen, nincs evidence -> pending no_exact_match
    let null_match = seed_match(&pool, research, None, "Totally Unknown Ref", "b").await;

    // --- dry-run nem ír ---
    let (dry_out, dry_ok) = run_cli(&["referee-alias-seed"]);
    assert!(dry_ok, "{dry_out}");
    assert!(dry_out.contains("dry-run"), "{dry_out}");
    let alias_count_dry: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.alias")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(alias_count_dry, 0, "dry-run nem írhat");
    // research megjelenítés + match érintetlen dry-run után
    let official_dry: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fact.match_official WHERE match_id=$1 AND name='Mohammed Al-Huwaish'",
    )
    .bind(null_match)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(official_dry, 0);

    // --- apply ---
    let (apply_out, apply_ok) = run_cli(&["referee-alias-seed", "--apply"]);
    assert!(apply_ok, "{apply_out}");

    // reviewed alias beszúrva, kitalált egyezés nélkül
    let alias: (Uuid, String, String, String) = sqlx::query_as(
        "SELECT entity_id, alias, status, method FROM identity.alias WHERE alias='Mohammed Al-Huwaish'",
    )
    .fetch_one(&pool).await.expect("alias beszúrva");
    assert_eq!(alias.0, referee);
    assert_eq!(alias.2, "reviewed");
    assert_eq!(alias.3, "import");

    // nem ismert név: NINCS kitalált alias, marad NULL + pending review
    let unknown_alias: i64 =
        sqlx::query_scalar("SELECT count(*) FROM identity.alias WHERE alias='Totally Unknown Ref'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unknown_alias, 0);
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM identity.review_queue
         WHERE entity_kind='referee' AND status='pending' AND reason='no_exact_match'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(pending >= 1, "no-match -> pending review");
    let match_ref_unknown: Option<Uuid> =
        sqlx::query_scalar("SELECT referee_entity_id FROM core.match WHERE id=$1")
            .bind(null_match)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(match_ref_unknown.is_none(), "no-match marad NULL");

    // research megjelenítés változatlan
    let official_name: String = sqlx::query_scalar(
        "SELECT name FROM fact.match_official WHERE match_id=$1 AND source_id=$2",
    )
    .bind(null_match)
    .bind(research)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(official_name, "Totally Unknown Ref");

    // az evidence-szel rendelkező meccs referee_entity_id nem módosul a CLI-vel
    let linked: Option<Uuid> = sqlx::query_scalar(
        "SELECT referee_entity_id FROM core.match WHERE referee_entity_id=$1 LIMIT 1",
    )
    .bind(referee)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked, Some(referee));
}

#[tokio::test]
async fn ambiguous_evidence_is_pending_not_seeded() {
    let pool = test_pool().await;
    let research = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ops.source (id, code, display_name) VALUES ($1,'research','Research')",
    )
    .bind(research)
    .execute(&pool)
    .await
    .unwrap();

    let r1 = Uuid::now_v7();
    let r2 = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'referee','Ref One')",
    )
    .bind(r1)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'referee','Ref Two')",
    )
    .bind(r2)
    .execute(&pool)
    .await
    .unwrap();

    // ugyanaz a foldolt név két különböző entitásra mutat -> ambiguous
    seed_match(&pool, research, Some(r1), "Common Ref", "c").await;
    seed_match(&pool, research, Some(r2), "Common Ref", "d").await;

    let (_out, ok) = run_cli(&["referee-alias-seed", "--apply"]);
    assert!(ok);
    let seeded: i64 =
        sqlx::query_scalar("SELECT count(*) FROM identity.alias WHERE alias='Common Ref'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(seeded, 0, "ambiguous nem seed");
    let pending_multi: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM identity.review_queue
         WHERE entity_kind='referee' AND status='pending' AND reason='multiple_candidates'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(pending_multi >= 1, "ambiguous -> pending review");
}
