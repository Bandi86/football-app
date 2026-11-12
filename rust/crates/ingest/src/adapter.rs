use chrono::{DateTime, NaiveDate, Utc};
use football_domain::error::ParseError;
use football_domain::policy::{FetchJob, Transport};

use crate::fetch::{FetchRequest, RawPayload};

#[derive(Debug, Clone)]
pub struct AdapterMeta {
    pub source_slug: String,
    pub adapter_version: String,
    pub parser_version: String,
    pub domains: Vec<String>,
    pub transport: Transport,
}

#[derive(Debug, Clone)]
pub struct ParseCtx {
    pub requested_date: NaiveDate,
    pub request_key: String,
}

#[derive(Debug, Clone)]
pub struct Observation {
    pub entity_kind: String,
    pub aspect: String,
    pub natural_key: String,
    pub source_record_key: String,
    pub observed_at: DateTime<Utc>,
    pub payload: serde_json::Value,
}

pub trait SourceAdapter: Send + Sync {
    fn meta(&self) -> AdapterMeta;
    fn plan(&self, job: &FetchJob) -> Vec<FetchRequest>;
    fn parse(&self, raw: &RawPayload, ctx: &ParseCtx) -> Result<Vec<Observation>, ParseError>;
}
