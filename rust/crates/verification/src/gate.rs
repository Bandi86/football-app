use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use football_domain::{PublishState, VerificationReport, evaluate_publish_state};
use sqlx::PgPool;
use uuid::Uuid;

pub fn local_publish_date(kickoff_at: DateTime<Utc>, timezone: &str) -> Result<NaiveDate> {
    let tz: chrono_tz::Tz = timezone.parse()?;
    Ok(kickoff_at.with_timezone(&tz).date_naive())
}

pub async fn evaluate_and_persist_gate(
    pool: &PgPool,
    match_id: Uuid,
    publish_date: NaiveDate,
    report: &VerificationReport,
    rule_set_version: &str,
) -> Result<PublishState> {
    let state = evaluate_publish_state(report);
    let blockers = serde_json::to_value(
        report
            .findings
            .iter()
            .filter(|f| f.severity == football_domain::Severity::Blocker)
            .collect::<Vec<_>>(),
    )?;

    sqlx::query(
        "INSERT INTO ops.publish_gate
         (id, match_id, publish_date, state, rule_set_version, blockers, evaluated_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7)
         ON CONFLICT (match_id, publish_date) DO UPDATE SET
           state = EXCLUDED.state,
           rule_set_version = EXCLUDED.rule_set_version,
           blockers = EXCLUDED.blockers,
           evaluated_at = EXCLUDED.evaluated_at",
    )
    .bind(Uuid::now_v7())
    .bind(match_id)
    .bind(publish_date)
    .bind(state.as_db_str())
    .bind(rule_set_version)
    .bind(blockers)
    .bind(Utc::now())
    .execute(pool)
    .await?;

    Ok(state)
}
