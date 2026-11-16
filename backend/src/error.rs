//! API error type rendered as `{ "error": { "code", "message" } }`.

use axum::Json;
use axum::extract::rejection::{BytesRejection, JsonRejection, PathRejection};
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
    #[error("{0}")]
    Validation(String),
    #[error("sign in required")]
    Unauthorized,
    #[error("passkey sign-in failed")]
    AuthenticationFailed,
    #[error("this passkey request expired, please try again")]
    CeremonyExpired,
    #[error("this invite link is invalid, expired or already used")]
    InvalidInvite,
    #[error("you are not allowed to do this")]
    Forbidden,
    #[error("request origin not allowed")]
    BadOrigin,
    #[error("resource not found")]
    NotFound,
    #[error("{0}")]
    Conflict(&'static str),
    #[error("request body is too large")]
    PayloadTooLarge,
    #[error("this card was deleted")]
    CardDeleted,
    #[error("{0}")]
    QuotaExceeded(&'static str),
    #[error("too many requests, slow down")]
    RateLimited,
    #[error("service unavailable")]
    Unavailable,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    /// Unexpected failure; the detail is logged, never returned.
    #[error("{0}")]
    Internal(String),
}

impl ApiError {
    pub fn internal(context: &str, error: impl std::fmt::Display) -> Self {
        Self::Internal(format!("{context}: {error}"))
    }

    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::Validation(_) => (StatusCode::BAD_REQUEST, "invalid_request"),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::AuthenticationFailed => (StatusCode::UNAUTHORIZED, "authentication_failed"),
            Self::CeremonyExpired => (StatusCode::BAD_REQUEST, "ceremony_expired"),
            Self::InvalidInvite => (StatusCode::GONE, "invalid_invite"),
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            Self::BadOrigin => (StatusCode::FORBIDDEN, "bad_origin"),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            Self::PayloadTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large"),
            Self::CardDeleted => (StatusCode::GONE, "card_deleted"),
            Self::QuotaExceeded(_) => (StatusCode::CONFLICT, "quota_exceeded"),
            Self::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
            Self::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
            Self::Database(_) | Self::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        }
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        Self::Validation(rejection.body_text())
    }
}

impl From<BytesRejection> for ApiError {
    fn from(rejection: BytesRejection) -> Self {
        if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            Self::PayloadTooLarge
        } else {
            Self::Validation(rejection.body_text())
        }
    }
}

impl From<PathRejection> for ApiError {
    fn from(rejection: PathRejection) -> Self {
        Self::Validation(rejection.body_text())
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

/// `Json<T>` whose rejections use the API error envelope instead of plain text.
#[derive(axum::extract::FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct ApiJson<T>(pub T);

/// `Path<T>` whose rejections use the API error envelope instead of plain text.
#[derive(axum::extract::FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(ApiError))]
pub struct ApiPath<T>(pub T);
