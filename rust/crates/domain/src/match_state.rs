use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::verification::Finding;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchStatus {
    Scheduled,
    Confirmed,
    Live,
    Halftime,
    Finished,
    Postponed,
    Canceled,
    Abandoned,
    Awarded,
}

impl MatchStatus {
    pub fn requires_result(self) -> bool {
        matches!(self, Self::Finished | Self::Awarded)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchResultSnapshot {
    pub home_ft: Option<i16>,
    pub away_ft: Option<i16>,
    pub home_ht: Option<i16>,
    pub away_ht: Option<i16>,
    pub home_et: Option<i16>,
    pub away_et: Option<i16>,
    pub home_pens: Option<i16>,
    pub away_pens: Option<i16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchSnapshot {
    pub id: Uuid,
    pub home_team_id: Uuid,
    pub away_team_id: Uuid,
    pub kickoff_at: DateTime<Utc>,
    pub status: MatchStatus,
    pub result: Option<MatchResultSnapshot>,
}

pub fn result_consistency_findings(snapshot: &MatchSnapshot) -> Vec<Finding> {
    let Some(result) = snapshot.result.as_ref() else {
        if snapshot.status.requires_result() {
            return vec![Finding::blocker(
                "MATCH_RESULT_REQUIRED",
                "result",
                "Lezárt meccshez hiányzik az érvényes eredmény.",
            )];
        }
        return vec![];
    };

    let mut findings = vec![];

    if snapshot.status.requires_result() && (result.home_ft.is_none() || result.away_ft.is_none()) {
        findings.push(Finding::blocker(
            "MATCH_RESULT_REQUIRED",
            "result",
            "Lezárt meccshez hiányzik a rendes játékidejű eredmény.",
        ));
    }

    if let (Some(home_ht), Some(home_ft)) = (result.home_ht, result.home_ft) {
        if home_ht > home_ft {
            findings.push(Finding::blocker(
                "HT_GREATER_THAN_FT",
                "home_ht",
                "A hazai félidőeredmény nem lehet nagyobb a rendes játékidejű eredménynél.",
            ));
        }
    }

    if let (Some(away_ht), Some(away_ft)) = (result.away_ht, result.away_ft) {
        if away_ht > away_ft {
            findings.push(Finding::blocker(
                "HT_GREATER_THAN_FT",
                "away_ht",
                "A vendég félidőeredmény nem lehet nagyobb a rendes játékidejű eredménynél.",
            ));
        }
    }

    if (result.home_pens.is_some() || result.away_pens.is_some())
        && (result.home_et.is_none() || result.away_et.is_none())
    {
        findings.push(Finding::blocker(
            "PENS_WITHOUT_ET",
            "penalties",
            "Tizenegyeseredmény hosszabbítási eredmény nélkül nem értelmezhető.",
        ));
    }

    findings
}
