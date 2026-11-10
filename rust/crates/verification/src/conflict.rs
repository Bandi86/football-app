use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use football_domain::MatchStatus;
use sqlx::PgPool;
use uuid::Uuid;

use crate::engine::SourceMaterialValues;

pub async fn load_material_values(
    pool: &PgPool,
    match_id: Uuid,
) -> Result<Vec<SourceMaterialValues>> {
    let rows = sqlx::query_as::<_, RawSourceValue>(
        "SELECT DISTINCT ON (o.source_id)
            s.code AS source_code,
            o.normalized_payload
         FROM stage.observation o
         JOIN ops.source s ON s.id = o.source_id
         WHERE o.entity_kind = 'match'
           AND o.aspect = 'match_core'
           AND o.natural_key = $1
         ORDER BY o.source_id, o.observed_at DESC",
    )
    .bind(match_id.to_string())
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let payload = row.normalized_payload;
            let home_team_id = parse_uuid(&payload, "home_team_id")?;
            let away_team_id = parse_uuid(&payload, "away_team_id")?;
            let kickoff_at = required_str(&payload, "kickoff_at")?.parse::<DateTime<Utc>>()?;
            let status = match required_str(&payload, "status")? {
                "scheduled" => MatchStatus::Scheduled,
                "confirmed" => MatchStatus::Confirmed,
                "live" => MatchStatus::Live,
                "halftime" => MatchStatus::Halftime,
                "finished" => MatchStatus::Finished,
                "postponed" => MatchStatus::Postponed,
                "canceled" => MatchStatus::Canceled,
                "abandoned" => MatchStatus::Abandoned,
                "awarded" => MatchStatus::Awarded,
                other => anyhow::bail!("ismeretlen státusz: {other}"),
            };

            Ok(SourceMaterialValues {
                source_code: row.source_code,
                home_team_id,
                away_team_id,
                kickoff_at,
                status,
                home_ft: payload["result"]["home_ft"].as_i64().map(|v| v as i16),
                away_ft: payload["result"]["away_ft"].as_i64().map(|v| v as i16),
            })
        })
        .collect()
}

fn required_str<'a>(payload: &'a serde_json::Value, field: &str) -> Result<&'a str> {
    payload
        .get(field)
        .and_then(serde_json::Value::as_str)
        .with_context(|| format!("hiányzó mező: {field}"))
}

fn parse_uuid(payload: &serde_json::Value, field: &str) -> Result<Uuid> {
    Ok(Uuid::parse_str(required_str(payload, field)?)?)
}

#[derive(Debug, sqlx::FromRow)]
struct RawSourceValue {
    source_code: String,
    normalized_payload: serde_json::Value,
}
