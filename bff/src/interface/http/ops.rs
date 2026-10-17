//! Liveness, readiness and Prometheus metrics.

use std::time::Instant;

use axum::extract::{MatchedPath, Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::state::AppState;

pub async fn healthz() -> impl IntoResponse {
    axum::Json(json!({ "status": "ok" }))
}

/// Ready when the `discordy_admin` database answers; other dependencies degrade individual pages.
pub async fn readyz(State(state): State<AppState>) -> Response {
    if state.readiness.is_reachable().await {
        axum::Json(json!({ "status": "ready" })).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(json!({ "status": "database unreachable" })),
        )
            .into_response()
    }
}

pub async fn metrics(State(state): State<AppState>) -> Response {
    match &state.prometheus {
        Some(handle) => (
            [("content-type", "text/plain; version=0.0.4")],
            handle.render(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Records `http_requests_total` and `http_request_duration_seconds` per route template.
pub async fn track_requests(request: Request, next: Next) -> Response {
    let started = Instant::now();
    let method = request.method().to_string();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or_else(|| "unmatched".to_owned(), |p| p.as_str().to_owned());
    let response = next.run(request).await;
    let status = response.status().as_u16().to_string();
    metrics::counter!("http_requests_total", "method" => method.clone(), "route" => route.clone(), "status" => status).increment(1);
    metrics::histogram!("http_request_duration_seconds", "method" => method, "route" => route)
        .record(started.elapsed().as_secs_f64());
    response
}
