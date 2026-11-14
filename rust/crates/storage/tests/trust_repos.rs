//! S2b-2 Task 3: storage trust repository-k integrációs tesztjei.
//!
//! upsert_trust + trust_for_sources (két forrás, domainenkénti sorok),
//! load_match_odds (két forrás oddsai + provider, fail-closed hiányzó mező),
//! update_odds_trust_state (a meccs összes odds sora egy állapotra).

use football_domain::trust::{OddsTrustState, TrustClass};
use football_storage::repositories::trust::{
    load_match_odds, trust_for_sources, update_odds_trust_state, upsert_trust,
};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

// (market, outcome, line, price) — a default/closing quote-blokk.
const FULL_ODDS_A: &[(&str, &str, f64, f64)] = &[
    ("1x2", "home", 0.0, 1.85),
    ("1x2", "draw", 0.0, 3.4),
    ("1x2", "away", 0.0, 4.2),
    ("btts", "yes", 0.0, 1.7),
    ("btts", "no", 0.0, 2.1),
    ("ou", "over", 2.5, 1.9),
    ("ou", "under", 2.5, 1.9),
];
const FULL_ODDS_B: &[(&str, &str, f64, f64)] = &[
    ("1x2", "home", 0.0, 1.9),
    ("1x2", "draw", 0.0, 3.3),
    ("1x2", "away", 0.0, 4.0),
    ("btts", "yes", 0.0, 1.75),
    ("btts", "no", 0.0, 2.05),
    ("ou", "over", 2.5, 1.95),
    ("ou", "under", 2.5, 1.85),
];

