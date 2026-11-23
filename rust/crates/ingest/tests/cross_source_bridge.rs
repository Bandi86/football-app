//! S86: cross-source fixture-híd integrációs tesztjei (izolált teszt-DB).
//!
//! A napi envelope több upstream forrásból ugyanazt a meccset különböző
//! verseny- és csapatnévvel hozza. A kanonizálás versenyenként/csapatonként
//! oldja fel, ezért a verseny-scoped keresés nem talál; a híd (ország +
//! kickoff ±15 perc + ID-szerint egyező csapat) köti a meccshez.

use chrono::{DateTime, TimeZone, Utc};
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::normalized::{
    NormalizedCompetition, NormalizedDay, NormalizedMatch, NormalizedOdds, NormalizedQuote,
    NormalizedResult, NormalizedSeason, NormalizedTeam,
};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

async fn seed_source(pool: &PgPool, code: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ops.source (id, code, display_name, identity_mode)
         VALUES ($1,$2,$3,'create_on_miss')",
    )
    .bind(id)
    .bind(code)
    .bind(format!("{code} forrás"))
    .execute(pool)
    .await
    .unwrap();
    id
}

#[allow(clippy::too_many_arguments)]
fn sv_day(
    source_code: &str,
    competition_name: &str,
    competition_source_entity_id: Option<&str>,
    teams: &[(&str, &str)],
    home: &str,
    away: &str,
    kickoff: DateTime<Utc>,
) -> NormalizedDay {
    day(
        source_code,
        competition_name,
        competition_source_entity_id,
        "SV",
        "El Salvador",
        teams,
        home,
        away,
        kickoff,
    )
}

#[allow(clippy::too_many_arguments)]
fn day(
    source_code: &str,
    competition_name: &str,
    competition_source_entity_id: Option<&str>,
    country_code: &str,
    country_name: &str,
    teams: &[(&str, &str)],
    home: &str,
    away: &str,
    kickoff: DateTime<Utc>,
) -> NormalizedDay {
    NormalizedDay {
        source_code: source_code.into(),
        captured_at: kickoff,
        competition: NormalizedCompetition {
            id: None,
            source_entity_id: competition_source_entity_id.map(str::to_string),
            name: competition_name.into(),
            country_code: country_code.into(),
            country_name: country_name.into(),
            tier: 1,
            priority: 50,
        },
        season: NormalizedSeason {
            id: None,
            label: "2026/27".into(),
        },
        teams: teams
            .iter()
            .map(|(ext, name)| NormalizedTeam {
                id: None,
                source_entity_id: (*ext).into(),
                name: (*name).into(),
                country_code: country_code.into(),
                gender: "male".into(),
                age_group: "senior".into(),
                branch: "senior".into(),
            })
            .collect(),
        matches: vec![NormalizedMatch {
            id: None,
            source_match_id: format!("{source_code}-m-1"),
            home_source_entity_id: home.into(),
            away_source_entity_id: away.into(),
            round_label: None,
            kickoff_at: kickoff,
            kickoff_precision: "exact".into(),
            competition_timezone: "America/El_Salvador".into(),
            status: "scheduled".into(),
            result: None,
            odds: None,
            stats: None,
            referee: None,
            referee_country: None,
            environment: None,
            incidents: Vec::new(),
            insight: None,
        }],
        standings: None,
    }
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

async fn seed_team(pool: &PgPool, country_id: Uuid, name: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.team
         (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$1,$2,$3,'male','senior','senior')",
    )
    .bind(id)
    .bind(country_id)
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn seed_team_source_ref(pool: &PgPool, source_id: Uuid, ext: &str, team_id: Uuid) {
    sqlx::query(
        "INSERT INTO identity.source_entity_ref
         (id, source_id, entity_kind, source_entity_id, entity_id, method)
         VALUES ($1,$2,'team',$3,$4,'manual_review')",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(ext)
    .bind(team_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_competition(pool: &PgPool, country_id: Option<Uuid>, name: &str) -> (Uuid, Uuid) {
    let competition_id = Uuid::now_v7();
    let entity_id = competition_id;
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition',$2)")
        .bind(entity_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name)
         VALUES ($1,$2,$3,$4)",
    )
    .bind(competition_id)
    .bind(entity_id)
    .bind(country_id)
    .bind(name)
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
    home: Uuid,
    away: Uuid,
    kickoff: DateTime<Utc>,
    status: &str,
) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, home_team_id, away_team_id,
          kickoff_at, kickoff_precision, competition_timezone, status)
         VALUES ($1,$2,$3,$4,$5,$6,'exact','America/El_Salvador',$7)",
    )
    .bind(id)
    .bind(competition_id)
    .bind(season_id)
    .bind(home)
    .bind(away)
    .bind(kickoff)
    .bind(status)
    .execute(pool)
    .await
    .unwrap();
    id
}

