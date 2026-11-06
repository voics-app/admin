use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::application::error::AppError;
use crate::application::ports::PortError;
use crate::application::use_cases::authenticate_admin::AuthenticationError;

/// Error codes in the platform envelope (P9 style).
pub mod codes {
    pub const UNAUTHORIZED: u32 = 40001;
    pub const MISSING_ACCESS: u32 = 50001;
    pub const INVALID_FORM_BODY: u32 = 50035;
    pub const NOT_FOUND: u32 = 10000;
    pub const CONFLICT: u32 = 40900;
    pub const UPSTREAM_UNAVAILABLE: u32 = 50200;
    pub const UPSTREAM_REJECTED: u32 = 50201;
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: u32,
    pub message: String,
}

/// An HTTP error carrying the `{code, message}` envelope.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: u32,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: u32, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, codes::INVALID_FORM_BODY, message)
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, codes::NOT_FOUND, "not found")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                code: self.code,
                message: self.message,
            }),
        )
            .into_response()
    }
}

impl From<AppError> for ApiError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::Validation(message) => ApiError::bad_request(message),
            AppError::NotFound => ApiError::not_found(),
            AppError::Conflict(message) => {
                ApiError::new(StatusCode::CONFLICT, codes::CONFLICT, message)
            }
            AppError::Dependency(PortError::Rejected { status, message }) => ApiError::new(
                StatusCode::BAD_GATEWAY,
                codes::UPSTREAM_REJECTED,
                format!("upstream rejected the request ({status}): {message}"),
            ),
            AppError::Dependency(other) => {
                tracing::warn!(error = %other, "dependency failure");
                ApiError::new(
                    StatusCode::BAD_GATEWAY,
                    codes::UPSTREAM_UNAVAILABLE,
                    "a platform dependency is unavailable",
                )
            }
        }
    }
}

impl From<PortError> for ApiError {
    fn from(error: PortError) -> Self {
        AppError::from(error).into()
    }
}

impl From<AuthenticationError> for ApiError {
    fn from(error: AuthenticationError) -> Self {
        match error {
            AuthenticationError::NotAllowed => ApiError::new(
                StatusCode::FORBIDDEN,
                codes::MISSING_ACCESS,
                error.to_string(),
            ),
            AuthenticationError::KeysUnavailable => ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                codes::UPSTREAM_UNAVAILABLE,
                error.to_string(),
            ),
            AuthenticationError::MissingCredentials | AuthenticationError::InvalidAssertion(_) => {
                ApiError::new(
                    StatusCode::UNAUTHORIZED,
                    codes::UNAUTHORIZED,
                    error.to_string(),
                )
            }
        }
    }
}
