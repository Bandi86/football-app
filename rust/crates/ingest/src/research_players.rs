//! A helyi research detail-JSON-ek játékos-mezőinek olvasása.
//!
//! A meglévő `research.rs` adapter csak `meta/uid/odds/events/team_statistics`
//! mezőket deserializál; a `lineups`, `player_statistics` és `shotmap` blokkokat
//! a serde eldobja. Ez a modul ezeket olvassa, séma-drift-tűrően (hiányzó mező
//! → None, szöveges/formázott érték → nem találgatott numerikus érték).
//!
//! A modul nem ír adatot és nem hív hálózatot — az importot a CLI végzi.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct ResearchPlayerDetail {
    /// A research match uid, pl. `fm:1000015510` — a kanonikus meccsref kulcsa.
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub meta: Option<PlayerDetailMeta>,
    #[serde(default)]
    pub lineups: Option<Lineups>,
    #[serde(default)]
    pub player_statistics: Option<Vec<PlayerStatistic>>,
    #[serde(default)]
    pub shotmap: Option<Vec<Shot>>,
}

#[derive(Debug, Deserialize)]
pub struct PlayerDetailMeta {
    #[serde(default)]
    pub generated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct Lineups {
    #[serde(default)]
    pub home: Option<LineupSide>,
    #[serde(default)]
    pub away: Option<LineupSide>,
}

#[derive(Debug, Deserialize)]
pub struct LineupSide {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub team_id: Option<i64>,
    #[serde(default)]
    pub formation: Option<String>,
    #[serde(default)]
    pub starters: Vec<LineupPlayer>,
    #[serde(default)]
    pub substitutes: Vec<LineupPlayer>,
}

/// A játékos stabíl forrás-ID-ja (`id`) és kontextusa. A `market_value_eur`
/// csak a forrásban marad, nem kerül automatikusan kanonikus táblába.
#[derive(Debug, Deserialize)]
pub struct LineupPlayer {
    #[serde(deserialize_with = "number_or_string_i64")]
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "optional_number_or_string")]
    pub shirt_number: Option<i32>,
    #[serde(default)]
    pub position_id: Option<i32>,
    #[serde(default)]
    pub age: Option<i32>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub rating: Option<f64>,
    #[serde(default)]
    pub captain: bool,
}

#[derive(Debug, Deserialize)]
pub struct PlayerStatistic {
    #[serde(deserialize_with = "number_or_string_i64")]
    pub player_id: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub team_id: Option<i64>,
    #[serde(default)]
    pub stats: std::collections::BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
pub struct Shot {
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub minute: Option<i32>,
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default, deserialize_with = "optional_number_or_string_i64")]
    pub player_id: Option<i64>,
    #[serde(default)]
    pub player: Option<String>,
    #[serde(default)]
    pub xg: Option<f64>,
    #[serde(default)]
    pub xgot: Option<f64>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    #[serde(default)]
    pub shot_type: Option<String>,
}

/// A forrásban a player-id és a mezszám egyaránt lehet szám vagy szöveg;
/// csak tiszta egész számot engedünk át, minden más None (nem találgatunk).
fn number_or_string_i64<'de, D>(deserializer: D) -> std::result::Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Number(n) => n
            .as_i64()
            .ok_or_else(|| serde::de::Error::custom("player_id nem egész szám")),
        Value::String(s) => s
            .trim()
            .parse::<i64>()
            .map_err(|_| serde::de::Error::custom("player_id nem számítható")),
        _ => Err(serde::de::Error::custom("player_id hiányzik")),
    }
}

fn optional_number_or_string_i64<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(|v| match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }))
}

/// `shirt_number` lehet szám vagy szöveg a forrásban; csak tiszta egész számot
/// engedünk át, minden más None (nem találgatunk).
fn optional_number_or_string<'de, D>(deserializer: D) -> std::result::Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(|v| match v {
        Value::Number(n) => n.as_i64().map(|n| n as i32),
        Value::String(s) => s.trim().parse::<i32>().ok(),
        _ => None,
    }))
}

pub fn parse(bytes: &[u8]) -> Result<ResearchPlayerDetail> {
    serde_json::from_slice(bytes).context("research player detail JSON")
}

/// Csak a numerikus stat értékeket engedjük be; a formázott szöveg nem lesz 0.
pub fn numeric_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_players_stats_and_shots_tolerantly() {
        let json = serde_json::json!({
            "id": "fm:1000015510",
            "meta": {"generated_at": "2026-11-22T11:44:31Z"},
            "lineups": {
                "home": {"team_id": 1173026, "starters": [
                    {"id": 1553027, "name": "Argus", "shirt_number": "81",
                     "position_id": 11, "age": 19, "country": "Belgium", "rating": 6.7}
                ], "substitutes": []},
                "away": {"team_id": 8174, "starters": [], "substitutes": []}
            },
            "player_statistics": [
                {"player_id": 149150, "name": "Denis", "team_id": 1173026,
                 "stats": {"FotMob rating": 7.26, "Goals": 0, "Shots": "n/a"}}
            ],
            "shotmap": [
                {"id": 299871760, "minute": 8, "team": "home", "player_id": 1553027,
                 "xg": 0.0355, "xgot": 0.0233, "x": 79.6, "y": 32.7}
            ],
            "unknown_future_field": {"ignored": true}
        });
        let parsed = parse(&serde_json::to_vec(&json).unwrap()).unwrap();
        assert_eq!(parsed.id.as_deref(), Some("fm:1000015510"));
        let home = parsed.lineups.as_ref().unwrap().home.as_ref().unwrap();
        assert_eq!(home.starters[0].shirt_number, Some(81));
        assert_eq!(home.starters[0].country.as_deref(), Some("Belgium"));
        let stats = &parsed.player_statistics.as_ref().unwrap()[0].stats;
        assert_eq!(numeric_value(&stats["FotMob rating"]), Some(7.26));
        assert_eq!(
            numeric_value(&stats["Shots"]),
            None,
            "formázott szöveg nem 0"
        );
        assert_eq!(parsed.shotmap.as_ref().unwrap()[0].player_id, Some(1553027));
    }
}
