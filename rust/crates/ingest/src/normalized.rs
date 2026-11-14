//! Forrás-független normalizált nap: a kanonikus út közös bemenete.
//!
//! A fixture-út a kanonikus id-hinteket kitölti (demo id-stabilitás); az élő
//! források `None`-t adnak, és a kanonikus id-k natural-key upserttel képződnek.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::adapter::Observation;
use crate::fixture::{FixtureOdds, FixtureSource};

#[derive(Debug, Clone)]
pub struct NormalizedDay {
    pub source_code: String,
    pub captured_at: DateTime<Utc>,
    pub competition: NormalizedCompetition,
    pub season: NormalizedSeason,
    pub teams: Vec<NormalizedTeam>,
    pub matches: Vec<NormalizedMatch>,
    pub standings: Option<Vec<NormalizedStanding>>,
}

#[derive(Debug, Clone)]
pub struct NormalizedCompetition {
    pub id: Option<Uuid>,
    pub name: String,
    pub country_code: String,
    pub country_name: String,
    pub tier: i16,
    pub priority: i32,
}

#[derive(Debug, Clone)]
pub struct NormalizedSeason {
    pub id: Option<Uuid>,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct NormalizedTeam {
    pub id: Option<Uuid>,
    pub source_entity_id: String,
    pub name: String,
    pub country_code: String,
    pub gender: String,
    pub age_group: String,
    pub branch: String,
}

#[derive(Debug, Clone)]
pub struct NormalizedMatch {
    pub id: Option<Uuid>,
    pub source_match_id: String,
    pub home_source_entity_id: String,
    pub away_source_entity_id: String,
    pub round_label: Option<String>,
    pub kickoff_at: DateTime<Utc>,
    pub competition_timezone: String,
    pub status: String,
    pub result: Option<NormalizedResult>,
    pub odds: Option<NormalizedOdds>,
}

#[derive(Debug, Clone)]
pub struct NormalizedResult {
    pub home_ft: Option<i16>,
    pub away_ft: Option<i16>,
    pub home_ht: Option<i16>,
    pub away_ht: Option<i16>,
    pub home_et: Option<i16>,
    pub away_et: Option<i16>,
    pub home_pens: Option<i16>,
    pub away_pens: Option<i16>,
}

#[derive(Debug, Clone)]
pub struct NormalizedOdds {
    pub market_count: Option<i32>,
    pub quotes: Vec<NormalizedQuote>,
}

#[derive(Debug, Clone)]
pub struct NormalizedQuote {
    pub bookmaker: String,
    pub market: String,
    pub outcome: String,
    pub line: f64,
    pub phase: String,
    pub price: f64,
}

#[derive(Debug, Clone)]
pub struct NormalizedStanding {
    pub team_source_entity_id: String,
    pub position: i16,
}

/// Flat 7 érték → kanonikus quote-ok (default bookmaker, closing).
#[allow(clippy::too_many_arguments)]
fn flat_quotes(
    home: f64,
    draw: f64,
    away: f64,
    btts_yes: f64,
    btts_no: f64,
    over25: f64,
    under25: f64,
) -> Vec<NormalizedQuote> {
    let quote = |market: &str, outcome: &str, line: f64, price: f64| NormalizedQuote {
        bookmaker: "default".into(),
        market: market.into(),
        outcome: outcome.into(),
        line,
        phase: "closing".into(),
        price,
    };
    vec![
        quote("1x2", "home", 0.0, home),
        quote("1x2", "draw", 0.0, draw),
        quote("1x2", "away", 0.0, away),
        quote("btts", "yes", 0.0, btts_yes),
        quote("btts", "no", 0.0, btts_no),
        quote("ou", "over", 2.5, over25),
        quote("ou", "under", 2.5, under25),
    ]
}

fn default_quotes(odds: &FixtureOdds) -> Vec<NormalizedQuote> {
    flat_quotes(
        odds.home,
        odds.draw,
        odds.away,
        odds.btts_yes,
        odds.btts_no,
        odds.over25,
        odds.under25,
    )
}

#[derive(Deserialize)]
struct TeamPayload {
    id: Option<Uuid>,
    source_entity_id: String,
    name: String,
    country_code: String,
    gender: String,
    age_group: String,
    branch: String,
}

#[derive(Deserialize)]
struct CompetitionPayload {
    id: Option<Uuid>,
    name: String,
    country_code: String,
    country_name: String,
    tier: i16,
    priority: i32,
}

#[derive(Deserialize)]
struct SeasonPayload {
    id: Option<Uuid>,
    label: String,
}

#[derive(Deserialize)]
struct ResultPayload {
    home_ft: Option<i16>,
    away_ft: Option<i16>,
    home_ht: Option<i16>,
    away_ht: Option<i16>,
    home_et: Option<i16>,
    away_et: Option<i16>,
    home_pens: Option<i16>,
    away_pens: Option<i16>,
}

#[derive(Deserialize)]
struct MatchCorePayload {
    canonical_match_id: Option<Uuid>,
    source_home_id: String,
    source_away_id: String,
    kickoff_at: DateTime<Utc>,
    status: String,
    result: Option<ResultPayload>,
    round_label: Option<String>,
    competition_timezone: String,
    competition: CompetitionPayload,
    season: SeasonPayload,
    source_code: String,
    captured_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct FlatOddsPayload {
    home: f64,
    draw: f64,
    away: f64,
    btts_yes: f64,
    btts_no: f64,
    over25: f64,
    under25: f64,
}

#[derive(Deserialize)]
struct QuotePayload {
    bookmaker: String,
    market: String,
    outcome: String,
    line: f64,
    phase: String,
    price: f64,
}

#[derive(Deserialize)]
struct OddsPayload {
    odds: Option<FlatOddsPayload>,
    quotes: Option<Vec<QuotePayload>>,
    market_count: Option<i32>,
}

#[derive(Deserialize)]
struct StandingPayload {
    position: i16,
}

impl NormalizedDay {
    /// Az observation-aspketekből (verziózott payload-séma) épített nap.
    pub fn from_observations(observations: &[Observation]) -> Result<Self> {
        let mut teams = Vec::new();
        let mut matches = Vec::new();
        let mut standings = Vec::new();
        let mut odds_by_key: Vec<(String, NormalizedOdds)> = Vec::new();
        let mut meta: Option<(
            NormalizedCompetition,
            NormalizedSeason,
            String,
            DateTime<Utc>,
        )> = None;

        for obs in observations {
            match obs.aspect.as_str() {
                "team_identity" => {
                    let p: TeamPayload = serde_json::from_value(obs.payload.clone())
                        .context("team_identity payload")?;
                    teams.push(NormalizedTeam {
                        id: p.id,
                        source_entity_id: p.source_entity_id,
                        name: p.name,
                        country_code: p.country_code,
                        gender: p.gender,
                        age_group: p.age_group,
                        branch: p.branch,
                    });
                }
                "match_core" => {
                    let p: MatchCorePayload = serde_json::from_value(obs.payload.clone())
                        .context("match_core payload")?;
                    if meta.is_none() {
                        meta = Some((
                            NormalizedCompetition {
                                id: p.competition.id,
                                name: p.competition.name,
                                country_code: p.competition.country_code,
                                country_name: p.competition.country_name,
                                tier: p.competition.tier,
                                priority: p.competition.priority,
                            },
                            NormalizedSeason {
                                id: p.season.id,
                                label: p.season.label,
                            },
                            p.source_code,
                            p.captured_at,
                        ));
                    }
                    matches.push(NormalizedMatch {
                        id: p.canonical_match_id,
                        source_match_id: obs.source_record_key.clone(),
                        home_source_entity_id: p.source_home_id,
                        away_source_entity_id: p.source_away_id,
                        round_label: p.round_label,
                        kickoff_at: p.kickoff_at,
                        competition_timezone: p.competition_timezone,
                        status: p.status,
                        result: p.result.map(|r| NormalizedResult {
                            home_ft: r.home_ft,
                            away_ft: r.away_ft,
                            home_ht: r.home_ht,
                            away_ht: r.away_ht,
                            home_et: r.home_et,
                            away_et: r.away_et,
                            home_pens: r.home_pens,
                            away_pens: r.away_pens,
                        }),
                        odds: None,
                    });
                }
                "match_odds" => {
                    let p: OddsPayload = serde_json::from_value(obs.payload.clone())
                        .context("match_odds payload")?;
                    let quotes = match (p.quotes, p.odds) {
                        (Some(quotes), _) => quotes
                            .into_iter()
                            .map(|q| NormalizedQuote {
                                bookmaker: q.bookmaker,
                                market: q.market,
                                outcome: q.outcome,
                                line: q.line,
                                phase: q.phase,
                                price: q.price,
                            })
                            .collect(),
                        (None, Some(flat)) => flat_quotes(
                            flat.home,
                            flat.draw,
                            flat.away,
                            flat.btts_yes,
                            flat.btts_no,
                            flat.over25,
                            flat.under25,
                        ),
                        (None, None) => Vec::new(),
                    };
                    odds_by_key.push((
                        obs.source_record_key.clone(),
                        NormalizedOdds {
                            market_count: p.market_count,
                            quotes,
                        },
                    ));
                }
                "standing" => {
                    let p: StandingPayload =
                        serde_json::from_value(obs.payload.clone()).context("standing payload")?;
                    standings.push(NormalizedStanding {
                        team_source_entity_id: obs.source_record_key.clone(),
                        position: p.position,
                    });
                }
                other => anyhow::bail!("ismeretlen observation-aspect: {other}"),
            }
        }

        for m in &mut matches {
            if let Some((_, odds)) = odds_by_key
                .iter()
                .find(|(key, _)| *key == m.source_match_id)
            {
                m.odds = Some(odds.clone());
            }
        }

        let (competition, season, source_code, captured_at) =
            meta.context("hiányzó match_core meta az observationökből")?;
        Ok(Self {
            source_code,
            captured_at,
            competition,
            season,
            teams,
            matches,
            standings: if standings.is_empty() {
                None
            } else {
                Some(standings)
            },
        })
    }
}

impl From<&FixtureSource> for NormalizedDay {
    fn from(fixture: &FixtureSource) -> Self {
        Self {
            source_code: fixture.source_code.clone(),
            captured_at: fixture.captured_at,
            competition: NormalizedCompetition {
                id: Some(fixture.competition.id),
                name: fixture.competition.name.clone(),
                country_code: fixture.competition.country_code.clone(),
                country_name: fixture.competition.country_name.clone(),
                tier: fixture.competition.tier,
                priority: fixture.competition.priority,
            },
            season: NormalizedSeason {
                id: Some(fixture.season.id),
                label: fixture.season.label.clone(),
            },
            teams: fixture
                .teams
                .iter()
                .map(|team| NormalizedTeam {
                    id: Some(team.id),
                    source_entity_id: team.source_entity_id.clone(),
                    name: team.name.clone(),
                    country_code: team.country_code.clone(),
                    gender: team.gender.clone(),
                    age_group: team.age_group.clone(),
                    branch: team.branch.clone(),
                })
                .collect(),
            matches: fixture
                .matches
                .iter()
                .map(|m| NormalizedMatch {
                    id: Some(m.id),
                    source_match_id: m.source_match_id.clone(),
                    home_source_entity_id: m.home_source_entity_id.clone(),
                    away_source_entity_id: m.away_source_entity_id.clone(),
                    round_label: m.round_label.clone(),
                    kickoff_at: m.kickoff_at,
                    competition_timezone: m.competition_timezone.clone(),
                    status: m.status.clone(),
                    result: m.result.as_ref().map(|r| NormalizedResult {
                        home_ft: r.home_ft,
                        away_ft: r.away_ft,
                        home_ht: r.home_ht,
                        away_ht: r.away_ht,
                        home_et: r.home_et,
                        away_et: r.away_et,
                        home_pens: r.home_pens,
                        away_pens: r.away_pens,
                    }),
                    odds: m.odds.as_ref().map(|odds| NormalizedOdds {
                        market_count: m.market_count,
                        quotes: default_quotes(odds),
                    }),
                })
                .collect(),
            standings: fixture.standings.as_ref().map(|rows| {
                rows.iter()
                    .map(|standing| NormalizedStanding {
                        team_source_entity_id: standing.team_source_entity_id.clone(),
                        position: standing.position,
                    })
                    .collect()
            }),
        }
    }
}
