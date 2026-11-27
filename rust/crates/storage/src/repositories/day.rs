use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DayFilter {
    pub date: NaiveDate,
    pub timezone: String,
    pub include_all: bool,
    /// Opcionális verseny-szűrő (verseny-adatlap).
    pub competition_id: Option<Uuid>,
    /// A nap körüli ablak napokban (0 = csak az adott nap).
    pub window_days: i64,
    /// Opcionális csapat-szűrő (csapat-adatlap: hazai vagy vendég).
    pub team_id: Option<Uuid>,
}

#[derive(Debug, Clone, FromRow, serde::Serialize)]
pub struct DayMatchRow {
    pub match_id: Uuid,
    /// S215: a duplikátum-szűrés ID-alapú, nem név-alapú (a név-fuzzy merge tiltott).
    pub home_team_id: Uuid,
    pub away_team_id: Uuid,
    pub kickoff_at: DateTime<Utc>,
    pub kickoff_precision: String,
    pub status: String,
    pub home_name: String,
    pub away_name: String,
    pub home_score: Option<i16>,
    pub away_score: Option<i16>,
    pub publish_state: String,
    pub blockers: serde_json::Value,
    pub competition_id: Uuid,
    pub competition_name: String,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub tier: i16,
    pub priority: i32,
    pub round_label: Option<String>,
    pub home_ht: Option<i16>,
    pub away_ht: Option<i16>,
    pub home_et: Option<i16>,
    pub away_et: Option<i16>,
    pub home_pens: Option<i16>,
    pub away_pens: Option<i16>,
    pub home_position: Option<i16>,
    pub away_position: Option<i16>,
    pub odds: Option<serde_json::Value>,
    pub market_count: Option<i32>,
    pub odds_source: Option<String>,
    pub odds_captured_at: Option<DateTime<Utc>>,
    pub odds_trust_state: Option<String>,
    pub home_logo: Option<String>,
    pub away_logo: Option<String>,
    pub competition_logo: Option<String>,
    pub home_elo: Option<f64>,
    pub away_elo: Option<f64>,
    /// A research élő játékideje (perc), ha van — S134.
    pub minute: Option<i32>,
}

