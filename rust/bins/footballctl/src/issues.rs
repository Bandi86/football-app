//! `footballctl issues detect` — napi hiányjelek rögzítése.
//! A detektor csak `ops.issue`-t ír; a kanonikus adatot nem érinti.

use anyhow::Result;
use chrono::NaiveDate;
use football_domain::ops::{IssueCode, IssueSource};
use football_storage::repositories::detect::{self, DetectedIssue};
use football_storage::repositories::issue::{self, IssueFilter, IssueScope, NewIssue};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

fn managed_source(source: &str) -> bool {
    matches!(source, "daily-check" | "integrity")
}

fn to_new(issue: &DetectedIssue) -> NewIssue {
    NewIssue {
        subject_kind: issue.subject_kind,
        subject_id: issue.subject_id,
        code: issue.code,
        severity: issue.severity,
        source: IssueSource::Integrity,
        detail: issue.detail.clone(),
    }
}

/// Dry-run: csak a magyar összegzés. `--apply`: upsert + lezárás + audit.
pub async fn run_detect(pool: &PgPool, date: NaiveDate, apply: bool) -> Result<()> {
    let detected = detect::detect_day(pool, date).await?;
    let mut counts: Vec<(IssueCode, usize)> = Vec::new();
    for issue in &detected {
        if let Some(entry) = counts.iter_mut().find(|(code, _)| *code == issue.code) {
            entry.1 += 1;
        } else {
            counts.push((issue.code, 1));
        }
    }
    println!("{date} — detektált hiányjelek: {}", detected.len());
    for (code, count) in &counts {
        println!("  {}: {}", code.as_db_str(), count);
    }
    if !apply {
        println!("dry-run — a rögzítéshez --apply kell");
        return Ok(());
    }

    let mut written = 0usize;
    for issue in &detected {
        issue::upsert_seen(pool, &to_new(issue)).await?;
        written += 1;
    }

    // A már nem detektált, általunk írt nyitott jelek lezárása — az idővonal
    // miatt a sor megmarad. A kézi/ügynöki jeleket nem érintjük.
    let open = issue::list(
        pool,
        &IssueFilter {
            scope: IssueScope::Day(date),
            timezone: "Europe/Budapest".to_owned(),
            open_only: true,
            ..IssueFilter::default()
        },
    )
    .await?;
    let current: std::collections::HashSet<(String, Uuid, String)> =
        detected.iter().map(|issue| issue.key()).collect();
    let mut resolved = Vec::new();
    for row in open {
        if !managed_source(&row.source) {
            continue;
        }
        let key = (row.subject_kind.clone(), row.subject_id, row.code.clone());
        if !current.contains(&key) && issue::resolve(pool, row.id).await? {
            resolved.push(row.id);
        }
    }

    let audit_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO ops.audit_event (id, actor, action, subject_kind, subject_id, detail)
         VALUES ($1, 'detect', 'issues_detected', 'day', NULL, $2)",
    )
    .bind(audit_id)
    .bind(json!({
        "date": date,
        "detected": written,
        "resolved": resolved.len(),
        "by_code": counts.iter().map(|(code, count)| (code.as_db_str(), count)).collect::<Vec<_>>()
    }))
    .execute(pool)
    .await?;

    println!(
        "rögzítve: {written} jel, lezárva: {}, audit: {audit_id}",
        resolved.len()
    );
    Ok(())
}
