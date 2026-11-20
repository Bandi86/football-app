//! Duplikált kanonikus meccsek merge-eszköze (S16).
//!
//! Ugyanaz a valós meccs több kanonikus sorként (forrás/név-változat miatt)
//! egyszerre létezhet. A merge: az áldozat provenance-sorait átmozgatja a
//! túlélőhöz (selected-ütközésnél jelöletlenre állítva), pótolja a hiányzó
//! eredményt/oddsot/statisztikát, a törölt sort **archiválja** (JSONB), és
//! audit-eseményt ír. A hívó CLI dry-run módban csak a tervet kapja.

use anyhow::Result;
use chrono::{DateTime, Utc};
use football_domain::identity::{
    canonical_country_code, names_likely_same, names_strictly_same, normalize_name,
};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct DuplicateMatchRow {
    pub match_id: Uuid,
    pub competition_id: Uuid,
    pub competition_name: String,
    /// S41: a verseny ország-kódja (a cross-competition variáns-egyezéshez).
    pub competition_country_code: Option<String>,
    /// A verseny prioritása (a CLI túlélő-választásához; kisebb a jobb).
    pub competition_priority: i32,
    pub home_team_id: Uuid,
    pub away_team_id: Uuid,
    pub home_name: String,
    pub away_name: String,
    /// S41: csapat-kontextus a cross-competition egyezéshez (`core.team`).
    pub home_gender: String,
    pub away_gender: String,
    pub home_age_group: String,
    pub away_age_group: String,
    pub home_branch: String,
    pub away_branch: String,
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
    let rows = match_rows(pool, from, to, true).await?;

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

/// A duplikátum-keresés közös SELECT-je; a hívók a WHERE/ORDER BY-t fűzik hozzá.
const MATCH_ROW_SELECT: &str = "SELECT
            m.id AS match_id,
            m.competition_id,
            c.name AS competition_name,
            cc.code AS competition_country_code,
            c.priority AS competition_priority,
            m.home_team_id,
            m.away_team_id,
            h.name AS home_name,
            a.name AS away_name,
            h.gender AS home_gender,
            a.gender AS away_gender,
            h.age_group AS home_age_group,
            a.age_group AS away_age_group,
            h.branch AS home_branch,
            a.branch AS away_branch,
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
         LEFT JOIN core.country cc ON cc.id = c.country_id
         LEFT JOIN LATERAL (
             SELECT state FROM ops.publish_gate
             WHERE match_id = m.id ORDER BY publish_date DESC LIMIT 1
         ) g ON true";

/// A nap meccssorai (S37c: a név-variáns kereséshez is ez adja az alapot).
///
/// `only_exact` = true: csak az azonos csapat-pár + kickoff párok (a klasszikus
/// duplikátumlista); false: a nap összes sora (a variáns-keresés ehhez szűr).
async fn match_rows(
    pool: &PgPool,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
    only_exact: bool,
) -> Result<Vec<DuplicateMatchRow>> {
    let sql = format!(
        "{MATCH_ROW_SELECT}
         WHERE m.kickoff_at::date BETWEEN $1 AND $2
           AND (
             NOT $3
             OR EXISTS (
                 SELECT 1 FROM core.match m2
                 WHERE m2.id <> m.id
                   AND m2.kickoff_at = m.kickoff_at
                   AND LEAST(m2.home_team_id, m2.away_team_id)
                       = LEAST(m.home_team_id, m.away_team_id)
                   AND GREATEST(m2.home_team_id, m2.away_team_id)
                       = GREATEST(m.home_team_id, m.away_team_id)
             )
           )
         ORDER BY m.kickoff_at, h.name, a.name, m.created_at"
    );
    let rows = sqlx::query_as::<_, DuplicateMatchRow>(&sql)
        .bind(from)
        .bind(to)
        .bind(only_exact)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// S37c: a nap név-variáns duplikátumai. Ugyanaz a verseny és legfeljebb 12
/// órán belüli kickoff, de a csapatnevek forrásonként eltérnek ("SV Darmstadt
/// 98" / "Darmstadt"), ezért a szigorú csapat-azonosságra épülő
/// `duplicate_groups` nem látja őket.
pub async fn variant_duplicate_groups(
    pool: &PgPool,
    date: chrono::NaiveDate,
) -> Result<Vec<DuplicateGroup>> {
    variant_duplicate_groups_between(pool, date, date, false).await
}

/// S41: `allow_cross_competition` esetén a különböző verseny-entitások alatti
/// sorok is párosíthatók, de csak egyező normalizált verseny-országgal és
/// egyező csapat-kontextussal (gender/age_group/branch).
pub async fn variant_duplicate_groups_between(
    pool: &PgPool,
    from: chrono::NaiveDate,
    to: chrono::NaiveDate,
    allow_cross_competition: bool,
) -> Result<Vec<DuplicateGroup>> {
    let rows = match_rows(pool, from, to, false).await?;

    // Összefüggő komponensek (union-find a páronkénti jelölt-élekre).
    let mut parent: Vec<usize> = (0..rows.len()).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut edges = 0usize;
    for (i, a) in rows.iter().enumerate() {
        for (j, b) in rows.iter().enumerate().skip(i + 1) {
            if variant_pair(a, b, allow_cross_competition) {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                if ri != rj {
                    parent[ri] = rj;
                }
                edges += 1;
            }
        }
    }
    if edges == 0 {
        return Ok(Vec::new());
    }

    let mut order: Vec<usize> = Vec::new();
    let mut by_root: std::collections::HashMap<usize, Vec<DuplicateMatchRow>> =
        std::collections::HashMap::new();
    for (i, row) in rows.iter().enumerate() {
        let root = find(&mut parent, i);
        if !by_root.contains_key(&root) {
            order.push(root);
        }
        by_root.entry(root).or_default().push(row.clone());
    }
    Ok(order
        .into_iter()
        .filter_map(|root| by_root.remove(&root))
        .filter(|matches| matches.len() > 1)
        .map(|matches| {
            let first = &matches[0];
            DuplicateGroup {
                key: format!(
                    "variant|{}|{}|{}",
                    first.competition_name,
                    first.home_name,
                    first.kickoff_at.date_naive()
                ),
                matches,
            }
        })
        .collect())
}

/// Két sor név-variáns párja-e (<=12 óra, név-egyezés)?
///
/// Azonos versenyen belül a régi viselkedés. Különböző verseny csak
/// `allow_cross_competition` esetén, és akkor is csak egyező normalizált
/// verseny-országgal és egyező csapat-kontextussal. Az azonos csapat-azonosítójú
/// párokat szándékosan kihagyja: azok a klasszikus `duplicate_groups` hatáskörébe
/// tartoznak.
fn variant_pair(
    a: &DuplicateMatchRow,
    b: &DuplicateMatchRow,
    allow_cross_competition: bool,
) -> bool {
    if a.match_id == b.match_id || !competitions_compatible(a, b, allow_cross_competition) {
        return false;
    }
    let delta = (a.kickoff_at - b.kickoff_at).num_minutes().abs();
    if delta > 12 * 60 {
        return false;
    }
    let same_teams = (a.home_team_id == b.home_team_id && a.away_team_id == b.away_team_id)
        || (a.home_team_id == b.away_team_id && a.away_team_id == b.home_team_id);
    if same_teams {
        // Ugyanaz a két csapat 12 órán belül: azonos versenyen belül ezt a
        // pontos duplikátum-út kezeli; különböző verseny-entitás alatt vi-
        // szont ez a legerősebb bizonyíték ugyanarra a fixture-re (pl.
        // „Primera RFEF - 2. csoport" / „Primera Federacion - Group 2").
        return allow_cross_competition && a.competition_id != b.competition_id;
    }
    let normal = names_likely_same(&a.home_name, &b.home_name)
        && names_likely_same(&a.away_name, &b.away_name);
    let flipped = names_likely_same(&a.home_name, &b.away_name)
        && names_likely_same(&a.away_name, &b.home_name);
    normal || flipped
}

/// A két sor verseny-szinten összevonható-e? Azonos verseny mindig; különböző
/// csak engedélyezett cross-competition módban, egyező normalizált országgal.
fn competitions_compatible(
    a: &DuplicateMatchRow,
    b: &DuplicateMatchRow,
    allow_cross_competition: bool,
) -> bool {
    if a.competition_id == b.competition_id {
        return true;
    }
    allow_cross_competition && cross_competition_compatible(a, b)
}

/// Cross-competition kompatibilitás: a normalizált verseny-ország egyezik, és a
/// csapat-kontextus (gender/age_group/branch, home–home és away–away) egyezik.
fn cross_competition_compatible(a: &DuplicateMatchRow, b: &DuplicateMatchRow) -> bool {
    competition_country_key(&a.competition_country_code)
        == competition_country_key(&b.competition_country_code)
        && a.home_gender == b.home_gender
        && a.away_gender == b.away_gender
        && a.home_age_group == b.home_age_group
        && a.away_age_group == b.away_age_group
        && a.home_branch == b.home_branch
        && a.away_branch == b.away_branch
        // A v1-importban a női csapatok `gender` oszlopa is `male` lehet
        // (pl. „Marseille W"), ezért a név nem-jelölője is számít: férfi és
        // női meccs soha nem olvadhat össze.
        && name_marks_womens(&a.home_name) == name_marks_womens(&b.home_name)
        && name_marks_womens(&a.away_name) == name_marks_womens(&b.away_name)
}

/// A név jelöli-e, hogy női csapat (`W`, `Women`, `női`, `femenine`).
fn name_marks_womens(name: &str) -> bool {
    normalize_name(name).split(' ').any(|token| {
        matches!(
            token,
            "w" | "women" | "woman" | "női" | "noi" | "femenine" | "feminine"
        )
    })
}

fn competition_country_key(code: &Option<String>) -> String {
    canonical_country_code(code.as_deref().unwrap_or_default())
}

/// Egyetlen meccssor betöltése a duplikátum-vizsgálathoz.
async fn match_row(pool: &PgPool, match_id: Uuid) -> Result<Option<DuplicateMatchRow>> {
    let sql = format!("{MATCH_ROW_SELECT} WHERE m.id = $1");
    let row = sqlx::query_as::<_, DuplicateMatchRow>(&sql)
        .bind(match_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// S37c: a név-variáns pár merge-elhető-e (a klasszikus `is_same_match`
/// helyett, amely azonos csapat-azonosítókat követel)?
///
/// Laza (kézi döntésű) ág: legfeljebb 12 órás kickoff-különbség, lazább
/// név-egyezés. `allow_cross_competition` esetén különböző verseny-entitás is
/// jöhet, de csak egyező normalizált ország + csapat-kontextus mellett.
pub async fn is_variant_same_match(
    pool: &PgPool,
    survivor: Uuid,
    duplicate: Uuid,
    allow_cross_competition: bool,
) -> Result<bool> {
    let Some(s) = match_row(pool, survivor).await? else {
        return Ok(false);
    };
    let Some(d) = match_row(pool, duplicate).await? else {
        return Ok(false);
    };
    Ok(variant_pair_mergeable(&s, &d, allow_cross_competition))
}

fn variant_pair_mergeable(
    s: &DuplicateMatchRow,
    d: &DuplicateMatchRow,
    allow_cross_competition: bool,
) -> bool {
    if !competitions_compatible(s, d, allow_cross_competition) {
        return false;
    }
    if (s.kickoff_at - d.kickoff_at).num_minutes().abs() > 12 * 60 {
        return false;
    }
    let same_teams = (s.home_team_id == d.home_team_id && s.away_team_id == d.away_team_id)
        || (s.home_team_id == d.away_team_id && s.away_team_id == d.home_team_id);
    if same_teams && s.competition_id != d.competition_id && allow_cross_competition {
        let flipped = s.home_team_id == d.away_team_id;
        // Tükrözött variáns: az áldozat eredménye elveszne — kézi döntés kell.
        return !(flipped && d.has_result && !s.has_result);
    }
    let normal = names_likely_same(&s.home_name, &d.home_name)
        && names_likely_same(&s.away_name, &d.away_name);
    let flipped = names_likely_same(&s.home_name, &d.away_name)
        && names_likely_same(&s.away_name, &d.home_name);
    if flipped && d.has_result && !s.has_result {
        // Tükrözött variáns: az áldozat eredménye elveszne — kézi döntés kell.
        return false;
    }
    normal || flipped
}

/// S37c szigorú pár: azonos kickoff (vagy pontosan 2 óra eltérés) ÉS a
/// korosztály-jelölés elhagyásával egyező nevek. A backfill csak ezt a fajtát
/// mergeli automatikusan; a 12 órás, lazább jelölést a `is_variant_same_match`
/// adja (kézi döntéshez).
pub async fn is_strict_variant_same_match(
    pool: &PgPool,
    survivor: Uuid,
    duplicate: Uuid,
    allow_cross_competition: bool,
) -> Result<bool> {
    let Some(s) = match_row(pool, survivor).await? else {
        return Ok(false);
    };
    let Some(d) = match_row(pool, duplicate).await? else {
        return Ok(false);
    };
    Ok(strict_variant_pair_mergeable(
        &s,
        &d,
        allow_cross_competition,
    ))
}

fn strict_variant_pair_mergeable(
    s: &DuplicateMatchRow,
    d: &DuplicateMatchRow,
    allow_cross_competition: bool,
) -> bool {
    if !competitions_compatible(s, d, allow_cross_competition) {
        return false;
    }
    // Azonos nap kell; a kickoff vagy pontosan egyezik, vagy pontosan 2 órával
    // tér el (a források egy része nyáron is CEST-eltolást használt novemberben —
    // mért adathiba a 2026-11-17-es U17/U19 sorokon).
    let delta = (s.kickoff_at - d.kickoff_at).num_minutes().abs();
    if delta != 0 && delta != 120 {
        return false;
    }
    let normal = names_strictly_same(&s.home_name, &d.home_name)
        && names_strictly_same(&s.away_name, &d.away_name);
    let flipped = names_strictly_same(&s.home_name, &d.away_name)
        && names_strictly_same(&s.away_name, &d.home_name);
    if flipped && d.has_result {
        // A tükrözött pár eredménye csak kézi döntéssel vihető át.
        return false;
    }
    normal || flipped
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
                        FROM fact.match_stats s WHERE s.match_id=$3),
            'source_ref', (SELECT COALESCE(jsonb_agg(to_jsonb(r)), '[]'::jsonb)
                             FROM identity.source_entity_ref r
                            WHERE r.entity_kind = 'match' AND r.entity_id = $3)
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

    // 5b. S37c: a meccs-szintű forrás-hidak átkerülnek a túlélőhöz (az
    //     entitás-azonosító nem része a unique kulcsnak, ütközés nincs).
    sqlx::query(
        "UPDATE identity.source_entity_ref SET entity_id = $2
          WHERE entity_kind = 'match' AND entity_id = $1",
    )
    .bind(duplicate)
    .bind(survivor)
    .execute(&mut *tx)
    .await?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, day, hour, 0, 0)
            .single()
            .expect("érvényes teszt-időpont")
    }

    /// Cross-competition nélkül is összevonható alap-sor (azonos verseny).
    fn row(match_id: u128) -> DuplicateMatchRow {
        DuplicateMatchRow {
            match_id: Uuid::from_u128(match_id),
            competition_id: Uuid::from_u128(100),
            competition_name: "Cymru Premier".to_string(),
            competition_country_code: Some("GB".to_string()),
            competition_priority: 13,
            home_team_id: Uuid::from_u128(1_000 + match_id),
            away_team_id: Uuid::from_u128(2_000 + match_id),
            home_name: "Bahrain".to_string(),
            away_name: "Kyrgyzstan U17".to_string(),
            home_gender: "male".to_string(),
            away_gender: "male".to_string(),
            home_age_group: "senior".to_string(),
            away_age_group: "u17".to_string(),
            home_branch: "senior".to_string(),
            away_branch: "youth".to_string(),
            kickoff_at: at(20, 18),
            created_at: at(20, 10),
            provenance_count: 1,
            has_result: false,
            has_odds: false,
            gate_state: None,
        }
    }

    #[test]
    fn variant_pair_merges_within_the_same_competition() {
        let a = row(1);
        let mut b = row(2);
        b.competition_id = a.competition_id;
        assert!(variant_pair(&a, &b, false));
        assert!(variant_pair(&a, &b, true));
    }

    #[test]
    fn variant_pair_accepts_same_teams_across_competitions() {
        // Ugyanaz a két csapat két verseny-entitás alatt, eltérő (lokalizált)
        // névvel: a csapat-identitás a bizonyíték, nem a név-hasonlóság.
        let a = row(1);
        let mut b = row(2);
        b.competition_id = Uuid::from_u128(200);
        b.competition_name = "Primera Federacion - Group 2".to_string();
        b.home_team_id = a.home_team_id;
        b.away_team_id = a.away_team_id;
        b.home_name = "St. Pölten".to_string();
        b.away_name = "Voitsberg".to_string();
        assert!(
            !variant_pair(&a, &b, false),
            "cross-competition nélkül nem pár"
        );
        assert!(variant_pair(&a, &b, true));
        assert!(variant_pair_mergeable(&a, &b, true));
    }

    #[test]
    fn variant_pair_same_teams_same_competition_needs_the_exact_path() {
        let a = row(1);
        let mut b = row(2);
        b.home_team_id = a.home_team_id;
        b.away_team_id = a.away_team_id;
        assert!(!variant_pair(&a, &b, true));
    }

    #[test]
    fn variant_pair_same_teams_flip_with_result_is_manual() {
        let a = row(1);
        let mut b = row(2);
        b.competition_id = Uuid::from_u128(200);
        b.home_team_id = a.away_team_id;
        b.away_team_id = a.home_team_id;
        b.has_result = true;
        assert!(variant_pair(&a, &b, true));
        assert!(!variant_pair_mergeable(&a, &b, true));
    }

    #[test]
    fn variant_pair_requires_flag_for_cross_competition() {
        let a = row(1);
        let mut b = row(2);
        b.competition_id = Uuid::from_u128(200);
        b.competition_name = "Premier League".to_string();
        assert!(!variant_pair(&a, &b, false));
        assert!(variant_pair(&a, &b, true));
    }

    #[test]
    fn variant_pair_rejects_different_countries() {
        let a = row(1);
        let mut b = row(2);
        b.competition_id = Uuid::from_u128(200);
        b.competition_country_code = Some("ES".to_string());
        assert!(!variant_pair(&a, &b, true));
    }

    #[test]
    fn variant_pair_compares_normalized_country_codes() {
        let a = row(1);
        let mut b = row(2);
        b.competition_id = Uuid::from_u128(200);
        b.competition_country_code = Some("GBR".to_string());
        assert!(variant_pair(&a, &b, true));
    }

    #[test]
    fn variant_pair_rejects_womens_against_mens() {
        // A csapat `gender` oszlopa mindkét oldalon `male`, de a női csapat
        // nevét a `W` jelölő elárulja: nem olvadhat össze férfi meccsel.
        let a = row(1);
        let mut b = row(2);
        b.competition_id = Uuid::from_u128(200);
        b.competition_name = "Premiere Ligue Femenine".to_string();
        b.away_name = "Kyrgyzstan U17 W".to_string();
        assert!(!variant_pair(&a, &b, true));
    }

    #[test]
    fn variant_pair_allows_matching_womens_markers() {
        let a = row(1);
        let mut b = row(2);
        b.competition_id = Uuid::from_u128(200);
        b.home_name = a.home_name.clone();
        b.away_name = a.away_name.clone();
        assert!(variant_pair(&a, &b, true));
    }

    #[test]
    fn variant_pair_rejects_different_team_context() {
        let a = row(1);
        let mut b = row(2);
        b.competition_id = Uuid::from_u128(200);
        b.home_gender = "female".to_string();
        assert!(!variant_pair(&a, &b, true));

        let mut c = row(3);
        c.competition_id = Uuid::from_u128(200);
        c.away_age_group = "u19".to_string();
        assert!(!variant_pair(&a, &c, true));

        let mut d = row(4);
        d.competition_id = Uuid::from_u128(200);
        d.home_branch = "reserve".to_string();
        assert!(!variant_pair(&a, &d, true));
    }

    #[test]
    fn variant_pair_same_team_ids_requires_cross_competition() {
        // S41: azonos csapatpár különböző verseny-entitás alatt cross módban
        // pár; cross nélkül marad a régi, konzervatív viselkedés.
        let a = row(1);
        let mut b = row(2);
        b.competition_id = Uuid::from_u128(200);
        b.home_team_id = a.home_team_id;
        b.away_team_id = a.away_team_id;
        assert!(!variant_pair(&a, &b, false));
        assert!(variant_pair(&a, &b, true));
    }

    #[test]
    fn variant_pair_accepts_flipped_names() {
        let a = row(1);
        let mut b = row(2);
        b.competition_id = Uuid::from_u128(200);
        b.home_name = a.away_name.clone();
        b.away_name = a.home_name.clone();
        assert!(variant_pair(&a, &b, true));
    }

    #[test]
    fn variant_pair_rejects_more_than_twelve_hours() {
        let a = row(1);
        let mut b = row(2);
        b.competition_id = Uuid::from_u128(200);
        b.kickoff_at = at(21, 7); // 13 óra
        assert!(!variant_pair(&a, &b, true));

        let mut c = row(3);
        c.competition_id = Uuid::from_u128(200);
        c.kickoff_at = at(21, 6); // pontosan 12 óra — engedett
        assert!(variant_pair(&a, &c, true));
    }

    #[test]
    fn loose_mergeable_rejects_mirrored_variant_without_survivor_result() {
        let a = row(1);
        let mut b = row(2);
        b.competition_id = Uuid::from_u128(200);
        b.home_name = a.away_name.clone();
        b.away_name = a.home_name.clone();
        b.has_result = true;
        assert!(!variant_pair_mergeable(&a, &b, true));

        let mut survivor_with_result = a.clone();
        survivor_with_result.has_result = true;
        assert!(variant_pair_mergeable(&survivor_with_result, &b, true));
    }

    #[test]
    fn strict_pair_accepts_zero_or_two_hour_delta_only() {
        let a = row(1);
        let mut b = row(2);
        b.competition_id = Uuid::from_u128(200);
        assert!(strict_variant_pair_mergeable(&a, &b, true));

        let mut two_hours = row(3);
        two_hours.competition_id = Uuid::from_u128(200);
        two_hours.kickoff_at = at(20, 20);
        assert!(strict_variant_pair_mergeable(&a, &two_hours, true));

        let mut one_hour = row(4);
        one_hour.competition_id = Uuid::from_u128(200);
        one_hour.kickoff_at = at(20, 19);
        assert!(!strict_variant_pair_mergeable(&a, &one_hour, true));
    }

    #[test]
    fn strict_pair_rejects_cross_competition_without_matching_country() {
        let a = row(1);
        let mut b = row(2);
        b.competition_id = Uuid::from_u128(200);
        b.competition_country_code = Some("PL".to_string());
        assert!(!strict_variant_pair_mergeable(&a, &b, true));
    }
}
