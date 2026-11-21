//! Meccs-részletező (S10): egy kanonikus meccs teljes, ellenőrzött képe —
//! eredmény-bontás, statisztika, játékvezető, odds-lánc + trust, Elo és a
//! publikálási kapu állapota. A napi listánál részletesebb, de ugyanazokat
//! a bizalmi mezőket használja.

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct MatchDetailRow {
    pub id: Uuid,
    pub kickoff_at: DateTime<Utc>,
    pub kickoff_precision: String,
    pub status: String,
    pub round_label: Option<String>,
    pub competition_id: Uuid,
    pub competition_name: String,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub tier: i16,
    pub priority: i32,
    pub season_label: String,
    pub home_name: String,
    pub away_name: String,
    pub home_logo: Option<String>,
    pub away_logo: Option<String>,
    pub home_elo: Option<f64>,
    pub away_elo: Option<f64>,
    pub home_ft: Option<i16>,
    pub away_ft: Option<i16>,
    pub home_ht: Option<i16>,
    pub away_ht: Option<i16>,
    pub home_et: Option<i16>,
    pub away_et: Option<i16>,
    pub home_pens: Option<i16>,
    pub away_pens: Option<i16>,
    pub decided_by: Option<String>,
    pub referee: Option<String>,
    pub stats_source: Option<String>,
    pub stats_captured_at: Option<DateTime<Utc>>,
    pub home_shots: Option<i32>,
    pub away_shots: Option<i32>,
    pub home_shots_on_target: Option<i32>,
    pub away_shots_on_target: Option<i32>,
    pub home_corners: Option<i32>,
    pub away_corners: Option<i32>,
    pub home_yellow: Option<i32>,
    pub away_yellow: Option<i32>,
    pub home_red: Option<i32>,
    pub away_red: Option<i32>,
    pub home_fouls: Option<i32>,
    pub away_fouls: Option<i32>,
    pub home_xg: Option<f64>,
    pub away_xg: Option<f64>,
    #[schema(value_type = Option<football_domain::trust::OddsValues>)]
    pub odds: Option<serde_json::Value>,
    pub market_count: Option<i32>,
    pub odds_source: Option<String>,
    pub odds_captured_at: Option<DateTime<Utc>>,
    pub odds_trust_state: Option<String>,
    pub gate_state: Option<String>,
    #[schema(value_type = Option<Vec<football_domain::verification::Finding>>)]
    pub gate_blockers: Option<serde_json::Value>,
    pub gate_evaluated_at: Option<DateTime<Utc>>,
    pub gate_rule_set_version: Option<String>,
    pub publish_date: Option<NaiveDate>,
    /// Az utolsó öt lejátszott meccs a hazai csapattól (a meccs kezdete előtt).
    #[serde(default)]
    pub home_form: Vec<FormEntry>,
    /// Az utolsó öt lejátszott meccs a vendég csapattól (a meccs kezdete előtt).
    #[serde(default)]
    pub away_form: Vec<FormEntry>,
    /// Az utolsó öt egymás elleni meccs (a meccs kezdete előtt).
    #[serde(default)]
    pub head_to_head: Vec<HeadToHeadEntry>,
    /// A hazai csapat következő legfeljebb három meccse.
    #[serde(default)]
    pub home_next: Vec<NextMatchEntry>,
    /// A vendég csapat következő legfeljebb három meccse.
    #[serde(default)]
    pub away_next: Vec<NextMatchEntry>,
}

