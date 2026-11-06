//! Router-level tests: auth middleware and handlers over in-memory ports.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::router::router;
use super::state::{AppState, Ports};
use crate::application::fakes::*;
use crate::application::ports::platform_metrics::TargetAvailability;
use crate::application::use_cases::authenticate_admin::AuthenticateAdmin;
use crate::domain::admin_allowlist::AdminAllowlist;

const ADMIN: &str = "ops@discordy.dev";

struct Harness {
    app: Router,
    api: Arc<FakeApi>,
    gateway: Arc<FakeGateway>,
    audit: Arc<FakeAudit>,
    rollups: Arc<FakeRollups>,
}

fn harness_with(accept_dev_header: bool, metrics: FakeMetrics) -> Harness {
    let api = Arc::new(FakeApi::default());
    let gateway = Arc::new(FakeGateway::default());
    let audit = Arc::new(FakeAudit::default());
    let rollups = Arc::new(FakeRollups::default());
    let ports = Ports {
        api: api.clone(),
        gateway: gateway.clone(),
        rollups: rollups.clone(),
        voice: Arc::new(FakeVoice::default()),
        metrics: Arc::new(metrics),
        flags: Arc::new(FakeFlags::default()),
        audit: audit.clone(),
        clock: Arc::new(FixedClock::default()),
    };
    let auth = AuthenticateAdmin::new(
        Some(Arc::new(FakeVerifier)),
        AdminAllowlist::parse(ADMIN),
        accept_dev_header,
    );
    Harness {
        app: router(AppState::new(ports, auth, None)),
        api,
        gateway,
        audit,
        rollups,
    }
}

fn harness() -> Harness {
    harness_with(false, FakeMetrics::default())
}

fn request(method: Method, uri: &str) -> axum::http::request::Builder {
    Request::builder().method(method).uri(uri)
}

fn as_admin(method: Method, uri: &str) -> axum::http::request::Builder {
    request(method.clone(), uri)
        .header("cf-access-jwt-assertion", format!("valid:{ADMIN}"))
        .header("x-discordy-admin", "1")
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
    send(app, as_admin(Method::GET, uri).body(Body::empty()).unwrap()).await
}

fn json_body(builder: axum::http::request::Builder, value: Value) -> Request<Body> {
    builder
        .header("content-type", "application/json")
        .body(Body::from(value.to_string()))
        .unwrap()
}

// --- auth middleware ---

#[tokio::test]
async fn requests_without_credentials_are_unauthorized() {
    let h = harness();
    let (status, body) = send(
        &h.app,
        request(Method::GET, "/admin-api/me")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], 40001);
}

#[tokio::test]
async fn forged_assertions_are_unauthorized() {
    let h = harness();
    let req = request(Method::GET, "/admin-api/me")
        .header("cf-access-jwt-assertion", "forged")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&h.app, req).await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn verified_emails_outside_the_allowlist_are_forbidden() {
    let h = harness();
    let req = request(Method::GET, "/admin-api/me")
        .header("cf-access-jwt-assertion", "valid:eve@discordy.dev")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&h.app, req).await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn an_identity_provider_outage_is_service_unavailable() {
    let h = harness();
    let req = request(Method::GET, "/admin-api/me")
        .header("cf-access-jwt-assertion", "keys-down")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&h.app, req).await.0, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn admins_see_their_identity() {
    let h = harness();
    let (status, body) = get(&h.app, "/admin-api/me").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "email": ADMIN }));
}

