//! S82: a research detail eseményeinek és csapatstatisztikájának
//! kanonizálása. Kizárólag a teszt adatbázison fut
//! (`TEST_DATABASE_URL` → football_v2_test_research).

use chrono::{TimeZone, Utc};
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::normalized::{
    NormalizedCompetition, NormalizedDay, NormalizedEnvironment, NormalizedIncident,
    NormalizedMatch, NormalizedSeason, NormalizedStats, NormalizedTeam,
};
use football_storage::repositories::match_detail::fetch_match_detail;
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_research_source(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ops.source (id, code, display_name, identity_mode)
         VALUES ($1,'research','Research','create_on_miss')",
    )
    .bind(id)
    .execute(pool)
    .await
    .expect("research forrás");
    id
}

async fn seed_country(pool: &PgPool) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Hungary')")
        .bind(id)
        .execute(pool)
        .await
        .expect("ország");
    id
}

fn incident(
    minute: Option<i32>,
    incident_type: &str,
    side: Option<&str>,
    player_name: Option<&str>,
    related_player_name: Option<&str>,
    detail: Option<&str>,
    score_after: Option<(i16, i16)>,
) -> NormalizedIncident {
    NormalizedIncident {
        minute,
        incident_type: incident_type.into(),
        side: side.map(Into::into),
        player_name: player_name.map(Into::into),
        related_player_name: related_player_name.map(Into::into),
        player_source_id: None,
        related_player_source_id: None,
        detail: detail.map(Into::into),
        score_after_home: score_after.map(|(home, _)| home),
        score_after_away: score_after.map(|(_, away)| away),
    }
}

fn research_day(
    stats: Option<NormalizedStats>,
    incidents: Vec<NormalizedIncident>,
) -> NormalizedDay {
    let kickoff = Utc.with_ymd_and_hms(2026, 11, 20, 18, 0, 0).unwrap();
    NormalizedDay {
        source_code: "research".into(),
        captured_at: kickoff,
        competition: NormalizedCompetition {
            id: None,
            source_entity_id: None,
            name: "Bundesliga".into(),
            country_code: "HU".into(),
            country_name: "Hungary".into(),
            tier: 1,
            priority: 6,
        },
        season: NormalizedSeason {
            id: None,
            label: "2026/27".into(),
        },
        teams: vec![
            NormalizedTeam {
                id: None,
                source_entity_id: "espn:359".into(),
                name: "Bayern Munich".into(),
                country_code: "HU".into(),
                gender: "male".into(),
                age_group: "senior".into(),
                branch: "senior".into(),
            },
            NormalizedTeam {
                id: None,
                source_entity_id: "espn:132".into(),
                name: "1. FC Koln".into(),
                country_code: "HU".into(),
                gender: "male".into(),
                age_group: "senior".into(),
                branch: "senior".into(),
            },
        ],
        matches: vec![NormalizedMatch {
            id: None,
            source_match_id: "espn:401932578".into(),
            home_source_entity_id: "espn:359".into(),
            away_source_entity_id: "espn:132".into(),
            round_label: None,
            kickoff_at: kickoff,
            kickoff_precision: "exact".into(),
            competition_timezone: "Europe/Budapest".into(),
            status: "finished".into(),
            result: None,
            odds: None,
            stats,
            referee: None,
            referee_country: None,
            environment: None,
            incidents,
        }],
        standings: None,
    }
}

/// (incident_no, side, incident_type, minute, player_name, related,
/// score_after_home, score_after_away)
type IncidentRow = (
    i32,
    Option<String>,
    String,
    Option<i32>,
    Option<String>,
    Option<String>,
    Option<i16>,
    Option<i16>,
);