pub async fn fetch_match_detail(pool: &PgPool, match_id: Uuid) -> Result<Option<MatchDetailRow>> {
    let row = sqlx::query_as::<_, MatchDetailDbRow>(
        "SELECT
            m.id,
            m.kickoff_at,
            m.kickoff_precision,
            m.status,
            m.round_label,
            c.id AS competition_id,
            c.name AS competition_name,
            co.code AS country_code,
            co.name AS country_name,
            c.tier,
            c.priority,
            s.label AS season_label,
            home.name AS home_name,
            away.name AS away_name,
            hl.logo AS home_logo,
            al.logo AS away_logo,
            he.elo AS home_elo,
            ae.elo AS away_elo,
            r.home_ft, r.away_ft, r.home_ht, r.away_ht,
            r.home_et, r.away_et, r.home_pens, r.away_pens, r.decided_by,
            referee_entity.canonical_name AS referee,
            st.source_code AS stats_source,
            st.captured_at AS stats_captured_at,
            st.home_shots, st.away_shots,
            st.home_shots_on_target, st.away_shots_on_target,
            st.home_corners, st.away_corners,
            st.home_yellow, st.away_yellow,
            st.home_red, st.away_red,
            st.home_fouls, st.away_fouls,
            st.home_xg, st.away_xg,
            o.odds, o.market_count, o.display_name AS odds_source,
            o.captured_at AS odds_captured_at, o.trust_state AS odds_trust_state,
            g.state AS gate_state,
            g.blockers AS gate_blockers,
            g.evaluated_at AS gate_evaluated_at,
            g.rule_set_version AS gate_rule_set_version,
            g.publish_date
         FROM core.match m
         JOIN core.team home ON home.id = m.home_team_id
         JOIN core.team away ON away.id = m.away_team_id
         JOIN core.competition c ON c.id = m.competition_id
         JOIN core.season s ON s.id = m.season_id
         LEFT JOIN core.country co ON co.id = c.country_id
         LEFT JOIN core.match_result r ON r.match_id = m.id
         LEFT JOIN core.entity referee_entity ON referee_entity.id = m.referee_entity_id
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
             SELECT e.elo::double precision AS elo
             FROM fact.club_elo e
             WHERE e.club_entity_id = m.home_team_id
               AND e.as_of <= m.kickoff_at::date
             ORDER BY e.as_of DESC
             LIMIT 1
         ) he ON true
         LEFT JOIN LATERAL (
             SELECT e.elo::double precision AS elo
             FROM fact.club_elo e
             WHERE e.club_entity_id = m.away_team_id
               AND e.as_of <= m.kickoff_at::date
             ORDER BY e.as_of DESC
             LIMIT 1
         ) ae ON true
         LEFT JOIN LATERAL (
             SELECT src.code AS source_code,
                    ms.captured_at,
                    ms.home_shots, ms.away_shots,
                    ms.home_shots_on_target, ms.away_shots_on_target,
                    ms.home_corners, ms.away_corners,
                    ms.home_yellow, ms.away_yellow,
                    ms.home_red, ms.away_red,
                    ms.home_fouls, ms.away_fouls,
                    ms.home_xg::double precision AS home_xg,
                    ms.away_xg::double precision AS away_xg
             FROM fact.match_stats ms
             JOIN ops.source src ON src.id = ms.source_id
             WHERE ms.match_id = m.id
             ORDER BY ms.captured_at DESC
             LIMIT 1
         ) st ON true
         LEFT JOIN LATERAL (
             SELECT jsonb_build_object(
                        'home', COALESCE(max(q.price) FILTER (WHERE q.bookmaker='default' AND q.market='1x2' AND q.outcome='home'), max(q.price) FILTER (WHERE q.bookmaker='B365' AND q.market='1x2' AND q.outcome='home'), max(q.price) FILTER (WHERE q.bookmaker='Avg' AND q.market='1x2' AND q.outcome='home'), max(q.price) FILTER (WHERE q.bookmaker='Max' AND q.market='1x2' AND q.outcome='home'), max(q.price) FILTER (WHERE q.market='1x2' AND q.outcome='home')),
                        'draw', COALESCE(max(q.price) FILTER (WHERE q.bookmaker='default' AND q.market='1x2' AND q.outcome='draw'), max(q.price) FILTER (WHERE q.bookmaker='B365' AND q.market='1x2' AND q.outcome='draw'), max(q.price) FILTER (WHERE q.bookmaker='Avg' AND q.market='1x2' AND q.outcome='draw'), max(q.price) FILTER (WHERE q.bookmaker='Max' AND q.market='1x2' AND q.outcome='draw'), max(q.price) FILTER (WHERE q.market='1x2' AND q.outcome='draw')),
                        'away', COALESCE(max(q.price) FILTER (WHERE q.bookmaker='default' AND q.market='1x2' AND q.outcome='away'), max(q.price) FILTER (WHERE q.bookmaker='B365' AND q.market='1x2' AND q.outcome='away'), max(q.price) FILTER (WHERE q.bookmaker='Avg' AND q.market='1x2' AND q.outcome='away'), max(q.price) FILTER (WHERE q.bookmaker='Max' AND q.market='1x2' AND q.outcome='away'), max(q.price) FILTER (WHERE q.market='1x2' AND q.outcome='away')),
                        'btts_yes', COALESCE(max(q.price) FILTER (WHERE q.bookmaker='default' AND q.market='btts' AND q.outcome='yes'), max(q.price) FILTER (WHERE q.bookmaker='B365' AND q.market='btts' AND q.outcome='yes'), max(q.price) FILTER (WHERE q.bookmaker='Avg' AND q.market='btts' AND q.outcome='yes'), max(q.price) FILTER (WHERE q.bookmaker='Max' AND q.market='btts' AND q.outcome='yes'), max(q.price) FILTER (WHERE q.market='btts' AND q.outcome='yes')),
                        'btts_no', COALESCE(max(q.price) FILTER (WHERE q.bookmaker='default' AND q.market='btts' AND q.outcome='no'), max(q.price) FILTER (WHERE q.bookmaker='B365' AND q.market='btts' AND q.outcome='no'), max(q.price) FILTER (WHERE q.bookmaker='Avg' AND q.market='btts' AND q.outcome='no'), max(q.price) FILTER (WHERE q.bookmaker='Max' AND q.market='btts' AND q.outcome='no'), max(q.price) FILTER (WHERE q.market='btts' AND q.outcome='no')),
                        'over25', COALESCE(max(q.price) FILTER (WHERE q.bookmaker='default' AND q.market='ou' AND q.outcome='over' AND q.line=2.5), max(q.price) FILTER (WHERE q.bookmaker='B365' AND q.market='ou' AND q.outcome='over' AND q.line=2.5), max(q.price) FILTER (WHERE q.bookmaker='Avg' AND q.market='ou' AND q.outcome='over' AND q.line=2.5), max(q.price) FILTER (WHERE q.bookmaker='Max' AND q.market='ou' AND q.outcome='over' AND q.line=2.5), max(q.price) FILTER (WHERE q.market='ou' AND q.outcome='over' AND q.line=2.5)),
                        'under25', COALESCE(max(q.price) FILTER (WHERE q.bookmaker='default' AND q.market='ou' AND q.outcome='under' AND q.line=2.5), max(q.price) FILTER (WHERE q.bookmaker='B365' AND q.market='ou' AND q.outcome='under' AND q.line=2.5), max(q.price) FILTER (WHERE q.bookmaker='Avg' AND q.market='ou' AND q.outcome='under' AND q.line=2.5), max(q.price) FILTER (WHERE q.bookmaker='Max' AND q.market='ou' AND q.outcome='under' AND q.line=2.5), max(q.price) FILTER (WHERE q.market='ou' AND q.outcome='under' AND q.line=2.5))
                    ) AS odds,
                    os.market_count, src.display_name AS display_name,
                    os.captured_at, os.trust_state
             FROM fact.odds_snapshot os
             JOIN LATERAL (
                 SELECT DISTINCT ON (bookmaker, market, outcome, line) *
                 FROM fact.odds_quote
                 WHERE match_id = os.match_id AND source_id = os.source_id
                 ORDER BY bookmaker, market, outcome, line, (phase = 'closing') DESC
             ) q ON true
             JOIN ops.source src ON src.id = os.source_id
             WHERE os.match_id = m.id
             GROUP BY os.market_count, os.source_id, os.captured_at,
                      os.trust_state, src.display_name
             ORDER BY (os.trust_state = 'verified') DESC, os.source_id
             LIMIT 1
         ) o ON true
         LEFT JOIN LATERAL (
             SELECT state, blockers, evaluated_at, rule_set_version, publish_date
             FROM ops.publish_gate
             WHERE match_id = m.id
             ORDER BY publish_date DESC
             LIMIT 1
         ) g ON true
         WHERE m.id = $1",
    )
    .bind(match_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(Into::into))
}

