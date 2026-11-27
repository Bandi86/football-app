//! Auditált season-stats import a `fact.match_insight.season_stats` JSON-ból
//! a `fact.season_team_stats` táblába (S-szelet A).
//!
//! Hálózati kérés nélkül, dry-run alapból; `--apply`-nál tábla-backup +
//! tranzakciós, napi bontású `INSERT ... ON CONFLICT DO NOTHING`.
//!
//! Névre kötött, evidence-alapú mapping (nincs név-alapú találgatás):
//! - forrás: `fact.match_insight` sor `source_id=research` (kód `research`),
//! - `season_stats` JSON-objektum `home`/`away` alkulcsa;
//! - `home` -> `core.match.home_team_id`, `away` -> `core.match.away_team_id`;
//! - `season_id` -> `core.match.season_id` (mind NOT NULL, exact egyezés);
//! - `stat_key` = a forrás JSON-kulcs változatlanul;
//! - `value` = csak JSON-szám (numeric); `null` / szöveg / nem-szám kulcs
//!   NEM jelölt; `captured_at` = `fact.match_insight.captured_at`;
//! - `source_id` = `research`;
//! - `ON CONFLICT DO NOTHING` (nem felülír);
//! - szakaszos: napi (kickoff-dátum) batches + batchonként tranzakció.
//!
//! Bizonyíték hiányában utasított sor: `season_stats` nem objektum /
//! `home` vagy `away` nem objektum / a meccs sora hiányzik. Számszerűség
//! hiánya (null / nem-szám érték) külön számláló, nem kitalált `0`.
//!
//! `fact.match_team_stats` és `fact.season_player_stats` szándékosan
//! érintetlen: nincs per-meccs csapatstat forrás a `match_insight`-ban, és
//! nincs játékosszintű szezonforrás (lásd NEM-implementált lista).

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Forrás-kód a `match_insight` sorokhoz.
const SOURCE_CODE: &str = "research";