/// A valós SV eset (2026-11-21): ugyanaz a fixture három verseny-névváltozat és
/// két csapat-névváltozat alatt. A második nap nem hozhat létre új meccset, a
/// túlélő versenyhez kell linkelni, és alias íródik a verseny- és a csapatnévre.
#[tokio::test]
async fn cross_source_fixture_links_instead_of_creating() {
    let pool = test_pool().await;
    seed_source(&pool, "sa").await;
    seed_source(&pool, "sb").await;

    let kickoff = Utc.with_ymd_and_hms(2026, 11, 21, 21, 0, 0).unwrap();
    let day1 = sv_day(
        "sa",
        "Primera Division: Apertura",
        Some("sa:comp1"),
        &[("h1", "Isidro Metapan"), ("a1", "Inter Santa Tecla")],
        "h1",
        "a1",
        kickoff,
    );
    let first = Canonicalizer::materialize(&pool, &day1).await.unwrap();
    assert_eq!(first.matches.len(), 1);
    let canonical_match = first.matches[0].match_id;
    let canonical_away = first.matches[0].away_team_id;
    let survivor_competition: Uuid =
        sqlx::query_scalar("SELECT competition_id FROM core.match WHERE id=$1")
            .bind(canonical_match)
            .fetch_one(&pool)
            .await
            .unwrap();
    let survivor_entity: Uuid =
        sqlx::query_scalar("SELECT entity_id FROM core.competition WHERE id=$1")
            .bind(survivor_competition)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 1);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.competition").await,
        1
    );

    let mut day2 = sv_day(
        "sb",
        "Salvadoran Primera",
        Some("sb:comp1"),
        &[("h2", "Isidro Metapan"), ("a2", "Inter FA")],
        "h2",
        "a2",
        kickoff,
    );
    // S90: eredmény csak konzisztens (lezárt/élő) státusz mellett íródik;
    // a korábbi teszt scheduled státusszal kérte az eredményt, ami pont a
    // javított scheduled+score hiba volt.
    day2.matches[0].status = "finished".into();
    day2.matches[0].result = Some(NormalizedResult {
        home_ft: Some(2),
        away_ft: Some(1),
        home_ht: Some(1),
        away_ht: Some(0),
        home_et: None,
        away_et: None,
        home_pens: None,
        away_pens: None,
        decided_by: None,
    });
    day2.matches[0].odds = Some(NormalizedOdds {
        market_count: Some(1),
        quotes: vec![NormalizedQuote {
            bookmaker: "primer".into(),
            market: "1x2".into(),
            outcome: "home".into(),
            line: 0.0,
            phase: "opening".into(),
            price: 2.1,
        }],
    });
    let second = Canonicalizer::materialize(&pool, &day2).await.unwrap();

    assert_eq!(
        second.matches[0].match_id, canonical_match,
        "a híd a meglévő kanonikus meccshez linkel"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.match").await,
        1,
        "nem keletkezett második meccs"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.competition").await,
        1,
        "az új verseny beolvadt a túlélőbe"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.season").await,
        1,
        "nem maradt ghost season"
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM core.entity WHERE kind='competition'"
        )
        .await,
        1,
        "nem maradt ghost verseny-entitás"
    );
    let match_competition: Uuid =
        sqlx::query_scalar("SELECT competition_id FROM core.match WHERE id=$1")
            .bind(canonical_match)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        match_competition, survivor_competition,
        "a meccs a túlélő verseny alatt marad"
    );

    // A megfigyelt verseny-név a túlélő entitás reviewed aliasa.
    let (status, method, lang, country): (String, String, String, Option<String>) = sqlx::query_as(
        "SELECT status, method, lang, country_code FROM identity.alias
         WHERE entity_id=$1 AND alias=$2",
    )
    .bind(survivor_entity)
    .bind("Salvadoran Primera")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "reviewed");
    assert_eq!(method, "import");
    assert_eq!(lang, "und");
    assert_eq!(country.as_deref(), Some("SV"));

    // A híd által feloldott MÁSIK csapatnév: pending alias a túlélő csapaton.
    let (astatus, amethod, alang, acountry): (String, String, String, Option<String>) =
        sqlx::query_as(
            "SELECT status, method, lang, country_code FROM identity.alias
             WHERE entity_id=$1 AND alias=$2",
        )
        .bind(canonical_away)
        .bind("Inter FA")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(astatus, "pending");
    assert_eq!(amethod, "import");
    assert_eq!(alang, "und");
    assert_eq!(acountry.as_deref(), Some("SV"));

    // A korábbi reviewed csapat-alias érintetlen marad.
    let reviewed_away_alias: i64 = count(
        &pool,
        &format!(
            "SELECT count(*) FROM identity.alias
             WHERE entity_id='{canonical_away}' AND alias='Inter Santa Tecla'
               AND status='reviewed'"
        ),
    )
    .await;
    assert_eq!(reviewed_away_alias, 1);

    // A verseny source-refjei a túlélő entitásra íródtak át (mindkét forrásból).
    let competition_refs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM identity.source_entity_ref
         WHERE entity_kind='competition' AND entity_id=$1",
    )
    .bind(survivor_entity)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(competition_refs, 2, "mindkét verseny-ref a túlélőn");

    // A további írások (eredmény, odds) a híd által talált meccshez mennek.
    let result_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM core.match_result WHERE match_id=$1")
            .bind(canonical_match)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(result_rows, 1, "az eredmény a túlélő meccshez kerül");
    let quotes: i64 =
        sqlx::query_scalar("SELECT count(*) FROM fact.odds_quote WHERE match_id=$1 AND price=2.1")
            .bind(canonical_match)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(quotes, 1, "az odds a túlélő meccshez kerül");

    let match_refs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM identity.source_entity_ref
         WHERE entity_kind='match' AND entity_id=$1",
    )
    .bind(canonical_match)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        match_refs, 2,
        "mindkét forrás-sor meccs-hídja a túlélőre mutat"
    );
}