pub async fn fetch_day(pool: &PgPool, filter: &DayFilter) -> Result<Vec<DayMatchRow>> {
    let rows = sqlx::query_as::<_, DayMatchRow>(
        "SELECT
            m.id AS match_id,
            m.home_team_id, m.away_team_id,
            m.kickoff_at,
            m.kickoff_precision,
            m.status,
            home.name AS home_name,
            away.name AS away_name,
            r.home_ft AS home_score,
            r.away_ft AS away_score,
            g.state AS publish_state,
            g.blockers,
            c.id AS competition_id,
            c.name AS competition_name,
            co.code AS country_code,
            co.name AS country_name,
            c.tier,
            c.priority,
            m.round_label,
            r.home_ht, r.away_ht, r.home_et, r.away_et, r.home_pens, r.away_pens,
            sh.position AS home_position,
            sa.position AS away_position,
            o.odds,
            o.market_count,
            os.display_name AS odds_source,
            o.captured_at AS odds_captured_at,
            o.trust_state AS odds_trust_state,
            hl.logo AS home_logo,
            al.logo AS away_logo,
            cl.logo AS competition_logo,
            he.elo AS home_elo,
            ae.elo AS away_elo,
            sm.minute
         FROM core.match m
         JOIN core.team home ON home.id = m.home_team_id
         JOIN core.team away ON away.id = m.away_team_id
         JOIN core.competition c ON c.id = m.competition_id
         LEFT JOIN core.country co ON co.id = c.country_id
         LEFT JOIN core.match_result r ON r.match_id = m.id
         LEFT JOIN core.standing sh ON sh.season_id = m.season_id AND sh.team_id = m.home_team_id
         LEFT JOIN core.standing sa ON sa.season_id = m.season_id AND sa.team_id = m.away_team_id
         LEFT JOIN LATERAL (
             SELECT jsonb_build_object(
                        'home', COALESCE(max(q.price) FILTER (WHERE q.bookmaker='default' AND q.market='1x2' AND q.outcome='home'), max(q.price) FILTER (WHERE q.bookmaker='B365' AND q.market='1x2' AND q.outcome='home'), max(q.price) FILTER (WHERE q.bookmaker='Tippmix' AND q.market='1x2' AND q.outcome='home'), max(q.price) FILTER (WHERE q.bookmaker='Avg' AND q.market='1x2' AND q.outcome='home'), max(q.price) FILTER (WHERE q.bookmaker='Max' AND q.market='1x2' AND q.outcome='home'), max(q.price) FILTER (WHERE q.market='1x2' AND q.outcome='home')),
                        'draw', COALESCE(max(q.price) FILTER (WHERE q.bookmaker='default' AND q.market='1x2' AND q.outcome='draw'), max(q.price) FILTER (WHERE q.bookmaker='B365' AND q.market='1x2' AND q.outcome='draw'), max(q.price) FILTER (WHERE q.bookmaker='Tippmix' AND q.market='1x2' AND q.outcome='draw'), max(q.price) FILTER (WHERE q.bookmaker='Avg' AND q.market='1x2' AND q.outcome='draw'), max(q.price) FILTER (WHERE q.bookmaker='Max' AND q.market='1x2' AND q.outcome='draw'), max(q.price) FILTER (WHERE q.market='1x2' AND q.outcome='draw')),
                        'away', COALESCE(max(q.price) FILTER (WHERE q.bookmaker='default' AND q.market='1x2' AND q.outcome='away'), max(q.price) FILTER (WHERE q.bookmaker='B365' AND q.market='1x2' AND q.outcome='away'), max(q.price) FILTER (WHERE q.bookmaker='Tippmix' AND q.market='1x2' AND q.outcome='away'), max(q.price) FILTER (WHERE q.bookmaker='Avg' AND q.market='1x2' AND q.outcome='away'), max(q.price) FILTER (WHERE q.bookmaker='Max' AND q.market='1x2' AND q.outcome='away'), max(q.price) FILTER (WHERE q.market='1x2' AND q.outcome='away')),
                        'btts_yes', COALESCE(max(q.price) FILTER (WHERE q.bookmaker='default' AND q.market='btts' AND q.outcome='yes'), max(q.price) FILTER (WHERE q.bookmaker='B365' AND q.market='btts' AND q.outcome='yes'), max(q.price) FILTER (WHERE q.bookmaker='Tippmix' AND q.market='btts' AND q.outcome='yes'), max(q.price) FILTER (WHERE q.bookmaker='Avg' AND q.market='btts' AND q.outcome='yes'), max(q.price) FILTER (WHERE q.bookmaker='Max' AND q.market='btts' AND q.outcome='yes'), max(q.price) FILTER (WHERE q.market='btts' AND q.outcome='yes')),
                        'btts_no', COALESCE(max(q.price) FILTER (WHERE q.bookmaker='default' AND q.market='btts' AND q.outcome='no'), max(q.price) FILTER (WHERE q.bookmaker='B365' AND q.market='btts' AND q.outcome='no'), max(q.price) FILTER (WHERE q.bookmaker='Tippmix' AND q.market='btts' AND q.outcome='no'), max(q.price) FILTER (WHERE q.bookmaker='Avg' AND q.market='btts' AND q.outcome='no'), max(q.price) FILTER (WHERE q.bookmaker='Max' AND q.market='btts' AND q.outcome='no'), max(q.price) FILTER (WHERE q.market='btts' AND q.outcome='no')),
                        'over25', COALESCE(max(q.price) FILTER (WHERE q.bookmaker='default' AND q.market='ou' AND q.outcome='over' AND q.line=2.5), max(q.price) FILTER (WHERE q.bookmaker='B365' AND q.market='ou' AND q.outcome='over' AND q.line=2.5), max(q.price) FILTER (WHERE q.bookmaker='Tippmix' AND q.market='ou' AND q.outcome='over' AND q.line=2.5), max(q.price) FILTER (WHERE q.bookmaker='Avg' AND q.market='ou' AND q.outcome='over' AND q.line=2.5), max(q.price) FILTER (WHERE q.bookmaker='Max' AND q.market='ou' AND q.outcome='over' AND q.line=2.5), max(q.price) FILTER (WHERE q.market='ou' AND q.outcome='over' AND q.line=2.5)),
                        'under25', COALESCE(max(q.price) FILTER (WHERE q.bookmaker='default' AND q.market='ou' AND q.outcome='under' AND q.line=2.5), max(q.price) FILTER (WHERE q.bookmaker='B365' AND q.market='ou' AND q.outcome='under' AND q.line=2.5), max(q.price) FILTER (WHERE q.bookmaker='Tippmix' AND q.market='ou' AND q.outcome='under' AND q.line=2.5), max(q.price) FILTER (WHERE q.bookmaker='Avg' AND q.market='ou' AND q.outcome='under' AND q.line=2.5), max(q.price) FILTER (WHERE q.bookmaker='Max' AND q.market='ou' AND q.outcome='under' AND q.line=2.5), max(q.price) FILTER (WHERE q.market='ou' AND q.outcome='under' AND q.line=2.5))
                    ) AS odds,
                    o.market_count, o.source_id, o.captured_at, o.trust_state
             FROM fact.odds_snapshot o
             JOIN LATERAL (
                 SELECT DISTINCT ON (bookmaker, market, outcome, line) *
                 FROM fact.odds_quote
                 WHERE match_id = o.match_id AND source_id = o.source_id
                 ORDER BY bookmaker, market, outcome, line, (phase = 'closing') DESC
             ) q ON true
             WHERE o.match_id = m.id
             GROUP BY o.market_count, o.source_id, o.captured_at, o.trust_state
             ORDER BY (o.trust_state = 'verified') DESC, o.source_id
             LIMIT 1
         ) o ON true
         LEFT JOIN ops.source os ON os.id = o.source_id
         -- S134: a research pillanatkép játékideje a legfrissebb kiválasztott
         -- match_core obszervációból. Nincs új oszlop a kanonikus meccsen.
         LEFT JOIN LATERAL (
             SELECT (o2.normalized_payload->>'status_minute')::int AS minute
             FROM fact.provenance p
             JOIN stage.observation o2 ON o2.id = p.observation_id
             JOIN ops.source s ON s.id = o2.source_id
             WHERE p.subject_id = m.id AND s.code = 'research'
               AND o2.normalized_payload->>'status_minute' IS NOT NULL
             ORDER BY o2.observed_at DESC
             LIMIT 1
         ) sm ON true
         LEFT JOIN LATERAL (
             SELECT CASE WHEN a.storage_key IS NOT NULL
                         THEN '/v1/asset/' || a.storage_key
                         ELSE a.url END AS logo
             FROM ops.asset a
             WHERE a.entity_kind = 'team' AND a.kind = 'logo'
               AND a.entity_id = m.home_team_id
             ORDER BY (a.storage_key IS NULL), a.captured_at DESC
             LIMIT 1
         ) hl ON true
         LEFT JOIN LATERAL (
             SELECT CASE WHEN a.storage_key IS NOT NULL
                         THEN '/v1/asset/' || a.storage_key
                         ELSE a.url END AS logo
             FROM ops.asset a
             WHERE a.entity_kind = 'team' AND a.kind = 'logo'
               AND a.entity_id = m.away_team_id
             ORDER BY (a.storage_key IS NULL), a.captured_at DESC
             LIMIT 1
         ) al ON true
         LEFT JOIN LATERAL (
             SELECT CASE WHEN a.storage_key IS NOT NULL
                         THEN '/v1/asset/' || a.storage_key
                         ELSE a.url END AS logo
             FROM ops.asset a
             WHERE a.entity_kind = 'competition' AND a.kind = 'logo'
               AND a.entity_id = c.entity_id
             ORDER BY (a.storage_key IS NULL), a.captured_at DESC
             LIMIT 1
         ) cl ON true
         LEFT JOIN LATERAL (
             SELECT e.elo::double precision AS elo
             FROM fact.club_elo e
             WHERE e.club_entity_id = m.home_team_id AND e.as_of <= $1
             ORDER BY e.as_of DESC
             LIMIT 1
         ) he ON true
         LEFT JOIN LATERAL (
             SELECT e.elo::double precision AS elo
             FROM fact.club_elo e
             WHERE e.club_entity_id = m.away_team_id AND e.as_of <= $1
             ORDER BY e.as_of DESC
             LIMIT 1
         ) ae ON true
         JOIN ops.publish_gate g ON g.match_id = m.id
           AND g.publish_date = (m.kickoff_at AT TIME ZONE $2)::date
         -- S216: leállított (canceled) meccsek nem jelennek meg a napi listában.
         WHERE m.status <> 'canceled'
           AND (m.kickoff_at AT TIME ZONE $2)::date
                 BETWEEN $1 - ($5::int) AND $1 + ($5::int)
           AND ($4::uuid IS NULL OR m.competition_id = $4)
           AND ($6::uuid IS NULL OR m.home_team_id = $6 OR m.away_team_id = $6)
           AND ($3::boolean
               OR g.state IN ('verified_publish', 'provisional_publish'))
           AND NOT EXISTS (
               SELECT 1 FROM ops.editorial_hold h WHERE h.match_id = m.id
           )
         ORDER BY m.kickoff_at, home.name, away.name",
    )
    .bind(filter.date)
    .bind(&filter.timezone)
    .bind(filter.include_all)
    .bind(filter.competition_id)
    .bind(filter.window_days)
    .bind(filter.team_id)
    .fetch_all(pool)
    .await?;

    // S215: a napi lekérés determinisztikusan kiszűri a feloldatlan
    // cross-source duplikátumokat. Az adatot nem írjuk át — a mögöttes sorok
    // továbbra is auditálhatók, és az integritási riport kemény hibát jelez.
    let keys: Vec<_> = rows
        .iter()
        .map(|r| {
            (
                r.home_team_id,
                r.away_team_id,
                r.kickoff_at.date_naive(),
                r.kickoff_precision.as_str(),
                r.match_id,
            )
        })
        .collect();
    let mut losers = duplicate_fixture_losers(&keys);
    // Ugyanaz a csapatpár ugyanabban a kickoff-pillanatban szintén duplikátum
    // (a két pontos sor esete a paraguayi csoportban) — determinisztikusan
    // egyet megjelenítünk, az adatot itt sem írjuk át.
    let instant_keys: Vec<_> = rows
        .iter()
        .map(|r| {
            (
                r.home_team_id,
                r.away_team_id,
                r.kickoff_at,
                r.kickoff_precision.as_str(),
                r.match_id,
            )
        })
        .collect();
    losers.extend(duplicate_instant_losers(&instant_keys));
    let rows = rows
        .into_iter()
        .filter(|r| !losers.contains(&r.match_id))
        .collect();
    Ok(rows)
}

