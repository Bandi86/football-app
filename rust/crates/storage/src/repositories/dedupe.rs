//! Duplikált kanonikus meccsek merge-eszköze (S16).
//!
//! Ugyanaz a valós meccs több kanonikus sorként (forrás/név-változat miatt)
//! egyszerre létezhet. A merge: az áldozat provenance-sorait átmozgatja a
//! túlélőhöz (selected-ütközésnél jelöletlenre állítva), pótolja a hiányzó
//! eredményt/oddsot/statisztikát, a törölt sort **archiválja** (JSONB), és
//! audit-eseményt ír. A hívó CLI dry-run módban csak a tervet kapja.

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct DuplicateMatchRow {
    pub match_id: Uuid,
    pub competition_name: String,
    pub home_team_id: Uuid,
    pub away_team_id: Uuid,
    pub home_name: String,
    pub away_name: String,
    pub kickoff_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub provenance_count: i64,
    pub has_result: bool,
    pub has_odds: bool,
    pub gate_state: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DuplicateGroup {
    pub key: String,
    pub matches: Vec<DuplicateMatchRow>,
}

/// A nap duplikált csoportjai: azonos hazai/vendég/kickoff, több sor.
pub async fn duplicate_groups(
    pool: &PgPool,
    date: chrono::NaiveDate,
) -> Result<Vec<DuplicateGroup>> {
    duplicate_groups_between(pool, date, date).await
}

/// Duplikált csoportok egy dátumtartományban (backfill-takarításhoz).
pub async fn duplicate_groups_between(
    pool: &PgPool,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
) -> Result<Vec<DuplicateGroup>> {
    let rows = sqlx::query_as::<_, DuplicateMatchRow>(
        "SELECT
            m.id AS match_id,
            c.name AS competition_name,
            m.home_team_id,
            m.away_team_id,
            h.name AS home_name,
            a.name AS away_name,
            m.kickoff_at,
            m.created_at,
            (SELECT count(*) FROM fact.provenance p
              WHERE p.subject_kind = 'match' AND p.subject_id = m.id)::bigint AS provenance_count,
            EXISTS (SELECT 1 FROM core.match_result r WHERE r.match_id = m.id) AS has_result,
            EXISTS (SELECT 1 FROM fact.odds_snapshot o WHERE o.match_id = m.id) AS has_odds,
            g.state AS gate_state
         FROM core.match m
         JOIN core.team h ON h.id = m.home_team_id
         JOIN core.team a ON a.id = m.away_team_id
         JOIN core.competition c ON c.id = m.competition_id
         LEFT JOIN LATERAL (
             SELECT state FROM ops.publish_gate
             WHERE match_id = m.id ORDER BY publish_date DESC LIMIT 1
         ) g ON true
         WHERE m.kickoff_at::date BETWEEN $1 AND $2
           AND EXISTS (
               SELECT 1 FROM core.match m2
               WHERE m2.id <> m.id
                 AND m2.kickoff_at = m.kickoff_at
                 AND LEAST(m2.home_team_id, m2.away_team_id)
                     = LEAST(m.home_team_id, m.away_team_id)
                 AND GREATEST(m2.home_team_id, m2.away_team_id)
                     = GREATEST(m.home_team_id, m.away_team_id)
           )
         ORDER BY m.kickoff_at, h.name, a.name, m.created_at",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;

    // Kulcs szerinti csoportosítás (a tükrözött sorok nem feltétlenül
    // szomszédosak a név-rendezés miatt — nem lehet last_mut-ra építeni).
    let mut order: Vec<String> = Vec::new();
    let mut by_key: std::collections::HashMap<String, Vec<DuplicateMatchRow>> =
        std::collections::HashMap::new();
    for row in rows {
        let (lo, hi) = if row.home_team_id < row.away_team_id {
            (row.home_team_id, row.away_team_id)
        } else {
            (row.away_team_id, row.home_team_id)
        };
        let key = format!("{}|{}|{}", row.kickoff_at, lo, hi);
        if !by_key.contains_key(&key) {
            order.push(key.clone());
        }
        by_key.entry(key).or_default().push(row);
    }
    let groups = order
        .into_iter()
        .map(|key| DuplicateGroup {
            matches: by_key.remove(&key).unwrap_or_default(),
            key,
        })
        .collect();
    Ok(groups)
}

/// Mergelehető-e a pár (a túlélő és az áldozat)? Ugyanaz a nap/meccs.
pub async fn is_same_match(pool: &PgPool, survivor: Uuid, duplicate: Uuid) -> Result<bool> {
    let same: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM core.match s JOIN core.match d
              ON d.id = $2
             WHERE s.id = $1
               AND s.kickoff_at = d.kickoff_at
               AND (
                 (s.home_team_id = d.home_team_id AND s.away_team_id = d.away_team_id)
                 OR (
                   s.home_team_id = d.away_team_id
                   AND s.away_team_id = d.home_team_id
                   AND (
                     EXISTS (SELECT 1 FROM core.match_result r WHERE r.match_id = s.id)
                     OR NOT EXISTS (SELECT 1 FROM core.match_result r WHERE r.match_id = d.id)
                   )
                 )
               )
         )",
    )
    .bind(survivor)
    .bind(duplicate)
    .fetch_one(pool)
    .await?;
    Ok(same)
}