#[derive(sqlx::FromRow)]
struct MatchDetailDbRow {
    id: Uuid,
    kickoff_at: DateTime<Utc>,
    kickoff_precision: String,
    status: String,
    round_label: Option<String>,
    competition_id: Uuid,
    competition_name: String,
    country_code: Option<String>,
    country_name: Option<String>,
    tier: i16,
    priority: i32,
    season_label: String,
    home_name: String,
    away_name: String,
    home_logo: Option<String>,
    away_logo: Option<String>,
    home_elo: Option<f64>,
    away_elo: Option<f64>,
    home_ft: Option<i16>,
    away_ft: Option<i16>,
    home_ht: Option<i16>,
    away_ht: Option<i16>,
    home_et: Option<i16>,
    away_et: Option<i16>,
    home_pens: Option<i16>,
    away_pens: Option<i16>,
    decided_by: Option<String>,
    referee: Option<String>,
    stats_source: Option<String>,
    stats_captured_at: Option<DateTime<Utc>>,
    home_shots: Option<i32>,
    away_shots: Option<i32>,
    home_shots_on_target: Option<i32>,
    away_shots_on_target: Option<i32>,
    home_corners: Option<i32>,
    away_corners: Option<i32>,
    home_yellow: Option<i32>,
    away_yellow: Option<i32>,
    home_red: Option<i32>,
    away_red: Option<i32>,
    home_fouls: Option<i32>,
    away_fouls: Option<i32>,
    home_xg: Option<f64>,
    away_xg: Option<f64>,
    odds: Option<serde_json::Value>,
    market_count: Option<i32>,
    odds_source: Option<String>,
    odds_captured_at: Option<DateTime<Utc>>,
    odds_trust_state: Option<String>,
    gate_state: Option<String>,
    gate_blockers: Option<serde_json::Value>,
    gate_evaluated_at: Option<DateTime<Utc>>,
    gate_rule_set_version: Option<String>,
    publish_date: Option<NaiveDate>,
}

