use chrono::{TimeZone, Utc};
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::normalized::{
    NormalizedCompetition, NormalizedDay, NormalizedMatch, NormalizedResult, NormalizedSeason,
    NormalizedTeam,
};
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_source(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,'live-src','Élő')")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn seed_source_with_mode(pool: &PgPool, code: &str, identity_mode: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ops.source (id, code, display_name, identity_mode)
         VALUES ($1,$2,$3,$4)",
    )
    .bind(id)
    .bind(code)
    .bind(format!("{code} forrás"))
    .bind(identity_mode)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn seed_country(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Hungary')")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn seed_linked_team(
    pool: &PgPool,
    source_id: Uuid,
    country_id: Uuid,
    ext: &str,
    name: &str,
) -> Uuid {
    let entity_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
        .bind(entity_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.team
         (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$1,$2,$3,'male','senior','senior')",
    )
    .bind(entity_id)
    .bind(country_id)
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO identity.source_entity_ref
         (id, source_id, entity_kind, source_entity_id, entity_id, method)
         VALUES ($1,$2,'team',$3,$4,'manual_review')",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(ext)
    .bind(entity_id)
    .execute(pool)
    .await
    .unwrap();
    entity_id
}

fn live_day(teams: &[(&str, &str)], home: &str, away: &str) -> NormalizedDay {
    let kickoff = Utc.with_ymd_and_hms(2026, 11, 21, 15, 0, 0).unwrap();
    NormalizedDay {
        source_code: "live-src".into(),
        captured_at: kickoff,
        competition: NormalizedCompetition {
            id: None,
            source_entity_id: None,
            // S152: semleges név — a versenyszótár most GB-ENG-re kötötte a
            // „Premier League"-t, ez a teszt viszont az ország NULL kezelését
            // vizsgálja, függetlenül a verseny névtől.
            name: "Test Liga".into(),
            country_code: "HU".into(),
            country_name: "Hungary".into(),
            tier: 1,
            priority: 6,
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
                country_code: "HU".into(),
                gender: "male".into(),
                age_group: "senior".into(),
                branch: "senior".into(),
            })
            .collect(),
        matches: vec![NormalizedMatch {
            id: None,
            source_match_id: "m-1".into(),
            home_source_entity_id: home.into(),
            away_source_entity_id: away.into(),
            round_label: None,
            kickoff_at: kickoff,
            kickoff_precision: "exact".into(),
            competition_timezone: "Europe/Budapest".into(),
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

#[tokio::test]
async fn create_on_miss_source_creates_unknown_teams_and_match() {
    let pool = test_pool().await;
    seed_source_with_mode(&pool, "research", "create_on_miss").await;
    seed_country(&pool).await;

    let mut day = live_day(
        &[("s1", "Caernarfon Town"), ("s2", "Connah's Quay")],
        "s1",
        "s2",
    );
    day.source_code = "research".into();
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(materialized.matches.len(), 1, "a meccs materializálódik");
    assert_eq!(count(&pool, "SELECT count(*) FROM core.team").await, 2);
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 1);
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM identity.source_entity_ref WHERE entity_kind='team'"
        )
        .await,
        2
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM identity.review_queue WHERE status='approved' AND decided_by='create_on_miss'"
        )
        .await,
        2
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM identity.alias
             WHERE method='import' AND status='reviewed'
               AND entity_id IN (SELECT id FROM core.entity WHERE kind='team')"
        )
        .await,
        2
    );
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn live_day_materializes_and_is_idempotent() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    seed_linked_team(&pool, source_id, country_id, "ujpest", "Újpest").await;
    seed_linked_team(&pool, source_id, country_id, "gyor", "Győr").await;

    let day = live_day(&[("ujpest", "Újpest"), ("gyor", "Győr")], "ujpest", "gyor");
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(materialized.matches.len(), 1, "egy meccs materializálva");
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.competition").await,
        1
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM core.season").await, 1);
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 1);
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM identity.source_entity_ref WHERE method='fixture_seed'"
        )
        .await,
        0,
        "élő úton nincs fixture_seed ref"
    );

    // Újrafuttatás: natural-key upsert, nincs duplikátum.
    Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.competition").await,
        1
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM core.season").await, 1);
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 1);
}

