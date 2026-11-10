use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl AppError {
    pub fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "UNAUTHORIZED",
            message: "Érvénytelen vagy hiányzó hozzáférési token.".into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "BAD_REQUEST",
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "INTERNAL",
            message: message.into(),
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
