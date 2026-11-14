use crate::error::DbValueError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    RustHttp,
    Fixture,
    Obscura,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceMode {
    Shadow,
    Canary,
    Live,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverageState {
    Pending,
    Fetching,
    Done,
    NoData,
    Blocked,
    Failed,
    Stale,
    Quarantined,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchOutcome {
    Success,
    HttpError,
    Timeout,
    Oversize,
    RobotsBlocked,
    QuotaBlocked,
    BreakerOpen,
    TransportError,
    ParseError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    Leased,
    Done,
    Failed,
    Cancelled,
}

macro_rules! db_str {
    ($t:ty, $($variant:ident => $s:literal),+ $(,)?) => {
        impl $t {
            pub fn as_db_str(self) -> &'static str {
                match self { $(Self::$variant => $s),+ }
            }
            pub fn from_db_str(s: &str) -> Result<Self, DbValueError> {
                match s { $($s => Ok(Self::$variant),)+ other => Err(DbValueError(other.to_string())) }
            }
        }
    };
}

db_str!(Transport, RustHttp => "rust_http", Fixture => "fixture", Obscura => "obscura");
db_str!(SourceMode, Shadow => "shadow", Canary => "canary", Live => "live");
db_str!(BreakerState, Closed => "closed", Open => "open", HalfOpen => "half_open");
db_str!(
    CoverageState,
    Pending => "pending",
    Fetching => "fetching",
    Done => "done",
    NoData => "no_data",
    Blocked => "blocked",
    Failed => "failed",
    Stale => "stale",
    Quarantined => "quarantined",
    Skipped => "skipped"
);
db_str!(
    FetchOutcome,
    Success => "success",
    HttpError => "http_error",
    Timeout => "timeout",
    Oversize => "oversize",
    RobotsBlocked => "robots_blocked",
    QuotaBlocked => "quota_blocked",
    BreakerOpen => "breaker_open",
    TransportError => "transport_error",
    ParseError => "parse_error"
);
db_str!(
    JobStatus,
    Pending => "pending",
    Leased => "leased",
    Done => "done",
    Failed => "failed",
    Cancelled => "cancelled"
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTemplate {
    pub event_type: String,
    pub offset_minutes: Option<i64>,
    pub interval_minutes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePolicy {
    pub source_slug: String,
    pub transport: Transport,
    pub mode: SourceMode,
    pub daily_budget: i32,
    pub min_delay_ms: i64,
    pub jitter_ms: i64,
    pub breaker_threshold: i32,
    pub breaker_cooldown_secs: i64,
    pub adapter_version: String,
    pub parser_version: String,
    pub domains: Vec<String>,
    pub events: Vec<EventTemplate>,
    pub enabled: bool,
    pub max_bytes: i64,
}

#[derive(Debug, Clone)]
pub struct FetchJob {
    pub id: Uuid,
    pub source_slug: String,
    pub domain: String,
    pub entity_key: String,
    pub event_type: String,
    pub time_window: String,
    pub due_at: DateTime<Utc>,
    pub priority: i32,
}
