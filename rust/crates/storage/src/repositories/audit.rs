//! S47: olvasható (read-only) heti duplikátum- és konzisztencia-audit.
//!
//! A `dedupe` parancsok a *javítást* végzik; ez a modul a *bizonyítékot*
//! adja: egy dátumtartomány összes meccsén megkeresi a még meglévő
//! duplikátum-klasztereket (pontos csapat-egyezés, mindkét oldalon egyező
//! névvariáns, illetve az „egyik oldal biztos egyezés" esetét), a duplikált
//! verseny-entitásokat és a triviális konzisztencia-hibákat (lezárt meccs
//! eredmény nélkül, eredmény nem terminális státusszal).
//!
//! A CLI a jelentést magyarul írja ki, és ha bármi találat van, exit 1-gyel
//! zár — így ugyanaz a parancs kapuként és emberi ellenőrzésként is használható.

use std::collections::HashMap;

use anyhow::Result;
use chrono::NaiveDate;
use football_domain::identity::{fold_name, names_likely_same};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::dedupe::{DuplicateMatchRow, match_rows};

/// A név-variáns keresés ablaka: a források legfeljebb ennyit térhetnek el.
const PAIR_WINDOW_MINUTES: i64 = 12 * 60;

/// Két sor név-egyezésének bizonyossága.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MatchReason {
    /// Azonos csapat-azonosítók (a klasszikus pontos duplikátum).
    Exact,
    /// Mindkét oldalon egyező csapatnév (normál vagy tükrözött orientáció).
    Variant,
}

impl MatchReason {
    pub fn as_str(self) -> &'static str {
        match self {
            MatchReason::Exact => "pontos",
            MatchReason::Variant => "név-variáns",
        }
    }
}

/// Egy összetartozó duplikátum-csoport a vizsgált időszakban.
#[derive(Debug, Clone, Serialize)]
pub struct AuditCluster {
    pub reason: MatchReason,
    pub matches: Vec<DuplicateMatchRow>,
}

/// Ugyanaz a csapat két különböző meccsen ugyanabban az időablakban — szinte
/// mindig hibás csapat-hozzárendelés az egyik sorban.
#[derive(Debug, Clone, Serialize)]
pub struct DoubleBooking {
    pub team_id: Uuid,
    pub team_name: String,
    pub matches: Vec<DuplicateMatchRow>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CompetitionRow {
    pub competition_id: Uuid,
    pub name: String,
    pub country_code: Option<String>,
    pub match_count: i64,
}

/// Azonos foldolt nevű verseny-entitások (országon belül, illetve semleges
/// ország mellett).
#[derive(Debug, Clone, Serialize)]
pub struct CompetitionDuplicate {
    pub name_key: String,
    pub competitions: Vec<CompetitionRow>,
}

#[derive(Debug, Clone, Default, Serialize, sqlx::FromRow)]
pub struct ConsistencyReport {
    /// `finished` státusz, de nincs eredmény-sor.
    pub finished_without_result: i64,
    /// Van eredmény, de a státusz nem terminális.
    pub result_without_terminal_status: i64,
    /// A tartományban visszatartott (withheld/quarantined) meccsek.
    pub withheld: i64,
    /// Olyan meccs, amelynek valamelyik csapatának nincs országa.
    pub matches_with_countryless_team: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateAuditReport {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub matches_scanned: usize,
    pub clusters: Vec<AuditCluster>,
    pub double_bookings: Vec<DoubleBooking>,
    pub competition_duplicates: Vec<CompetitionDuplicate>,
    pub consistency: ConsistencyReport,
}

impl DuplicateAuditReport {
    /// Hány javítandó találat van (a withheld szándékos, ezért nem számít).
    pub fn finding_count(&self) -> usize {
        self.clusters.len()
            + self.double_bookings.len()
            + self.competition_duplicates.len()
            + (self.consistency.finished_without_result > 0) as usize
            + (self.consistency.result_without_terminal_status > 0) as usize
            + (self.consistency.matches_with_countryless_team > 0) as usize
    }

