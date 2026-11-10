use anyhow::Result;
use chrono::Utc;
use football_domain::{Severity, VerificationReport, VerificationStatus};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn persist_report(
    pool: &PgPool,
    match_id: Uuid,
    report: &VerificationReport,
    rule_set_version: &str,
) -> Result<Uuid> {
    let run_id = Uuid::now_v7();
    let outcome = match report.status() {
        VerificationStatus::Verified => "verified",
        VerificationStatus::Provisional | VerificationStatus::Unverified => "provisional",
        VerificationStatus::Blocked => "blocked",
    };

    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO ops.validation_run
         (id, subject_kind, subject_id, rule_set_version, started_at, finished_at, outcome)
         VALUES ($1,'match',$2,$3,$4,$4,$5)",
    )
    .bind(run_id)
    .bind(match_id)
    .bind(rule_set_version)
    .bind(Utc::now())
    .bind(outcome)
    .execute(&mut *tx)
    .await?;

    for finding in &report.findings {
        sqlx::query(
            "INSERT INTO ops.validation_finding
             (id, run_id, rule_key, severity, field_key, message)
             VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(Uuid::now_v7())
        .bind(run_id)
        .bind(finding.rule_key)
        .bind(match_severity(finding.severity))
        .bind(finding.field)
        .bind(&finding.message)
        .execute(&mut *tx)
        .await?;

        if finding.severity == Severity::Blocker {
            sqlx::query(
                "INSERT INTO fact.conflict
                 (id, subject_kind, subject_id, field_key, severity, status, opened_at)
                 VALUES ($1,'match',$2,$3,'material','unresolved',$4)
                 ON CONFLICT (subject_kind, subject_id, field_key) DO UPDATE SET
                   severity='material',
                   status='unresolved',
                   opened_at=EXCLUDED.opened_at,
                   resolved_at=NULL,
                   resolution_reason=NULL",
            )
            .bind(Uuid::now_v7())
            .bind(match_id)
            .bind(finding.field)
            .bind(Utc::now())
            .execute(&mut *tx)
            .await?;
        }
    }

    for field in [
        "home_team_id",
        "away_team_id",
        "kickoff_at",
        "status",
        "result",
    ] {
        let severity = report
            .findings
            .iter()
            .filter(|f| f.field == field)
            .map(|f| f.severity)
            .max()
            .unwrap_or(Severity::Info);
        let status = match severity {
            Severity::Blocker => "blocked",
            Severity::Warning => "provisional",
            Severity::Info => "verified",
        };

        sqlx::query(
            "INSERT INTO fact.verification
             (subject_kind, subject_id, field_key, rule_set_version, status, severity, last_run_id)
             VALUES ('match',$1,$2,$3,$4,$5,$6)
             ON CONFLICT (subject_kind, subject_id, field_key) DO UPDATE SET
               rule_set_version=EXCLUDED.rule_set_version,
               status=EXCLUDED.status,
               severity=EXCLUDED.severity,
               last_run_id=EXCLUDED.last_run_id,
               updated_at=now()",
        )
        .bind(match_id)
        .bind(field)
        .bind(rule_set_version)
        .bind(status)
        .bind(match_severity(severity))
        .bind(run_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(run_id)
}

fn match_severity(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Blocker => "blocker",
    }
}
