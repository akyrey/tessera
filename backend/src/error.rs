//! API error type rendered as `{ "error": { "code", "message" } }`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("resource not found")]
    NotFound,
    #[error("service unavailable")]
    Unavailable,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl ApiError {
    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
            Self::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();
        // Internal details go to the log, never to the client.
        let message = if status.is_server_error() {
            tracing::error!(error = %self, "request failed");
            "internal error".to_owned()
        } else {
            self.to_string()
        };
        let body = ErrorBody {
            error: ErrorDetail {
                code: code.to_owned(),
                message,
            },
        };
        (status, Json(body)).into_response()
    }
}