/// S215: determinisztikus duplikátum-kulcs. A csapatok kanonikus ID-i és az
/// UTC nap alapján dönt — ez nem fuzzy név-összevonás. Csak akkor avatkozik
/// közbe, ha a csoportban van `date_only` placeholder sor; ilyenkor a pontos
/// kezdésű sort tartja meg (több ilyen esetén a legalacsonyabb UUID-t).
pub fn duplicate_fixture_losers(
    rows: &[(Uuid, Uuid, NaiveDate, &str, Uuid)],
) -> std::collections::HashSet<Uuid> {
    use std::collections::{HashMap, HashSet};

    let mut groups: HashMap<(Uuid, Uuid, NaiveDate), Vec<usize>> = HashMap::new();
    for (i, (home, away, day, _, _)) in rows.iter().enumerate() {
        let key = if home <= away {
            (*home, *away, *day)
        } else {
            (*away, *home, *day)
        };
        groups.entry(key).or_default().push(i);
    }

    let mut losers = HashSet::new();
    for indices in groups.values() {
        if indices.len() < 2 || !indices.iter().any(|&i| rows[i].3 == "date_only") {
            continue;
        }
        let winner = indices
            .iter()
            .copied()
            .filter(|&i| rows[i].3 == "exact")
            .min_by_key(|&i| rows[i].4)
            .or_else(|| indices.iter().copied().min_by_key(|&i| rows[i].4))
            .expect("a csoport nem lehet üres");
        for &i in indices {
            if i != winner {
                losers.insert(rows[i].4);
            }
        }
    }
    losers
}