/// Tükrözött páros: a második forrás a home/away oldalt cserélve hozza.
#[tokio::test]
async fn mirrored_cross_source_pair_links_instead_of_creating() {
    let pool = test_pool().await;
    seed_source(&pool, "sa").await;
    seed_source(&pool, "sb").await;

    let kickoff = Utc.with_ymd_and_hms(2026, 11, 21, 21, 0, 0).unwrap();
    let day1 = sv_day(
        "sa",
        "Primera Division: Apertura",
        Some("sa:comp1"),
        &[("h1", "Isidro Metapan"), ("a1", "Inter Santa Tecla")],
        "h1",
        "a1",
        kickoff,
    );
    let first = Canonicalizer::materialize(&pool, &day1).await.unwrap();
    let canonical_match = first.matches[0].match_id;

    let day2 = sv_day(
        "sb",
        "Salvadoran Primera",
        Some("sb:comp1"),
        &[("h2", "Inter Santa Tecla"), ("a2", "Isidro Metapan")],
        "h2",
        "a2",
        kickoff,
    );
    let second = Canonicalizer::materialize(&pool, &day2).await.unwrap();

    assert_eq!(
        second.matches[0].match_id, canonical_match,
        "a tükrözött pár is a meglévő meccshez linkel"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.match").await,
        1,
        "nem keletkezett második meccs"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.competition").await,
        1,
        "a verseny beolvadt"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.match_result").await,
        0,
        "tükrözött (reversed) forrás-sor eredménye nem írható"
    );
}

