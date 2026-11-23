//! A teljes HTTP-szerződés OpenAPI 3.1 leírása.
//!
//! A dokumentum a handlerekre tett `#[utoipa::path]` annotációkból és a
//! választípusok `ToSchema` derive-jaiból fordítás időben épül, ezért nem
//! tud csendben elszakadni a szerializált mezőktől. A `router()` és a
//! dokumentum útvonalainak egyezését a `tests/openapi.rs` ellenőrzi.

use axum::Json;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::routes::competition::CompetitionDetailResponse;
use crate::routes::day::{DayMatch, DayResponse};
use crate::routes::health::HealthResponse;
use crate::routes::manual::{ManualResultRequest, ManualResultResponse};
use crate::routes::ops::{OpsIntegrity, OpsSource, OpsStatus};
use crate::routes::search::SearchResponse;
use crate::routes::team::TeamDetailResponse;
use crate::routes::verification::GateRow;
use crate::routes::weather::WeatherResponse;

use crate::error::ApiError;

/// A dokumentum, amit a `GET /openapi.json` kiszolgál.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "football-app v2 API",
        version = env!("CARGO_PKG_VERSION"),
        description = "Helyi API az ellenőrzött napi meccslistához. \
                       Minden adat forrás- és ellenőrzési állapottal érkezik; a hiányzó érték null, nem nulla. \
                       Az egyetlen írási művelet a kézi eredménybevitel (PUT, ops token, auditált). \
                       A token `Authorization: Bearer <read|ops token>` fejlécben megy."
    ),
    paths(
        crate::routes::health::health,
        crate::routes::day::day,
        crate::routes::search::search,
        crate::routes::match_detail::match_detail,
        crate::routes::manual::manual_result,
        crate::routes::asset::asset,
        crate::routes::competition::competitions,
        crate::routes::competition::competition_detail,
        crate::routes::competition::competition_seasons,
        crate::routes::team::teams,
        crate::routes::team::team_detail,
        crate::routes::verification::gates,
        crate::routes::ops::status,
        crate::routes::weather::weather,
    ),
    components(schemas(
        ApiError,
        HealthResponse,
        ManualResultRequest,
        ManualResultResponse,
        DayResponse,
        DayMatch,
        CompetitionDetailResponse,
        TeamDetailResponse,
        GateRow,
        WeatherResponse,
        SearchResponse,
        football_storage::repositories::search::SearchHit,
        OpsStatus,
        OpsIntegrity,
        OpsSource,
        football_storage::repositories::competition::CompetitionListRow,
        football_storage::repositories::competition::CompetitionDetailRow,
        football_storage::repositories::competition::CompetitionTeamRow,
        football_storage::repositories::competition::CompetitionSeasonRow,
        football_storage::repositories::team::TeamListRow,
        football_storage::repositories::team::TeamDetailRow,
        football_storage::repositories::match_detail::MatchDetailRow,
        football_storage::repositories::match_detail::MatchIncidentRow,
        football_storage::repositories::match_detail::VenueRow,
        football_storage::repositories::match_detail::WeatherRow,
        football_domain::verification::Finding,
        football_domain::verification::Severity,
        football_domain::trust::OddsValues,
    )),
    modifiers(&BearerAuth),
    tags(
        (name = "health", description = "Életjel"),
        (name = "nap", description = "Napi meccslista"),
        (name = "keresés", description = "Globális keresés csapatokra, versenyekre és meccsekre"),
        (name = "meccs", description = "Meccs-részletek"),
        (name = "verseny", description = "Verseny-lista és -adatlap"),
        (name = "csapat", description = "Csapatkereső és -adatlap"),
        (name = "asset", description = "Tartalom-címzett blobok"),
        (name = "ops", description = "Üzemeltetési és publikálási kapu"),
        (name = "időjárás", description = "A fejléc időjárása a helyi cache-ből"),
    )
)]
pub struct ApiDoc;

/// A bearer token biztonsági sémája (a `authorize` fejléc-ellenőrzésének tükre).
struct BearerAuth;

impl Modify for BearerAuth {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("token")
                        .build(),
                ),
            );
        }
    }
}

/// `GET /openapi.json` — publikus, token nélkül (a séma leírása nem titok).
pub async fn document() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_is_openapi_31_with_all_paths() {
        let doc = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let version = doc["openapi"].as_str().unwrap();
        assert!(version.starts_with("3.1"), "openapi verzió: {version}");
        let paths = doc["paths"].as_object().unwrap();
        assert_eq!(paths.len(), 14, "dokumentált útvonalak: {paths:?}");
        for (path, item) in paths {
            let method = if item.get("get").is_some() {
                "get"
            } else if item.get("put").is_some() {
                "put"
            } else {
                panic!("a {path} útvonalon nincs GET vagy PUT művelet");
            };
            assert!(
                item[method].get("responses").is_some(),
                "a {path} {method} műveletének nincs válaszleírása"
            );
        }
    }

    #[test]
    fn bearer_scheme_is_declared_and_used() {
        let doc = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let scheme = &doc["components"]["securitySchemes"]["bearer"];
        assert_eq!(scheme["type"], "http");
        assert_eq!(scheme["scheme"], "bearer");

        let paths = doc["paths"].as_object().unwrap();
        for (path, item) in paths {
            let method = if item.get("get").is_some() {
                "get"
            } else {
                "put"
            };
            let security = &item[method]["security"];
            if path == "/v1/health" {
                assert!(
                    security.is_null() || security.as_array().is_some_and(|a| a.is_empty()),
                    "a health végpont nem lehet tokenhez kötve: {security}"
                );
            } else {
                assert_eq!(
                    security[0]["bearer"],
                    serde_json::json!([]),
                    "a {path} végpont nem hivatkozik a bearer sémára"
                );
            }
        }
    }
}