/// Ugyanaz a kanonikus csapatpár ugyanabban a pontos kickoff-pillanatban.
/// Itt nem kell `date_only` jelzés: két `exact` sor esetén is egyet mutatunk.
pub fn duplicate_instant_losers(
    rows: &[(Uuid, Uuid, DateTime<Utc>, &str, Uuid)],
) -> std::collections::HashSet<Uuid> {
    use std::collections::{HashMap, HashSet};
    let mut groups: HashMap<(Uuid, Uuid, DateTime<Utc>), Vec<usize>> = HashMap::new();
    for (i, (home, away, instant, precision, _)) in rows.iter().enumerate() {
        if *precision != "exact" {
            continue;
        }
        let key = if home <= away {
            (*home, *away, *instant)
        } else {
            (*away, *home, *instant)
        };
        groups.entry(key).or_default().push(i);
    }
    let mut losers = HashSet::new();
    for indices in groups.values() {
        if indices.len() < 2 {
            continue;
        }
        let winner = indices
            .iter()
            .copied()
            .min_by_key(|&i| rows[i].4)
            .expect("nem üres");
        for &i in indices {
            if i != winner {
                losers.insert(rows[i].4);
            }
        }
    }
    losers
}

/// A nap legutóbbi gate-kiértékelésének ideje (batch/frissítés jelzés).
pub async fn fetch_day_refreshed_at(
    pool: &PgPool,
    date: NaiveDate,
) -> Result<Option<DateTime<Utc>>> {
    let value: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT max(evaluated_at) FROM ops.publish_gate WHERE publish_date = $1",
    )
    .bind(date)
    .fetch_one(pool)
    .await?;
    Ok(value)
}