/// Az áldozat teljes sorainak archiválása (`ops.dedupe_archive`) majd a
/// merge + törlés egy tranzakcióban. Visszatérés: audit-id.
pub async fn merge_duplicate(
    pool: &PgPool,
    survivor: Uuid,
    duplicate: Uuid,
    note: &str,
) -> Result<Uuid> {
    let mut tx = pool.begin().await?;

    // 0. Tükrözött pár: az eredmény/orientáció-érzékeny blokkok nem másolhatók.
    //    Elutasítás csak akkor, ha a túlélőnél nincs eredmény, az áldozatnál
    //    viszont van (különben az eredmény elveszne).
    let (mirrored, survivor_result, duplicate_result): (bool, bool, bool) = sqlx::query_as(
        "SELECT (s.home_team_id = d.away_team_id AND s.away_team_id = d.home_team_id),
                EXISTS (SELECT 1 FROM core.match_result r WHERE r.match_id = s.id),
                EXISTS (SELECT 1 FROM core.match_result r WHERE r.match_id = d.id)
           FROM core.match s, core.match d
          WHERE s.id = $1 AND d.id = $2",
    )
    .bind(survivor)
    .bind(duplicate)
    .fetch_one(&mut *tx)
    .await?;
    if mirrored && duplicate_result && !survivor_result {
        anyhow::bail!("tükrözött pár: az áldozat eredménye elveszne (kézi döntés kell)");
    }

    // 1. Archívum (visszaállítási út): a törlendő sor és gyermekei JSONB-ben.
    sqlx::query(
        "INSERT INTO ops.dedupe_archive (id, merged_at, survivor_id, duplicate_id, note, payload)
         SELECT $1, now(), $2, $3, $4, jsonb_build_object(
            'match', (SELECT to_jsonb(m) FROM core.match m WHERE m.id = $3),
            'gate', (SELECT COALESCE(jsonb_agg(to_jsonb(g)), '[]'::jsonb)
                       FROM ops.publish_gate g WHERE g.match_id = $3),
            'result', (SELECT to_jsonb(r) FROM core.match_result r WHERE r.match_id = $3),
            'provenance', (SELECT COALESCE(jsonb_agg(to_jsonb(p)), '[]'::jsonb)
                             FROM fact.provenance p
                            WHERE p.subject_kind='match' AND p.subject_id=$3),
            'odds_snapshot', (SELECT COALESCE(jsonb_agg(to_jsonb(o)), '[]'::jsonb)
                                FROM fact.odds_snapshot o WHERE o.match_id=$3),
            'odds_quote', (SELECT COALESCE(jsonb_agg(to_jsonb(q)), '[]'::jsonb)
                             FROM fact.odds_quote q WHERE q.match_id=$3),
            'stats', (SELECT COALESCE(jsonb_agg(to_jsonb(s)), '[]'::jsonb)
                        FROM fact.match_stats s WHERE s.match_id=$3)
         )",
    )
    .bind(Uuid::now_v7())
    .bind(survivor)
    .bind(duplicate)
    .bind(note)
    .execute(&mut *tx)
    .await?;

    // 2. Provenance átmozgatás: selected csak akkor, ha a túlélőnél az adott
    //    mezőn nincs még selected sor.
    sqlx::query(
        "INSERT INTO fact.provenance
           (subject_kind, subject_id, field_key, observation_id, is_selected,
            policy_version, valid_from, valid_to)
         SELECT p.subject_kind, $2, p.field_key, p.observation_id,
                p.is_selected AND NOT EXISTS (
                    SELECT 1 FROM fact.provenance s
                     WHERE s.subject_kind = p.subject_kind AND s.subject_id = $2
                       AND s.field_key = p.field_key AND s.is_selected),
                p.policy_version, p.valid_from, p.valid_to
           FROM fact.provenance p
          WHERE p.subject_kind = 'match' AND p.subject_id = $1
         ON CONFLICT (subject_kind, subject_id, field_key, observation_id) DO NOTHING",
    )
    .bind(duplicate)
    .bind(survivor)
    .execute(&mut *tx)
    .await?;

    // 3. Hiányzó eredmény pótlása a túlélőnél (tükrözött párnál nincs).
    if !mirrored {
        sqlx::query(
            "INSERT INTO core.match_result
           (match_id, home_ft, away_ft, home_ht, away_ht, home_et, away_et,
            home_pens, away_pens, decided_by)
         SELECT $2, r.home_ft, r.away_ft, r.home_ht, r.away_ht, r.home_et,
                r.away_et, r.home_pens, r.away_pens, r.decided_by
           FROM core.match_result r
          WHERE r.match_id = $1
            AND NOT EXISTS (SELECT 1 FROM core.match_result s WHERE s.match_id = $2)
         ON CONFLICT (match_id) DO NOTHING",
        )
        .bind(duplicate)
        .bind(survivor)
        .execute(&mut *tx)
        .await?;
    }

    // 4. Odds-blokkok átmozgatása forrásonként (additívan): a túlélő meglévő
    //    forrás-snapshotjai maradnak, az áldozat többi forrása hozzáadódik —
    //    így a merge után két független forrás is jelen lehet (trust).
    if !mirrored {
        sqlx::query(
            "INSERT INTO fact.odds_snapshot (match_id, source_id, captured_at, market_count, trust_state)
             SELECT $2, o.source_id, o.captured_at, o.market_count, o.trust_state
               FROM fact.odds_snapshot o
              WHERE o.match_id = $1
             ON CONFLICT (match_id, source_id) DO NOTHING",
        )
        .bind(duplicate)
        .bind(survivor)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO fact.odds_quote
               (match_id, source_id, bookmaker, market, outcome, line, phase, price)
             SELECT $2, q.source_id, q.bookmaker, q.market, q.outcome, q.line, q.phase, q.price
               FROM fact.odds_quote q
              WHERE q.match_id = $1
             ON CONFLICT DO NOTHING",
        )
        .bind(duplicate)
        .bind(survivor)
        .execute(&mut *tx)
        .await?;

        // 5. Hiányzó statisztika pótlása.
        sqlx::query(
            "INSERT INTO fact.match_stats
           (match_id, source_id, captured_at, home_shots, away_shots,
            home_shots_on_target, away_shots_on_target, home_corners, away_corners,
            home_yellow, away_yellow, home_red, away_red, home_fouls, away_fouls,
            home_xg, away_xg)
         SELECT $2, s.source_id, s.captured_at, s.home_shots, s.away_shots,
                s.home_shots_on_target, s.away_shots_on_target, s.home_corners,
                s.away_corners, s.home_yellow, s.away_yellow, s.home_red,
                s.away_red, s.home_fouls, s.away_fouls, s.home_xg, s.away_xg
           FROM fact.match_stats s
          WHERE s.match_id = $1
            AND NOT EXISTS (SELECT 1 FROM fact.match_stats t WHERE t.match_id = $2)
         ON CONFLICT (match_id, source_id) DO NOTHING",
        )
        .bind(duplicate)
        .bind(survivor)
        .execute(&mut *tx)
        .await?;
    }

    // 6. Az áldozat gyermeksorai: a publish_gate NEM kaszkádol, ezért
    //    explicit törlés (archívumban megvan); a maradék provenance is megy.
    sqlx::query("DELETE FROM ops.publish_gate WHERE match_id = $1")
        .bind(duplicate)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM fact.provenance WHERE subject_kind = 'match' AND subject_id = $1")
        .bind(duplicate)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM core.match WHERE id = $1")
        .bind(duplicate)
        .execute(&mut *tx)
        .await?;

    // 7. Audit.
    let audit_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ops.audit_event (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1, 'cli', 'match_dedupe_merge', 'match', $2, $3)",
    )
    .bind(audit_id)
    .bind(survivor)
    .bind(serde_json::json!({ "duplicate_id": duplicate, "note": note }))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(audit_id)
}