#[tokio::test]
async fn unresolved_team_skips_the_match() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    seed_linked_team(&pool, source_id, country_id, "ujpest", "Újpest").await;
    // A vendég nincs kötve: a linker review-sort nyit, a meccs kimarad.

    let day = live_day(
        &[("ujpest", "Újpest"), ("ismeretlen", "Ismeretlen SE")],
        "ujpest",
        "ismeretlen",
    );
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(
        materialized.matches.len(),
        0,
        "feloldatlan csapatnál nincs meccs"
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 0);
    assert!(
        count(&pool, "SELECT count(*) FROM identity.review_queue").await >= 1,
        "a linker review-sort nyitott"
    );
}

#[tokio::test]
async fn unresolved_match_odds_are_not_written() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    seed_linked_team(&pool, source_id, country_id, "ujpest", "Újpest").await;

    let mut day = live_day(
        &[("ujpest", "Újpest"), ("ismeretlen", "Ismeretlen SE")],
        "ujpest",
        "ismeretlen",
    );
    let quotes = (0..7)
        .map(|i| football_ingest::normalized::NormalizedQuote {
            bookmaker: "B365".into(),
            market: "1x2".into(),
            outcome: format!("o{i}"),
            line: 0.0,
            phase: "closing".into(),
            price: 2.0,
        })
        .collect();
    day.matches[0].odds = Some(football_ingest::normalized::NormalizedOdds {
        market_count: Some(1),
        quotes,
    });

    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(materialized.matches.len(), 0);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM fact.odds_snapshot").await,
        0,
        "kimaradt meccshez nincs odds-fejléc (FK-biztos)"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM fact.odds_quote").await,
        0
    );
}

#[tokio::test]
async fn stats_and_referee_are_materialized() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    let home = seed_linked_team(&pool, source_id, country_id, "ujpest", "Újpest").await;
    let away = seed_linked_team(&pool, source_id, country_id, "gyor", "Győr").await;
    // Játékvezető-entitás + ref (linkeres Exact).
    let referee = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'referee','M. Oliver')",
    )
    .bind(referee)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO identity.source_entity_ref
         (id, source_id, entity_kind, source_entity_id, entity_id, method)
         VALUES ($1,$2,'referee','m-oliver',$3,'manual_review')",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(referee)
    .execute(&pool)
    .await
    .unwrap();
    let _ = (home, away);

    let mut day = live_day(&[("ujpest", "Újpest"), ("gyor", "Győr")], "ujpest", "gyor");
    day.matches[0].stats = Some(football_ingest::normalized::NormalizedStats {
        home_shots: Some(12),
        away_shots: Some(9),
        ..Default::default()
    });
    day.matches[0].referee = Some("M. Oliver".into());

    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(materialized.matches.len(), 1);
    let (hs, as_): (Option<i32>, Option<i32>) =
        sqlx::query_as("SELECT home_shots, away_shots FROM fact.match_stats WHERE match_id=$1")
            .bind(materialized.matches[0].match_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((hs, as_), (Some(12), Some(9)));
    let linked: Option<Uuid> =
        sqlx::query_scalar("SELECT referee_entity_id FROM core.match WHERE id=$1")
            .bind(materialized.matches[0].match_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(linked, Some(referee));
}

/// A date_only megfigyelés nem rontja le a pontos kickoffot (nincs
/// duplikált kanonikus meccs), a decided_by pedig perzisztálódik.
#[tokio::test]
async fn date_only_observation_keeps_exact_kickoff_and_persists_decided_by() {
    use chrono::{DateTime, Utc};

    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    seed_linked_team(&pool, source_id, country_id, "ujpest", "Újpest").await;
    seed_linked_team(&pool, source_id, country_id, "gyor", "Győr").await;

    let teams = [("ujpest", "Újpest"), ("gyor", "Győr")];
    let day = live_day(&teams, "ujpest", "gyor");
    Canonicalizer::materialize(&pool, &day).await.unwrap();

    let mut second = live_day(&teams, "ujpest", "gyor");
    let m = &mut second.matches[0];
    m.kickoff_at = Utc.with_ymd_and_hms(2026, 11, 21, 12, 0, 0).unwrap();
    m.kickoff_precision = "date_only".into();
    m.status = "finished".into();
    m.result = Some(NormalizedResult {
        home_ft: None,
        away_ft: None,
        home_ht: None,
        away_ht: None,
        home_et: Some(1),
        away_et: Some(1),
        home_pens: Some(4),
        away_pens: Some(5),
        decided_by: Some("pens".into()),
    });
    Canonicalizer::materialize(&pool, &second).await.unwrap();

    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 1);
    let (kickoff, precision): (DateTime<Utc>, String) =
        sqlx::query_as("SELECT kickoff_at, kickoff_precision FROM core.match")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        kickoff,
        Utc.with_ymd_and_hms(2026, 11, 21, 15, 0, 0).unwrap()
    );
    assert_eq!(precision, "exact");
    let (hp, ap, decided): (Option<i16>, Option<i16>, Option<String>) =
        sqlx::query_as("SELECT home_pens, away_pens, decided_by FROM core.match_result")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((hp, ap), (Some(4), Some(5)));
    assert_eq!(decided.as_deref(), Some("pens"));
}