/// Napi hiány-mutatók (a nap adatminőségének áttekintéséhez).
#[derive(Debug, Clone, Default, FromRow, serde::Serialize)]
pub struct DayGaps {
    /// Kezdésidő nélküli (date_only) meccsek.
    pub missing_kickoff: i64,
    /// Kezdés elmúlt, de a meccs nincs lezárva.
    pub open_past: i64,
    /// Lezárt meccs eredmény nélkül.
    pub missing_result: i64,
    /// Legalább egyik csapatlogó hiányzik.
    pub missing_team_logo: i64,
    /// Verseny-logó hiányzik.
    pub missing_competition_logo: i64,
    /// Nincs odds-snapshot.
    pub missing_odds: i64,
    /// A napon visszatartott (withheld) meccsek.
    pub withheld: i64,
}

/// A nap hiány-mutatói a publishable (verified/provisional) meccsekre.
pub async fn fetch_day_gaps(pool: &PgPool, filter: &DayFilter) -> Result<DayGaps> {
    let gaps = sqlx::query_as::<_, DayGaps>(
        "WITH base AS (
            SELECT m.id, m.kickoff_at, m.kickoff_precision, m.status, r.home_ft,
                   hl.logo IS NOT NULL AS has_home_logo,
                   al.logo IS NOT NULL AS has_away_logo,
                   cl.logo IS NOT NULL AS has_comp_logo,
                   EXISTS (SELECT 1 FROM fact.odds_snapshot o WHERE o.match_id = m.id) AS has_odds
            FROM core.match m
            JOIN core.competition c ON c.id = m.competition_id
            JOIN ops.publish_gate g ON g.match_id = m.id
              AND g.publish_date = (m.kickoff_at AT TIME ZONE $2)::date
            LEFT JOIN core.match_result r ON r.match_id = m.id
            LEFT JOIN LATERAL (
                SELECT COALESCE(a.storage_key, a.url) AS logo FROM ops.asset a
                WHERE a.entity_kind = 'team' AND a.kind = 'logo' AND a.entity_id = m.home_team_id
                ORDER BY (a.storage_key IS NULL), a.captured_at DESC LIMIT 1
            ) hl ON true
            LEFT JOIN LATERAL (
                SELECT COALESCE(a.storage_key, a.url) AS logo FROM ops.asset a
                WHERE a.entity_kind = 'team' AND a.kind = 'logo' AND a.entity_id = m.away_team_id
                ORDER BY (a.storage_key IS NULL), a.captured_at DESC LIMIT 1
            ) al ON true
            LEFT JOIN LATERAL (
                SELECT COALESCE(a.storage_key, a.url) AS logo FROM ops.asset a
                WHERE a.entity_kind = 'competition' AND a.kind = 'logo' AND a.entity_id = c.entity_id
                ORDER BY (a.storage_key IS NULL), a.captured_at DESC LIMIT 1
            ) cl ON true
            -- S216: két pontos sor ugyanabban a pillanatban: a nagyobb UUID
            -- kimarad a hiány-számokból is (ugyanaz, mint a fetch_day).
            AND NOT EXISTS (
                SELECT 1 FROM core.match m3
                 WHERE least(m3.home_team_id, m3.away_team_id)
                       = least(m.home_team_id, m.away_team_id)
                   AND greatest(m3.home_team_id, m3.away_team_id)
                       = greatest(m.home_team_id, m.away_team_id)
                   AND m3.kickoff_at = m.kickoff_at
                   AND m3.kickoff_precision = 'exact'
                   AND m.kickoff_precision = 'exact'
                   AND m3.id < m.id
            )
            -- S216: a leállított sorok nem számítanak hiánynak sem.
            WHERE m.status <> 'canceled'
              -- S215: ugyanaz a determinisztikus duplikátum-szűrés, mint a
              -- fetch_day-ben — a hiány-számok ne számolják a placeholder sorokat.
              AND NOT EXISTS (
                SELECT 1 FROM core.match m2
                 WHERE least(m2.home_team_id, m2.away_team_id)
                       = least(m.home_team_id, m.away_team_id)
                   AND greatest(m2.home_team_id, m2.away_team_id)
                       = greatest(m.home_team_id, m.away_team_id)
                   AND m2.kickoff_at::date = m.kickoff_at::date
                   AND m2.id <> m.id
                   AND (
                         (m.kickoff_precision = 'date_only'
                          AND m2.kickoff_precision = 'exact')
                         OR (m.kickoff_precision = 'date_only'
                             AND m2.kickoff_precision = 'date_only'
                             AND m2.id < m.id)
                       )
            )
              AND (m.kickoff_at AT TIME ZONE $2)::date
                    BETWEEN $1 - ($4::int) AND $1 + ($4::int)
              AND ($3::uuid IS NULL OR m.competition_id = $3)
              AND g.state IN ('verified_publish', 'provisional_publish')
              AND NOT EXISTS (
                  SELECT 1 FROM ops.editorial_hold h WHERE h.match_id = m.id
              )
        )
        SELECT
            count(*) FILTER (WHERE kickoff_precision = 'date_only') AS missing_kickoff,

            -- S163: a kliens szűrőjével azonos küszöb. Az elo/felidos sorok
            -- addig az Elo csoportban vannak (150 perc), csak utana szamitanak
            -- lejartnak, kulonben ketszer jelennenek meg.
            count(*) FILTER (WHERE kickoff_precision <> 'date_only'
                AND status NOT IN ('finished','postponed','canceled','abandoned','awarded')
                AND kickoff_at < now() - CASE
                    WHEN status IN ('live','halftime') THEN interval '150 minutes'
                    ELSE interval '10 minutes' END) AS open_past,
            count(*) FILTER (WHERE status = 'finished' AND home_ft IS NULL) AS missing_result,
            count(*) FILTER (WHERE NOT has_home_logo OR NOT has_away_logo) AS missing_team_logo,
            count(*) FILTER (WHERE NOT has_comp_logo) AS missing_competition_logo,
            count(*) FILTER (WHERE NOT has_odds) AS missing_odds,
            (SELECT count(*) FROM ops.publish_gate gw
              WHERE gw.publish_date = $1 AND gw.state = 'withheld') AS withheld
        FROM base",
    )
    .bind(filter.date)
    .bind(&filter.timezone)
    .bind(filter.competition_id)
    .bind(filter.window_days)
    .fetch_one(pool)
    .await?;
    Ok(gaps)
}

