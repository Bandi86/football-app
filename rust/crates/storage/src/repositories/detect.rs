//! Napi hiányjel-detektor: a `fetch_day_gaps` meccs-szintű megfelelője.
//! Csak jeleket számol — kanonikus adatot nem ír, és nem fuzzy-egyeztet.

use std::collections::HashSet;

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use football_domain::ops::{IssueCode, IssueSeverity, IssueSubjectKind};
use serde_json::json;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DetectedIssue {
    pub subject_kind: IssueSubjectKind,
    pub subject_id: Uuid,
    pub code: IssueCode,
    pub severity: IssueSeverity,
    pub detail: serde_json::Value,
}

impl DetectedIssue {
    /// Ugyanaz a natural key, mint az `ops.issue` részleges unique indexe.
    pub fn key(&self) -> (String, Uuid, String) {
        (
            self.subject_kind.as_db_str().to_owned(),
            self.subject_id,
            self.code.as_db_str().to_owned(),
        )
    }
}

#[derive(Debug, FromRow)]
struct GapRow {
    id: Uuid,
    kickoff_at: DateTime<Utc>,
    kickoff_precision: String,
    status: String,
    result_missing: bool,
    has_home_logo: bool,
    has_away_logo: bool,
    has_comp_logo: bool,
    has_odds: bool,
}

/// Az adott nap publikálható meccseinek hiányai. A küszöbök és a logo/odds
/// szabályok pontosan a `day.rs::fetch_day_gaps`-ból származnak.
pub async fn detect_day(pool: &PgPool, date: NaiveDate) -> Result<Vec<DetectedIssue>> {
    let rows = sqlx::query_as::<_, GapRow>(
        "SELECT m.id, m.kickoff_at, m.kickoff_precision, m.status,
                (r.home_ft IS NULL) AS result_missing,
                hl.logo IS NOT NULL AS has_home_logo,
                al.logo IS NOT NULL AS has_away_logo,
                cl.logo IS NOT NULL AS has_comp_logo,
                EXISTS (SELECT 1 FROM fact.odds_snapshot o WHERE o.match_id = m.id) AS has_odds
           FROM core.match m
           JOIN core.competition c ON c.id = m.competition_id
           JOIN ops.publish_gate g ON g.match_id = m.id AND g.publish_date = $1
           LEFT JOIN core.match_result r ON r.match_id = m.id
           LEFT JOIN LATERAL (
               SELECT COALESCE(a.storage_key, a.url) AS logo FROM ops.asset a
                WHERE a.entity_kind='team' AND a.kind='logo' AND a.entity_id=m.home_team_id
                ORDER BY (a.storage_key IS NULL), a.captured_at DESC LIMIT 1
           ) hl ON true
           LEFT JOIN LATERAL (
               SELECT COALESCE(a.storage_key, a.url) AS logo FROM ops.asset a
                WHERE a.entity_kind='team' AND a.kind='logo' AND a.entity_id=m.away_team_id
                ORDER BY (a.storage_key IS NULL), a.captured_at DESC LIMIT 1
           ) al ON true
           LEFT JOIN LATERAL (
               SELECT COALESCE(a.storage_key, a.url) AS logo FROM ops.asset a
                WHERE a.entity_kind='competition' AND a.kind='logo' AND a.entity_id=c.entity_id
                ORDER BY (a.storage_key IS NULL), a.captured_at DESC LIMIT 1
           ) cl ON true
          WHERE g.state IN ('verified_publish','provisional_publish')
            AND NOT EXISTS (SELECT 1 FROM ops.editorial_hold h WHERE h.match_id=m.id)",
    )
    .bind(date)
    .fetch_all(pool)
    .await?;

    let mut found = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |issue: DetectedIssue| {
        if seen.insert(issue.key()) {
            found.push(issue);
        }
    };

    for row in rows {
        let match_issue = |code, severity, detail| DetectedIssue {
            subject_kind: IssueSubjectKind::Match,
            subject_id: row.id,
            code,
            severity,
            detail: json!(detail),
        };
        if !row.has_odds {
            push(match_issue(
                IssueCode::MissingOdds,
                IssueSeverity::Warning,
                json!({ "checked": "odds_snapshot", "date": date }),
            ));
        }
        if !row.has_home_logo || !row.has_away_logo || !row.has_comp_logo {
            push(match_issue(
                IssueCode::MissingLogo,
                IssueSeverity::Info,
                json!({
                    "home": !row.has_home_logo,
                    "away": !row.has_away_logo,
                    "competition": !row.has_comp_logo
                }),
            ));
        }
        if row.kickoff_precision != "date_only"
            && !matches!(
                row.status.as_str(),
                "finished" | "postponed" | "canceled" | "abandoned" | "awarded"
            )
            && row.kickoff_at
                < Utc::now()
                    - chrono::Duration::minutes(
                        if matches!(row.status.as_str(), "live" | "halftime") {
                            150
                        } else {
                            10
                        },
                    )
        {
            push(match_issue(
                IssueCode::OpenPast,
                IssueSeverity::Warning,
                json!({ "status": row.status, "kickoff_at": row.kickoff_at }),
            ));
        }
        if row.status == "finished" && row.result_missing {
            push(match_issue(
                IssueCode::StaleStatus,
                IssueSeverity::Hard,
                json!({ "status": row.status, "result": "missing" }),
            ));
        }
    }

    // Visszatartott publikáció: forrásütközés jele.
    let withheld: Vec<Uuid> = sqlx::query_scalar(
        "SELECT match_id FROM ops.publish_gate
          WHERE publish_date = $1 AND state = 'withheld' AND match_id IS NOT NULL",
    )
    .bind(date)
    .fetch_all(pool)
    .await?;
    for id in withheld {
        push(DetectedIssue {
            subject_kind: IssueSubjectKind::Match,
            subject_id: id,
            code: IssueCode::SourceConflict,
            severity: IssueSeverity::Hard,
            detail: json!({ "gate": "withheld", "date": date }),
        });
    }

    // Függő identitás-review: a forrás-entitás azonosítója UUID kell legyen.
    let reviews: Vec<(String, String)> = sqlx::query_as(
        "SELECT entity_kind, source_entity_id FROM identity.review_queue
          WHERE status = 'pending'",
    )
    .fetch_all(pool)
    .await?;
    for (kind, raw_id) in reviews {
        let Some(subject_kind) = (match kind.as_str() {
            "team" => Some(IssueSubjectKind::Team),
            "competition" => Some(IssueSubjectKind::Competition),
            _ => None,
        }) else {
            continue;
        };
        let Ok(subject_id) = Uuid::parse_str(&raw_id) else {
            continue;
        };
        push(DetectedIssue {
            subject_kind,
            subject_id,
            code: IssueCode::IdentityReview,
            severity: IssueSeverity::Info,
            detail: json!({ "source_entity_id": raw_id }),
        });
    }

    Ok(found)
}