/// S9: kupa/nemzetközi nap ország nélkül — a verseny és a csapat országa
/// NULL, a materializálás nem bukik.
#[tokio::test]
async fn cup_day_without_country_stores_null_country() {
    let pool = test_pool().await;
    let _source_id = seed_source(&pool).await;
    let mut day = live_day(&[("a1", "A1"), ("a2", "A2")], "a1", "a2");
    day.competition.country_code = String::new();
    day.competition.country_name = String::new();
    for team in &mut day.teams {
        team.country_code = String::new();
        team.id = Some(Uuid::now_v7());
    }
    let materialized = Canonicalizer::materialize(&pool, &day).await.unwrap();
    assert_eq!(materialized.matches.len(), 1);

    let competition_country: Option<Uuid> =
        sqlx::query_scalar("SELECT country_id FROM core.competition LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(competition_country.is_none(), "verseny-ország NULL");
    let team_country: Option<Uuid> = sqlx::query_scalar("SELECT country_id FROM core.team LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(team_country.is_none(), "csapat-ország NULL");
}

/// S9: a meglévő csapat országát a NULL-os megfigyelés nem írja felül.
#[tokio::test]
async fn null_country_does_not_overwrite_existing_team_country() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    seed_linked_team(&pool, source_id, country_id, "ujpest", "Újpest").await;
    seed_linked_team(&pool, source_id, country_id, "gyor", "Győr").await;

    let teams = [("ujpest", "Újpest"), ("gyor", "Győr")];
    let first = live_day(&teams, "ujpest", "gyor");
    Canonicalizer::materialize(&pool, &first).await.unwrap();

    let (competition_id, season_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT c.id, s.id FROM core.competition c JOIN core.season s ON s.competition_id=c.id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let match_id: Uuid = sqlx::query_scalar("SELECT id FROM core.match")
        .fetch_one(&pool)
        .await
        .unwrap();
    let team_id: Uuid = sqlx::query_scalar("SELECT id FROM core.team WHERE name='Újpest'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let mut second = live_day(&teams, "ujpest", "gyor");
    second.competition.id = Some(competition_id);
    second.competition.country_code = String::new();
    second.competition.country_name = String::new();
    second.season.id = Some(season_id);
    second.matches[0].id = Some(match_id);
    for team in &mut second.teams {
        team.country_code = String::new();
        if team.source_entity_id == "ujpest" {
            team.id = Some(team_id);
        }
    }
    Canonicalizer::materialize(&pool, &second).await.unwrap();

    let kept: Option<Uuid> =
        sqlx::query_scalar("SELECT country_id FROM core.team WHERE name='Újpest'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(kept, Some(country_id), "a meglévő ország megmarad");
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.match").await,
        1,
        "nincs duplikált meccs"
    );
}

/// Nemzetközi versenynél (INT/XX) a csapat országát a meglévő kanonikus
/// névből kell átvenni, különben minden nemzeti csapat XX-országú duplikátum
/// lesz (S69).
#[tokio::test]
async fn international_day_links_team_by_name_to_existing_country() {
    let pool = test_pool().await;
    let seed_source_id = seed_source(&pool).await;
    let _int_source_id = seed_source_with_mode(&pool, "int-src", "create_on_miss").await;
    let country_id = seed_country(&pool).await;
    let existing = seed_linked_team(&pool, seed_source_id, country_id, "seed-hun", "Hungary").await;

    let mut day = live_day(
        &[("hun", "Hungary"), ("other", "Otherland")],
        "hun",
        "other",
    );
    day.source_code = "int-src".into();
    day.competition.name = "Friendly International".into();
    day.competition.country_code = "INT".into();
    day.competition.country_name = "World".into();
    for team in &mut day.teams {
        team.country_code = "XX".into();
    }
    Canonicalizer::materialize(&pool, &day).await.unwrap();

    let hungary_id: Uuid = sqlx::query_scalar("SELECT id FROM core.team WHERE name='Hungary'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        hungary_id, existing,
        "a meglévő HU csapatot kellett megtalálni"
    );

    let xx_hungary: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM core.team t JOIN core.country c ON c.id=t.country_id
         WHERE c.code='XX' AND t.name='Hungary'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        xx_hungary, 0,
        "a Hungary nem kaphat XX-országú párhuzamos entitást"
    );
}

/// A forduló megjelenítési adat: két forrás eltérő forduló-jelöléssel
/// ugyanazt a meccset írja le → egy kanonikus sor, frissülő címkével.
#[tokio::test]
async fn round_label_is_display_only_in_match_identity() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    seed_linked_team(&pool, source_id, country_id, "ujpest", "Újpest").await;
    seed_linked_team(&pool, source_id, country_id, "gyor", "Győr").await;
    let teams = [("ujpest", "Újpest"), ("gyor", "Győr")];

    let mut first = live_day(&teams, "ujpest", "gyor");
    first.matches[0].round_label = Some("Round 6".into());
    Canonicalizer::materialize(&pool, &first).await.unwrap();

    let mut second = live_day(&teams, "ujpest", "gyor");
    second.matches[0].round_label = Some("League A".into());
    Canonicalizer::materialize(&pool, &second).await.unwrap();

    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 1);
    let round: Option<String> = sqlx::query_scalar("SELECT round_label FROM core.match")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(round.as_deref(), Some("League A"), "a címke frissül");
}

