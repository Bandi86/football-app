//! Tartós üzemeltetési hiányjelek. A `validation_finding` nyers szabályeredmény,
//! az `issue` futásokon átívelő életciklus: mióta látszik, mikor látták utoljára,
//! megoldva-e. A detector csak `upsert_seen`-tel ír.

use anyhow::{Result, anyhow};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, query_builder::QueryBuilder};
use uuid::Uuid;

use football_domain::ops::{IssueCode, IssueSeverity, IssueSource, IssueSubjectKind};

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, utoipa::ToSchema)]
pub struct IssueRow {
    pub id: Uuid,
    pub subject_kind: String,
    pub subject_id: Uuid,
    pub code: String,
    pub severity: String,
    pub detail: Value,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub source: String,
}

#[derive(Debug, Clone)]
pub enum IssueScope {
    /// A napi lista napja (Europe/Budapest szerint), csak meccs-tárgyú jelek.
    Day(NaiveDate),
    All,
}

#[derive(Debug, Clone)]
pub struct IssueFilter {
    pub scope: IssueScope,
    pub timezone: String,
    pub code: Option<IssueCode>,
    pub severity: Option<IssueSeverity>,
    pub open_only: bool,
}

impl Default for IssueFilter {
    fn default() -> Self {
        Self {
            scope: IssueScope::All,
            timezone: "Europe/Budapest".to_owned(),
            code: None,
            severity: None,
            open_only: false,
        }
    }
}

pub async fn list(pool: &PgPool, filter: &IssueFilter) -> Result<Vec<IssueRow>> {
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT i.id, i.subject_kind, i.subject_id, i.code, i.severity, i.detail,
                i.first_seen_at, i.last_seen_at, i.resolved_at, i.source
           FROM ops.issue i",
    );
    if let IssueScope::Day(date) = &filter.scope {
        query.push(
            " JOIN core.match m ON m.id = i.subject_id AND i.subject_kind = 'match'
               WHERE (m.kickoff_at AT TIME ZONE ",
        );
        query.push_bind(&filter.timezone);
        query.push(")::date = ");
        query.push_bind(date);
    } else {
        query.push(" WHERE true");
    }
    if let Some(code) = filter.code {
        query.push(" AND i.code = ").push_bind(code.as_db_str());
    }
    if let Some(severity) = filter.severity {
        query
            .push(" AND i.severity = ")
            .push_bind(severity.as_db_str());
    }
    if filter.open_only {
        query.push(" AND i.resolved_at IS NULL");
    }
    query.push(" ORDER BY i.last_seen_at DESC, i.id");
    Ok(query.build_query_as::<IssueRow>().fetch_all(pool).await?)
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<IssueRow>> {
    Ok(sqlx::query_as::<_, IssueRow>(
        "SELECT id, subject_kind, subject_id, code, severity, detail,
                first_seen_at, last_seen_at, resolved_at, source
           FROM ops.issue WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

#[derive(Debug, Clone)]
pub struct NewIssue {
    pub subject_kind: IssueSubjectKind,
    pub subject_id: Uuid,
    pub code: IssueCode,
    pub severity: IssueSeverity,
    pub source: IssueSource,
    pub detail: Value,
}

/// Ugyanaz a nyitott jel nem duplikálódik: a részleges unique index miatt
/// `first_seen_at` megmarad, a `last_seen_at` és a részlet frissül.
pub async fn upsert_seen(pool: &PgPool, issue: &NewIssue) -> Result<Uuid> {
    let id = Uuid::now_v7();
    let row: Uuid = sqlx::query_scalar(
        "INSERT INTO ops.issue
            (id, subject_kind, subject_id, code, severity, detail, source,
             first_seen_at, last_seen_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, now(), now())
         ON CONFLICT (subject_kind, subject_id, code, source) WHERE resolved_at IS NULL
         DO UPDATE SET severity = EXCLUDED.severity,
                       detail = EXCLUDED.detail,
                       last_seen_at = now()
         RETURNING id",
    )
    .bind(id)
    .bind(issue.subject_kind.as_db_str())
    .bind(issue.subject_id)
    .bind(issue.code.as_db_str())
    .bind(issue.severity.as_db_str())
    .bind(&issue.detail)
    .bind(issue.source.as_db_str())
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// A jel lezárása; a megoldott sor megmarad az idővonal kedvéért.
pub async fn resolve(pool: &PgPool, id: Uuid) -> Result<bool> {
    let affected = sqlx::query(
        "UPDATE ops.issue SET resolved_at = now()
          WHERE id = $1 AND resolved_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(affected > 0)
}

/// A DB CHECK ellenére is tipizált hiba, ha egy sor mégis ismeretlen kóddal jön.
pub fn parse_row(
    row: &IssueRow,
) -> Result<(IssueSubjectKind, IssueCode, IssueSeverity, IssueSource)> {
    Ok((
        IssueSubjectKind::parse(&row.subject_kind)
            .ok_or_else(|| anyhow!("ismeretlen issue subject: {}", row.subject_kind))?,
        IssueCode::parse(&row.code).ok_or_else(|| anyhow!("ismeretlen issue kód: {}", row.code))?,
        IssueSeverity::parse(&row.severity)
            .ok_or_else(|| anyhow!("ismeretlen issue severity: {}", row.severity))?,
        IssueSource::parse(&row.source)
            .ok_or_else(|| anyhow!("ismeretlen issue forrás: {}", row.source))?,
    ))
}
