//! S108: játékosadatok importja a MÁR TÁROLT helyi research JSON-ekből.
//!
//! Hálózati kérés nincs. A játékosentitás kizárólag stabil forrás-ID-ból
//! (`fm:<player_id>`, `exact_external_id`) jön létre — nincs fuzzy egyeztetés,
//! nincs auto-create szöveges névből. Minden fact-sorhoz tartozik
//! `source_id` és `captured_at` (a research meta.generated_at-je).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate};
use football_ingest::countries::by_english_name;
use football_ingest::research_players::{self, LineupPlayer};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct ImportReport {
    pub files: usize,
    pub matches: usize,
    pub players_created: usize,
    pub lineup_rows: usize,
    pub stat_rows: usize,
    pub shot_rows: usize,
    pub spell_rows: usize,
    pub skipped: Vec<String>,
    pub applied: bool,
}

/// `root` a research `api/v1` könyvtár (FOOTBALL_RESEARCH_API_DIR).
fn detail_dir(root: &Path, date: NaiveDate) -> PathBuf {
    root.join("matches")
        .join(format!("{:04}", date.year()))
        .join(format!("{:02}", date.month()))
        .join(format!("{:02}", date.day()))
}

async fn resolve_player(
    pool: &PgPool,
    source_id: Uuid,
    source_player_id: &str,
    name: &str,
    country_name: Option<&str>,
    apply: bool,
    report: &mut ImportReport,
) -> Result<Option<Uuid>> {
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT entity_id FROM identity.source_entity_ref
          WHERE source_id=$1 AND entity_kind='player' AND source_entity_id=$2",
    )
    .bind(source_id)
    .bind(source_player_id)
    .fetch_optional(pool)
    .await?;
    if let Some(id) = existing {
        return Ok(Some(id));
    }
    if !apply || name.trim().is_empty() {
        return Ok(None);
    }

    let country_id: Option<Uuid> = match country_name.and_then(by_english_name) {
        Some((code, _)) => {
            sqlx::query_scalar("SELECT id FROM core.country WHERE code=$1")
                .bind(code)
                .fetch_optional(pool)
                .await?
        }
        None => None,
    };
    let entity_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'player',$2)")
        .bind(entity_id)
        .bind(name.trim())
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO core.player (entity_id, country_id) VALUES ($1,$2)")
        .bind(entity_id)
        .bind(country_id)
        .execute(pool)
        .await?;
    sqlx::query(
        "INSERT INTO identity.source_entity_ref
           (id, source_id, entity_kind, source_entity_id, entity_id, method)
         VALUES ($1,$2,'player',$3,$4,'exact_external_id')",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(source_player_id)
    .bind(entity_id)
    .execute(pool)
    .await?;
    report.players_created += 1;
    Ok(Some(entity_id))
}

struct LineupImport<'a> {
    pool: &'a PgPool,
    source_id: Uuid,
    match_id: Uuid,
    team_id: Uuid,
    side: &'a str,
    role: &'a str,
    captured_at: chrono::DateTime<chrono::Utc>,
    date: NaiveDate,
    apply: bool,
    report: &'a mut ImportReport,
}

async fn import_lineup(ctx: LineupImport<'_>, players: &[LineupPlayer]) -> Result<()> {
    let LineupImport {
        pool,
        source_id,
        match_id,
        team_id,
        side,
        role,
        captured_at,
        date,
        apply,
        report,
    } = ctx;
    for player in players {
        // A forrás néha `id: 0`-t ad placeholderként; ez nem stabil azonosító,
        // és minden ilyen játékost egy entitásba olvasztana. Fail-closed: kimarad.
        if player.id <= 0 {
            continue;
        }
        let Some(name) = player.name.as_deref().filter(|n| !n.trim().is_empty()) else {
            continue;
        };
        let source_player_id = format!("fm:{}", player.id);
        let entity_id = resolve_player(
            pool,
            source_id,
            &source_player_id,
            name,
            player.country.as_deref(),
            apply,
            report,
        )
        .await?;
        report.lineup_rows += 1;
        if !apply {
            continue;
        }
        let Some(player_entity_id) = entity_id else {
            report
                .skipped
                .push(format!("{date}: {source_player_id} nem oldható fel"));
            continue;
        };
        sqlx::query(
            "INSERT INTO fact.match_lineups
               (match_id, source_id, captured_at, side, source_player_id,
                player_entity_id, role, position, shirt_number, rating)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
             ON CONFLICT (match_id, source_id, side, source_player_id) DO UPDATE SET
               captured_at=EXCLUDED.captured_at, player_entity_id=EXCLUDED.player_entity_id,
               role=EXCLUDED.role, position=EXCLUDED.position,
               shirt_number=EXCLUDED.shirt_number, rating=EXCLUDED.rating",
        )
        .bind(match_id)
        .bind(source_id)
        .bind(captured_at)
        .bind(side)
        .bind(&source_player_id)
        .bind(player_entity_id)
        .bind(role)
        .bind(player.position_id.map(|p| p.to_string()))
        .bind(player.shirt_number)
        .bind(player.rating)
        .execute(pool)
        .await?;

        // S119: a `team_entity_id` FK a core.entity.id-re mutat, de a `team_id`
        // egy core.team.id — régi adatoknál a kettő eltérhet (17 sor ahol
        // team.id != team.entity_id). A tényleges entity_id-t oldjuk fel.
        let team_entity_id: Uuid =
            sqlx::query_scalar("SELECT entity_id FROM core.team WHERE id=$1")
                .bind(team_id)
                .fetch_one(pool)
                .await?;
        sqlx::query(
            "INSERT INTO core.player_team_spell
               (id, player_entity_id, team_entity_id, source_id, valid_from, valid_to, captured_at)
             VALUES ($1,$2,$3,$4,$5,$5,$6)
             ON CONFLICT (player_entity_id, team_entity_id, source_id, valid_from) DO UPDATE SET
               captured_at=EXCLUDED.captured_at",
        )
        .bind(Uuid::now_v7())
        .bind(player_entity_id)
        .bind(team_entity_id)
        .bind(source_id)
        .bind(date)
        .bind(captured_at)
        .execute(pool)
        .await?;
        report.spell_rows += 1;
    }
    Ok(())
}