    pub fn is_clean(&self) -> bool {
        self.finding_count() == 0
    }
}

/// A tartomány teljes duplikátum- és konzisztencia-auditja.
pub async fn audit_range(
    pool: &PgPool,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<DuplicateAuditReport> {
    let rows = match_rows(pool, from, to, false).await?;
    // A visszatartott sorok nem jelennek meg a napi listán, ezért a
    // duplikáció- és dupla-szereplés-vizsgálatból kimaradnak.
    let publishable: Vec<DuplicateMatchRow> = rows
        .iter()
        .filter(|row| {
            !matches!(
                row.gate_state.as_deref(),
                Some("withheld") | Some("quarantined")
            )
        })
        .cloned()
        .collect();
    // A le nem játszott (elmaradt, törölt, félbeszakadt) meccsek nem
    // számítanak dupla szereplésnek.
    let ids: Vec<Uuid> = publishable.iter().map(|row| row.match_id).collect();
    let played: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM core.match \
         WHERE id = ANY($1) AND status NOT IN ('canceled', 'postponed', 'abandoned')",
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?;
    let played: std::collections::HashSet<Uuid> = played.into_iter().collect();
    // A szerkesztői holddal kizárt meccsek szintén nem részei a napi listának.
    let held: std::collections::HashSet<Uuid> =
        sqlx::query_scalar("SELECT match_id FROM ops.editorial_hold WHERE match_id = ANY($1)")
            .bind(&ids)
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();
    let active: Vec<DuplicateMatchRow> = publishable
        .into_iter()
        .filter(|row| played.contains(&row.match_id) && !held.contains(&row.match_id))
        .collect();
    let clusters = cluster_rows(&active);
    let double_bookings = find_double_bookings(&active);
    let competition_duplicates = competition_duplicates(pool, from, to).await?;
    let consistency = consistency_report(pool, from, to).await?;
    Ok(DuplicateAuditReport {
        from,
        to,
        matches_scanned: rows.len(),
        clusters,
        double_bookings,
        competition_duplicates,
        consistency,
    })
}

/// Két sor viszonya — a klaszterezés és a tesztek közös magja.
pub fn pair_reason(a: &DuplicateMatchRow, b: &DuplicateMatchRow) -> Option<MatchReason> {
    if a.match_id == b.match_id {
        return None;
    }
    let delta = (a.kickoff_at - b.kickoff_at).num_minutes().abs();
    if delta > PAIR_WINDOW_MINUTES {
        return None;
    }
    if !context_compatible(a, b) {
        return None;
    }
    let same_teams = (a.home_team_id == b.home_team_id && a.away_team_id == b.away_team_id)
        || (a.home_team_id == b.away_team_id && a.away_team_id == b.home_team_id);
    if same_teams {
        return Some(MatchReason::Exact);
    }
    let normal = names_likely_same(&a.home_name, &b.home_name)
        && names_likely_same(&a.away_name, &b.away_name);
    let flipped = names_likely_same(&a.home_name, &b.away_name)
        && names_likely_same(&a.away_name, &b.home_name);
    if normal || flipped {
        return Some(MatchReason::Variant);
    }
    // Az „egyik oldal egyezik" heurisztikát szándékosan NEM használjuk: az
    // azonos nevű külön klubokat (Coventry / Coventry United, Lincoln /
    // Lincoln United) összekeverné. A valódi adathibát (ugyanaz a csapat két
    // meccsen ugyanabban az időben) a `double_bookings` ellenőrzés fogja meg.
    None
}

/// Klaszterezés union-finddal a `pair_reason` éleire; a klaszter indoka a
/// legerősebb él (exact > variant > one_sided).
pub fn cluster_rows(rows: &[DuplicateMatchRow]) -> Vec<AuditCluster> {
    let mut parent: Vec<usize> = (0..rows.len()).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut reasons: HashMap<(usize, usize), MatchReason> = HashMap::new();
    for (i, a) in rows.iter().enumerate() {
        for (j, b) in rows.iter().enumerate().skip(i + 1) {
            if let Some(reason) = pair_reason(a, b) {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                if ri != rj {
                    parent[ri] = rj;
                }
                let key = (ri.min(rj), ri.max(rj));
                reasons
                    .entry(key)
                    .and_modify(|existing| {
                        if reason_rank(reason) > reason_rank(*existing) {
                            *existing = reason;
                        }
                    })
                    .or_insert(reason);
            }
        }
    }
    let mut members: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..rows.len() {
        let root = find(&mut parent, i);
        members.entry(root).or_default().push(i);
    }
    let mut clusters: Vec<AuditCluster> = Vec::new();
    for (_root, indices) in members {
        if indices.len() < 2 {
            continue;
        }
        let reason = indices
            .iter()
            .flat_map(|&i| indices.iter().skip(1).map(move |&j| (i, j)))
            .filter_map(|(i, j)| reasons.get(&(i.min(j), i.max(j))).copied())
            .max_by_key(|r| reason_rank(*r))
            .unwrap_or(MatchReason::Variant);
        clusters.push(AuditCluster {
            reason,
            matches: indices.into_iter().map(|i| rows[i].clone()).collect(),
        });
    }
    // Determinisztikus sorrend: a legkorábbi kickoff szerint.
    clusters.sort_by_key(|c| {
        c.matches
            .iter()
            .map(|m| m.kickoff_at)
            .min()
            .unwrap_or_else(chrono::Utc::now)
    });
    clusters
}

fn reason_rank(reason: MatchReason) -> u8 {
    match reason {
        MatchReason::Exact => 3,
        MatchReason::Variant => 2,
    }
}

/// Csapat-kontextus: nem (név-jelölő is), korosztály (ha mindkettő mondja).
fn context_compatible(a: &DuplicateMatchRow, b: &DuplicateMatchRow) -> bool {
    women_marked(&a.home_name) == women_marked(&b.home_name)
        && women_marked(&a.away_name) == women_marked(&b.away_name)
        && age_compatible(&a.home_name, &b.home_name)
        && age_compatible(&a.away_name, &b.away_name)
}

fn women_marked(name: &str) -> bool {
    fold_name(name).split(' ').any(|token| {
        matches!(
            token,
            "w" | "women" | "womens" | "woman" | "noi" | "femenine" | "feminine" | "ladies"
        )
    })
}

/// Az audit szándékosan szigorú: az egyik oldalon jelölt, a másikon jelöletlen
/// korosztály nem elég bizonyíték (a név-marker hiányát nem tekintjük
/// egyezésnek), mert egy U17-es és egy felnőtt meccs ugyanabban az időben is
/// létezhet ugyanazzal a nemzetnevezéssel.
fn age_compatible(a: &str, b: &str) -> bool {
    match (age_marker(a), age_marker(b)) {
        (Some(x), Some(y)) => x == y,
        (None, None) => true,
        _ => false,
    }
}

fn age_marker(name: &str) -> Option<String> {
    let folded = fold_name(name);
    let tokens: Vec<&str> = folded.split(' ').filter(|t| !t.is_empty()).collect();
    for (i, token) in tokens.iter().enumerate() {
        let digits = if let Some(rest) = token.strip_prefix('u') {
            rest
        } else if *token == "u" {
            tokens.get(i + 1).copied().unwrap_or("")
        } else {
            continue;
        };
        if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) && digits.len() <= 3 {
            return Some(digits.to_string());
        }
    }
    None
}

/// Ugyanaz a csapat 6 órán belül két különböző meccsen.
///
/// A küszöb szándékosan 6 óra: ennyi időn belül egy csapat nem játszhat két
/// meccset, tehát valamelyik sor ellenfele hibás (forrás-összekeveredés).
pub fn find_double_bookings(rows: &[DuplicateMatchRow]) -> Vec<DoubleBooking> {
    const WINDOW_MINUTES: i64 = 6 * 60;
    let mut by_team: HashMap<Uuid, Vec<usize>> = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        by_team.entry(row.home_team_id).or_default().push(index);
        by_team.entry(row.away_team_id).or_default().push(index);
    }
    let mut out: Vec<DoubleBooking> = Vec::new();
    for (team_id, mut indices) in by_team {
        if indices.len() < 2 {
            continue;
        }
        indices.sort_by_key(|&index| rows[index].kickoff_at);
        let mut involved: Vec<usize> = Vec::new();
        for (position, &index) in indices.iter().enumerate() {
            for &other in indices.iter().skip(position + 1) {
                let delta = (rows[other].kickoff_at - rows[index].kickoff_at)
                    .num_minutes()
                    .abs();
                if delta > WINDOW_MINUTES {
                    break;
                }
                for candidate in [index, other] {
                    if !involved.contains(&candidate) {
                        involved.push(candidate);
                    }
                }
            }
        }
        if involved.is_empty() {
            continue;
        }
        involved.sort_unstable();
        let team_name = involved
            .first()
            .map(|&index| {
                let row = &rows[index];
                if row.home_team_id == team_id {
                    row.home_name.clone()
                } else {
                    row.away_name.clone()
                }
            })
            .unwrap_or_default();
        out.push(DoubleBooking {
            team_id,
            team_name,
            matches: involved
                .into_iter()
                .map(|index| rows[index].clone())
                .collect(),
        });
    }
    out.sort_by(|a, b| a.team_name.cmp(&b.team_name));
    out
}

/// Azonos foldolt nevű, de külön entitásként élő versenyek a tartományban.
pub async fn competition_duplicates(
    pool: &PgPool,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<CompetitionDuplicate>> {
    let rows = sqlx::query_as::<_, CompetitionRow>(
        "SELECT c.id AS competition_id,
                c.name,
                cc.code AS country_code,
                count(m.id)::bigint AS match_count
           FROM core.competition c
           LEFT JOIN core.country cc ON cc.id = c.country_id
           JOIN core.match m ON m.competition_id = c.id
          WHERE m.kickoff_at::date BETWEEN $1 AND $2
          GROUP BY c.id, c.name, cc.code
          HAVING count(m.id) > 0",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;

    let mut by_name: HashMap<String, Vec<CompetitionRow>> = HashMap::new();
    for row in rows {
        by_name.entry(fold_name(&row.name)).or_default().push(row);
    }
    let mut out: Vec<CompetitionDuplicate> = Vec::new();
    for (name_key, group) in by_name {
        if group.len() < 2 {
            continue;
        }
        // Országonként (semleges = wildcard) klaszterezzük a versenyeket.
        let mut assigned: Vec<bool> = vec![false; group.len()];
        for i in 0..group.len() {
            if assigned[i] {
                continue;
            }
            let mut cluster = vec![i];
            assigned[i] = true;
            let mut changed = true;
            while changed {
                changed = false;
                for j in 0..group.len() {
                    if assigned[j] {
                        continue;
                    }
                    if cluster
                        .iter()
                        .any(|&k| countries_compatible(&group[k], &group[j]))
                    {
                        assigned[j] = true;
                        cluster.push(j);
                        changed = true;
                    }
                }
            }
            if cluster.len() >= 2 {
                out.push(CompetitionDuplicate {
                    name_key: name_key.clone(),
                    competitions: cluster.into_iter().map(|k| group[k].clone()).collect(),
                });
            }
        }
    }
    out.sort_by(|a, b| a.name_key.cmp(&b.name_key));
    Ok(out)
}

fn countries_compatible(a: &CompetitionRow, b: &CompetitionRow) -> bool {
    let key = |c: &Option<String>| {
        c.as_deref()
            .map(football_domain::identity::canonical_country_code)
            .unwrap_or_default()
    };
    let (ka, kb) = (key(&a.country_code), key(&b.country_code));
    let neutral = |k: &str| k.is_empty() || k == "XX" || k == "INT";
    ka == kb || neutral(&ka) || neutral(&kb)
}

async fn consistency_report(
    pool: &PgPool,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<ConsistencyReport> {
    let row = sqlx::query_as::<_, ConsistencyReport>(
        "SELECT
            (SELECT count(*) FROM core.match m
              WHERE m.kickoff_at::date BETWEEN $1 AND $2
                AND m.status = 'finished'
                AND NOT EXISTS (SELECT 1 FROM core.match_result r WHERE r.match_id = m.id))::bigint
                AS finished_without_result,
            (SELECT count(*) FROM core.match m
              JOIN core.match_result r ON r.match_id = m.id
              WHERE m.kickoff_at::date BETWEEN $1 AND $2
                AND m.status NOT IN ('finished','awarded','postponed','canceled','abandoned'))::bigint
                AS result_without_terminal_status,
            (SELECT count(*) FROM ops.publish_gate g
              JOIN core.match m ON m.id = g.match_id
              WHERE m.kickoff_at::date BETWEEN $1 AND $2
                AND g.state IN ('withheld','quarantined'))::bigint AS withheld,
            (SELECT count(*) FROM core.match m
              JOIN core.team h ON h.id = m.home_team_id
              JOIN core.team a ON a.id = m.away_team_id
              WHERE m.kickoff_at::date BETWEEN $1 AND $2
                AND (h.country_id IS NULL OR a.country_id IS NULL))::bigint
                AS matches_with_countryless_team",
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn row(
        id: u128,
        comp: u128,
        home: u128,
        away: u128,
        home_name: &str,
        away_name: &str,
        kickoff: &str,
    ) -> DuplicateMatchRow {
        let kickoff_at =
            chrono::NaiveDateTime::parse_from_str(kickoff, "%Y-%m-%d %H:%M").expect("kickoff");
        DuplicateMatchRow {
            match_id: Uuid::from_u128(id),
            competition_id: Uuid::from_u128(comp),
            competition_name: "Teszt Liga".into(),
            competition_country_code: Some("HU".into()),
            competition_priority: 30,
            home_team_id: Uuid::from_u128(home),
            away_team_id: Uuid::from_u128(away),
            home_name: home_name.into(),
            away_name: away_name.into(),
            home_gender: "male".into(),
            away_gender: "male".into(),
            home_age_group: "senior".into(),
            away_age_group: "senior".into(),
            home_branch: "senior".into(),
            away_branch: "senior".into(),
            kickoff_at: Utc.from_utc_datetime(&kickoff_at),
            created_at: Utc.from_utc_datetime(&kickoff_at),
            provenance_count: 1,
            has_result: false,
            has_odds: false,
            gate_state: None,
        }
    }

    #[test]
    fn exact_team_ids_are_flagged() {
        let a = row(1, 10, 100, 101, "A", "B", "2026-11-20 19:00");
        let b = row(2, 10, 100, 101, "A", "B", "2026-11-20 19:00");
        assert_eq!(pair_reason(&a, &b), Some(MatchReason::Exact));
    }

    #[test]
    fn name_variant_on_both_sides_is_flagged_in_both_orientations() {
        let a = row(
            1,
            10,
            100,
            101,
            "Sheffield Weds",
            "Southend Utd",
            "2026-11-20 19:00",
        );
        let b = row(
            2,
            11,
            200,
            201,
            "Sheffield Wednesday",
            "Southend United",
            "2026-11-20 19:45",
        );
        assert_eq!(pair_reason(&a, &b), Some(MatchReason::Variant));
        let flipped = row(
            3,
            11,
            201,
            200,
            "Southend United",
            "Sheffield Wednesday",
            "2026-11-20 20:00",
        );
        assert_eq!(pair_reason(&a, &flipped), Some(MatchReason::Variant));
    }

    #[test]
    fn different_age_groups_never_match() {
        let a = row(
            1,
            10,
            100,
            101,
            "Germany U17",
            "France U17",
            "2026-11-20 19:00",
        );
        let b = row(2, 10, 200, 201, "Germany", "France", "2026-11-20 19:00");
        assert_eq!(pair_reason(&a, &b), None);
    }

    #[test]
    fn womens_and_mens_fixture_never_merge() {
        let a = row(
            1,
            10,
            100,
            101,
            "Toulouse W",
            "Marseille W",
            "2026-11-20 19:00",
        );
        let b = row(2, 10, 200, 201, "Toulouse", "Marseille", "2026-11-20 19:00");
        assert_eq!(pair_reason(&a, &b), None);
    }

    #[test]
    fn double_booking_finds_the_same_team_in_two_matches() {
        let a = row(1, 10, 100, 101, "Ards", "H&W Welders", "2026-11-21 12:00");
        let b = row(
            2,
            10,
            101,
            300,
            "H&W Welders",
            "Newington",
            "2026-11-21 12:00",
        );
        let c = row(3, 10, 400, 401, "C", "D", "2026-11-21 20:00");
        let bookings = find_double_bookings(&[a, b, c]);
        assert_eq!(bookings.len(), 1);
        assert_eq!(bookings[0].team_name, "H&W Welders");
        assert_eq!(bookings[0].matches.len(), 2);
    }

    #[test]
    fn clusters_contain_only_multi_row_groups() {
        let a = row(1, 10, 100, 101, "A", "B", "2026-11-20 19:00");
        let b = row(2, 10, 100, 101, "A", "B", "2026-11-20 19:00");
        let c = row(3, 10, 300, 301, "C", "D", "2026-11-20 21:00");
        let clusters = cluster_rows(&[a, b, c]);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].matches.len(), 2);
        assert_eq!(clusters[0].reason, MatchReason::Exact);
    }

    #[test]
    fn out_of_window_pairs_are_not_flagged() {
        let a = row(1, 10, 100, 101, "A", "B", "2026-11-20 10:00");
        let b = row(2, 10, 200, 201, "A", "B", "2026-11-21 10:00");
        assert_eq!(pair_reason(&a, &b), None);
    }
}