#[cfg(test)]
mod duplicate_fixture_tests {
    use super::{duplicate_fixture_losers, duplicate_instant_losers};
    use chrono::{NaiveDate, TimeZone, Utc};
    use uuid::Uuid;

    fn row(
        id: u8,
        home: u8,
        away: u8,
        precision: &'static str,
    ) -> (Uuid, Uuid, NaiveDate, &'static str, Uuid) {
        (
            Uuid::from_u128(home as u128),
            Uuid::from_u128(away as u128),
            NaiveDate::from_ymd_opt(2026, 11, 25).unwrap(),
            precision,
            Uuid::from_u128(id as u128),
        )
    }

    #[test]
    fn date_only_placeholder_duplicates_drop_all_but_exact() {
        // A paraguayi eset: egy pontos ESPN-sor + ket FotMob placeholder.
        let rows = vec![
            row(1, 10, 20, "exact"),
            row(2, 10, 20, "date_only"),
            row(3, 10, 20, "date_only"),
        ];
        let losers = duplicate_fixture_losers(&rows);
        assert_eq!(losers.len(), 2);
        assert!(!losers.contains(&rows[0].4));
        assert!(losers.contains(&rows[1].4));
        assert!(losers.contains(&rows[2].4));
    }

    #[test]
    fn reversed_teams_are_still_one_fixture() {
        let rows = vec![row(1, 20, 10, "exact"), row(2, 10, 20, "date_only")];
        let losers = duplicate_fixture_losers(&rows);
        assert_eq!(losers.len(), 1);
        assert!(losers.contains(&rows[1].4));
    }

