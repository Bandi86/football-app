//! Offline visszatöltés: research `api/v1/matches/**` dokumentumok gazdag
//! szekciói a `fact.match_insight` táblába. Hálózati kérés nélkül, dry-run
//! alapból; a meccs kizárólag a stabil `fm:` source-ref úton kötődik.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use football_ingest::research_insight::{insight_payload, link_payload};
use football_storage::repositories::match_insight;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct InsightReport {
    pub files: usize,
    pub matches: usize,
    pub written: usize,
    pub skipped: Vec<String>,
    pub applied: bool,
}

fn detail_dir(api_root: &Path, date: NaiveDate) -> PathBuf {
    api_root
        .join("matches")
        .join(format!("{:04}", date.year()))
        .join(format!("{:02}", date.month()))
        .join(format!("{:02}", date.day()))
}

pub async fn run(
    pool: &PgPool,
    api_root: &Path,
    from: NaiveDate,
    to: NaiveDate,
    apply: bool,
) -> Result<InsightReport> {
    let source_id: Uuid = sqlx::query_scalar("SELECT id FROM ops.source WHERE code='research'")
        .fetch_one(pool)
        .await
        .context("research forrás hiányzik")?;
    let mut report = InsightReport {
        applied: apply,
        ..Default::default()
    };
    let mut date = from;
    loop {
        let dir = detail_dir(api_root, date);
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
        for path in files {
            let bytes = std::fs::read(&path)
                .with_context(|| format!("detail olvasása: {}", path.display()))?;
            let doc: serde_json::Value = serde_json::from_slice(&bytes)
                .with_context(|| format!("{} JSON", path.display()))?;
            report.files += 1;
            let Some(uid) = doc.get("id").and_then(|v| v.as_str()) else {
                report.skipped.push(format!("{}: nincs id", path.display()));
                continue;
            };
            let Some(payload) = insight_payload(&doc) else {
                report.skipped.push(format!("{uid}: nincs insight szekció"));
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
            let captured_at: DateTime<Utc> = payload
                .get("captured_at")
                .and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(Utc::now);
            report.matches += 1;
            if apply {
                match_insight::upsert(pool, match_id, source_id, captured_at, &payload).await?;
                if let Some(links) = link_payload(&doc) {
                    football_storage::repositories::match_link::upsert(
                        pool,
                        match_id,
                        source_id,
                        captured_at,
                        &links,
                    )
                    .await?;
                }
                report.written += 1;
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