/// (home_shots, away_shots, home_xg, away_xg, home_red, away_red, away_fouls)
type StatsRow = (
    Option<i32>,
    Option<i32>,
    Option<f64>,
    Option<f64>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

async fn incidents_of(pool: &PgPool, match_id: Uuid) -> Vec<IncidentRow> {
    sqlx::query_as(
        "SELECT incident_no, side, incident_type, minute, player_name,
                related_player_name, score_after_home, score_after_away
         FROM fact.match_incidents WHERE match_id = $1 ORDER BY incident_no",
    )
    .bind(match_id)
    .fetch_all(pool)
    .await
    .expect("események")
}

#[tokio::test]
async fn research_incidents_and_stats_are_written() {
    let pool = test_pool().await;
    seed_research_source(&pool).await;
    seed_country(&pool).await;

    let stats = NormalizedStats {
        home_shots: Some(16),
        away_shots: Some(8),
        home_shots_on_target: Some(5),
        away_shots_on_target: Some(3),
        home_corners: Some(7),
        away_corners: Some(2),
        home_yellow: Some(2),
        away_yellow: Some(3),
        home_red: Some(0),
        away_red: None,
        home_fouls: Some(11),
        away_fouls: Some(14),
        home_xg: Some(1.32),
        away_xg: None,
    };
    // Szándékosan nem perc-sorrendben: az `incident_no` a forrás sorrendje.
    let day = research_day(
        Some(stats),
        vec![
            incident(
                Some(34),
                "Card",
                Some("away"),
                Some("Jan Thielmann"),
                None,
                Some("Yellow card"),
                None,
            ),
            incident(
                Some(12),
                "Goal",
                Some("home"),
                Some("Harry Kane"),
                Some("Joshua Kimmich"),
                None,
                Some((1, 0)),
            ),
            incident(Some(46), "Half", None, None, None, None, Some((1, 1))),
        ],
    );
    let materialized = Canonicalizer::materialize(&pool, &day)
        .await
        .expect("materialize");
    let match_id = materialized.matches[0].match_id;

    let rows = incidents_of(&pool, match_id).await;
    assert_eq!(rows.len(), 3, "minden forrás-esemény bekerül");
    assert_eq!(
        rows[0],
        (
            1,
            Some("away".into()),
            "Card".into(),
            Some(34),
            Some("Jan Thielmann".into()),
            None,
            None,
            None
        ),
        "incident_no = forrás-sorrend, az ismeretlen score_after NULL"
    );
    assert_eq!(
        rows[1],
        (
            2,
            Some("home".into()),
            "Goal".into(),
            Some(12),
            Some("Harry Kane".into()),
            Some("Joshua Kimmich".into()),
            Some(1),
            Some(0)
        )
    );
    assert_eq!(
        rows[2],
        (
            3,
            None,
            "Half".into(),
            Some(46),
            None,
            None,
            Some(1),
            Some(1)
        ),
        "a félidőnek nincs csapata → side NULL, de a score_after megvan"
    );

    let stats_row: StatsRow = sqlx::query_as(
        "SELECT ms.home_shots, ms.away_shots, ms.home_xg::float8, ms.away_xg::float8,
                    ms.home_red, ms.away_red, ms.away_fouls
             FROM fact.match_stats ms
             JOIN ops.source src ON src.id = ms.source_id
             WHERE ms.match_id = $1 AND src.code = 'research'",
    )
    .bind(match_id)
    .fetch_one(&pool)
    .await
    .expect("statisztika");
    assert_eq!(
        stats_row,
        (Some(16), Some(8), Some(1.32), None, Some(0), None, Some(14)),
        "hiányzó érték NULL; a 0 valós érték marad 0"
    );
}

#[tokio::test]
async fn reingest_replaces_incidents_and_stats() {
    let pool = test_pool().await;
    seed_research_source(&pool).await;
    seed_country(&pool).await;

    let first = research_day(
        Some(NormalizedStats {
            home_shots: Some(10),
            away_shots: None,
            ..NormalizedStats::default()
        }),
        vec![
            incident(
                Some(10),
                "Goal",
                Some("home"),
                Some("Régi Játékos"),
                None,
                None,
                Some((1, 0)),
            ),
            incident(
                Some(20),
                "Card",
                Some("away"),
                Some("Másik Játékos"),
                None,
                None,
                None,
            ),
            incident(None, "Half", None, None, None, None, None),
        ],
    );
    let materialized = Canonicalizer::materialize(&pool, &first)
        .await
        .expect("első materialize");
    let match_id = materialized.matches[0].match_id;
    assert_eq!(incidents_of(&pool, match_id).await.len(), 3);

    let second = research_day(
        Some(NormalizedStats {
            home_shots: Some(12),
            away_shots: Some(4),
            ..NormalizedStats::default()
        }),
        vec![
            incident(
                Some(30),
                "Goal",
                Some("away"),
                Some("Új Játékos"),
                None,
                None,
                Some((2, 0)),
            ),
            incident(
                Some(75),
                "Substitution",
                Some("home"),
                Some("Bejövő"),
                Some("Kimenő"),
                None,
                None,
            ),
        ],
    );
    Canonicalizer::materialize(&pool, &second)
        .await
        .expect("második materialize");

    let rows = incidents_of(&pool, match_id).await;
    assert_eq!(
        rows.len(),
        2,
        "a régi események helyét az új lista veszi át"
    );
    assert_eq!(rows[0].0, 1);
    assert_eq!(rows[0].2, "Goal");
    assert_eq!(rows[0].4, Some("Új Játékos".into()));
    assert_eq!(rows[1].0, 2);
    assert_eq!(rows[1].2, "Substitution");
    assert_eq!(rows[1].5, Some("Kimenő".into()));

    let (home_shots, away_shots): (Option<i32>, Option<i32>) =
        sqlx::query_as("SELECT home_shots, away_shots FROM fact.match_stats WHERE match_id = $1")
            .bind(match_id)
            .fetch_one(&pool)
            .await
            .expect("frissített statisztika");
    assert_eq!((home_shots, away_shots), (Some(12), Some(4)));
}

/// (captured_at, venue_name, capacity, wind_direction, precipitation)
type EnvironmentRow = (
    chrono::DateTime<Utc>,
    Option<String>,
    Option<i32>,
    Option<String>,
    Option<f64>,
);

#[tokio::test]
async fn environment_is_written_and_served_in_match_detail() {
    let pool = test_pool().await;
    seed_research_source(&pool).await;
    seed_country(&pool).await;

    let observed_at = Utc.with_ymd_and_hms(2026, 11, 20, 16, 41, 32).unwrap();
    let weather_observed_at = Utc.with_ymd_and_hms(2026, 11, 21, 0, 30, 0).unwrap();
    let mut day = research_day(None, Vec::new());
    day.matches[0].environment = Some(NormalizedEnvironment {
        observed_at,
        venue_name: Some("Estadio Ricardo Gregor".into()),
        venue_city: Some("Luque".into()),
        venue_country: Some("Paraguay".into()),
        venue_capacity: Some(4000),
        venue_surface: Some("grass".into()),
        venue_lat: Some(-25.275_764_744),
        venue_lon: Some(-57.551_740_408),
        temperature_c: Some(27.0),
        weather_description: Some("Thunder".into()),
        wind_speed: Some(2.0),
        wind_direction: None,
        humidity_pct: Some(86),
        precipitation: Some(0.0),
        cloud_cover_pct: Some(54),
        weather_observed_at: Some(weather_observed_at),
    });

    let materialized = Canonicalizer::materialize(&pool, &day)
        .await
        .expect("materialize");
    let match_id = materialized.matches[0].match_id;

    let (captured_at, venue_name, capacity, wind_direction, precipitation): EnvironmentRow =
        sqlx::query_as(
            "SELECT e.captured_at, e.venue_name, e.venue_capacity, e.wind_direction,
                e.precipitation::float8
         FROM fact.match_environment e
         JOIN ops.source src ON src.id = e.source_id
         WHERE e.match_id = $1 AND src.code = 'research'",
        )
        .bind(match_id)
        .fetch_one(&pool)
        .await
        .expect("környezet sor");

    assert_eq!(
        captured_at, observed_at,
        "captured_at = az observation observed_at-ja, nem a feldolgozás ideje"
    );
    assert_eq!(venue_name.as_deref(), Some("Estadio Ricardo Gregor"));
    assert_eq!(capacity, Some(4000));
    assert_eq!(wind_direction, None, "hiányzó szélirány NULL, nem 0/üres");
    assert_eq!(precipitation, Some(0.0), "a 0 csapadék valós 0 marad");

    // A részletező API ugyanazt adja vissza (VenueRow/WeatherRow a LATERAL-ból).
    let detail = fetch_match_detail(&pool, match_id)
        .await
        .expect("detail lekérdezés")
        .expect("van kanonikus sor");
    let venue = detail.venue.expect("venue a részletezőben");
    assert_eq!(venue.name.as_deref(), Some("Estadio Ricardo Gregor"));
    assert_eq!(venue.city.as_deref(), Some("Luque"));
    assert_eq!(venue.country.as_deref(), Some("Paraguay"));
    assert_eq!(venue.capacity, Some(4000));
    assert_eq!(venue.surface.as_deref(), Some("grass"));
    let weather = detail.weather.expect("weather a részletezőben");
    assert_eq!(weather.temperature_c, Some(27.0));
    assert_eq!(weather.description.as_deref(), Some("Thunder"));
    assert_eq!(weather.wind_speed, Some(2.0));
    assert_eq!(weather.wind_direction, None);
    assert_eq!(weather.humidity_pct, Some(86));
    assert_eq!(weather.cloud_cover_pct, Some(54));
    assert_eq!(weather.observed_at, Some(weather_observed_at));
}

/// (official name, official captured_at)
type OfficialRow = (String, chrono::DateTime<Utc>);

/// S97: a research által kimondott játékvezető-nevet akkor is eltároljuk és
/// megjelenítjük, ha a katalógusban nincs egzakt entitás (nincs automatikus
/// fuzzy link). Az identitás-link ilyenkor szándékosan NULL marad.
#[tokio::test]
async fn referee_name_is_stored_and_served_without_identity_link() {
    let pool = test_pool().await;
    seed_research_source(&pool).await;
    seed_country(&pool).await;

    let mut day = research_day(None, Vec::new());
    day.matches[0].referee = Some("Marco Di Bello".into());

    let materialized = Canonicalizer::materialize(&pool, &day)
        .await
        .expect("materialize");
    let match_id = materialized.matches[0].match_id;

    let referee_entity: Option<Uuid> =
        sqlx::query_scalar("SELECT referee_entity_id FROM core.match WHERE id = $1")
            .bind(match_id)
            .fetch_one(&pool)
            .await
            .expect("kanonikus meccs");
    assert!(
        referee_entity.is_none(),
        "ismeretlen névformánál nincs automatikus entitás-link"
    );

    let (name, captured_at): OfficialRow = sqlx::query_as(
        "SELECT o.name, o.captured_at
           FROM fact.match_official o
           JOIN ops.source s ON s.id = o.source_id
          WHERE o.match_id = $1 AND o.role = 'referee' AND s.code = 'research'",
    )
    .bind(match_id)
    .fetch_one(&pool)
    .await
    .expect("hivatalos személy sor");
    assert_eq!(name, "Marco Di Bello");
    assert_eq!(
        captured_at, day.captured_at,
        "captured_at a forrás rögzítési ideje, nem a feldolgozásé"
    );

    let detail = fetch_match_detail(&pool, match_id)
        .await
        .expect("detail lekérdezés")
        .expect("van kanonikus sor");
    assert_eq!(
        detail.referee.as_deref(),
        Some("Marco Di Bello"),
        "entitás-link nélkül a forrás által kimondott név jelenik meg"
    );
}

/// S97: ha az entitás-link létrejön, a részletező a kanonikus entitásnevet
/// adja vissza (a forrás-nevet csak visszaesésként használja).
#[tokio::test]
async fn linked_referee_entity_name_wins_over_the_source_name() {
    let pool = test_pool().await;
    seed_research_source(&pool).await;
    seed_country(&pool).await;

    // A katalógusban pontosan ezen a néven létezik játékvezető.
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'referee','Marco Di Bello')",
    )
    .bind(Uuid::now_v7())
    .execute(&pool)
    .await
    .expect("referee entitás");

    let mut day = research_day(None, Vec::new());
    day.matches[0].referee = Some("Marco Di Bello".into());

    let materialized = Canonicalizer::materialize(&pool, &day)
        .await
        .expect("materialize");
    let match_id = materialized.matches[0].match_id;

    let referee_entity: Option<Uuid> =
        sqlx::query_scalar("SELECT referee_entity_id FROM core.match WHERE id = $1")
            .bind(match_id)
            .fetch_one(&pool)
            .await
            .expect("kanonikus meccs");
    assert!(
        referee_entity.is_some(),
        "egzakt teljes névvel a linker entitásra old"
    );

    let detail = fetch_match_detail(&pool, match_id)
        .await
        .expect("detail lekérdezés")
        .expect("van kanonikus sor");
    assert_eq!(detail.referee.as_deref(), Some("Marco Di Bello"));
}
