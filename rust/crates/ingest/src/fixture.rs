use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use football_storage::object_store::ObjectStore;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use sqlx::PgPool;
use uuid::Uuid;

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

        let source_id =
            upsert_source(pool, &fixture.source_code, &fixture.source_display_name).await?;

        let artifact_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO raw.artifact
             (id, content_sha256, compression, byte_size, storage_key, media_type)
             VALUES ($1, $2, $3, $4, $5, 'application/json')
             ON CONFLICT (content_sha256) DO UPDATE SET content_sha256 = EXCLUDED.content_sha256
             RETURNING id",
        )
        .bind(Uuid::now_v7())
        .bind(&artifact.sha256)
        .bind(artifact.compression)
        .bind(artifact.byte_size as i64)
        .bind(&artifact.storage_key)
        .fetch_one(pool)
        .await?;

        let request_key = format!(
            "{}:{}:{}",
            fixture.source_code, fixture.endpoint_key, fixture.captured_at
        );
        let capture_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO raw.capture
             (id, source_id, artifact_id, endpoint_key, request_url, requested_at,
              received_at, http_status, adapter_version, parser_version, request_key)
             VALUES ($1,$2,$3,$4,$5,$6,$7,200,$8,$9,$10)
             ON CONFLICT (source_id, request_key) DO NOTHING",
        )
        .bind(capture_id)
        .bind(source_id)
        .bind(artifact_id)
        .bind(&fixture.endpoint_key)
        .bind(&fixture.request_url)
        .bind(fixture.captured_at)
        .bind(fixture.captured_at)
        .bind(&fixture.adapter_version)
        .bind(&fixture.parser_version)
        .bind(&request_key)
        .execute(pool)
        .await?;

        let capture_id: Uuid = sqlx::query_scalar(
            "SELECT id FROM raw.capture WHERE source_id = $1 AND request_key = $2",
        )
        .bind(source_id)
        .bind(&request_key)
        .fetch_one(pool)
        .await?;

        let mut observation_ids = Vec::new();

        for team in &fixture.teams {
            let observation_id = insert_observation(
                pool,
                source_id,
                capture_id,
                "team",
                "team_identity",
                &team.id.to_string(),
                &team.source_entity_id,
                fixture.captured_at,
                serde_json::to_value(team)?,
            )
            .await?;
            observation_ids.push(observation_id);
        }

        for fixture_match in &fixture.matches {
            let home = fixture
                .teams
                .iter()
                .find(|team| team.source_entity_id == fixture_match.home_source_entity_id)
                .with_context(|| {
                    format!(
                        "ismeretlen hazai forrásentitás: {}",
                        fixture_match.home_source_entity_id
                    )
                })?;
            let away = fixture
                .teams
                .iter()
                .find(|team| team.source_entity_id == fixture_match.away_source_entity_id)
                .with_context(|| {
                    format!(
                        "ismeretlen vendég forrásentitás: {}",
                        fixture_match.away_source_entity_id
                    )
                })?;

            let payload = serde_json::json!({
                "canonical_match_id": fixture_match.id,
                "home_team_id": home.id,
                "away_team_id": away.id,
                "source_home_id": fixture_match.home_source_entity_id,
                "source_away_id": fixture_match.away_source_entity_id,
                "kickoff_at": fixture_match.kickoff_at,
                "status": fixture_match.status,
                "result": fixture_match.result,
                "round_label": fixture_match.round_label,
                "competition_timezone": fixture_match.competition_timezone,
            });
            let observation_id = insert_observation(
                pool,
                source_id,
                capture_id,
                "match",
                "match_core",
                &fixture_match.id.to_string(),
                &fixture_match.source_match_id,
                fixture.captured_at,
                payload,
            )
            .await?;
            observation_ids.push(observation_id);
        }

        for fixture_match in &fixture.matches {
            if let Some(odds) = &fixture_match.odds {
                let payload = serde_json::json!({
                    "canonical_match_id": fixture_match.id,
                    "odds": odds,
                    "market_count": fixture_match.market_count,
                });
                let observation_id = insert_observation(
                    pool,
                    source_id,
                    capture_id,
                    "match",
                    "match_odds",
                    &fixture_match.id.to_string(),
                    &fixture_match.source_match_id,
                    fixture.captured_at,
                    payload,
                )
                .await?;
                observation_ids.push(observation_id);
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
                let observation_id = insert_observation(
                    pool,
                    source_id,
                    capture_id,
                    "team",
                    "standing",
                    &team.id.to_string(),
                    &team.source_entity_id,
                    fixture.captured_at,
                    payload,
                )
                .await?;
                observation_ids.push(observation_id);
            }
        }

        Ok(IngestReport {
            source_id,
            capture_id,
            observation_ids,
        })
    }
}

async fn upsert_source(pool: &PgPool, code: &str, name: &str) -> Result<Uuid> {
    sqlx::query_scalar(
        "INSERT INTO ops.source (id, code, display_name)
         VALUES ($1, $2, $3)
         ON CONFLICT (code) DO UPDATE SET display_name = EXCLUDED.display_name
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(code)
    .bind(name)
    .fetch_one(pool)
    .await
    .context("forrás upsert")
}

#[allow(clippy::too_many_arguments)]
async fn insert_observation(
    pool: &PgPool,
    source_id: Uuid,
    capture_id: Uuid,
    entity_kind: &str,
    aspect: &str,
    natural_key: &str,
    source_record_key: &str,
    observed_at: DateTime<Utc>,
    normalized_payload: serde_json::Value,
) -> Result<Uuid> {
    let content_bytes = serde_json::to_vec(&normalized_payload)?;
    let content_hash = hex::encode(sha2::Sha256::digest(&content_bytes));
    let id = Uuid::now_v7();

    sqlx::query(
        "INSERT INTO stage.observation
         (id, source_id, capture_id, entity_kind, aspect, natural_key,
          source_record_key, observed_at, event_time, normalized_payload,
          schema_version, quality_flags, content_hash)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$8,$9,'1.0','{}',$10)
         ON CONFLICT (source_id, entity_kind, aspect, source_record_key, content_hash)
         DO NOTHING",
    )
    .bind(id)
    .bind(source_id)
    .bind(capture_id)
    .bind(entity_kind)
    .bind(aspect)
    .bind(natural_key)
    .bind(source_record_key)
    .bind(observed_at)
    .bind(normalized_payload)
    .bind(&content_hash)
    .execute(pool)
    .await?;

    sqlx::query_scalar(
        "SELECT id FROM stage.observation
         WHERE source_id=$1 AND entity_kind=$2 AND aspect=$3
           AND source_record_key=$4 AND content_hash=$5",
    )
    .bind(source_id)
    .bind(entity_kind)
    .bind(aspect)
    .bind(source_record_key)
    .bind(content_hash)
    .fetch_one(pool)
    .await
    .context("megfigyelés azonosító")
}