#[tokio::test]
async fn localised_competition_titles_merge_into_one_entity() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    seed_linked_team(&pool, source_id, country_id, "ujpest", "Újpest").await;
    seed_linked_team(&pool, source_id, country_id, "gyor", "Győr").await;

    // Ugyanaz a kupa két lokalizált címmel, más napon/kickoffal.
    let mut hu = live_day(&[("ujpest", "Újpest"), ("gyor", "Győr")], "ujpest", "gyor");
    hu.competition = NormalizedCompetition {
        id: None,
        source_entity_id: None,
        name: "Kings Cup - Thaiföld".into(),
        country_code: String::new(),
        country_name: String::new(),
        tier: 0,
        priority: 40,
    };
    let first = Canonicalizer::materialize(&pool, &hu).await.unwrap();
    assert_eq!(first.matches.len(), 1);

    let mut en = hu.clone();
    en.competition.name = "Kings Cup (Thailand)".into();
    let kickoff = Utc.with_ymd_and_hms(2026, 11, 22, 15, 0, 0).unwrap();
    en.matches[0].kickoff_at = kickoff;
    en.matches[0].source_match_id = "m-2".into();
    let second = Canonicalizer::materialize(&pool, &en).await.unwrap();
    assert_eq!(second.matches.len(), 1);

    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.competition").await,
        1,
        "a két cím egy verseny-entitásba olvad"
    );
    let name: String = sqlx::query_scalar("SELECT name FROM core.competition")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "Kings Cup (Thailand)");
    assert_eq!(count(&pool, "SELECT count(*) FROM core.match").await, 2);
}