impl From<MatchDetailDbRow> for MatchDetailRow {
    fn from(row: MatchDetailDbRow) -> Self {
        Self {
            id: row.id,
            kickoff_at: row.kickoff_at,
            kickoff_precision: row.kickoff_precision,
            status: row.status,
            round_label: row.round_label,
            competition_id: row.competition_id,
            competition_name: row.competition_name,
            country_code: row.country_code,
            country_name: row.country_name,
            tier: row.tier,
            priority: row.priority,
            season_label: row.season_label,
            home_name: row.home_name,
            away_name: row.away_name,
            home_logo: row.home_logo,
            away_logo: row.away_logo,
            home_elo: row.home_elo,
            away_elo: row.away_elo,
            home_ft: row.home_ft,
            away_ft: row.away_ft,
            home_ht: row.home_ht,
            away_ht: row.away_ht,
            home_et: row.home_et,
            away_et: row.away_et,
            home_pens: row.home_pens,
            away_pens: row.away_pens,
            decided_by: row.decided_by,
            referee: row.referee,
            stats_source: row.stats_source,
            stats_captured_at: row.stats_captured_at,
            home_shots: row.home_shots,
            away_shots: row.away_shots,
            home_shots_on_target: row.home_shots_on_target,
            away_shots_on_target: row.away_shots_on_target,
            home_corners: row.home_corners,
            away_corners: row.away_corners,
            home_yellow: row.home_yellow,
            away_yellow: row.away_yellow,
            home_red: row.home_red,
            away_red: row.away_red,
            home_fouls: row.home_fouls,
            away_fouls: row.away_fouls,
            home_xg: row.home_xg,
            away_xg: row.away_xg,
            odds: row.odds,
            market_count: row.market_count,
            odds_source: row.odds_source,
            odds_captured_at: row.odds_captured_at,
            odds_trust_state: row.odds_trust_state,
            gate_state: row.gate_state,
            gate_blockers: row.gate_blockers,
            gate_evaluated_at: row.gate_evaluated_at,
            gate_rule_set_version: row.gate_rule_set_version,
            publish_date: row.publish_date,
            home_form: Vec::new(),
            away_form: Vec::new(),
            head_to_head: Vec::new(),
            home_next: Vec::new(),
            away_next: Vec::new(),
        }
    }
}