/// Mely JSON-kulcsok NEM számszerű statisztika (nem jelölt sor).
fn is_non_numeric_meta_key(key: &str) -> bool {
    matches!(key, "group" | "source" | "source_url" | "rank_label")
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SeasonStatsReport {
    pub applied: bool,
    pub days: usize,
    pub matches_scanned: usize,
    /// Jelölt `fact.season_team_stats` sorok (numeric, exact season+team).
    pub candidate_rows: usize,
    /// Bizonyíték hiányában utasított sorok (nem objektum / hiányzó meccs).
    pub rejected_missing_evidence: usize,
    /// Számszerűség hiánya miatt nem jelölt (null / nem-szám) értékek.
    pub rejected_non_numeric: usize,
    /// Ténylegesen beszúrt sorok (`apply`-nál; ON CONFLICT DO NOTHING).
    pub inserted: usize,
    /// Backup/táblakép-fájl útvonala (`apply`-nál, írás ELŐTT készül).
    pub backup_path: Option<String>,
}

/// Egy jelölt szezon-stat sor.
#[derive(Debug, Clone, PartialEq)]
pub struct SeasonStatCandidate {
    pub season_id: Uuid,
    pub team_id: Uuid,
    pub stat_key: String,
    /// NUMERIC-kompatibilis string; DB-ben `CAST(... AS numeric)`.
    pub value: String,
}

/// Egy meccs `season_stats` JSON-jának osztályozása. Tiszta függvény:
/// season_id + home/away team id a `core.match`-ből jön (exact, NOT NULL);
/// itt csak a JSON-oldali bizonyíték dől el.
#[derive(Debug, Default, PartialEq)]
pub struct ClassifiedMatch {
    pub candidates: Vec<SeasonStatCandidate>,
    pub rejected_missing_evidence: usize,
    pub rejected_non_numeric: usize,
}

pub fn classify_match(
    season_id: Uuid,
    home_team_id: Uuid,
    away_team_id: Uuid,
    season_stats: Option<&serde_json::Value>,
) -> ClassifiedMatch {
    let mut out = ClassifiedMatch::default();
    let Some(stats) = season_stats else {
        out.rejected_missing_evidence += 1;
        return out;
    };
    let Some(obj) = stats.as_object() else {
        out.rejected_missing_evidence += 1;
        return out;
    };
    for (side_key, team_id) in [("home", home_team_id), ("away", away_team_id)] {
        let Some(side_obj) = obj.get(side_key).and_then(|s| s.as_object()) else {
            out.rejected_missing_evidence += 1;
            continue;
        };
        // Determinisztikus kimenet: kulcs-sorrend.
        let mut keys: Vec<&String> = side_obj.keys().collect();
        keys.sort();
        for key in keys {
            if is_non_numeric_meta_key(key) {
                continue;
            }
            match side_obj.get(key) {
                Some(serde_json::Value::Number(n)) => {
                    out.candidates.push(SeasonStatCandidate {
                        season_id,
                        team_id,
                        stat_key: key.clone(),
                        value: n.to_string(),
                    });
                }
                // Számszerűség hiánya: null / szöveg -> nem kitalált 0.
                Some(_) => out.rejected_non_numeric += 1,
                None => {}
            }
        }
    }
    out
}

struct MatchRow {
    season_id: Uuid,
    home_team_id: Uuid,
    away_team_id: Uuid,
    captured_at: DateTime<Utc>,
    season_stats: serde_json::Value,
}

async fn research_source_id(pool: &PgPool) -> Result<Uuid> {
    sqlx::query_scalar("SELECT id FROM ops.source WHERE code=$1")
        .bind(SOURCE_CODE)
        .fetch_one(pool)
        .await
        .context("research forrás hiányzik")
}

/// Napi bontású futtatás. Dry-run (`apply=false`): nem ír; csak jelölt /
/// utasított számolás. `apply=true`: backup + napi tranzakciós insert.
pub async fn run(pool: &PgPool, apply: bool) -> Result<SeasonStatsReport> {
    let source_id = research_source_id(pool).await?;
    let mut report = SeasonStatsReport {
        applied: apply,
        ..Default::default()
    };

    // Napi bontás (kickoff-dátum UTC), kulcs szerinti sorrend -> szakaszos.
    let days: Vec<NaiveDate> = sqlx::query_scalar(
        "SELECT DISTINCT (m.kickoff_at AT TIME ZONE 'UTC')::date AS d
           FROM fact.match_insight mi
           JOIN core.match m ON m.id = mi.match_id
          WHERE mi.source_id = $1
            AND mi.season_stats IS NOT NULL
            AND jsonb_typeof(mi.season_stats) = 'object'
          ORDER BY d",
    )
    .bind(source_id)
    .fetch_all(pool)
    .await?;

    // Backup/táblakép mentése írás ELŐTT (nem destruktív; visszaállási pont).
    // Fájl-backup (nem DB-tábla), hogy a séma/adattár érintetlen maradjon.
    if apply {
        let snapshot: Vec<(Uuid, Uuid, Uuid, DateTime<Utc>, String, String)> = sqlx::query_as(
            "SELECT season_id, team_id, source_id, captured_at, stat_key, value::text
             FROM fact.season_team_stats ORDER BY season_id, team_id, source_id, stat_key",
        )
        .fetch_all(pool)
        .await
        .context("táblakép olvasása")?;
        let rows: Vec<serde_json::Value> = snapshot
            .into_iter()
            .map(
                |(season_id, team_id, source_id, captured_at, stat_key, value)| {
                    serde_json::json!({
                        "season_id": season_id, "team_id": team_id, "source_id": source_id,
                        "captured_at": captured_at, "stat_key": stat_key, "value": value,
                    })
                },
            )
            .collect();
        let dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
        let backup_dir = std::path::Path::new(&dir).join("backups");
        std::fs::create_dir_all(&backup_dir).context("backup-könyvtár")?;
        let path = backup_dir.join(format!(
            "season_team_stats_backup_{}_{}.json",
            Utc::now().timestamp(),
            Uuid::now_v7().simple()
        ));
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&rows).context("backup JSON")?,
        )
        .context("backup-fájl írása")?;
        report.backup_path = Some(path.to_string_lossy().into_owned());
    }

    for day in days {
        report.days += 1;
        let rows: Vec<MatchRow> = sqlx::query_as::<
            _,
            (Uuid, Uuid, Uuid, DateTime<Utc>, serde_json::Value),
        >(
            "SELECT m.season_id, m.home_team_id, m.away_team_id, mi.captured_at, mi.season_stats
               FROM fact.match_insight mi
               JOIN core.match m ON m.id = mi.match_id
              WHERE mi.source_id = $1
                AND mi.season_stats IS NOT NULL
                AND jsonb_typeof(mi.season_stats) = 'object'
                AND (m.kickoff_at AT TIME ZONE 'UTC')::date = $2
              ORDER BY m.id",
        )
        .bind(source_id)
        .bind(day)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(
            |(season_id, home_team_id, away_team_id, captured_at, season_stats)| MatchRow {
                season_id,
                home_team_id,
                away_team_id,
                captured_at,
                season_stats,
            },
        )
        .collect();

        let mut day_candidates: Vec<(SeasonStatCandidate, DateTime<Utc>)> = Vec::new();
        for row in &rows {
            report.matches_scanned += 1;
            let classified = classify_match(
                row.season_id,
                row.home_team_id,
                row.away_team_id,
                Some(&row.season_stats),
            );
            report.rejected_missing_evidence += classified.rejected_missing_evidence;
            report.rejected_non_numeric += classified.rejected_non_numeric;
            report.candidate_rows += classified.candidates.len();
            for c in classified.candidates {
                day_candidates.push((c, row.captured_at));
            }
        }

        if apply && !day_candidates.is_empty() {
            let mut tx = pool.begin().await?;
            for (c, captured_at) in &day_candidates {
                let res = sqlx::query(
                    "INSERT INTO fact.season_team_stats
                       (season_id, team_id, source_id, captured_at, stat_key, value)
                     VALUES ($1,$2,$3,$4,$5, CAST($6 AS numeric))
                     ON CONFLICT DO NOTHING",
                )
                .bind(c.season_id)
                .bind(c.team_id)
                .bind(source_id)
                .bind(captured_at)
                .bind(&c.stat_key)
                .bind(&c.value)
                .execute(&mut *tx)
                .await?;
                report.inserted += res.rows_affected() as usize;
            }
            tx.commit().await?;
        }
    }

    Ok(report)
}

