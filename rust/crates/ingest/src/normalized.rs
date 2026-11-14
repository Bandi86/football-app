//! Forrás-független normalizált nap: a kanonikus út közös bemenete.
//!
//! A fixture-út a kanonikus id-hinteket kitölti (demo id-stabilitás); az élő
//! források `None`-t adnak, és a kanonikus id-k natural-key upserttel képződnek.

use chrono::{DateTime, Utc};
use uuid::Uuid;

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

/// A demó flat odds → kanonikus quote-ok (default bookmaker, closing).
fn default_quotes(odds: &FixtureOdds) -> Vec<NormalizedQuote> {
    let quote = |market: &str, outcome: &str, line: f64, price: f64| NormalizedQuote {
        bookmaker: "default".into(),
        market: market.into(),
        outcome: outcome.into(),
        line,
        phase: "closing".into(),
        price,
    };
    vec![
        quote("1x2", "home", 0.0, odds.home),
        quote("1x2", "draw", 0.0, odds.draw),
        quote("1x2", "away", 0.0, odds.away),
        quote("btts", "yes", 0.0, odds.btts_yes),
        quote("btts", "no", 0.0, odds.btts_no),
        quote("ou", "over", 2.5, odds.over25),
        quote("ou", "under", 2.5, odds.under25),
    ]
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