/// S37b: ugyanaz a meccs két forrásból, név-variáns csapatnevekkel
/// (`Újpest` / `Újpest FC`) ne hozzon létre második kanonikus meccset,
/// hanem a meglévőhöz linkeljen, és az odds is oda kerüljön.
#[tokio::test]
async fn variant_team_names_link_to_the_existing_canonical_match() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    seed_linked_team(&pool, source_id, country_id, "ujpest", "Újpest").await;
    seed_linked_team(&pool, source_id, country_id, "gyor", "Győr").await;

    let first = live_day(&[("ujpest", "Újpest"), ("gyor", "Győr")], "ujpest", "gyor");
    let day1 = Canonicalizer::materialize(&pool, &first).await.unwrap();
    let canonical_id = day1.matches[0].match_id;

    seed_linked_team(&pool, source_id, country_id, "ujpest-fc", "Újpest FC").await;
    seed_linked_team(&pool, source_id, country_id, "gyor-2", "Győr").await;
    let mut second = live_day(
        &[("ujpest-fc", "Újpest FC"), ("gyor-2", "Győr")],
        "ujpest-fc",
        "gyor-2",
    );
    second.matches[0].source_match_id = "m-2".into();
    second.matches[0].odds = Some(football_ingest::normalized::NormalizedOdds {
        market_count: Some(1),
        quotes: vec![football_ingest::normalized::NormalizedQuote {
            bookmaker: "primer".into(),
            market: "1x2".into(),
            outcome: "home".into(),
            line: 0.0,
            phase: "opening".into(),
            price: 2.4,
        }],
    });
    let day2 = Canonicalizer::materialize(&pool, &second).await.unwrap();

    assert_eq!(
        day2.matches[0].match_id, canonical_id,
        "a név-variáns sor a meglévő kanonikus meccshez linkel"
    );
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.match").await,
        1,
        "nem keletkezett második meccs"
    );
    let match_refs: i64 = count(
        &pool,
        "SELECT count(*) FROM identity.source_entity_ref WHERE entity_kind='match'",
    )
    .await;
    assert_eq!(match_refs, 2, "mindkét forrás-sor meccs-hídja beíródik");
    let quotes: i64 =
        sqlx::query_scalar("SELECT count(*) FROM fact.odds_quote WHERE match_id=$1 AND price=2.4")
            .bind(canonical_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(quotes, 1, "az odds a meglévő meccshez kerül");
}

/// S90 (C): név-variáns linknél a forrás `finished` státusza és eredménye is
/// a kanonikus sorra kerül — nem marad `scheduled` + 0–3 eredmény.
#[tokio::test]
async fn variant_source_finished_result_updates_status_and_result() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    seed_linked_team(&pool, source_id, country_id, "ujpest", "Újpest").await;
    seed_linked_team(&pool, source_id, country_id, "gyor", "Győr").await;

    let first = live_day(&[("ujpest", "Újpest"), ("gyor", "Győr")], "ujpest", "gyor");
    let day1 = Canonicalizer::materialize(&pool, &first).await.unwrap();
    let canonical_id = day1.matches[0].match_id;

    seed_linked_team(&pool, source_id, country_id, "ujpest-fc", "Újpest FC").await;
    seed_linked_team(&pool, source_id, country_id, "gyor-2", "Győr").await;
    let mut second = live_day(
        &[("ujpest-fc", "Újpest FC"), ("gyor-2", "Győr")],
        "ujpest-fc",
        "gyor-2",
    );
    second.matches[0].source_match_id = "m-2".into();
    second.matches[0].status = "finished".into();
    second.matches[0].result = Some(NormalizedResult {
        home_ft: Some(0),
        away_ft: Some(3),
        home_ht: Some(0),
        away_ht: Some(1),
        home_et: None,
        away_et: None,
        home_pens: None,
        away_pens: None,
        decided_by: None,
    });
    let day2 = Canonicalizer::materialize(&pool, &second).await.unwrap();
    assert_eq!(
        day2.matches[0].match_id, canonical_id,
        "a név-variáns sor a meglévő kanonikus meccshez linkel"
    );

    let status: String = sqlx::query_scalar("SELECT status FROM core.match WHERE id=$1")
        .bind(canonical_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        status, "finished",
        "a variáns forrás lezárt státusza felkerül a kanonikus sorra"
    );
    let (home_ft, away_ft): (Option<i16>, Option<i16>) =
        sqlx::query_as("SELECT home_ft, away_ft FROM core.match_result WHERE match_id=$1")
            .bind(canonical_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (home_ft, away_ft),
        (Some(0), Some(3)),
        "az eredmény konzisztensen a státusszal íródik (nincs scheduled+score)"
    );
}

