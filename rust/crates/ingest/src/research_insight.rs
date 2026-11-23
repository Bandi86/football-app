//! A research detail JSON gazdag szekcióinak olvasása.
//!
//! A meglévő `research.rs` csak odds/events/team_statisticset dolgoz fel; ez a
//! modul a preview-t, predikciót, H2H-streaket, tabellát, szezonstatisztikát,
//! sérüléseket, fan-pollt, teljes odds-piacokat és stadion-részleteket
//! gyűjti egy `match_insight` observation payloadjába.
//!
//! Séma-drift-tűrő: ismeretlen mezők ignorálva, hiány → None. A számított
//! blokkok (`generated`, `verified`, `model`, `note`) változatlanul átmennek —
//! a kliens ezeket kötelezően jelöli.

use serde_json::{Map, Value};

/// A szekciók nevei a payloadban és a DB oszlopokban.
pub const SECTIONS: [&str; 9] = [
    "preview_hu",
    "prediction",
    "h2h_streaks",
    "standings",
    "season_stats",
    "injuries",
    "fan_poll",
    "odds_markets",
    "venue_details",
];

fn non_empty(value: Option<Value>) -> Option<Value> {
    match value {
        Some(Value::Null) => None,
        Some(Value::Object(map)) if map.is_empty() => None,
        Some(Value::Array(items)) if items.is_empty() => None,
        other => other,
    }
}

/// Csak akkor ad observationt, ha van legalább egy nem üres szekció.
pub fn insight_payload(doc: &Value) -> Option<Value> {
    let mut out = Map::new();
    let odds = doc.get("odds");
    let venue = doc.get("match").and_then(|m| m.get("venue"));

    let values: [Option<Value>; 9] = [
        non_empty(doc.get("preview_hu").cloned()),
        non_empty(doc.get("prediction").cloned()),
        non_empty(
            doc.get("head_to_head")
                .and_then(|h| h.get("streaks"))
                .cloned(),
        ),
        non_empty(doc.get("standings").cloned()),
        non_empty(doc.get("season_statistics").cloned()),
        non_empty(doc.get("injuries").cloned()),
        non_empty(doc.get("fan_poll").cloned()),
        non_empty(odds.map(|o| {
            let mut markets = Map::new();
            for key in [
                "markets_raw",
                "by_bookmaker",
                "markets_source",
                "observed_at",
            ] {
                if let Some(value) = o.get(key).cloned() {
                    markets.insert(key.to_string(), value);
                }
            }
            Value::Object(markets)
        })),
        non_empty(venue.cloned()),
    ];

    let mut has_section = false;
    for (key, value) in SECTIONS.iter().zip(values) {
        if let Some(value) = value {
            out.insert((*key).to_string(), value);
            has_section = true;
        }
    }
    if !has_section {
        return None;
    }
    // A research detail saját generálási ideje — ez a pontos as-of, nem a
    // napi envelope ideje. Hiány esetén a canonicalize a nap idejét használja.
    if let Some(generated) = doc.get("meta").and_then(|m| m.get("generated_at")).cloned() {
        out.insert("captured_at".to_string(), generated);
    }
    Some(Value::Object(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_sections_and_preserves_generated_flags() {
        let doc = json!({
            "preview_hu": {"generated": true, "verified": false, "title": "Előnézet"},
            "prediction": {"model": "poisson_v1", "note": "not a source fact"},
            "head_to_head": {"streaks": {"home": {"no_win": 1}}},
            "injuries": [{"player_id": 1, "name": "X"}],
            "fan_poll": {"available": true, "questions": []},
            "odds": {"markets_raw": {"1X2": {"Hazai": 3.9}},
                     "by_bookmaker": {"tipplap": {"markets": {}}},
                     "markets_source": "tipplap", "observed_at": "2026-11-22T05:33:15Z"},
            "match": {"venue": {"name": "Aréna", "capacity": 8400}},
            "unknown_future_field": 42
        });
        let payload = insight_payload(&doc).unwrap();
        assert_eq!(payload["preview_hu"]["generated"], true);
        assert_eq!(payload["prediction"]["model"], "poisson_v1");
        assert_eq!(payload["odds_markets"]["markets_source"], "tipplap");
        assert_eq!(payload["venue_details"]["capacity"], 8400);
        assert!(payload.get("standings").is_none(), "hiány nem kerül be");
    }

    #[test]
    fn empty_detail_has_no_insight() {
        assert!(insight_payload(&json!({"object": "match_detail"})).is_none());
    }
}
