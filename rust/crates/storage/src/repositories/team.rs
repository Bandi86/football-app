//! Csapat-lista és csapat-adatlap (S12): kanonikus csapatok kereséssel,
//! megfigyelt mérleggel (W/D/L, gólarány) és a legfrissebb Elo-val.

use anyhow::Result;
use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct TeamListRow {
    pub team_id: Uuid,
    pub name: String,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub gender: String,
    pub branch: String,
    pub logo: Option<String>,
    pub recent_matches: i64,
}

pub async fn fetch_teams(
    pool: &PgPool,
    query: &str,
    date: NaiveDate,
    timezone: &str,
) -> Result<Vec<TeamListRow>> {
    // A korábbi LATERAL a 400 csapat mindegyikére külön pásztázta a
    // core.match táblát (~27 ms/csapat): a 60 napos ablakot egyszer
    // aggregáljuk, és a csapat-listához joinoljuk (S40 teljesítmény-fix).
    let rows = sqlx::query_as::<_, TeamListRow>(
        "WITH recent AS (
             SELECT team_id, count(*)::bigint AS recent_matches
             FROM (
                 SELECT m.home_team_id AS team_id
                 FROM core.match m
                 JOIN ops.publish_gate g ON g.match_id = m.id
                   AND g.publish_date = (m.kickoff_at AT TIME ZONE $3)::date
                 WHERE (m.kickoff_at AT TIME ZONE $3)::date BETWEEN $2 - 30 AND $2 + 30
                   AND g.state = 'verified_publish'
                 UNION ALL
                 SELECT m.away_team_id
                 FROM core.match m
                 JOIN ops.publish_gate g ON g.match_id = m.id
                   AND g.publish_date = (m.kickoff_at AT TIME ZONE $3)::date
                 WHERE (m.kickoff_at AT TIME ZONE $3)::date BETWEEN $2 - 30 AND $2 + 30
                   AND g.state = 'verified_publish'
             ) sides
             GROUP BY team_id
         )
         SELECT
            t.id AS team_id,
            t.name,
            co.code AS country_code,
            co.name AS country_name,
            t.gender,
            t.branch,
            tl.logo,
            COALESCE(recent.recent_matches, 0)::bigint AS recent_matches
         FROM core.team t
         LEFT JOIN core.country co ON co.id = t.country_id
         LEFT JOIN recent ON recent.team_id = t.id
         LEFT JOIN LATERAL (
             SELECT CASE WHEN a.storage_key IS NOT NULL
                         THEN '/v1/asset/' || a.storage_key
                         ELSE a.url END AS logo
             FROM ops.asset a
             WHERE a.entity_kind = 'team' AND a.kind = 'logo'
               AND a.entity_id = t.id
             ORDER BY (a.storage_key IS NULL), a.captured_at DESC
             LIMIT 1
         ) tl ON true
         WHERE ($1 = '' OR t.name ILIKE '%' || $1 || '%')
         ORDER BY COALESCE(recent.recent_matches, 0) DESC, t.name
         LIMIT 400",
    )
    .bind(query.trim())
    .bind(date)
    .bind(timezone)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// A csapat as-of kerete a meccs-szintű felállásokból. A játékos neve és
/// entitása csak akkor jelenik meg, ha a forrás feloldotta; különben a
/// `source_player_id` marad. Hiány esetén üres lista.
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct TeamPlayerRow {
    pub source_player_id: String,
    pub player_entity_id: Option<Uuid>,
    pub player_name: Option<String>,
    pub position: Option<String>,
    pub shirt_number: Option<i32>,
    pub captured_at: chrono::DateTime<chrono::Utc>,
}

