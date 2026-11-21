use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Natural key ütközésnél (409) a másik, már létező meccs azonosítója.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict_match_id: Option<Uuid>,
}

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub conflict_match_id: Option<Uuid>,
}

impl AppError {
    pub fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "UNAUTHORIZED",
            message: "Érvénytelen vagy hiányzó hozzáférési token.".into(),
            conflict_match_id: None,
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "BAD_REQUEST",
            message: message.into(),
            conflict_match_id: None,
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "NOT_FOUND",
            message: message.into(),
            conflict_match_id: None,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "INTERNAL",
            message: message.into(),
            conflict_match_id: None,
        }
    }

    /// A natural key más meccsre illeszkedik: 409, a válaszban a másik id.
    pub fn conflict(existing_match_id: Uuid) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code: "CONFLICT",
            message: format!(
                "A megadott kezdés és meccs-azonosítók egy másik meccsre illeszkednek: {existing_match_id}."
            ),
            conflict_match_id: Some(existing_match_id),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiError {
                code: self.code,
                message: self.message,
                request_id: None,
                conflict_match_id: self.conflict_match_id,
            }),
        )
            .into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        tracing::error!(error = %value, "adatbázis hiba");
        Self::internal("Adatbázis hiba.")
    }
}
