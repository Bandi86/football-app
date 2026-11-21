use chrono::{TimeZone, Utc};
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::fixture::{FixtureAdapter, FixtureOdds, FixtureSource};
use football_ingest::normalized::{
    NormalizedCompetition, NormalizedDay, NormalizedMatch, NormalizedSeason, NormalizedTeam,
};
use football_storage::object_store::ObjectStore;
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

#[tokio::test]
async fn fixture_materializes_one_canonical_match_with_exact_identity_and_provenance() {
    let pool = test_pool().await;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();

    FixtureAdapter::ingest(&pool, &store, &fixture_path)
        .await
        .unwrap();
    let fixture: FixtureSource =
        serde_json::from_slice(&std::fs::read(&fixture_path).unwrap()).unwrap();

    let day = Canonicalizer::materialize(
        &pool,
        &football_ingest::normalized::NormalizedDay::from(&fixture),
    )
    .await
    .unwrap();

    assert_eq!(day.matches.len(), 1);
    assert_eq!(
        day.matches[0].home_team_id.to_string(),
        "10000000-0000-7000-8000-000000000011"
    );
    assert_eq!(
        day.matches[0].away_team_id.to_string(),
        "10000000-0000-7000-8000-000000000012"
    );

    let refs: i64 = sqlx::query_scalar("SELECT count(*) FROM identity.source_entity_ref")
        .fetch_one(&pool)
        .await
        .unwrap();
    let provenance: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.provenance")
        .fetch_one(&pool)
        .await
        .unwrap();

    // 2 csapat-ref + 1 meccs-szintű forrás-híd (S37b)
    assert_eq!(refs, 3);
    assert!(provenance >= 5);
}

/// S2b-2 (Task 3 halasztott minor): újra-ingestnél az odds sor trust_state-je
/// provisional-re áll vissza — a verified állapotot csak a gate utáni
/// odds-kiértékelés adhatja vissza, a canonicalize nem őrizheti meg.
#[tokio::test]
async fn reingest_resets_odds_trust_state_to_provisional() {
    let pool = test_pool().await;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();

    // A golden fixture odds mezőkkel kiegészítve (a fájl érintetlen marad).
    let mut fixture: FixtureSource =
        serde_json::from_slice(&std::fs::read(&fixture_path).unwrap()).unwrap();
    fixture.matches[0].odds = Some(FixtureOdds {
        home: 2.0,
        draw: 3.2,
        away: 3.5,
        btts_yes: 1.8,
        btts_no: 1.95,
        over25: 1.9,
        under25: 1.85,
    });
    fixture.matches[0].market_count = Some(7);
    let odds_fixture_path = temp.path().join("fixture_a_odds.json");
    std::fs::write(&odds_fixture_path, serde_json::to_vec(&fixture).unwrap()).unwrap();

    FixtureAdapter::ingest(&pool, &store, &odds_fixture_path)
        .await
        .unwrap();
    Canonicalizer::materialize(
        &pool,
        &football_ingest::normalized::NormalizedDay::from(&fixture),
    )
    .await
    .unwrap();
    let state: String = sqlx::query_scalar("SELECT trust_state FROM fact.odds_snapshot")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "provisional");

    // Mesterségesen verified állapot (mintha a gate utáni kiértékelés állította
    // volna be); az újra-materializálásnak vissza kell állítania provisional-re.
    sqlx::query("UPDATE fact.odds_snapshot SET trust_state='verified'")
        .execute(&pool)
        .await
        .unwrap();
    Canonicalizer::materialize(
        &pool,
        &football_ingest::normalized::NormalizedDay::from(&fixture),
    )
    .await
    .unwrap();

    let state: String = sqlx::query_scalar("SELECT trust_state FROM fact.odds_snapshot")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "provisional");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.odds_snapshot")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
}

