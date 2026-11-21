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
}

pub async fn fetch_day(pool: &PgPool, filter: &DayFilter) -> Result<Vec<DayMatchRow>> {
    let rows = sqlx::query_as::<_, DayMatchRow>(
        "SELECT
            m.id AS match_id,
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
            ae.elo AS away_elo
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
         WHERE (m.kickoff_at AT TIME ZONE $2)::date
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
    Ok(rows)
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
            WHERE (m.kickoff_at AT TIME ZONE $2)::date
                    BETWEEN $1 - ($4::int) AND $1 + ($4::int)
              AND ($3::uuid IS NULL OR m.competition_id = $3)
              AND g.state IN ('verified_publish', 'provisional_publish')
              AND NOT EXISTS (
                  SELECT 1 FROM ops.editorial_hold h WHERE h.match_id = m.id
              )
        )
        SELECT
            count(*) FILTER (WHERE kickoff_precision = 'date_only') AS missing_kickoff,
            count(*) FILTER (WHERE kickoff_at < now() - interval '3 hours'
                AND status NOT IN ('finished','postponed','canceled','abandoned','awarded')) AS open_past,
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