// ---- Tesztek ---------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(n: u8) -> Uuid {
        Uuid::from_bytes([n; 16])
    }

    #[test]
    fn classifies_numeric_keys_with_exact_season_team_mapping() {
        let season_stats = serde_json::json!({
            "home": {"wins": 3, "losses": 1, "points": 9, "group": "LaLiga2",
                     "source": "fotmob_table", "rank_label": "1/22",
                     "clean_sheets": null, "possession_pct": "55.0", "matches": 4},
            "away": {"wins": 1, "losses": 3, "points": 3, "goals_for": 2}
        });
        let out = classify_match(uuid(1), uuid(2), uuid(3), Some(&season_stats));
        assert_eq!(out.rejected_missing_evidence, 0);
        // null + nem-szám -> rejected_non_numeric, nem kitalált 0
        assert_eq!(out.rejected_non_numeric, 2);
        // metakulcsok (group/source/rank_label) nem jelölt sorok
        let home_keys: Vec<&str> = out
            .candidates
            .iter()
            .filter(|c| c.team_id == uuid(2))
            .map(|c| c.stat_key.as_str())
            .collect();
        assert_eq!(home_keys, vec!["losses", "matches", "points", "wins"]);
        // exact season+team pairing
        assert!(out.candidates.iter().all(|c| c.season_id == uuid(1)));
        assert!(out.candidates.iter().any(|c| c.team_id == uuid(3)));
        // numeric verbatim
        let wins = out
            .candidates
            .iter()
            .find(|c| c.stat_key == "wins")
            .unwrap();
        assert_eq!(wins.value, "3");
    }

    #[test]
    fn missing_evidence_is_rejected_not_invented() {
        // nincs season_stats
        assert_eq!(
            classify_match(uuid(1), uuid(2), uuid(3), None).rejected_missing_evidence,
            1
        );
        // nem objektum
        let arr = serde_json::json!([]);
        assert_eq!(
            classify_match(uuid(1), uuid(2), uuid(3), Some(&arr)).rejected_missing_evidence,
            1
        );
        // home hiányzik -> 1, away feldolgozva
        let only_away = serde_json::json!({"away": {"wins": 1}});
        let out = classify_match(uuid(1), uuid(2), uuid(3), Some(&only_away));
        assert_eq!(out.rejected_missing_evidence, 1);
        assert_eq!(out.candidates.len(), 1);
        // home nem objektum
        let bad_home = serde_json::json!({"home": 5, "away": {"wins": 1}});
        assert_eq!(
            classify_match(uuid(1), uuid(2), uuid(3), Some(&bad_home)).rejected_missing_evidence,
            1
        );
    }
}
