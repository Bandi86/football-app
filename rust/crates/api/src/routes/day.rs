use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use chrono::NaiveDate;
use football_storage::repositories::day::{DayFilter, fetch_day, fetch_day_refreshed_at};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::auth::{Scope, authorize};
use crate::error::{ApiError, AppError};

#[derive(Debug, Deserialize)]
pub struct DayQuery {
    pub include: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DayResponse {
    pub date: NaiveDate,
    pub timezone: &'static str,
    /// A nap legutóbbi gate-kiértékelésének ideje (batch-frissítés jelzés).
    pub refreshed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Napi hiány-mutatók (idő nélküli, lezáratlan, eredmény/logó/odds nélküli, withheld).
    pub gaps: DayGaps,
    pub matches: Vec<DayMatch>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DayGaps {
    pub missing_kickoff: i64,
    pub open_past: i64,
    pub missing_result: i64,
    pub missing_team_logo: i64,
    pub missing_competition_logo: i64,
    pub missing_odds: i64,
    pub withheld: i64,
}

impl From<football_storage::repositories::day::DayGaps> for DayGaps {
    fn from(g: football_storage::repositories::day::DayGaps) -> Self {
        Self {
            missing_kickoff: g.missing_kickoff,
            open_past: g.open_past,
            missing_result: g.missing_result,
            missing_team_logo: g.missing_team_logo,
            missing_competition_logo: g.missing_competition_logo,
            missing_odds: g.missing_odds,
            withheld: g.withheld,
        }
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DayMatch {
    pub id: uuid::Uuid,
    pub kickoff_at: chrono::DateTime<chrono::Utc>,
    pub kickoff_precision: String,
    pub status: String,
    pub home: String,
    pub away: String,
    pub home_score: Option<i16>,
    pub away_score: Option<i16>,
    pub publish_state: String,
    #[schema(value_type = Vec<football_domain::verification::Finding>)]
    pub blockers: serde_json::Value,
    pub competition_id: uuid::Uuid,
    pub competition_name: String,
    pub country_code: Option<String>,
    pub country_name: Option<String>,
    pub tier: i16,
    pub priority: i32,
    pub round_label: Option<String>,
    pub home_ht: Option<i16>,
    pub away_ht: Option<i16>,
    pub home_et: Option<i16>,
    pub away_et: Option<i16>,
    pub home_pens: Option<i16>,
    pub away_pens: Option<i16>,
    pub home_position: Option<i16>,
    pub away_position: Option<i16>,
    #[schema(value_type = Option<football_domain::trust::OddsValues>)]
    pub odds: Option<serde_json::Value>,
    pub market_count: Option<i32>,
    pub odds_source: Option<String>,
    pub home_logo: Option<String>,
    pub away_logo: Option<String>,
    pub competition_logo: Option<String>,
    pub home_elo: Option<f64>,
    pub away_elo: Option<f64>,
    pub odds_captured_at: Option<chrono::DateTime<chrono::Utc>>,
    pub odds_trust_state: Option<String>,
}

#[utoipa::path(
    get,
    path = "/v1/day/{date}",
    tag = "nap",
    summary = "Egy nap ellenőrzött meccslistája (Europe/Budapest nap szerint)",
    params(
        ("date" = String, Path, description = "Naptári nap, YYYY-MM-DD"),
        ("include" = Option<String>, Query, description = "`all` esetén a visszatartott meccsek is szerepelnek")
    ),
    responses(
        (status = 200, body = DayResponse),
        (status = 400, body = ApiError),
        (status = 401, body = ApiError),
        (status = 500, body = ApiError)
    ),
    security(("bearer" = []))
)]
pub async fn day(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(date): Path<String>,
    Query(query): Query<DayQuery>,
) -> Result<Json<DayResponse>, AppError> {
    authorize(&headers, Scope::Read, &state.read_token, &state.ops_token)?;
    let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|_| AppError::bad_request("A dátum formátuma YYYY-MM-DD legyen."))?;

    let rows = fetch_day(
        &state.pool,
        &DayFilter {
            date,
            timezone: "Europe/Budapest".into(),
            include_all: query.include.as_deref() == Some("all"),
            competition_id: None,
            window_days: 0,
            team_id: None,
        },
    )
    .await
    .map_err(|e| {
        tracing::error!(error=%e, "napi lista lekérdezés");
        AppError::internal("A napi lista lekérdezése sikertelen.")
    })?;

    let gaps = football_storage::repositories::day::fetch_day_gaps(
        &state.pool,
        &DayFilter {
            date,
            timezone: "Europe/Budapest".into(),
            include_all: false,
            competition_id: None,
            window_days: 0,
            team_id: None,
        },
    )
    .await
    .map_err(|e| {
        tracing::error!(error=%e, "napi hiány-mutatók lekérdezése");
        AppError::internal("A napi hiány-mutatók lekérdezése sikertelen.")
    })?;

    let refreshed_at = fetch_day_refreshed_at(&state.pool, date)
        .await
        .map_err(|e| {
            tracing::error!(error=%e, "napi frissítés-idő lekérdezés");
            AppError::internal("A napi frissítés-idő lekérdezése sikertelen.")
        })?;

    Ok(Json(DayResponse {
        date,
        timezone: "Europe/Budapest",
        refreshed_at,
        gaps: gaps.into(),
        matches: rows.into_iter().map(map_match).collect(),
    }))
}

/// A storage-sor → API-meccs leképezés (a napi és a verseny-végpont közös útja).
pub fn map_match(row: football_storage::repositories::day::DayMatchRow) -> DayMatch {
    DayMatch {
        id: row.match_id,
        kickoff_at: row.kickoff_at,
        kickoff_precision: row.kickoff_precision,
        status: row.status,
        home: row.home_name,
        away: row.away_name,
        home_score: row.home_score,
        away_score: row.away_score,
        publish_state: row.publish_state,
        blockers: row.blockers,
        competition_id: row.competition_id,
        competition_name: row.competition_name,
        country_code: row.country_code,
        country_name: row.country_name,
        tier: row.tier,
        priority: row.priority,
        round_label: row.round_label,
        home_ht: row.home_ht,
        away_ht: row.away_ht,
        home_et: row.home_et,
        away_et: row.away_et,
        home_pens: row.home_pens,
        away_pens: row.away_pens,
        home_position: row.home_position,
        away_position: row.away_position,
        odds: row.odds,
        market_count: row.market_count,
        odds_source: row.odds_source,
        odds_captured_at: row.odds_captured_at,
        odds_trust_state: row.odds_trust_state,
        home_logo: row.home_logo,
        away_logo: row.away_logo,
        competition_logo: row.competition_logo,
        home_elo: row.home_elo,
        away_elo: row.away_elo,
    }
}