    #[test]
    fn two_exact_matches_on_same_day_are_not_merged() {
        // Ket different competition, mindkETTŐ pontos idővel: nem döntünk,
        // az integritási riport jelez, de az API nem rejt el adatot.
        let rows = vec![row(1, 10, 20, "exact"), row(2, 10, 20, "exact")];
        assert!(duplicate_fixture_losers(&rows).is_empty());
    }

    #[test]
    fn two_exact_rows_at_same_instant_keep_only_one() {
        let instant = Utc.with_ymd_and_hms(2026, 11, 25, 20, 0, 0).unwrap();
        let rows = vec![
            (
                Uuid::from_u128(10),
                Uuid::from_u128(20),
                instant,
                "exact",
                Uuid::from_u128(1),
            ),
            (
                Uuid::from_u128(10),
                Uuid::from_u128(20),
                instant,
                "exact",
                Uuid::from_u128(2),
            ),
        ];
        let losers = duplicate_instant_losers(&rows);
        assert_eq!(losers.len(), 1);
        assert!(losers.contains(&rows[1].4));
    }

    #[test]
    fn different_teams_or_days_are_never_merged() {
        let rows = vec![
            row(1, 10, 20, "exact"),
            row(2, 10, 21, "date_only"),
            row(3, 11, 20, "date_only"),
        ];
        assert!(duplicate_fixture_losers(&rows).is_empty());
    }
}