#[tokio::test]
async fn the_dev_header_is_honoured_only_in_local_mode() {
    let dev = |app: &Router| {
        let app = app.clone();
        async move {
            send(
                &app,
                request(Method::GET, "/admin-api/me")
                    .header("x-dev-admin-email", ADMIN)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .0
        }
    };
    assert_eq!(
        dev(&harness_with(true, FakeMetrics::default()).app).await,
        StatusCode::OK
    );
    assert_eq!(dev(&harness().app).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn mutations_without_the_csrf_header_are_refused() {
    let h = harness();
    let req = request(Method::POST, "/admin-api/voice/nodes/nl-1/drain")
        .header("cf-access-jwt-assertion", format!("valid:{ADMIN}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&h.app, req).await.0, StatusCode::FORBIDDEN);
    assert!(h.gateway.drain_calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn ops_endpoints_need_no_admin_credentials() {
    let h = harness();
    assert_eq!(
        send(
            &h.app,
            request(Method::GET, "/healthz")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &h.app,
            request(Method::GET, "/readyz").body(Body::empty()).unwrap()
        )
        .await
        .0,
        StatusCode::OK
    );
    *h.rollups.fail_reads.lock().unwrap() = true;
    assert_eq!(
        send(
            &h.app,
            request(Method::GET, "/readyz").body(Body::empty()).unwrap()
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn unknown_api_routes_are_json_404s_after_auth() {
    let h = harness();
    let (status, body) = get(&h.app, "/admin-api/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], 10000);
}

// --- handlers ---

#[tokio::test]
async fn overview_reports_kpis_and_degraded_sources() {
    let h = harness();
    *h.gateway.stats.lock().unwrap() = Err(unavailable());
    let (status, body) = get(&h.app, "/admin-api/overview").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["platform"]["users_total"], 1200);
    assert_eq!(body["gateway"], Value::Null);
    assert_eq!(body["degraded"], json!(["gateway"]));
    assert_eq!(body["voice_nodes"], 1);
}

#[tokio::test]
async fn usage_defaults_to_24h_and_validates_the_range() {
    let h = harness();
    let (status, body) = get(&h.app, "/admin-api/usage").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["range"], "24h");
    assert_eq!(body["granularity"], "hour");
    assert_eq!(body["points"].as_array().unwrap().len(), 24);
    assert_eq!(
        get(&h.app, "/admin-api/usage?range=1y").await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn voice_regions_list_nodes() {
    let h = harness();
    let (status, body) = get(&h.app, "/admin-api/voice/regions").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["regions"][0]["region"], "nl-home");
    assert_eq!(body["regions"][0]["node_list"][0]["node_id"], "nl-1");
}

#[tokio::test]
async fn draining_a_node_is_audited_with_the_admin_email() {
    let h = harness();
    let (status, _) = send(
        &h.app,
        as_admin(Method::POST, "/admin-api/voice/nodes/nl-1/drain")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        *h.gateway.drain_calls.lock().unwrap(),
        vec![("nl-1".to_owned(), true)]
    );
    assert_eq!(h.audit.entries.lock().unwrap()[0].actor_email, ADMIN);
}

#[tokio::test]
async fn gateway_failures_surface_as_bad_gateway() {
    let h = harness();
    *h.gateway.drain_result.lock().unwrap() = Err(unavailable());
    let (status, body) = send(
        &h.app,
        as_admin(Method::POST, "/admin-api/voice/nodes/nl-1/undrain")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["code"], 50200);
}

#[tokio::test]
async fn platform_health_lists_services() {
    let metrics = FakeMetrics::default();
    *metrics.availability.lock().unwrap() = Ok(vec![TargetAvailability {
        service: "api".into(),
        up: 1,
        total: 1,
    }]);
    let h = harness_with(false, metrics);
    let (status, body) = get(&h.app, "/admin-api/platform/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["services"][0],
        json!({"service": "api", "status": "healthy", "instances_up": 1, "instances_total": 1, "request_rate": null, "error_ratio": null, "latency_p95_ms": null})
    );
}

#[tokio::test]
async fn platform_health_series_validates_the_panel() {
    let h = harness();
    let (status, body) = get(
        &h.app,
        "/admin-api/platform/health/series?panel=error_ratio&range=24h",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["series"][0]["label"], "api");
    assert_eq!(
        get(&h.app, "/admin-api/platform/health/series?panel=cpu")
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn user_search_and_suspension() {
    let h = harness();
    let (status, body) = get(&h.app, "/admin-api/users?query=ali&limit=5").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["users"], json!([]));
    assert_eq!(h.api.user_queries.lock().unwrap()[0].limit, 5);

    let req = json_body(
        as_admin(Method::POST, "/admin-api/users/42/suspend"),
        json!({"reason": "spam"}),
    );
    assert_eq!(send(&h.app, req).await.0, StatusCode::NO_CONTENT);
    assert_eq!(h.api.suspensions.lock().unwrap()[0].0, "42");

    let bad = json_body(
        as_admin(Method::POST, "/admin-api/users/42/suspend"),
        json!({"reason": ""}),
    );
    assert_eq!(send(&h.app, bad).await.0, StatusCode::BAD_REQUEST);
    let malformed = as_admin(Method::POST, "/admin-api/users/42/suspend")
        .header("content-type", "application/json")
        .body(Body::from("{"))
        .unwrap();
    assert_eq!(send(&h.app, malformed).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn guild_search_returns_guilds() {
    let h = harness();
    let (status, body) = get(&h.app, "/admin-api/guilds?query=x").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"guilds": []}));
}

#[tokio::test]
async fn feature_flag_lifecycle() {
    let h = harness();
    let put = json_body(
        as_admin(Method::PUT, "/admin-api/flags/watch_together"),
        json!({"description": "Sync", "enabled": true, "rollout_percent": 30}),
    );
    let (status, body) = send(&h.app, put).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["rollout_percent"], 30);
    assert_eq!(body["updated_by"], ADMIN);

    let (_, list) = get(&h.app, "/admin-api/flags").await;
    assert_eq!(list["flags"][0]["key"], "watch_together");

    let invalid = json_body(
        as_admin(Method::PUT, "/admin-api/flags/Bad%20Key"),
        json!({"enabled": true}),
    );
    assert_eq!(send(&h.app, invalid).await.0, StatusCode::BAD_REQUEST);

    let delete = as_admin(Method::DELETE, "/admin-api/flags/watch_together")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&h.app, delete).await.0, StatusCode::NO_CONTENT);
    let again = as_admin(Method::DELETE, "/admin-api/flags/watch_together")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&h.app, again).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn audit_log_pages_with_a_cursor() {
    let h = harness();
    for node in ["a", "b", "c"] {
        send(
            &h.app,
            as_admin(
                Method::POST,
                &format!("/admin-api/voice/nodes/{node}/drain"),
            )
            .body(Body::empty())
            .unwrap(),
        )
        .await;
    }
    let (status, body) = get(&h.app, "/admin-api/audit?limit=2").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["entries"][0]["target"], "c");
    assert_eq!(body["entries"][0]["details"], json!({}));
    assert_eq!(body["next_before"], 2);
}

#[tokio::test]
async fn responses_carry_security_headers() {
    let h = harness();
    let response = h
        .app
        .clone()
        .oneshot(
            request(Method::GET, "/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert!(
        response.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("frame-ancestors 'none'")
    );
}

#[tokio::test]
async fn missing_static_files_are_404_not_the_spa() {
    let h = harness();
    let response = h
        .app
        .clone()
        .oneshot(
            request(Method::GET, "/assets/missing.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn report_queue_lists_and_closes_reports() {
    let h = harness();
    *h.api.reports.lock().unwrap() = Ok(crate::application::ports::api_admin::ReportPage {
        reports: vec![report_summary("1")],
        next_after: None,
    });
    let (status, body) = get(&h.app, "/admin-api/reports?status=open&limit=10").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["reports"][0]["content"], "buy my coins");
    assert_eq!(body["reports"][0]["reporter"]["username"], "ben");
    assert_eq!(h.api.report_queries.lock().unwrap()[0].limit, 10);
    assert_eq!(
        get(&h.app, "/admin-api/reports?status=all").await.0,
        StatusCode::BAD_REQUEST
    );

    let close = json_body(
        as_admin(Method::POST, "/admin-api/reports/1/resolve"),
        json!({"resolution": "dismissed", "note": "not abuse"}),
    );
    let (status, closed) = send(&h.app, close).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(closed["id"], "1");
    {
        let closures = h.api.report_closures.lock().unwrap();
        assert_eq!(
            closures[0].2, ADMIN,
            "the signed-in operator is the reviewer"
        );
        let entries = h.audit.entries.lock().unwrap();
        assert_eq!(entries[0].actor_email, ADMIN);
        assert_eq!(entries[0].action.as_str(), "report.dismiss");
    }

    let bad = json_body(
        as_admin(Method::POST, "/admin-api/reports/1/resolve"),
        json!({"resolution": "ignored"}),
    );
    assert_eq!(send(&h.app, bad).await.0, StatusCode::BAD_REQUEST);

    *h.api.resolve_result.lock().unwrap() = Err(crate::application::ports::PortError::Rejected {
        status: 409,
        message: "closed".into(),
    });
    let again = json_body(
        as_admin(Method::POST, "/admin-api/reports/1/resolve"),
        json!({"resolution": "resolved"}),
    );
    assert_eq!(send(&h.app, again).await.0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn report_routes_need_an_admin() {
    let h = harness();
    let anonymous = Request::builder()
        .uri("/admin-api/reports")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&h.app, anonymous).await.0, StatusCode::UNAUTHORIZED);
}