pub async fn run(
    pool: &PgPool,
    research_root: &Path,
    from: NaiveDate,
    to: NaiveDate,
    apply: bool,
) -> Result<ImportReport> {
    let source_id: Uuid = sqlx::query_scalar("SELECT id FROM ops.source WHERE code='research'")
        .fetch_one(pool)
        .await
        .context("research forrás hiányzik")?;
    let mut report = ImportReport {
        applied: apply,
        ..Default::default()
    };
    let mut date = from;
    loop {
        let dir = detail_dir(research_root, date);
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| {
                p.extension().and_then(|e| e.to_str()) == Some("json")
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.contains("_fm"))
            })
            .collect();
        files.sort();
        if files.is_empty() {
            report
                .skipped
                .push(format!("{date}: nincs _fm detail JSON"));
        }
        for path in files {
            let bytes = std::fs::read(&path)
                .with_context(|| format!("detail JSON olvasása: {}", path.display()))?;
            let detail = match research_players::parse(&bytes) {
                Ok(detail) => detail,
                Err(err) => {
                    report.skipped.push(format!("{}: {err}", path.display()));
                    continue;
                }
            };
            report.files += 1;
            let Some(uid) = detail.id.as_deref() else {
                report.skipped.push(format!("{}: nincs id", path.display()));
                continue;
            };
            let match_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT entity_id FROM identity.source_entity_ref
                  WHERE source_id=$1 AND entity_kind='match' AND source_entity_id=$2",
            )
            .bind(source_id)
            .bind(uid)
            .fetch_optional(pool)
            .await?;
            let Some(match_id) = match_id else {
                report
                    .skipped
                    .push(format!("{uid}: nincs kanonikus meccs-ref"));
                continue;
            };
            // Egy régi source-ref mutathat már összeolvadt meccsre — ilyenkor
            // nincs core.match sor; a fájlt kihagyjuk és a riportban jelezzük.
            let teams: Option<(Uuid, Uuid)> =
                sqlx::query_as("SELECT home_team_id, away_team_id FROM core.match WHERE id=$1")
                    .bind(match_id)
                    .fetch_optional(pool)
                    .await?;
            let Some((home_team_id, away_team_id)) = teams else {
                report
                    .skipped
                    .push(format!("{uid}: a meccs sor már nem létezik"));
                continue;
            };
            let captured_at = detail
                .meta
                .as_ref()
                .and_then(|m| m.generated_at)
                .unwrap_or_else(chrono::Utc::now);
            report.matches += 1;

            if let Some(lineups) = &detail.lineups {
                if let Some(home) = &lineups.home {
                    import_lineup(
                        LineupImport {
                            pool,
                            source_id,
                            match_id,
                            team_id: home_team_id,
                            side: "home",
                            role: "starter",
                            captured_at,
                            date,
                            apply,
                            report: &mut report,
                        },
                        &home.starters,
                    )
                    .await?;
                    import_lineup(
                        LineupImport {
                            pool,
                            source_id,
                            match_id,
                            team_id: home_team_id,
                            side: "home",
                            role: "sub",
                            captured_at,
                            date,
                            apply,
                            report: &mut report,
                        },
                        &home.substitutes,
                    )
                    .await?;
                }
                if let Some(away) = &lineups.away {
                    import_lineup(
                        LineupImport {
                            pool,
                            source_id,
                            match_id,
                            team_id: away_team_id,
                            side: "away",
                            role: "starter",
                            captured_at,
                            date,
                            apply,
                            report: &mut report,
                        },
                        &away.starters,
                    )
                    .await?;
                    import_lineup(
                        LineupImport {
                            pool,
                            source_id,
                            match_id,
                            team_id: away_team_id,
                            side: "away",
                            role: "sub",
                            captured_at,
                            date,
                            apply,
                            report: &mut report,
                        },
                        &away.substitutes,
                    )
                    .await?;
                }
            }

            let home_fm_id = detail
                .lineups
                .as_ref()
                .and_then(|l| l.home.as_ref())
                .and_then(|s| s.team_id);
            let away_fm_id = detail
                .lineups
                .as_ref()
                .and_then(|l| l.away.as_ref())
                .and_then(|s| s.team_id);
            for stat in detail.player_statistics.as_deref().unwrap_or_default() {
                if stat.player_id <= 0 {
                    continue;
                }
                let side = if stat.team_id == home_fm_id && home_fm_id.is_some() {
                    Some("home")
                } else if stat.team_id == away_fm_id && away_fm_id.is_some() {
                    Some("away")
                } else {
                    None
                };
                let Some(side) = side else { continue };
                let source_player_id = format!("fm:{}", stat.player_id);
                let entity_id = resolve_player(
                    pool,
                    source_id,
                    &source_player_id,
                    stat.name.as_deref().unwrap_or(""),
                    None,
                    apply,
                    &mut report,
                )
                .await?;
                for (key, value) in &stat.stats {
                    let Some(number) = research_players::numeric_value(value) else {
                        continue;
                    };
                    report.stat_rows += 1;
                    if !apply {
                        continue;
                    }
                    sqlx::query(
                        "INSERT INTO fact.match_player_stats
                           (match_id, source_id, captured_at, source_player_id,
                            player_entity_id, side, stat_key, value)
                         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
                         ON CONFLICT (match_id, source_id, source_player_id, stat_key) DO UPDATE SET
                           captured_at=EXCLUDED.captured_at,
                           player_entity_id=EXCLUDED.player_entity_id,
                           side=EXCLUDED.side, value=EXCLUDED.value",
                    )
                    .bind(match_id)
                    .bind(source_id)
                    .bind(captured_at)
                    .bind(&source_player_id)
                    .bind(entity_id)
                    .bind(side)
                    .bind(key)
                    .bind(number)
                    .execute(pool)
                    .await?;
                }
            }

            for (index, shot) in detail
                .shotmap
                .as_deref()
                .unwrap_or_default()
                .iter()
                .enumerate()
            {
                let Some(side) = shot
                    .team
                    .as_deref()
                    .filter(|s| *s == "home" || *s == "away")
                else {
                    continue;
                };
                let Some(player_id) = shot.player_id else {
                    continue;
                };
                if player_id <= 0 {
                    continue;
                }
                let source_player_id = format!("fm:{player_id}");
                let entity_id = resolve_player(
                    pool,
                    source_id,
                    &source_player_id,
                    shot.player.as_deref().unwrap_or(""),
                    None,
                    apply,
                    &mut report,
                )
                .await?;
                report.shot_rows += 1;
                if !apply {
                    continue;
                }
                sqlx::query(
                    "INSERT INTO fact.match_shots
                       (match_id, source_id, captured_at, shot_no, side, source_player_id,
                        player_entity_id, minute, x, y, xg, xgot, shot_type)
                     VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
                     ON CONFLICT (match_id, source_id, shot_no) DO UPDATE SET
                       captured_at=EXCLUDED.captured_at, side=EXCLUDED.side,
                       source_player_id=EXCLUDED.source_player_id,
                       player_entity_id=EXCLUDED.player_entity_id, minute=EXCLUDED.minute,
                       x=EXCLUDED.x, y=EXCLUDED.y, xg=EXCLUDED.xg, xgot=EXCLUDED.xgot,
                       shot_type=EXCLUDED.shot_type",
                )
                .bind(match_id)
                .bind(source_id)
                .bind(captured_at)
                .bind((index + 1) as i32)
                .bind(side)
                .bind(&source_player_id)
                .bind(entity_id)
                .bind(shot.minute)
                .bind(shot.x)
                .bind(shot.y)
                .bind(shot.xg)
                .bind(shot.xgot)
                .bind(shot.shot_type.as_deref())
                .execute(pool)
                .await?;
            }

            if apply {
                let detail_json = serde_json::json!({
                    "uid": uid,
                    "lineups": detail.lineups.is_some(),
                    "statistics": detail.player_statistics.as_ref().is_some_and(|v| !v.is_empty()),
                    "shots": detail.shotmap.as_ref().is_some_and(|v| !v.is_empty()),
                    "captured_at": captured_at,
                });
                sqlx::query(
                    "INSERT INTO ops.audit_event (id, actor, action, subject_kind, subject_id, detail)
                     VALUES ($1,'cli','player_import','match',$2,$3)",
                )
                .bind(Uuid::now_v7())
                .bind(match_id)
                .bind(detail_json)
                .execute(pool)
                .await?;
            }
        }
        if date == to {
            break;
        }
        let Some(next) = date.succ_opt() else { break };
        date = next;
    }
    Ok(report)
}