/// Két közeli jelölt → nem linkelünk (nincs önkényes választás), új sor jön létre.
#[tokio::test]
async fn ambiguous_candidates_do_not_link() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "sa").await;
    let country = seed_country(&pool, "SV", "El Salvador").await;
    let home = seed_team(&pool, country, "Isidro Metapan").await;
    let z1 = seed_team(&pool, country, "Alianza FC").await;
    let z2 = seed_team(&pool, country, "Luis Angel Firpo").await;
    seed_team_source_ref(&pool, source_id, "h1", home).await;

    let kickoff = Utc.with_ymd_and_hms(2026, 11, 21, 21, 0, 0).unwrap();
    let (c1, s1) = seed_competition(&pool, Some(country), "League A").await;
    let (c2, s2) = seed_competition(&pool, Some(country), "League B").await;
    seed_match(&pool, c1, s1, home, z1, kickoff, "scheduled").await;
    seed_match(
        &pool,
        c2,
        s2,
        home,
        z2,
        kickoff + chrono::Duration::minutes(5),
        "scheduled",
    )
    .await;

    let day = sv_day(
        "sa",
        "Salvadoran Primera",
        None,
        &[("h1", "Isidro Metapan"), ("a1", "Inter FA")],
        "h1",
        "a1",
        kickoff,
    );
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();

    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.match").await,
        3,
        "két közeli jelöltnél nem linkelünk: új sor keletkezik"
    );
    let new_competition: Uuid =
        sqlx::query_scalar("SELECT competition_id FROM core.match WHERE id=$1")
            .bind(materialized.matches[0].match_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(new_competition, c1);
    assert_ne!(new_competition, c2);
}

/// NULL országú (kupa) versenynél nincs ország-kulcs, ezért a híd nem kereshet:
/// ugyanaz a fixture új sorral jön létre, ahogy a híd előtti viselkedés volt.
#[tokio::test]
async fn null_country_does_not_bridge() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "sa").await;
    let country = seed_country(&pool, "SV", "El Salvador").await;
    let home = seed_team(&pool, country, "Isidro Metapan").await;
    let away = seed_team(&pool, country, "Inter Santa Tecla").await;
    seed_team_source_ref(&pool, source_id, "h1", home).await;
    seed_team_source_ref(&pool, source_id, "a1", away).await;

    let kickoff = Utc.with_ymd_and_hms(2026, 11, 21, 21, 0, 0).unwrap();
    let (competition_id, season_id) = seed_competition(&pool, None, "Copa Uno").await;
    let seeded = seed_match(
        &pool,
        competition_id,
        season_id,
        home,
        away,
        kickoff,
        "scheduled",
    )
    .await;

    let day = day(
        "sa",
        "Copa Dos",
        None,
        "",
        "",
        &[("h1", "Isidro Metapan"), ("a1", "Inter Santa Tecla")],
        "h1",
        "a1",
        kickoff,
    );
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();

    assert_ne!(
        materialized.matches[0].match_id, seeded,
        "NULL ország mellett nem linkelhet a híd"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.match").await,
        2,
        "a híd nélkül új sor keletkezik"
    );
}

/// Lemondott ('canceled') meccshez a híd nem linkelhet.
#[tokio::test]
async fn canceled_candidate_is_not_bridged() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool, "sa").await;
    let country = seed_country(&pool, "SV", "El Salvador").await;
    let home = seed_team(&pool, country, "Isidro Metapan").await;
    let away = seed_team(&pool, country, "Inter Santa Tecla").await;
    seed_team_source_ref(&pool, source_id, "h1", home).await;
    seed_team_source_ref(&pool, source_id, "a1", away).await;

    let kickoff = Utc.with_ymd_and_hms(2026, 11, 21, 21, 0, 0).unwrap();
    let (competition_id, season_id) = seed_competition(&pool, Some(country), "League C").await;
    let canceled = seed_match(
        &pool,
        competition_id,
        season_id,
        home,
        away,
        kickoff,
        "canceled",
    )
    .await;

    let day = sv_day(
        "sa",
        "Salvadoran Primera",
        None,
        &[("h1", "Isidro Metapan"), ("a1", "Inter FA")],
        "h1",
        "a1",
        kickoff,
    );
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();

    assert_ne!(
        materialized.matches[0].match_id, canceled,
        "canceled meccshez nem linkelhet a híd"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.match").await,
        2,
        "a canceled jelölt kihagyásával új sor keletkezik"
    );
}