#[tokio::test]
async fn ingest_writes_odds_quotes_with_default_bookmaker() {
    let pool = test_pool().await;
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/fixture_a_day.json");
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();
    let mut fixture: FixtureSource =
        serde_json::from_slice(&std::fs::read(&fixture_path).unwrap()).unwrap();
    fixture.matches[0].odds = Some(FixtureOdds {
        home: 2.0,
        draw: 3.2,
        away: 3.5,
        btts_yes: 1.8,
        btts_no: 1.95,
        over25: 1.9,
        under25: 1.85,
    });
    fixture.matches[0].market_count = Some(7);
    let odds_fixture_path = temp.path().join("fixture_a_odds.json");
    std::fs::write(&odds_fixture_path, serde_json::to_vec(&fixture).unwrap()).unwrap();
    FixtureAdapter::ingest(&pool, &store, &odds_fixture_path)
        .await
        .unwrap();
    Canonicalizer::materialize(
        &pool,
        &football_ingest::normalized::NormalizedDay::from(&fixture),
    )
    .await
    .unwrap();

    let quotes: Vec<(String, String, f64, f64)> = sqlx::query_as(
        "SELECT market, outcome, line::float8, price::float8
         FROM fact.odds_quote
         WHERE match_id=$1 AND bookmaker='default' AND phase='closing'
         ORDER BY market, outcome",
    )
    .bind(fixture.matches[0].id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(quotes.len(), 7, "hét kanonikus quote íródik");
    assert_eq!(quotes[0], ("1x2".into(), "away".into(), 0.0, 3.5));
    assert_eq!(quotes[1], ("1x2".into(), "draw".into(), 0.0, 3.2));
    assert_eq!(quotes[2], ("1x2".into(), "home".into(), 0.0, 2.0));
    assert_eq!(quotes[5], ("ou".into(), "over".into(), 2.5, 1.9));
    assert_eq!(quotes[6], ("ou".into(), "under".into(), 2.5, 1.85));

    // Re-materializálás: a blokk cserélődik, nem duplikálódik.
    Canonicalizer::materialize(
        &pool,
        &football_ingest::normalized::NormalizedDay::from(&fixture),
    )
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM fact.odds_quote")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 7);
}

// ---------------------------------------------------------------------------
// S1.6/S2.9: create-on-miss alias ország és nyelv
// ---------------------------------------------------------------------------

async fn seed_create_on_miss_source(pool: &PgPool, code: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ops.source (id, code, display_name, identity_mode)
         VALUES ($1,$2,'Teszt forrás','create_on_miss')",
    )
    .bind(id)
    .bind(code)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

fn create_on_miss_day(source_code: &str) -> NormalizedDay {
    let kickoff = Utc.with_ymd_and_hms(2026, 11, 21, 15, 0, 0).unwrap();
    let team = |ext: &str, name: &str, country: &str| NormalizedTeam {
        id: None,
        source_entity_id: ext.into(),
        name: name.into(),
        country_code: country.into(),
        gender: "male".into(),
        age_group: "senior".into(),
        branch: "senior".into(),
    };
    NormalizedDay {
        source_code: source_code.into(),
        captured_at: kickoff,
        competition: NormalizedCompetition {
            id: None,
            name: "Ligue 2".into(),
            country_code: "FR".into(),
            country_name: "Franciaország".into(),
            tier: 2,
            priority: 5,
        },
        season: NormalizedSeason {
            id: None,
            label: "2026/27".into(),
        },
        teams: vec![
            // Semleges (INT) forrás-ország: az alias országa NULL, nem INT.
            team("s1", "Épinal", "INT"),
            // ISO3 megfigyelés: az alias országa a kanonikus ISO2 (FR).
            team("s2", "Bourg-Péronnas", "FRA"),
        ],
        matches: vec![NormalizedMatch {
            id: None,
            source_match_id: "m-1".into(),
            home_source_entity_id: "s1".into(),
            away_source_entity_id: "s2".into(),
            round_label: None,
            kickoff_at: kickoff,
            kickoff_precision: "exact".into(),
            competition_timezone: "Europe/Paris".into(),
            status: "scheduled".into(),
            result: None,
            odds: None,
            stats: None,
            referee: None,
        }],
        standings: None,
    }
}

#[tokio::test]
async fn create_on_miss_alias_country_is_canonical_not_int() {
    let pool = test_pool().await;
    let source_code = "test-create-on-miss";
    seed_create_on_miss_source(&pool, source_code).await;

    let materialized = Canonicalizer::materialize(&pool, &create_on_miss_day(source_code))
        .await
        .unwrap();
    assert_eq!(materialized.matches.len(), 1, "a meccs materializálódik");
    assert_eq!(count(&pool, "SELECT count(*) FROM core.team").await, 2);

    let aliases: Vec<(String, String, Option<String>, String, String)> = sqlx::query_as(
        "SELECT alias, lang, country_code, status, method FROM identity.alias ORDER BY alias",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(aliases.len(), 2, "minden új csapathoz alias íródik");
    // A hamis 'hu' helyett semleges 'und' nyelv.
    for (_, lang, _, status, method) in &aliases {
        assert_eq!(lang, "und");
        assert_eq!(status, "reviewed");
        assert_eq!(method, "import");
    }
    let epinal = aliases.iter().find(|(a, ..)| a == "Épinal").unwrap();
    assert_eq!(
        epinal.2, None,
        "semleges INT ország → NULL alias-ország, nem INT/XX"
    );
    let bourg = aliases
        .iter()
        .find(|(a, ..)| a == "Bourg-Péronnas")
        .unwrap();
    assert_eq!(
        bourg.2.as_deref(),
        Some("FR"),
        "ISO3 megfigyelés → kanonikus ISO2 alias-ország"
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM identity.alias WHERE country_code IN ('INT','XX','')",
        )
        .await,
        0,
        "az alias soha nem INT/XX/üres"
    );
}

/// S2.9: nemzetközi (INT) versenyben az ékezet nélküli név a fold révén
/// megtalálja a korábban létrehozott ékezetes csapatot, és nem születik
/// párhuzamos XX-országú entitás.
#[tokio::test]
async fn inferred_country_uses_fold_name_comparison() {
    let pool = test_pool().await;
    let source_code = "test-create-on-miss";
    let _source_id = seed_create_on_miss_source(&pool, source_code).await;
    let fr_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'FR','Franciaország')")
        .bind(fr_id)
        .execute(&pool)
        .await
        .unwrap();
    // Korábban létrehozott csapat ékezetes kanonikus névvel.
    let existing = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team','Épinal')")
        .bind(existing)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.team
         (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$1,$2,'Épinal','male','senior','senior')",
    )
    .bind(existing)
    .bind(fr_id)
    .execute(&pool)
    .await
    .unwrap();

    let day = create_on_miss_day(source_code);
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(materialized.matches.len(), 1);
    // A második csapat új entitás (nincs seedelt párja), az első variánsa a
    // fold miatt az "Épinal" entitásra oldódik: nem lesz 3. csapat.
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.team").await,
        2,
        "az 'Epinal' variáns nem hoz létre párhuzamos csapatot"
    );
    let epinal_country: String = sqlx::query_scalar(
        "SELECT co.code FROM core.team t JOIN core.country co ON co.id=t.country_id
         WHERE t.entity_id=$1",
    )
    .bind(existing)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(epinal_country, "FR", "a valódi ország megmarad");
}
