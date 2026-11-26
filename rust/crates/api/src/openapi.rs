//! A teljes HTTP-szerződés OpenAPI 3.1 leírása.
//!
//! A dokumentum a handlerekre tett `#[utoipa::path]` annotációkból és a
//! választípusok `ToSchema` derive-jaiból fordítás időben épül, ezért nem
//! tud csendben elszakadni a szerializált mezőktől. A `router()` és a
//! dokumentum útvonalainak egyezését a `tests/openapi.rs` ellenőrzi.

use axum::Json;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::routes::audit::AuditResponse;
use crate::routes::competition::CompetitionDetailResponse;
use crate::routes::day::{DayMatch, DayResponse};
use crate::routes::edit::{MatchEditRequest, MatchEditResponse};
use crate::routes::health::HealthResponse;
use crate::routes::history::HistoryResponse;
use crate::routes::issue_action::ResolveResponse;
use crate::routes::issues::IssuesResponse;
use crate::routes::manual::{ManualResultRequest, ManualResultResponse};
use crate::routes::notes::{NoteCreateRequest, NotePatchRequest, NotesResponse};
use crate::routes::ops::{OpsIntegrity, OpsSource, OpsStatus};
use crate::routes::player::PlayerResponse;
use crate::routes::search::SearchResponse;
use crate::routes::summary::{IssueCodeCount, OpsAction, OpsSummary};
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
                       Az írási műveletek (kézi eredmény, jegyzetek) ops tokent igényelnek és auditáltak. \
                       A token `Authorization: Bearer <read|ops token>` fejlécben megy."
    ),
    paths(
        crate::routes::health::health,
        crate::routes::day::day,
        crate::routes::search::search,
        crate::routes::match_detail::match_detail,
        crate::routes::manual::manual_result,
        crate::routes::edit::match_edit,
        crate::routes::notes::list_notes,
        crate::routes::notes::create_note,
        crate::routes::notes::edit_note,
        crate::routes::history::history,
        crate::routes::issues::issues,
        crate::routes::asset::asset,
        crate::routes::asset_upload::upload_asset,
        crate::routes::competition::competitions,
        crate::routes::competition::competition_detail,
        crate::routes::competition::competition_seasons,
        crate::routes::team::teams,
        crate::routes::team::team_detail,
        crate::routes::player::player_detail,
        crate::routes::match_micro::match_lineups,
        crate::routes::match_micro::match_player_stats,
        crate::routes::match_micro::match_shots,
        crate::routes::verification::gates,
        crate::routes::ops::status,
        crate::routes::audit::audit,
        crate::routes::summary::summary,
        crate::routes::issue_action::resolve_issue,
        crate::routes::weather::weather,
    ),
    components(schemas(
        ApiError,
        HealthResponse,
        ManualResultRequest,
        ManualResultResponse,
        MatchEditRequest,
        MatchEditResponse,
        NoteCreateRequest,
        NotePatchRequest,
        NotesResponse,
        IssuesResponse,
        HistoryResponse,
        football_storage::repositories::issue::IssueRow,
        football_storage::repositories::note::MatchNoteRow,
        football_storage::repositories::history::HistoryEntry,
        DayResponse,
        DayMatch,
        CompetitionDetailResponse,
        TeamDetailResponse,
        GateRow,
        WeatherResponse,
        SearchResponse,
        PlayerResponse,
        football_storage::repositories::search::SearchHit,
        football_storage::repositories::player::PlayerDetailRow,
        football_storage::repositories::player::PlayerSpellRow,
        football_storage::repositories::player::PlayerAppearanceRow,
        football_storage::repositories::player::PlayerStatRow,
        football_storage::repositories::player::PlayerIncidentRow,
        OpsStatus,
        OpsIntegrity,
        OpsSource,
        AuditResponse,
        football_storage::repositories::ops_audit::AuditEntryRow,
        OpsSummary,
        IssueCodeCount,
        OpsAction,
        ResolveResponse,
        football_storage::repositories::competition::CompetitionListRow,
        football_storage::repositories::competition::CompetitionDetailRow,
        football_storage::repositories::competition::CompetitionTeamRow,
        football_storage::repositories::competition::CompetitionSeasonRow,
        football_storage::repositories::team::TeamListRow,
        football_storage::repositories::team::TeamDetailRow,
        football_storage::repositories::team::TeamPlayerRow,
        football_storage::repositories::match_detail::MatchDetailRow,
        football_storage::repositories::match_link::MatchLinkRow,
        football_storage::repositories::match_micro::LineupRow,
        football_storage::repositories::match_micro::PlayerStatRow,
        football_storage::repositories::match_micro::ShotRow,
        crate::routes::match_micro::LineupsResponse,
        crate::routes::match_micro::PlayerStatsResponse,
        crate::routes::match_micro::ShotsResponse,
        crate::routes::asset_upload::UploadResponse,
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
        (name = "játékos", description = "Játékosadatlap: tagság, fellépések, statisztika, események"),
        (name = "asset", description = "Tartalom-címzett blobok"),
        (name = "ops", description = "Üzemeltetési és publikálási kapu"),
        (name = "hiány", description = "Tartós hiányjelek idővonallal"),
        (name = "jegyzet", description = "Ügynök és felhasználó közös meccsjegyzetei"),
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
        assert_eq!(paths.len(), 27, "dokumentált útvonalak: {paths:?}");
        for (path, item) in paths {
            let methods: Vec<(&str, &serde_json::Value)> = ["get", "post", "put", "patch"]
                .into_iter()
                .filter_map(|method| item.get(method).map(|operation| (method, operation)))
                .collect();
            assert!(!methods.is_empty(), "a {path} útvonalon nincs művelet");
            for (method, operation) in methods {
                assert!(
                    operation.get("responses").is_some(),
                    "a {path} {method} műveletének nincs válaszleírása"
                );
            }
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
            for method in ["get", "post", "put", "patch"] {
                let Some(operation) = item.get(method) else {
                    continue;
                };
                let security = &operation["security"];
                if path == "/v1/health" {
                    assert!(
                        security.is_null() || security.as_array().is_some_and(|a| a.is_empty()),
                        "a health végpont nem lehet tokenhez kötve: {security}"
                    );
                } else {
                    assert_eq!(
                        security[0]["bearer"],
                        serde_json::json!([]),
                        "a {path} {method} végpont nem hivatkozik a bearer sémára"
                    );
                }
            }
        }
    }
}