/// S90 (C): terminális állapotból nem lép vissza a rendszer — egy elavult
/// `scheduled` forrás-sor sem a státuszt nem nyitja újra, sem a meglévő
/// eredményt nem törli.
#[tokio::test]
async fn stale_scheduled_source_does_not_revert_finished_status_or_drop_result() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    seed_linked_team(&pool, source_id, country_id, "ujpest", "Újpest").await;
    seed_linked_team(&pool, source_id, country_id, "gyor", "Győr").await;

    let mut finished = live_day(&[("ujpest", "Újpest"), ("gyor", "Győr")], "ujpest", "gyor");
    finished.matches[0].status = "finished".into();
    finished.matches[0].result = Some(NormalizedResult {
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
    let day1 = Canonicalizer::materialize(&pool, &finished).await.unwrap();
    let canonical_id = day1.matches[0].match_id;

    // Elavult, még `scheduled` forrás-sor ugyanarra a meccsre.
    let stale = live_day(&[("ujpest", "Újpest"), ("gyor", "Győr")], "ujpest", "gyor");
    Canonicalizer::materialize(&pool, &stale).await.unwrap();

    let status: String = sqlx::query_scalar("SELECT status FROM core.match WHERE id=$1")
        .bind(canonical_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "finished", "terminálisból nincs visszalépés");
    let result_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM core.match_result WHERE match_id=$1")
            .bind(canonical_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(result_rows, 1, "a lezárt eredmény nem tűnik el");
}

/// S90 (C): lejátszott meccs eredmény-korrekciója (ugyanaz a státusz, más
/// score) átíródik — a fagyasztás csak a státuszra vonatkozik, az eredményre
/// nem.
#[tokio::test]
async fn finished_score_correction_updates_result() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    seed_linked_team(&pool, source_id, country_id, "ujpest", "Újpest").await;
    seed_linked_team(&pool, source_id, country_id, "gyor", "Győr").await;

    let mut first = live_day(&[("ujpest", "Újpest"), ("gyor", "Győr")], "ujpest", "gyor");
    first.matches[0].status = "finished".into();
    first.matches[0].result = Some(NormalizedResult {
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
    let day1 = Canonicalizer::materialize(&pool, &first).await.unwrap();
    let canonical_id = day1.matches[0].match_id;

    let mut corrected = first.clone();
    corrected.matches[0].result = Some(NormalizedResult {
        home_ft: Some(3),
        away_ft: Some(1),
        home_ht: Some(1),
        away_ht: Some(0),
        home_et: None,
        away_et: None,
        home_pens: None,
        away_pens: None,
        decided_by: None,
    });
    Canonicalizer::materialize(&pool, &corrected).await.unwrap();

    let status: String = sqlx::query_scalar("SELECT status FROM core.match WHERE id=$1")
        .bind(canonical_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "finished", "a státusz nem változik (nincs írás)");
    let (home_ft, away_ft): (Option<i16>, Option<i16>) =
        sqlx::query_as("SELECT home_ft, away_ft FROM core.match_result WHERE match_id=$1")
            .bind(canonical_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (home_ft, away_ft),
        (Some(3), Some(1)),
        "az eredmény-korrekció átíródik"
    );
}

/// S90 (C): valós forrás-javítás `canceled` → `postponed` irányban ír (a
/// 2026-11-21-i algériai nap esete), mert a `canceled` nem eredményt hordozó
/// terminális.
#[tokio::test]
async fn canceled_to_postponed_correction_is_written() {
    let pool = test_pool().await;
    let source_id = seed_source(&pool).await;
    let country_id = seed_country(&pool).await;
    seed_linked_team(&pool, source_id, country_id, "ujpest", "Újpest").await;
    seed_linked_team(&pool, source_id, country_id, "gyor", "Győr").await;

    let mut first = live_day(&[("ujpest", "Újpest"), ("gyor", "Győr")], "ujpest", "gyor");
    first.matches[0].status = "canceled".into();
    let day1 = Canonicalizer::materialize(&pool, &first).await.unwrap();
    let canonical_id = day1.matches[0].match_id;

    let mut corrected = first.clone();
    corrected.matches[0].status = "postponed".into();
    Canonicalizer::materialize(&pool, &corrected).await.unwrap();

    let status: String = sqlx::query_scalar("SELECT status FROM core.match WHERE id=$1")
        .bind(canonical_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        status, "postponed",
        "a nem eredményt hordozó terminális javítása átíródik"
    );
}
