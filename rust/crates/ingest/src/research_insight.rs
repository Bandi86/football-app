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
pub const SECTIONS: [&str; 10] = [
    "preview_hu",
    "prediction",
    "h2h_streaks",
    "standings",
    "season_stats",
    "injuries",
    "fan_poll",
    "odds_markets",
    "venue_details",
    "match_report_hu",
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

    let values: [Option<Value>; 10] = [
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
        non_empty(doc.get("match_report_hu").cloned()),
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

/// A research detailben ténylegesen szereplő, http(s) URL-ek.
/// Ismeretlen mezők és nem URL értékek kimaradnak; az azonos URL nem duplikálódik.
pub fn link_payload(doc: &Value) -> Option<Value> {
    use std::collections::HashSet;

    fn push(
        links: &mut Vec<Value>,
        seen: &mut HashSet<String>,
        kind: &str,
        label: &str,
        value: Option<&str>,
    ) {
        let Some(url) = value else { return };
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return;
        }
        if !seen.insert(url.to_string()) {
            return;
        }
        links.push(serde_json::json!({
            "kind": kind,
            "label": label,
            "url": url,
        }));
    }

    let mut seen = HashSet::new();
    let mut links = Vec::new();
    let match_obj = doc.get("match");
    push(
        &mut links,
        &mut seen,
        "source",
        "Meccs forrása",
        match_obj
            .and_then(|m| m.get("links"))
            .and_then(|l| l.get("source"))
            .and_then(Value::as_str),
    );
    if let Some(items) = match_obj
        .and_then(|m| m.get("source_urls"))
        .and_then(Value::as_array)
    {
        for item in items {
            let kind = match item.get("section").and_then(Value::as_str) {
                Some("standings") => "standing",
                Some("weather") => "weather",
                _ => "other",
            };
            let source = item
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or("forrás");
            push(
                &mut links,
                &mut seen,
                kind,
                &format!("Forrás: {source}"),
                item.get("url").and_then(Value::as_str),
            );
        }
    }
    push(
        &mut links,
        &mut seen,
        "reference",
        "Hivatkozás",
        doc.get("links")
            .and_then(|l| l.get("reference"))
            .and_then(Value::as_str),
    );
    push(
        &mut links,
        &mut seen,
        "standing",
        "Tabella forrása",
        doc.get("standings")
            .and_then(|s| s.get("url"))
            .and_then(Value::as_str),
    );
    if links.is_empty() {
        return None;
    }
    if let Some(generated) = doc
        .get("meta")
        .and_then(|m| m.get("generated_at"))
        .and_then(Value::as_str)
    {
        for link in &mut links {
            if let Some(map) = link.as_object_mut() {
                map.insert(
                    "captured_at".to_string(),
                    Value::String(generated.to_string()),
                );
            }
        }
    }
    Some(Value::Array(links))
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

    #[test]
    fn report_is_preserved_as_a_generated_section() {
        let doc = json!({
            "match_report_hu": {"title": "Riport", "text": "Leírás", "generated": true,
                                "verified": false, "source": "derived:api/v1"}
        });
        let payload = insight_payload(&doc).unwrap();
        assert_eq!(payload["match_report_hu"]["generated"], true);
        assert_eq!(payload["match_report_hu"]["verified"], false);
    }

    #[test]
    fn links_are_http_only_deduplicated_and_labelled() {
        let doc = json!({
            "match": {
                "links": {"source": "https://www.fotmob.com/match/1", "self": "/local"},
                "source_urls": [
                    {"section": "standings", "source": "fotmob", "url": "https://data.fotmob.com/tables.ext.1.fot.gz"},
                    {"section": "match", "source": "fotmob", "url": "https://www.fotmob.com/match/1"}
                ]
            },
            "links": {"reference": "https://www.thefa.com", "self": "/local"},
            "standings": {"url": "https://data.fotmob.com/tables.ext.1.fot.gz"}
        });
        let links = link_payload(&doc).unwrap();
        let items = links.as_array().unwrap();
        assert_eq!(
            items.len(),
            3,
            "a duplikátumokat és a helyi útvonalakat kihagyjuk"
        );
        assert_eq!(items[0]["kind"], "source");
        assert_eq!(items[1]["kind"], "standing");
        assert_eq!(items[2]["kind"], "reference");
        assert!(
            items
                .iter()
                .all(|item| item["url"].as_str().unwrap().starts_with("https://"))
        );
        assert!(link_payload(&json!({"links": {"reference": "/local"}})).is_none());
    }
}
