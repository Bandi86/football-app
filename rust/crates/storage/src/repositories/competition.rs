//! Verseny-lista és verseny-adatlap (S11): a napi/környéki, ellenőrzött
//! meccsekből számolt összesítők és csapat-mérlegek. A referencia a
//! Sofascore liga-oldala (fejléc + meccsek + csapatok), a mi adatainkra
//! szűkítve: nincs rating/forma, a W/D/L a megfigyelt ablakból számol.

use anyhow::{Context, Result};
use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct CompetitionListRow {
    pub competition_id: Uuid,
    pub name: String,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub tier: i16,
    pub priority: i32,
    pub logo: Option<String>,
    pub matches: i64,
    pub finished: i64,
    pub live: i64,
    pub scheduled: i64,
}

pub async fn fetch_competitions(
    pool: &PgPool,
    date: NaiveDate,
    timezone: &str,
) -> Result<Vec<CompetitionListRow>> {
    let rows = sqlx::query_as::<_, CompetitionListRow>(
        "SELECT
            c.id AS competition_id,
            c.name,
            co.code AS country_code,
            co.name AS country_name,
            c.tier,
            c.priority,
            lg.logo,
            count(*)::bigint AS matches,
            count(*) FILTER (WHERE m.status = 'finished')::bigint AS finished,
            count(*) FILTER (WHERE m.status IN ('live', 'halftime'))::bigint AS live,
            count(*) FILTER (WHERE m.status = 'scheduled')::bigint AS scheduled
         FROM core.match m
         JOIN ops.publish_gate g ON g.match_id = m.id
           AND g.publish_date = (m.kickoff_at AT TIME ZONE $2)::date
         JOIN core.competition c ON c.id = m.competition_id
         LEFT JOIN core.country co ON co.id = c.country_id
         LEFT JOIN LATERAL (
             SELECT CASE WHEN a.storage_key IS NOT NULL
                         THEN '/v1/asset/' || a.storage_key
                         ELSE a.url END AS logo
             FROM ops.asset a
             WHERE a.entity_kind = 'competition' AND a.kind = 'logo'
               AND a.entity_id = c.entity_id
             ORDER BY (a.storage_key IS NULL), a.captured_at DESC
             LIMIT 1
         ) lg ON true
         WHERE (m.kickoff_at AT TIME ZONE $2)::date = $1
           AND g.state = 'verified_publish'
         GROUP BY c.id, c.name, co.code, co.name, c.tier, c.priority, lg.logo
         ORDER BY c.priority, co.name NULLS LAST, c.name",
    )
    .bind(date)
    .bind(timezone)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct CompetitionDetailRow {
    pub competition_id: Uuid,
    pub name: String,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub tier: i16,
    pub priority: i32,
    pub logo: Option<String>,
    pub season_label: Option<String>,
    pub matches: i64,
    pub goals: i64,
    pub btts_matches: i64,
    pub home_wins: i64,
    pub draws: i64,
    pub away_wins: i64,
}

pub async fn fetch_competition_detail(
    pool: &PgPool,
    competition_id: Uuid,
    date: NaiveDate,
    timezone: &str,
    window_days: i64,
) -> Result<Option<CompetitionDetailRow>> {
    let row = sqlx::query_as::<_, CompetitionDetailRow>(
        "WITH gated AS (
             SELECT m.id,
                    COALESCE(r.home_et, r.home_ft) AS hf,
                    COALESCE(r.away_et, r.away_ft) AS af
             FROM core.match m
             JOIN ops.publish_gate g ON g.match_id = m.id
               AND g.publish_date = (m.kickoff_at AT TIME ZONE $3)::date
             JOIN core.season s ON s.id = m.season_id
             LEFT JOIN core.match_result r ON r.match_id = m.id
             WHERE m.competition_id = $1
               AND (m.kickoff_at AT TIME ZONE $3)::date
                     BETWEEN $2 - ($4::int) AND $2 + ($4::int)
               AND g.state = 'verified_publish'
         )
         SELECT
            c.id AS competition_id,
            c.name,
            co.code AS country_code,
            co.name AS country_name,
            c.tier,
            c.priority,
            lg.logo,
            (SELECT label FROM core.season s2
              WHERE s2.id = (SELECT m2.season_id FROM core.match m2
                              WHERE m2.competition_id = c.id
                              ORDER BY m2.kickoff_at DESC LIMIT 1)) AS season_label,
            count(gated.id)::bigint AS matches,
            COALESCE(sum(gated.hf + gated.af), 0)::bigint AS goals,
            count(*) FILTER (WHERE gated.hf > 0 AND gated.af > 0)::bigint AS btts_matches,
            count(*) FILTER (WHERE gated.hf > gated.af)::bigint AS home_wins,
            count(*) FILTER (WHERE gated.hf = gated.af)::bigint AS draws,
            count(*) FILTER (WHERE gated.hf < gated.af)::bigint AS away_wins
         FROM core.competition c
         LEFT JOIN core.country co ON co.id = c.country_id
         LEFT JOIN LATERAL (
             SELECT CASE WHEN a.storage_key IS NOT NULL
                         THEN '/v1/asset/' || a.storage_key
                         ELSE a.url END AS logo
             FROM ops.asset a
             WHERE a.entity_kind = 'competition' AND a.kind = 'logo'
               AND a.entity_id = c.entity_id
             ORDER BY (a.storage_key IS NULL), a.captured_at DESC
             LIMIT 1
         ) lg ON true
         LEFT JOIN gated ON true
         WHERE c.id = $1
         GROUP BY c.id, c.name, co.code, co.name, c.tier, c.priority, lg.logo",
    )
    .bind(competition_id)
    .bind(date)
    .bind(timezone)
    .bind(window_days)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct CompetitionTeamRow {
    pub team_id: Uuid,
    pub name: String,
    pub logo: Option<String>,
    pub played: i64,
    pub wins: i64,
    pub draws: i64,
    pub losses: i64,
    pub goals_for: i64,
    pub goals_against: i64,
}

pub async fn fetch_competition_teams(
    pool: &PgPool,
    competition_id: Uuid,
    date: NaiveDate,
    timezone: &str,
    window_days: i64,
) -> Result<Vec<CompetitionTeamRow>> {
    let rows = sqlx::query_as::<_, CompetitionTeamRow>(
        "WITH gated AS (
             SELECT m.home_team_id AS home_id, m.away_team_id AS away_id,
                    COALESCE(r.home_et, r.home_ft) AS hf,
                    COALESCE(r.away_et, r.away_ft) AS af
             FROM core.match m
             JOIN ops.publish_gate g ON g.match_id = m.id
               AND g.publish_date = (m.kickoff_at AT TIME ZONE $3)::date
             LEFT JOIN core.match_result r ON r.match_id = m.id
             WHERE m.competition_id = $1
               AND (m.kickoff_at AT TIME ZONE $3)::date
                     BETWEEN $2 - ($4::int) AND $2 + ($4::int)
               AND g.state = 'verified_publish'
               AND COALESCE(r.home_et, r.home_ft) IS NOT NULL
               AND COALESCE(r.away_et, r.away_ft) IS NOT NULL
         ), appearances AS (
             SELECT home_id AS team_id, hf AS goals_for, af AS goals_against FROM gated
             UNION ALL
             SELECT away_id, af, hf FROM gated
         )
         SELECT
            t.id AS team_id,
            t.name,
            tl.logo,
            count(*)::bigint AS played,
            count(*) FILTER (WHERE a.goals_for > a.goals_against)::bigint AS wins,
            count(*) FILTER (WHERE a.goals_for = a.goals_against)::bigint AS draws,
            count(*) FILTER (WHERE a.goals_for < a.goals_against)::bigint AS losses,
            COALESCE(sum(a.goals_for), 0)::bigint AS goals_for,
            COALESCE(sum(a.goals_against), 0)::bigint AS goals_against
         FROM appearances a
         JOIN core.team t ON t.id = a.team_id
         LEFT JOIN LATERAL (
             SELECT CASE WHEN ass.storage_key IS NOT NULL
                         THEN '/v1/asset/' || ass.storage_key
                         ELSE ass.url END AS logo
             FROM ops.asset ass
             WHERE ass.entity_kind = 'team' AND ass.kind = 'logo'
               AND ass.entity_id = t.id
             ORDER BY (ass.storage_key IS NULL), ass.captured_at DESC
             LIMIT 1
         ) tl ON true
         GROUP BY t.id, t.name, tl.logo
         ORDER BY wins DESC, (sum(a.goals_for) - sum(a.goals_against)) DESC, t.name",
    )
    .bind(competition_id)
    .bind(date)
    .bind(timezone)
    .bind(window_days)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Egy verseny szezonjai meccs/eredmény/odds darabszámmal (kliens szezon-nézet).
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct CompetitionSeasonRow {
    pub label: String,
    pub matches: i64,
    pub with_result: i64,
    pub with_odds: i64,
}

pub async fn fetch_competition_seasons(
    pool: &PgPool,
    competition_id: Uuid,
) -> Result<Vec<CompetitionSeasonRow>> {
    sqlx::query_as(
        "SELECT s.label, count(*)::bigint AS matches,
                count(r.match_id)::bigint AS with_result,
                count(*) FILTER (WHERE EXISTS (
                    SELECT 1 FROM fact.odds_snapshot o WHERE o.match_id = m.id))::bigint AS with_odds
         FROM core.match m
         JOIN core.season s ON s.id = m.season_id
         LEFT JOIN core.match_result r ON r.match_id = m.id
         WHERE m.competition_id = $1
         GROUP BY s.label
         ORDER BY s.label DESC",
    )
    .bind(competition_id)
    .fetch_all(pool)
    .await
    .context("verseny szezonjainak lekérése")
}