/// Forma-mutató egy korábbi meccsről (a csapat szemszögéből).
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct FormEntry {
    pub match_id: Uuid,
    pub kickoff_at: DateTime<Utc>,
    pub competition_name: String,
    pub opponent_name: String,
    pub is_home: bool,
    pub team_goals: i16,
    pub opponent_goals: i16,
    /// "W" | "D" | "L"
    pub outcome: String,
}

/// Egy korábbi egymás elleni meccs.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct HeadToHeadEntry {
    pub match_id: Uuid,
    pub kickoff_at: DateTime<Utc>,
    pub competition_name: String,
    pub home_name: String,
    pub away_name: String,
    pub home_goals: Option<i16>,
    pub away_goals: Option<i16>,
}

/// A részletező nézet kiegészítései: a két csapat utolsó meccsei és az
/// egymás elleni találkozók. Minden lekérdezés a meccs kezdete előtti
/// állapotra szűr, hogy jövőbeli információ ne szivárogjon vissza.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct MatchExtras {
    pub home_form: Vec<FormEntry>,
    pub away_form: Vec<FormEntry>,
    pub head_to_head: Vec<HeadToHeadEntry>,
    /// A meccs után következő legfeljebb három meccs csapatonként.
    pub home_next: Vec<NextMatchEntry>,
    pub away_next: Vec<NextMatchEntry>,
}

#[derive(sqlx::FromRow)]
struct FormDbRow {
    match_id: Uuid,
    kickoff_at: DateTime<Utc>,
    competition_name: String,
    opponent_name: String,
    is_home: bool,
    team_goals: i16,
    opponent_goals: i16,
}

pub async fn fetch_team_next(
    pool: &PgPool,
    team_id: Uuid,
    after: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<NextMatchEntry>> {
    #[derive(sqlx::FromRow)]
    struct NextDbRow {
        match_id: Uuid,
        kickoff_at: DateTime<Utc>,
        competition_name: String,
        opponent_name: String,
        is_home: bool,
    }

    let rows = sqlx::query_as::<_, NextDbRow>(
        "SELECT m.id AS match_id,
                m.kickoff_at,
                c.name AS competition_name,
                CASE WHEN m.home_team_id = $1 THEN away.name ELSE home.name END AS opponent_name,
                (m.home_team_id = $1) AS is_home
         FROM core.match m
         JOIN core.team home ON home.id = m.home_team_id
         JOIN core.team away ON away.id = m.away_team_id
         JOIN core.competition c ON c.id = m.competition_id
         WHERE (m.home_team_id = $1 OR m.away_team_id = $1)
           AND m.kickoff_at > $2
           AND m.status IN ('scheduled', 'confirmed')
         ORDER BY m.kickoff_at ASC
         LIMIT $3",
    )
    .bind(team_id)
    .bind(after)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| NextMatchEntry {
            match_id: row.match_id,
            kickoff_at: row.kickoff_at,
            competition_name: row.competition_name,
            opponent_name: row.opponent_name,
            is_home: row.is_home,
        })
        .collect())
}

