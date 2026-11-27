use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use football_storage::object_store::ObjectStore;
use football_storage::repositories::worker::{RawCapture, persist_raw};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::adapter::Observation;
use crate::observe::insert_observation;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureCompetition {
    pub id: Uuid,
    pub name: String,
    pub country_code: String,
    pub country_name: String,
    #[serde(default = "default_tier")]
    pub tier: i16,
    #[serde(default = "default_priority")]
    pub priority: i32,
}

fn default_tier() -> i16 {
    1
}

fn default_priority() -> i32 {
    100
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureOdds {
    pub home: f64,
    pub draw: f64,
    pub away: f64,
    pub btts_yes: f64,
    pub btts_no: f64,
    pub over25: f64,
    pub under25: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureStanding {
    pub team_source_entity_id: String,
    pub position: i16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureSeason {
    pub id: Uuid,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureTeam {
    pub id: Uuid,
    pub source_entity_id: String,
    pub name: String,
    pub country_code: String,
    pub gender: String,
    pub age_group: String,
    pub branch: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureResult {
    pub home_ft: Option<i16>,
    pub away_ft: Option<i16>,
    pub home_ht: Option<i16>,
    pub away_ht: Option<i16>,
    pub home_et: Option<i16>,
    pub away_et: Option<i16>,
    pub home_pens: Option<i16>,
    pub away_pens: Option<i16>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureMatch {
    pub id: Uuid,
    pub source_match_id: String,
    pub home_source_entity_id: String,
    pub away_source_entity_id: String,
    pub round_label: Option<String>,
    pub kickoff_at: DateTime<Utc>,
    pub competition_timezone: String,
    pub status: String,
    pub result: Option<FixtureResult>,
    #[serde(default)]
    pub odds: Option<FixtureOdds>,
    #[serde(default)]
    pub market_count: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixtureSource {
    pub source_code: String,
    pub source_display_name: String,
    pub captured_at: DateTime<Utc>,
    pub endpoint_key: String,
    pub request_url: String,
    pub adapter_version: String,
    pub parser_version: String,
    pub competition: FixtureCompetition,
    pub season: FixtureSeason,
    pub teams: Vec<FixtureTeam>,
    pub matches: Vec<FixtureMatch>,
    #[serde(default)]
    pub standings: Option<Vec<FixtureStanding>>,
}

#[derive(Debug, Clone)]
pub struct IngestReport {
    pub source_id: Uuid,
    pub capture_id: Uuid,
    pub observation_ids: Vec<Uuid>,
}

pub struct FixtureAdapter;

impl FixtureAdapter {
    pub async fn ingest(pool: &PgPool, store: &ObjectStore, path: &Path) -> Result<IngestReport> {
        let payload =
            std::fs::read(path).with_context(|| format!("fixture olvasása: {}", path.display()))?;
        let fixture: FixtureSource = serde_json::from_slice(&payload)?;
        let artifact = store.put_json(&payload)?;

        let request_key = format!(
            "{}:{}:{}",
            fixture.source_code, fixture.endpoint_key, fixture.captured_at
        );
        let (source_id, capture_id) = persist_raw(
            pool,
            &fixture.source_code,
            &fixture.source_display_name,
            &artifact,
            &RawCapture {
                endpoint_key: &fixture.endpoint_key,
                request_url: &fixture.request_url,
                request_key: &request_key,
                requested_at: fixture.captured_at,
                received_at: fixture.captured_at,
                http_status: 200,
                adapter_version: &fixture.adapter_version,
                parser_version: &fixture.parser_version,
            },
            "application/json",
        )
        .await
        .context("fixture nyers megőrzése")?;

        let observations = build_observations(&fixture)?;
        let mut observation_ids = Vec::new();
        for obs in observations {
            let observation_id = insert_observation(
                pool,
                source_id,
                capture_id,
                &obs.entity_kind,
                &obs.aspect,
                &obs.natural_key,
                &obs.source_record_key,
                obs.observed_at,
                obs.payload,
            )
            .await?;
            observation_ids.push(observation_id);
        }

        Ok(IngestReport {
            source_id,
            capture_id,
            observation_ids,
        })
    }
}

/// A fixture → staging observationök (tiszta építés, persist nélkül).
/// A `match_core` payload a verseny/szezon-meta-t is hordozza, hogy a
/// kanonikus út (`NormalizedDay::from_observations`) önálló legyen.
pub fn build_observations(fixture: &FixtureSource) -> Result<Vec<Observation>> {
    let mut observations = Vec::new();

    for team in &fixture.teams {
        observations.push(Observation {
            entity_kind: "team".into(),
            aspect: "team_identity".into(),
            natural_key: team.id.to_string(),
            source_record_key: team.source_entity_id.clone(),
            observed_at: fixture.captured_at,
            payload: serde_json::to_value(team)?,
        });
    }

    let competition = serde_json::json!({
        "id": fixture.competition.id,
        "source_entity_id": null,
        "name": fixture.competition.name,
        "country_code": fixture.competition.country_code,
        "country_name": fixture.competition.country_name,
        "tier": fixture.competition.tier,
        "priority": fixture.competition.priority,
    });
    let season = serde_json::json!({
        "id": fixture.season.id,
        "label": fixture.season.label,
    });

    for fixture_match in &fixture.matches {
        let payload = serde_json::json!({
            "canonical_match_id": fixture_match.id,
            "home_team_id": fixture.teams.iter()
                .find(|t| t.source_entity_id == fixture_match.home_source_entity_id)
                .map(|t| t.id),
            "away_team_id": fixture.teams.iter()
                .find(|t| t.source_entity_id == fixture_match.away_source_entity_id)
                .map(|t| t.id),
            "source_home_id": fixture_match.home_source_entity_id,
            "source_away_id": fixture_match.away_source_entity_id,
            "kickoff_at": fixture_match.kickoff_at,
            "status": fixture_match.status,
            "result": fixture_match.result,
            "round_label": fixture_match.round_label,
            "competition_timezone": fixture_match.competition_timezone,
            "competition": competition,
            "season": season,
            "source_code": fixture.source_code,
            "captured_at": fixture.captured_at,
        });
        observations.push(Observation {
            entity_kind: "match".into(),
            aspect: "match_core".into(),
            natural_key: fixture_match.id.to_string(),
            source_record_key: fixture_match.source_match_id.clone(),
            observed_at: fixture.captured_at,
            payload,
        });
    }

    for fixture_match in &fixture.matches {
        if let Some(odds) = &fixture_match.odds {
            let payload = serde_json::json!({
                "canonical_match_id": fixture_match.id,
                "odds": odds,
                "market_count": fixture_match.market_count,
            });
            observations.push(Observation {
                entity_kind: "match".into(),
                aspect: "match_odds".into(),
                natural_key: fixture_match.id.to_string(),
                source_record_key: fixture_match.source_match_id.clone(),
                observed_at: fixture.captured_at,
                payload,
            });
        }
    }

    if let Some(standings) = &fixture.standings {
        for standing in standings {
            let team = fixture
                .teams
                .iter()
                .find(|team| team.source_entity_id == standing.team_source_entity_id)
                .with_context(|| {
                    format!(
                        "ismeretlen csapatforrás a tabellában: {}",
                        standing.team_source_entity_id
                    )
                })?;
            let payload = serde_json::json!({
                "team_id": team.id,
                "position": standing.position,
                "season_id": fixture.season.id,
            });
            observations.push(Observation {
                entity_kind: "team".into(),
                aspect: "standing".into(),
                natural_key: team.id.to_string(),
                source_record_key: team.source_entity_id.clone(),
                observed_at: fixture.captured_at,
                payload,
            });
        }
    }

    Ok(observations)
}
