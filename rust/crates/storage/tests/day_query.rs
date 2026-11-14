use chrono::{NaiveDate, TimeZone, Utc};
use football_storage::repositories::day::{DayFilter, fetch_day};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

struct MatchBase {
    competition_id: Uuid,
    season_id: Uuid,
    home_team_id: Uuid,
    away_team_id: Uuid,
}

#[tokio::test]
async fn default_day_contains_only_verified_publish_matches() {
    let pool = test_pool().await;
    let base = seed_match_base(&pool).await;
    let match_id = seed_match(
        &pool,
        &base,
        "verified_publish",
        Utc.with_ymd_and_hms(2026, 11, 9, 18, 0, 0).unwrap(),
    )
    .await;
    let provisional_id = seed_match(
        &pool,
        &base,
        "provisional_publish",
        Utc.with_ymd_and_hms(2026, 11, 9, 19, 0, 0).unwrap(),
    )
    .await;

    let rows = fetch_day(
        &pool,
        &DayFilter {
            date: NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
            timezone: "Europe/Budapest".into(),
            include_all: false,
        },
    )
    .await
    .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].match_id, match_id);
    assert_ne!(rows[0].match_id, provisional_id);

    let all = fetch_day(
        &pool,
        &DayFilter {
            date: NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
            timezone: "Europe/Budapest".into(),
            include_all: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn day_row_includes_display_fields() {
    let pool = test_pool().await;
    let base = seed_match_base(&pool).await;
    let match_id = seed_match(
        &pool,
        &base,
        "verified_publish",
        Utc.with_ymd_and_hms(2026, 11, 9, 18, 0, 0).unwrap(),
    )
    .await;

    sqlx::query("UPDATE core.competition SET tier=1, priority=6 WHERE id=$1")
        .bind(base.competition_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.match_result (match_id, home_ft, away_ft, home_ht, away_ht)
         VALUES ($1,2,1,1,0)",
    )
    .bind(match_id)
    .execute(&pool)
    .await
    .unwrap();
    let source_id = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,'demo','Demó')")
        .bind(source_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count)
         VALUES ($1,$2,now(),18)",
    )
    .bind(match_id)
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
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.standing (season_id, team_id, position, source_id, captured_at)
         VALUES ($1,$2,8,$3,now()),($1,$4,6,$3,now())",
    )
    .bind(base.season_id)
    .bind(base.home_team_id)
    .bind(source_id)
    .bind(base.away_team_id)
    .execute(&pool)
    .await
    .unwrap();

    let rows = fetch_day(
        &pool,
        &DayFilter {
            date: NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
            timezone: "Europe/Budapest".into(),
            include_all: false,
        },
    )
    .await
    .unwrap();

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.competition_name, "NB I");
    assert_eq!(row.country_code.as_deref(), Some("HU"));
    assert_eq!((row.tier, row.priority), (1, 6));
    assert_eq!((row.home_ht, row.away_ht), (Some(1), Some(0)));
    assert_eq!((row.home_position, row.away_position), (Some(8), Some(6)));
    assert_eq!(row.market_count, Some(18));
    assert_eq!(row.odds_source.as_deref(), Some("Demó"));
    assert_eq!(row.odds.as_ref().unwrap()["home"], 2.2);
    assert!(row.odds_captured_at.is_some());
    // Friss odds sor alapértelmezett állapota a napi nézetben is látszik.
    assert_eq!(row.odds_trust_state.as_deref(), Some("provisional"));
}

