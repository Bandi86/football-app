//! Auditált ESPN-soccer dry-run: nyers ESPN JSON → observation + provenance +
//! mért számok. CSAK OLVAS; semmilyen DB/fájl írás, canonical entitás nincs.
//!
//! Használat:
//!   cargo run -p football-ingest --example espn_sportsleader_dry_run -- \
//!     <raw-espn-scoreboard.json> [source_url] [captured_at_rfc3339] [sport] [league]
//!
//! A raw payload a hívó fetch-eli (kicsi MCP-híd / `reqwest`); ez a példa nem
//! ér hálózatra. Kilatés: [`EspnDryRunReport`] JSON stdoutra.

use chrono::{DateTime, Utc};
use football_ingest::espn_sportsleader::{
    EspnSoccerLeague, REFERENCE_LEAGUE_ENG1, SPORT_SOCCER, parse_soccer_scoreboard,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("hiányzó raw ESPN JSON útvonal")?;
    let source_url = args.next().unwrap_or_else(|| "unknown".to_string());
    let captured_at = match args.next() {
        Some(s) => DateTime::parse_from_rfc3339(&s)?.with_timezone(&Utc),
        None => Utc::now(),
    };
    let sport = args.next().unwrap_or_else(|| SPORT_SOCCER.to_string());
    let league_slug = args
        .next()
        .unwrap_or_else(|| REFERENCE_LEAGUE_ENG1.to_string());

    let raw = std::fs::read_to_string(&path)?; // csak olvas
    let league = EspnSoccerLeague {
        sport,
        league: league_slug,
        ..EspnSoccerLeague::eng_1()
    };
    let out = parse_soccer_scoreboard(&raw, &source_url, captured_at, &league)?;

    // mért dry-run számok JSON-ban; stdout, nincs írás.
    println!("{}", serde_json::to_string_pretty(&out.report)?);
    eprintln!(
        "dry-run: {} observation (nem kanonikus írás); observation lista kihagyva",
        out.observations.len()
    );
    Ok(())
}