/// A legfrissebb felállás-sorok a csapat meccseiről (legfeljebb 30 játékos).
pub async fn fetch_team_players(pool: &PgPool, team_id: Uuid) -> Result<Vec<TeamPlayerRow>> {
    sqlx::query_as::<_, TeamPlayerRow>(
        "SELECT DISTINCT ON (l.source_player_id)
                l.source_player_id, l.player_entity_id,
                e.canonical_name AS player_name, l.position, l.shirt_number,
                l.captured_at
           FROM fact.match_lineups l
           JOIN core.match m ON m.id = l.match_id
           LEFT JOIN core.entity e ON e.id = l.player_entity_id
          WHERE (m.home_team_id = $1 AND l.side = 'home')
             OR (m.away_team_id = $1 AND l.side = 'away')
          ORDER BY l.source_player_id, l.captured_at DESC
          LIMIT 30",
    )
    .bind(team_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct TeamDetailRow {
    pub team_id: Uuid,
    pub name: String,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub gender: String,
    pub branch: String,
    pub logo: Option<String>,
    pub elo: Option<f64>,
    pub played: i64,
    pub wins: i64,
    pub draws: i64,
    pub losses: i64,
    pub goals_for: i64,
    pub goals_against: i64,
}

pub async fn fetch_team_detail(
    pool: &PgPool,
    team_id: Uuid,
    date: NaiveDate,
    timezone: &str,
    window_days: i64,
) -> Result<Option<TeamDetailRow>> {
    let row = sqlx::query_as::<_, TeamDetailRow>(
        "WITH results AS (
             SELECT m.id, m.home_team_id, m.away_team_id,
                    COALESCE(r.home_et, r.home_ft) AS hf,
                    COALESCE(r.away_et, r.away_ft) AS af
             FROM core.match m
             JOIN ops.publish_gate g ON g.match_id = m.id
               AND g.publish_date = (m.kickoff_at AT TIME ZONE $3)::date
             LEFT JOIN core.match_result r ON r.match_id = m.id
             WHERE (m.home_team_id = $1 OR m.away_team_id = $1)
               AND (m.kickoff_at AT TIME ZONE $3)::date
                     BETWEEN $2 - ($4::int) AND $2 + ($4::int)
               AND g.state = 'verified_publish'
               AND COALESCE(r.home_et, r.home_ft) IS NOT NULL
               AND COALESCE(r.away_et, r.away_ft) IS NOT NULL
         ), spelled AS (
             SELECT home_team_id AS tid, hf AS gf, af AS ga
             FROM results WHERE home_team_id = $1
             UNION ALL
             SELECT away_team_id, af, hf
             FROM results WHERE away_team_id = $1
         )
         SELECT
            t.id AS team_id,
            t.name,
            co.code AS country_code,
            co.name AS country_name,
            t.gender,
            t.branch,
            tl.logo,
            el.elo,
            count(s.tid)::bigint AS played,
            count(*) FILTER (WHERE s.gf > s.ga)::bigint AS wins,
            count(*) FILTER (WHERE s.gf = s.ga)::bigint AS draws,
            count(*) FILTER (WHERE s.gf < s.ga)::bigint AS losses,
            COALESCE(sum(s.gf), 0)::bigint AS goals_for,
            COALESCE(sum(s.ga), 0)::bigint AS goals_against
         FROM core.team t
         LEFT JOIN core.country co ON co.id = t.country_id
         LEFT JOIN LATERAL (
             SELECT CASE WHEN a.storage_key IS NOT NULL
                         THEN '/v1/asset/' || a.storage_key
                         ELSE a.url END AS logo
             FROM ops.asset a
             WHERE a.entity_kind = 'team' AND a.kind = 'logo'
               AND a.entity_id = t.id
             ORDER BY (a.storage_key IS NULL), a.captured_at DESC
             LIMIT 1
         ) tl ON true
         LEFT JOIN LATERAL (
             SELECT e.elo::double precision AS elo
             FROM fact.club_elo e
             WHERE e.club_entity_id = t.id AND e.as_of <= $2
             ORDER BY e.as_of DESC
             LIMIT 1
         ) el ON true
         LEFT JOIN spelled s ON true
         WHERE t.id = $1
         GROUP BY t.id, t.name, co.code, co.name, t.gender, t.branch, tl.logo, el.elo",
    )
    .bind(team_id)
    .bind(date)
    .bind(timezone)
    .bind(window_days)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}