#[tokio::test]
async fn day_picks_verified_odds_row() {
    let pool = test_pool().await;
    let base = seed_match_base(&pool).await;
    let match_id = seed_match(
        &pool,
        &base,
        "verified_publish",
        Utc.with_ymd_and_hms(2026, 11, 9, 18, 0, 0).unwrap(),
    )
    .await;

    // Két odds sor ugyanazon a meccsen (composite PK). A provisional forrás
    // uuid-ja szándékosan kisebb: a verified-előnynek a source_id rendezés
    // ellenére is érvényesülnie kell.
    let provisional_source = Uuid::from_u128(1);
    let verified_source = Uuid::from_u128(2);
    for (source_id, code, name) in [
        (provisional_source, "prov-src", "Provisional forrás"),
        (verified_source, "ver-src", "Verified forrás"),
    ] {
        sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,$2,$3)")
            .bind(source_id)
            .bind(code)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query(
        "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count, trust_state)
         VALUES ($1,$2,now(),18,'provisional'), ($1,$3,now(),18,'verified')",
    )
    .bind(match_id)
    .bind(provisional_source)
    .bind(verified_source)
    .execute(&pool)
    .await
    .unwrap();
    for (source_id, home) in [(provisional_source, 9.9), (verified_source, 1.5)] {
        sqlx::query(
            "INSERT INTO fact.odds_quote
             (match_id, source_id, bookmaker, market, outcome, line, phase, price)
             VALUES ($1,$2,'default','1x2','home',0,'closing',$3),
                    ($1,$2,'default','1x2','draw',0,'closing',3.3),
                    ($1,$2,'default','1x2','away',0,'closing',3.2)",
        )
        .bind(match_id)
        .bind(source_id)
        .bind(home)
        .execute(&pool)
        .await
        .unwrap();
    }

    let rows = fetch_day(
        &pool,
        &DayFilter {
            date: NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
            timezone: "Europe/Budapest".into(),
            include_all: false,
        },
    )
    .await
    .unwrap();

    // A LATERAL join miatt a meccs NEM duplikálódik, és a verified sor nyer.
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.match_id, match_id);
    assert_eq!(row.odds_trust_state.as_deref(), Some("verified"));
    assert_eq!(row.odds_source.as_deref(), Some("Verified forrás"));
    assert_eq!(row.odds.as_ref().unwrap()["home"], 1.5);
}

async fn seed_match_base(pool: &PgPool) -> MatchBase {
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

    MatchBase {
        competition_id,
        season_id,
        home_team_id,
        away_team_id,
    }
}

async fn seed_match(
    pool: &PgPool,
    base: &MatchBase,
    state: &str,
    kickoff_at: chrono::DateTime<Utc>,
) -> Uuid {
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
    .bind(kickoff_at)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO ops.publish_gate
         (id, match_id, publish_date, state, rule_set_version, blockers, evaluated_at)
         VALUES ($1,$2,'2026-11-09',$3,'verification-1.0','[]',now())",
    )
    .bind(Uuid::now_v7())
    .bind(match_id)
    .bind(state)
    .execute(pool)
    .await
    .unwrap();

    match_id
}

#[tokio::test]
async fn day_json_uses_bookmaker_chain() {
    let pool = test_pool().await;
    let base = seed_match_base(&pool).await;
    let match_id = seed_match(
        &pool,
        &base,
        "verified_publish",
        Utc.with_ymd_and_hms(2026, 11, 9, 18, 0, 0).unwrap(),
    )
    .await;
    let source_id = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,'b365','B365')")
        .bind(source_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count)
         VALUES ($1,$2,now(),3)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();
    // B365 + Avg ugyanarra a kulcsra: a lánc B365-öt választ.
    sqlx::query(
        "INSERT INTO fact.odds_quote
         (match_id, source_id, bookmaker, market, outcome, line, phase, price)
         VALUES ($1,$2,'B365','1x2','home',0,'closing',1.75),
                ($1,$2,'Avg','1x2','home',0,'closing',1.85)",
    )
    .bind(match_id)
    .bind(source_id)
    .execute(&pool)
    .await
    .unwrap();

    let rows = fetch_day(
        &pool,
        &DayFilter {
            date: NaiveDate::from_ymd_opt(2026, 11, 9).unwrap(),
            timezone: "Europe/Budapest".into(),
            include_all: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].odds.as_ref().unwrap()["home"], 1.75);
}
