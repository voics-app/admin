use axum::http::{HeaderName, HeaderValue};
use axum::routing::{get, post, put};
use axum::{Router, middleware};
use tower_http::compression::CompressionLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use super::state::AppState;
use super::{auth, handlers, ops, spa};

const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; \
     font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'";

/// The full HTTP surface: `/admin-api/*` (authenticated), ops endpoints, and the SPA.
pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/me", get(handlers::me))
        .route("/overview", get(handlers::overview))
        .route("/usage", get(handlers::usage))
        .route("/voice/regions", get(handlers::voice_regions))
        .route("/voice/nodes/{node_id}/drain", post(handlers::drain_node))
        .route(
            "/voice/nodes/{node_id}/undrain",
            post(handlers::undrain_node),
        )
        .route("/platform/health", get(handlers::platform_health))
        .route(
            "/platform/health/series",
            get(handlers::platform_health_series),
        )
        .route("/users", get(handlers::search_users))
        .route("/users/{user_id}/suspend", post(handlers::suspend_user))
        .route("/guilds", get(handlers::search_guilds))
        .route("/reports", get(handlers::list_reports))
        .route(
            "/reports/{report_id}/resolve",
            post(handlers::resolve_report),
        )
        .route("/flags", get(handlers::list_flags))
        .route(
            "/flags/{key}",
            put(handlers::put_flag).delete(handlers::delete_flag),
        )
        .route("/audit", get(handlers::audit_log))
        .fallback(handlers::unknown_api_route)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_admin,
        ));

    let header = |name: &'static str, value: &'static str| {
        SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        )
    };

    Router::new()
        .nest("/admin-api", api)
        .route("/healthz", get(ops::healthz))
        .route("/readyz", get(ops::readyz))
        .route("/metrics", get(ops::metrics))
        .fallback(spa::serve)
        .layer(middleware::from_fn(ops::track_requests))
        .layer(header("content-security-policy", CONTENT_SECURITY_POLICY))
        .layer(header("x-content-type-options", "nosniff"))
        .layer(header("referrer-policy", "no-referrer"))
        .layer(header("x-frame-options", "DENY"))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
