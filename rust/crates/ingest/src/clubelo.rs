//! clubelo API-adapter — S4a.
//!
//! `http://api.clubelo.com/{YYYY-MM-DD}` napi CSV-pillanatkép
//! (Rank,Club,Country,Level,Elo,From,To). A parse a közös `club_elo`
//! observation-aspketet állítja elő; a kanonikus út `ClubEloDay` →
//! `fact.club_elo` (as-of + kötelező provenance; a 2025-06-01 előtti adat
//! a határ-szabály szerint csak `provisional` lehet).

use chrono::NaiveDate;
use football_domain::error::ParseError;
use football_domain::policy::{FetchJob, Transport};

use crate::adapter::{AdapterMeta, Observation, ParseCtx, SourceAdapter};
use crate::fetch::{FetchRequest, RawPayload};

pub const SOURCE_SLUG: &str = "clubelo";
/// A historikus/provisionális határ (a clubelo visszamenőleges módosításai miatt).
pub const PROVENANCE_BOUNDARY: NaiveDate = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();

pub struct ClubEloAdapter;

impl SourceAdapter for ClubEloAdapter {
    fn meta(&self) -> AdapterMeta {
        AdapterMeta {
            source_slug: SOURCE_SLUG.into(),
            adapter_version: "1.0".into(),
            parser_version: "1.0".into(),
            domains: vec!["elo".into()],
            transport: Transport::RustHttp,
        }
    }

    fn plan(&self, job: &FetchJob) -> Vec<FetchRequest> {
        vec![FetchRequest {
            url: format!("http://api.clubelo.com/{}", job.time_window),
            request_key: format!("{SOURCE_SLUG}:elo:{}", job.time_window),
            endpoint_key: "elo".into(),
            ready_marker: None,
        }]
    }

    fn parse(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError> {
        let text = std::str::from_utf8(&raw.bytes)
            .map_err(|_| ParseError::Parse("a CSV nem érvényes UTF-8".into()))?;
        let text = text.trim_start_matches('\u{feff}');
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .flexible(true)
            .from_reader(text.as_bytes());
        let headers = reader
            .headers()
            .map_err(|e| ParseError::Parse(format!("CSV fejléc: {e}")))?
            .clone();
        let column = |name: &str| -> Result<usize, ParseError> {
            headers
                .iter()
                .position(|h| h.trim() == name)
                .ok_or_else(|| ParseError::SchemaDrift(format!("hiányzó CSV-oszlop: {name}")))
        };
        let c_club = column("Club")?;
        let c_country = column("Country")?;
        let c_elo = column("Elo")?;
        let as_of = ctx.requested_date;
        let provenance = if as_of >= PROVENANCE_BOUNDARY {
            "observed"
        } else {
            "provisional"
        };
        let captured_at = chrono::Utc::now();

        let mut out = Vec::new();
        for record in reader.records() {
            let record = record.map_err(|e| ParseError::Parse(format!("CSV sor: {e}")))?;
            let field = |i: usize| record.get(i).unwrap_or("").trim().to_string();
            let club = field(c_club);
            let country = field(c_country);
            let elo: f64 = field(c_elo)
                .parse()
                .map_err(|_| ParseError::Parse(format!("érvénytelen Elo: {}", field(c_elo))))?;
            if club.is_empty() || country.is_empty() {
                continue;
            }
            out.push(Observation {
                entity_kind: "club".into(),
                aspect: "club_elo".into(),
                natural_key: format!("{country}:{club}"),
                source_record_key: format!("{country}:{club}"),
                observed_at: captured_at,
                payload: serde_json::json!({
                    "country": country,
                    "club": club,
                    "elo": elo,
                    "as_of": as_of,
                    "provenance": provenance,
                }),
            });
        }
        Ok(out)
    }
}