// ---------------------------------------------------------------------------
// S26 — Fixture-szintű rekonciliáció: azonos verseny + szezon + csapat-pár +
// UTC-nap, de eltérő kickoff (a v1 import és a footballdata backfill párhuzamos
// sorai). A kulcs szándékosan nem tartalmaz kickoffot; az auto-merge őr az
// orientáció-helyes eredmény-egyezés (vagy az egyik eredmény hiánya).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct FixtureMatchRow {
    pub match_id: Uuid,
    pub competition_id: Uuid,
    pub competition_name: String,
    pub season_id: Uuid,
    pub season_label: String,
    pub home_team_id: Uuid,
    pub away_team_id: Uuid,
    pub home_name: String,
    pub away_name: String,
    pub kickoff_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub provenance_count: i64,
    pub has_result: bool,
    pub has_odds: bool,
    pub gate_state: Option<String>,
    /// A provenance-láncon keresztüli forrás-kódok (vesszővel), a túlélő-
    /// prioritáshoz (footballdata > v1-import).
    pub source_codes: String,
    pub home_ft: Option<i16>,
    pub away_ft: Option<i16>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FixtureGroup {
    pub key: String,
    pub matches: Vec<FixtureMatchRow>,
}

/// Fixture-szintű duplikált csoportok (competition + season + csapat-pár +
/// UTC-nap). Opcionális dátumtartomány és verseny-név szűrő.
pub async fn duplicate_fixture_groups(
    pool: &PgPool,
    from: Option<chrono::NaiveDate>,
    to: Option<chrono::NaiveDate>,
    competition: Option<&str>,
) -> Result<Vec<FixtureGroup>> {
    let rows = sqlx::query_as::<_, FixtureMatchRow>(
        "WITH keyed AS (
            SELECT m.id, m.competition_id, m.season_id, m.home_team_id, m.away_team_id,
                   m.kickoff_at, m.created_at,
                   LEAST(m.home_team_id, m.away_team_id) AS lo,
                   GREATEST(m.home_team_id, m.away_team_id) AS hi,
                   m.kickoff_at::date AS kd
              FROM core.match m
        ),
        grp AS (
            SELECT competition_id, season_id, lo, hi, kd
              FROM keyed
             GROUP BY 1, 2, 3, 4, 5
            HAVING count(*) > 1
        )
        SELECT k.id AS match_id, k.competition_id, c.name AS competition_name,
               k.season_id, s.label AS season_label,
               k.home_team_id, k.away_team_id, h.name AS home_name, a.name AS away_name,
               k.kickoff_at, k.created_at,
               (SELECT count(*) FROM fact.provenance p
                 WHERE p.subject_kind = 'match' AND p.subject_id = k.id)::bigint AS provenance_count,
               EXISTS (SELECT 1 FROM core.match_result r WHERE r.match_id = k.id) AS has_result,
               EXISTS (SELECT 1 FROM fact.odds_snapshot o WHERE o.match_id = k.id) AS has_odds,
               g.state AS gate_state,
               COALESCE((SELECT string_agg(DISTINCT src.code, ',' ORDER BY src.code)
                           FROM fact.provenance p
                           JOIN stage.observation o ON o.id = p.observation_id
                           JOIN ops.source src ON src.id = o.source_id
                          WHERE p.subject_kind = 'match' AND p.subject_id = k.id), '') AS source_codes,
               r.home_ft, r.away_ft
          FROM keyed k
          JOIN grp ON grp.competition_id = k.competition_id AND grp.season_id = k.season_id
                  AND grp.lo = k.lo AND grp.hi = k.hi AND grp.kd = k.kd
          JOIN core.competition c ON c.id = k.competition_id
          JOIN core.season s ON s.id = k.season_id
          JOIN core.team h ON h.id = k.home_team_id
          JOIN core.team a ON a.id = k.away_team_id
          LEFT JOIN core.match_result r ON r.match_id = k.id
          LEFT JOIN LATERAL (
              SELECT state FROM ops.publish_gate
               WHERE match_id = k.id ORDER BY publish_date DESC LIMIT 1
          ) g ON true
         WHERE ($1::date IS NULL OR k.kd >= $1)
           AND ($2::date IS NULL OR k.kd <= $2)
           AND ($3::text IS NULL OR c.name = $3)
         ORDER BY k.kd, c.name, s.label, h.name, a.name, k.created_at",
    )
    .bind(from)
    .bind(to)
    .bind(competition)
    .fetch_all(pool)
    .await?;

    let mut order: Vec<String> = Vec::new();
    let mut by_key: std::collections::HashMap<String, Vec<FixtureMatchRow>> =
        std::collections::HashMap::new();
    for row in rows {
        let (lo, hi) = if row.home_team_id < row.away_team_id {
            (row.home_team_id, row.away_team_id)
        } else {
            (row.away_team_id, row.home_team_id)
        };
        let key = format!(
            "{}|{}|{}|{}|{}",
            row.competition_id,
            row.season_id,
            lo,
            hi,
            row.kickoff_at.date_naive()
        );
        if !by_key.contains_key(&key) {
            order.push(key.clone());
        }
        by_key.entry(key).or_default().push(row);
    }
    let groups = order
        .into_iter()
        .map(|key| FixtureGroup {
            matches: by_key.remove(&key).unwrap_or_default(),
            key,
        })
        .collect();
    Ok(groups)
}

