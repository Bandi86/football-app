use std::collections::HashSet;

use football_domain::error::ParseError;
use football_domain::policy::{FetchJob, Transport};

use crate::adapter::{AdapterMeta, Observation, ParseCtx, SourceAdapter};
use crate::fetch::{FetchRequest, RawPayload};
use crate::fixture::FixtureSource;

pub struct DemoAdapter;

const KNOWN_FIELDS: [&str; 10] = [
    "source_code",
    "source_display_name",
    "captured_at",
    "endpoint_key",
    "request_url",
    "adapter_version",
    "parser_version",
    "competition",
    "season",
    "teams",
];
// matches/standings opcionális: a drift-arány a KNOWN_FIELDS + matches + standings halmazra számol

impl SourceAdapter for DemoAdapter {
    fn meta(&self) -> AdapterMeta {
        AdapterMeta {
            source_slug: "demo-synthetic".into(),
            adapter_version: "1.0".into(),
            parser_version: "1.0".into(),
            domains: vec!["match".into()],
            transport: Transport::Fixture,
        }
    }

    fn plan(&self, job: &FetchJob) -> Vec<FetchRequest> {
        vec![FetchRequest {
            url: format!("fixture://{}/{}", job.source_slug, job.time_window),
            request_key: format!("{}:{}:{}", job.source_slug, job.domain, job.time_window),
            endpoint_key: job.domain.clone(),
        }]
    }

    fn parse(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError> {
        let value: serde_json::Value =
            serde_json::from_slice(&raw.bytes).map_err(|e| ParseError::Parse(e.to_string()))?;
        let object = value
            .as_object()
            .ok_or_else(|| ParseError::DecoyDetected("nem JSON-objektum".into()))?;

        let mut known = KNOWN_FIELDS
            .iter()
            .map(|s| s.to_string())
            .collect::<HashSet<_>>();
        known.insert("matches".into());
        known.insert("standings".into());
        let unknown = object.keys().filter(|k| !known.contains(*k)).count();
        if unknown as f64 / object.len().max(1) as f64 > 0.2 {
            return Err(ParseError::SchemaDrift(format!(
                "{unknown} ismeretlen mező"
            )));
        }

        let fixture: FixtureSource =
            serde_json::from_value(value).map_err(|e| ParseError::Parse(e.to_string()))?;
        if fixture.matches.is_empty() {
            return Err(ParseError::DecoyDetected("üres meccslista".into()));
        }

        for m in &fixture.matches {
            let tz: chrono_tz::Tz = m
                .competition_timezone
                .parse()
                .map_err(|_| ParseError::Parse("ismeretlen meccs-időzóna".into()))?;
            let local_date = m.kickoff_at.with_timezone(&tz).date_naive();
            if local_date != ctx.requested_date {
                return Err(ParseError::DateGuardMismatch {
                    requested: ctx.requested_date.to_string(),
                    got: local_date.to_string(),
                });
            }
            if let Some(r) = &m.result {
                if r.home_et.is_some()
                    || r.away_et.is_some()
                    || r.home_pens.is_some()
                    || r.away_pens.is_some()
                {
                    return Err(ParseError::Parse(
                        "AET/pens állás nem írható 90 perces eredményként (S2a fail-closed)".into(),
                    ));
                }
            }
        }

        let mut out = Vec::new();
        for team in &fixture.teams {
            out.push(Observation {
                entity_kind: "team".into(),
                aspect: "team_identity".into(),
                natural_key: team.id.to_string(),
                source_record_key: team.source_entity_id.clone(),
                observed_at: fixture.captured_at,
                payload: serde_json::to_value(team)
                    .map_err(|e| ParseError::Parse(e.to_string()))?,
            });
        }
        for m in &fixture.matches {
            let home = fixture
                .teams
                .iter()
                .find(|t| t.source_entity_id == m.home_source_entity_id)
                .ok_or_else(|| ParseError::Parse("ismeretlen hazai forrásentitás".into()))?;
            let away = fixture
                .teams
                .iter()
                .find(|t| t.source_entity_id == m.away_source_entity_id)
                .ok_or_else(|| ParseError::Parse("ismeretlen vendég forrásentitás".into()))?;
            out.push(Observation {
                entity_kind: "match".into(),
                aspect: "match_core".into(),
                natural_key: m.id.to_string(),
                source_record_key: m.source_match_id.clone(),
                observed_at: fixture.captured_at,
                payload: serde_json::json!({
                    "canonical_match_id": m.id, "home_team_id": home.id, "away_team_id": away.id,
                    "source_home_id": m.home_source_entity_id, "source_away_id": m.away_source_entity_id,
                    "kickoff_at": m.kickoff_at, "status": m.status, "result": m.result,
                    "round_label": m.round_label, "competition_timezone": m.competition_timezone,
                }),
            });
        }
        Ok(out)
    }
}