pub async fn fetch_team_form(
    pool: &PgPool,
    team_id: Uuid,
    before: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<FormEntry>> {
    let rows = sqlx::query_as::<_, FormDbRow>(
        "SELECT m.id AS match_id,
                m.kickoff_at,
                c.name AS competition_name,
                CASE WHEN m.home_team_id = $1 THEN away.name ELSE home.name END AS opponent_name,
                (m.home_team_id = $1) AS is_home,
                CASE WHEN m.home_team_id = $1 THEN r.home_ft ELSE r.away_ft END AS team_goals,
                CASE WHEN m.home_team_id = $1 THEN r.away_ft ELSE r.home_ft END AS opponent_goals
         FROM core.match m
         JOIN core.match_result r ON r.match_id = m.id
         JOIN core.team home ON home.id = m.home_team_id
         JOIN core.team away ON away.id = m.away_team_id
         JOIN core.competition c ON c.id = m.competition_id
         WHERE (m.home_team_id = $1 OR m.away_team_id = $1)
           AND m.kickoff_at < $2
           AND m.status = 'finished'
         ORDER BY m.kickoff_at DESC
         LIMIT $3",
    )
    .bind(team_id)
    .bind(before)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let outcome = if row.team_goals > row.opponent_goals {
                "W"
            } else if row.team_goals == row.opponent_goals {
                "D"
            } else {
                "L"
            };
            FormEntry {
                match_id: row.match_id,
                kickoff_at: row.kickoff_at,
                competition_name: row.competition_name,
                opponent_name: row.opponent_name,
                is_home: row.is_home,
                team_goals: row.team_goals,
                opponent_goals: row.opponent_goals,
                outcome: outcome.to_string(),
            }
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct HeadToHeadDbRow {
    match_id: Uuid,
    kickoff_at: DateTime<Utc>,
    competition_name: String,
    home_name: String,
    away_name: String,
    home_goals: Option<i16>,
    away_goals: Option<i16>,
}

pub async fn fetch_head_to_head(
    pool: &PgPool,
    home_team_id: Uuid,
    away_team_id: Uuid,
    before: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<HeadToHeadEntry>> {
    let rows = sqlx::query_as::<_, HeadToHeadDbRow>(
        "SELECT m.id AS match_id,
                m.kickoff_at,
                c.name AS competition_name,
                home.name AS home_name,
                away.name AS away_name,
                r.home_ft AS home_goals,
                r.away_ft AS away_goals
         FROM core.match m
         JOIN core.team home ON home.id = m.home_team_id
         JOIN core.team away ON away.id = m.away_team_id
         JOIN core.competition c ON c.id = m.competition_id
         LEFT JOIN core.match_result r ON r.match_id = m.id
         WHERE ((m.home_team_id = $1 AND m.away_team_id = $2)
             OR (m.home_team_id = $2 AND m.away_team_id = $1))
           AND m.kickoff_at < $3
           AND m.status = 'finished'
         ORDER BY m.kickoff_at DESC
         LIMIT $4",
    )
    .bind(home_team_id)
    .bind(away_team_id)
    .bind(before)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| HeadToHeadEntry {
            match_id: row.match_id,
            kickoff_at: row.kickoff_at,
            competition_name: row.competition_name,
            home_name: row.home_name,
            away_name: row.away_name,
            home_goals: row.home_goals,
            away_goals: row.away_goals,
        })
        .collect())
}

pub async fn fetch_match_extras(
    pool: &PgPool,
    match_id: Uuid,
    before: DateTime<Utc>,
) -> Result<Option<MatchExtras>> {
    let Some((home_team_id, away_team_id)) = sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT home_team_id, away_team_id FROM core.match WHERE id = $1",
    )
    .bind(match_id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };
    let home_form = fetch_team_form(pool, home_team_id, before, 5).await?;
    let away_form = fetch_team_form(pool, away_team_id, before, 5).await?;
    let head_to_head = fetch_head_to_head(pool, home_team_id, away_team_id, before, 5).await?;
    let home_next = fetch_team_next(pool, home_team_id, before, 3).await?;
    let away_next = fetch_team_next(pool, away_team_id, before, 3).await?;
    Ok(Some(MatchExtras {
        home_form,
        away_form,
        head_to_head,
        home_next,
        away_next,
    }))
}

/// Egy soron következő (még le nem játszott) meccs a csapat szemszögéből.
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct NextMatchEntry {
    pub match_id: Uuid,
    pub kickoff_at: DateTime<Utc>,
    pub competition_name: String,
    pub opponent_name: String,
    pub is_home: bool,
}