/// Auto-merge őr fixture-párra: azonos verseny/szezon/csapat-pár/UTC-nap,
/// AZONOS orientáció (tükrözött pár → false, kézi döntés), és az eredmények
/// egyeznek vagy az egyik hiányzik. Eltérő eredmény → false (fail-closed).
pub async fn is_same_fixture(pool: &PgPool, survivor: Uuid, duplicate: Uuid) -> Result<bool> {
    let same: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
              FROM core.match s
              JOIN core.match d ON d.id = $2
              LEFT JOIN core.match_result rs ON rs.match_id = s.id
              LEFT JOIN core.match_result rd ON rd.match_id = d.id
             WHERE s.id = $1
               AND s.competition_id = d.competition_id
               AND s.season_id = d.season_id
               AND s.kickoff_at::date = d.kickoff_at::date
               AND s.home_team_id = d.home_team_id
               AND s.away_team_id = d.away_team_id
               AND (rs.match_id IS NULL OR rd.match_id IS NULL
                    OR (rs.home_ft IS NOT DISTINCT FROM rd.home_ft
                        AND rs.away_ft IS NOT DISTINCT FROM rd.away_ft))
        )",
    )
    .bind(survivor)
    .bind(duplicate)
    .fetch_one(pool)
    .await?;
    Ok(same)
}