#[tokio::test]
async fn upsert_trust_and_trust_for_sources_per_domain() {
    let pool = test_pool().await;
    let alpha = seed_source(&pool, "alpha", "Alpha").await;
    let beta = seed_source(&pool, "beta", "Béta").await;
    // Zavaró harmadik forrás: nem kérjük le, nem szerepelhet az eredményben.
    let gamma = seed_source(&pool, "gamma", "Gamma").await;
    upsert_trust(&pool, gamma, "odds", TrustClass::High, None)
        .await
        .unwrap();

    upsert_trust(&pool, alpha, "odds", TrustClass::High, Some("ProviderA"))
        .await
        .unwrap();
    upsert_trust(&pool, alpha, "result", TrustClass::Medium, None)
        .await
        .unwrap();
    upsert_trust(&pool, beta, "odds", TrustClass::DisplayOnly, None)
        .await
        .unwrap();
    // Upsert ugyanarra a (source, domain) kulcsra: frissít, nem duplikál.
    upsert_trust(&pool, alpha, "odds", TrustClass::Medium, Some("ProviderB"))
        .await
        .unwrap();

    let rows = trust_for_sources(&pool, &["alpha".to_string(), "beta".to_string()])
        .await
        .unwrap();
    assert_eq!(rows.len(), 3, "domainenkénti sorok, duplikátum nélkül");

    let alpha_odds = rows
        .iter()
        .find(|r| r.source_code == "alpha" && r.domain == "odds")
        .expect("alpha odds trust sor");
    assert_eq!(alpha_odds.trust_class, TrustClass::Medium);
    assert_eq!(alpha_odds.provider.as_deref(), Some("ProviderB"));

    let alpha_result = rows
        .iter()
        .find(|r| r.source_code == "alpha" && r.domain == "result")
        .expect("alpha result trust sor");
    assert_eq!(alpha_result.trust_class, TrustClass::Medium);
    assert_eq!(alpha_result.provider, None);

    let beta_odds = rows
        .iter()
        .find(|r| r.source_code == "beta" && r.domain == "odds")
        .expect("beta odds trust sor");
    assert_eq!(beta_odds.trust_class, TrustClass::DisplayOnly);
    assert_eq!(beta_odds.provider, None);

    assert!(rows.iter().all(|r| r.source_code != "gamma"));

    let empty = trust_for_sources(&pool, &[]).await.unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn load_match_odds_two_sources_with_provider() {
    let pool = test_pool().await;
    let match_id = seed_match(&pool).await;
    let alpha = seed_source(&pool, "alpha", "Alpha").await;
    let beta = seed_source(&pool, "beta", "Béta").await;
    // Csak az odds domainű trust sor ad providert; a result domainű nem.
    upsert_trust(&pool, alpha, "odds", TrustClass::High, Some("ProviderA"))
        .await
        .unwrap();
    upsert_trust(
        &pool,
        alpha,
        "result",
        TrustClass::High,
        Some("MásikProvider"),
    )
    .await
    .unwrap();
    // beta: nincs trust sor → provider None (LEFT JOIN).
    insert_odds(&pool, match_id, alpha, FULL_ODDS_A, "provisional").await;
    insert_odds(&pool, match_id, beta, FULL_ODDS_B, "provisional").await;

    let odds = load_match_odds(&pool, match_id).await.unwrap();
    assert_eq!(odds.len(), 2);

    let a = odds.iter().find(|o| o.source_code == "alpha").unwrap();
    assert_eq!(a.provider.as_deref(), Some("ProviderA"));
    assert_eq!(a.odds.home, 1.85);
    assert_eq!(a.odds.draw, 3.4);
    assert_eq!(a.odds.away, 4.2);
    assert_eq!(a.odds.btts_yes, 1.7);
    assert_eq!(a.odds.btts_no, 2.1);
    assert_eq!(a.odds.over25, 1.9);
    assert_eq!(a.odds.under25, 1.9);

    let b = odds.iter().find(|o| o.source_code == "beta").unwrap();
    assert_eq!(b.provider, None);
    assert_eq!(b.odds.home, 1.9);
    assert_eq!(b.odds.under25, 1.85);

    // Más meccs oddsai nem szivárognak be.
    let other_match = seed_match(&pool).await;
    let empty = load_match_odds(&pool, other_match).await.unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn load_match_odds_missing_field_is_error() {
    let pool = test_pool().await;
    let match_id = seed_match(&pool).await;
    let source = seed_source(&pool, "alpha", "Alpha").await;
    // Hiányzó under25: fail-closed, nem nulla és nem unwrap_or.
    let missing_under25: &[(&str, &str, f64, f64)] = &[
        ("1x2", "home", 0.0, 1.85),
        ("1x2", "draw", 0.0, 3.4),
        ("1x2", "away", 0.0, 4.2),
        ("btts", "yes", 0.0, 1.7),
        ("btts", "no", 0.0, 2.1),
        ("ou", "over", 2.5, 1.9),
    ];
    insert_odds(&pool, match_id, source, missing_under25, "provisional").await;

    let err = load_match_odds(&pool, match_id)
        .await
        .expect_err("hiányzó odds mező hibát kell adjon");
    let msg = format!("{err:#}");
    assert!(msg.contains("hiányzó odds mező"), "hibaüzenet: {msg}");
    assert!(msg.contains("under25"), "hibaüzenet: {msg}");
}

#[tokio::test]
async fn update_odds_trust_state_marks_all_rows_verified() {
    let pool = test_pool().await;
    let match_id = seed_match(&pool).await;
    let alpha = seed_source(&pool, "alpha", "Alpha").await;
    let beta = seed_source(&pool, "beta", "Béta").await;
    insert_odds(&pool, match_id, alpha, FULL_ODDS_A, "provisional").await;
    insert_odds(&pool, match_id, beta, FULL_ODDS_B, "provisional").await;

    let affected = update_odds_trust_state(&pool, match_id, OddsTrustState::Verified)
        .await
        .unwrap();
    assert_eq!(affected, 2);

    let states: Vec<String> = sqlx::query_scalar(
        "SELECT trust_state FROM fact.odds_snapshot WHERE match_id=$1 ORDER BY source_id",
    )
    .bind(match_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(states, vec!["verified".to_string(), "verified".to_string()]);

    // Odds nélküli meccs: 0 érintett sor, nem hiba.
    let other_match = seed_match(&pool).await;
    let zero = update_odds_trust_state(&pool, other_match, OddsTrustState::Provisional)
        .await
        .unwrap();
    assert_eq!(zero, 0);
}

async fn seed_source(pool: &PgPool, code: &str, display_name: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO ops.source (id, code, display_name) VALUES ($1,$2,$3) RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(code)
    .bind(display_name)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_odds(
    pool: &PgPool,
    match_id: Uuid,
    source_id: Uuid,
    quotes: &[(&str, &str, f64, f64)],
    state: &str,
) {
    sqlx::query(
        "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count, trust_state)
         VALUES ($1,$2,now(),18,$3)",
    )
    .bind(match_id)
    .bind(source_id)
    .bind(state)
    .execute(pool)
    .await
    .unwrap();
    for (market, outcome, line, price) in quotes {
        sqlx::query(
            "INSERT INTO fact.odds_quote
             (match_id, source_id, bookmaker, market, outcome, line, phase, price)
             VALUES ($1,$2,'default',$3,$4,$5,'closing',$6)",
        )
        .bind(match_id)
        .bind(source_id)
        .bind(market)
        .bind(outcome)
        .bind(line)
        .bind(price)
        .execute(pool)
        .await
        .unwrap();
    }
}

// Minimális kanonikus lánc egy meccshez: country → competition → season →
// csapatok → match (a trust_schema/day_query tesztek seedmintája).
async fn seed_match(pool: &PgPool) -> Uuid {
    let competition_entity_id = Uuid::now_v7();
    let competition_id = Uuid::now_v7();
    let season_id = Uuid::now_v7();
    let match_id = Uuid::now_v7();

    // Egy teszten belül több meccs is épülhet: az ország upsert, a többi láncszem új.
    let country_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Hungary')
         ON CONFLICT (code) DO UPDATE SET name = EXCLUDED.name RETURNING id",
    )
    .bind(Uuid::now_v7())
    .fetch_one(pool)
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
        "INSERT INTO core.competition (id, entity_id, country_id, name) VALUES ($1,$2,$3,'NB I')",
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

    let mut team_ids = Vec::new();
    for name in ["Újpest", "Győr"] {
        let entity_id = Uuid::now_v7();
        let team_id = Uuid::now_v7();
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
        team_ids.push(team_id);
    }

    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, round_label, home_team_id, away_team_id,
          kickoff_at, competition_timezone, status)
         VALUES ($1,$2,$3,'12',$4,$5,now(),'Europe/Budapest','scheduled')",
    )
    .bind(match_id)
    .bind(competition_id)
    .bind(season_id)
    .bind(team_ids[0])
    .bind(team_ids[1])
    .execute(pool)
    .await
    .unwrap();

    match_id
}
