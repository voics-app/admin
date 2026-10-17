use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::error::{ApiError, codes};
use super::state::AppState;
use crate::application::use_cases::authenticate_admin::AdminCredentials;

pub const ACCESS_ASSERTION_HEADER: &str = "cf-access-jwt-assertion";
pub const DEV_EMAIL_HEADER: &str = "x-dev-admin-email";
/// Required on state-changing requests. Browsers cannot attach it cross-site without a CORS
/// preflight (which this API never grants), so it blocks CSRF via the Access cookie.
pub const CSRF_HEADER: &str = "x-discordy-admin";

fn header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_owned)
}

/// Authenticates every `/admin-api` request and attaches the [`AdminIdentity`](crate::application::use_cases::authenticate_admin::AdminIdentity).
pub async fn require_admin(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let headers = request.headers();
    let credentials = AdminCredentials {
        access_assertion: header(headers, ACCESS_ASSERTION_HEADER),
        dev_email: header(headers, DEV_EMAIL_HEADER),
    };
    let is_mutation = !matches!(*request.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if is_mutation && header(headers, CSRF_HEADER).as_deref() != Some("1") {
        return ApiError::new(StatusCode::FORBIDDEN, codes::MISSING_ACCESS, "missing x-discordy-admin header").into_response();
    }
    match state.auth.execute(&credentials).await {
        Ok(identity) => {
            request.extensions_mut().insert(identity);
            next.run(request).await
        }
        Err(error) => {
            tracing::info!(%error, "admin request refused");
            ApiError::from(error).into_response()
        }
    }
}