/// Felülvizsgált kockázatos párok azonossága: verseny+szezon+csapat-pár+UTC-nap,
/// eredmény-őr nélkül. Csak akkor használandó, ha a túlélő eredményét külső
/// bizonyíték igazolta (a hívó `--review-note` indoklást ír az archívumba).
pub async fn same_fixture_identity(pool: &PgPool, survivor: Uuid, duplicate: Uuid) -> Result<bool> {
    let same: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM core.match s JOIN core.match d ON d.id=$2
             WHERE s.id=$1
               AND s.competition_id=d.competition_id
               AND s.season_id=d.season_id
               AND s.kickoff_at::date=d.kickoff_at::date
               AND s.home_team_id=d.home_team_id
               AND s.away_team_id=d.away_team_id
        )",
    )
    .bind(survivor)
    .bind(duplicate)
    .fetch_one(pool)
    .await?;
    Ok(same)
}

/// Dátumon átnyúló, felülvizsgált fixture-pár azonossága: verseny+szezon+
/// csapat-pár, UTC-nap nélkül. Kizárólag akkor használandó, ha a két időpont
/// 24 órán belül van és a review bizonyítja a halasztást (a CLI a jegyzetet
/// az archívumba írja).
pub async fn same_fixture_identity_any_day(
    pool: &PgPool,
    survivor: Uuid,
    duplicate: Uuid,
) -> Result<bool> {
    let same: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM core.match s JOIN core.match d ON d.id=$2
             WHERE s.id=$1
               AND s.competition_id=d.competition_id
               AND s.season_id=d.season_id
               AND s.home_team_id=d.home_team_id
               AND s.away_team_id=d.away_team_id
               AND abs(extract(epoch FROM (s.kickoff_at - d.kickoff_at))) < 86400
        )",
    )
    .bind(survivor)
    .bind(duplicate)
    .fetch_one(pool)
    .await?;
    Ok(same)
}
