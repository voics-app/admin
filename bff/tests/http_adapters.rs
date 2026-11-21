//! Contract tests for the outbound HTTP adapters against stub servers (wiremock).

use admin_bff::application::ports::PortError;
use admin_bff::application::ports::api_admin::{
    ApiAdmin, ReportQuery, ReportStatus, Suspension, UserQuery,
};
use admin_bff::application::ports::gateway_admin::GatewayAdmin;
use admin_bff::application::ports::notifier_admin::{
    ManualPush, NotifierAdmin, PushAudience, PushReport,
};
use admin_bff::application::ports::platform_metrics::{HealthPanel, PlatformMetrics};
use admin_bff::application::ports::voice_directory::VoiceDirectory;
use admin_bff::infrastructure::etcd::voice_directory::EtcdVoiceDirectory;
use admin_bff::infrastructure::http::api_admin_client::ApiAdminClient;
use admin_bff::infrastructure::http::gateway_admin_client::GatewayAdminClient;
use admin_bff::infrastructure::http::internal_client::{InternalClient, http_client};
use admin_bff::infrastructure::http::notifier_admin_client::{
    NotifierAdminClient, UnconfiguredNotifier,
};
use admin_bff::infrastructure::victoria::platform_metrics::VictoriaPlatformMetrics;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chrono::{TimeZone, Utc};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn internal(server: &MockServer) -> InternalClient {
    InternalClient::new(http_client(), &server.uri(), "dev-internal-token")
}

#[tokio::test]
async fn api_stats_are_fetched_with_the_internal_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/internal/v1/admin/stats"))
        .and(header("X-Internal-Token", "dev-internal-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "users_total": 10, "guilds_total": 2, "channels_total": 9, "bots_total": 1, "signups_24h": 3
        })))
        .mount(&server)
        .await;
    let stats = ApiAdminClient::new(internal(&server))
        .stats()
        .await
        .unwrap();
    assert_eq!((stats.users_total, stats.signups_24h), (10, 3));
}

#[tokio::test]
async fn user_search_passes_paging_and_reads_the_cursor() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/internal/v1/admin/users"))
        .and(query_param("query", "ali"))
        .and(query_param("limit", "2"))
        .and(query_param("after", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "users": [{"id": "7", "username": "alice", "email": "a@x.dev", "unknown_field": true}],
            "next_after": "7"
        })))
        .mount(&server)
        .await;
    let query = UserQuery {
        query: "ali".into(),
        limit: 2,
        after: Some("5".into()),
    };
    let page = ApiAdminClient::new(internal(&server))
        .search_users(&query)
        .await
        .unwrap();
    assert_eq!(page.users[0].username, "alice");
    assert_eq!(page.next_after.as_deref(), Some("7"));
}

#[tokio::test]
async fn a_bare_full_user_array_implies_a_next_cursor() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/internal/v1/admin/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id": "1", "username": "a"}, {"id": "2", "username": "b"}
        ])))
        .mount(&server)
        .await;
    let query = UserQuery {
        query: String::new(),
        limit: 2,
        after: None,
    };
    let page = ApiAdminClient::new(internal(&server))
        .search_users(&query)
        .await
        .unwrap();
    assert_eq!(page.next_after.as_deref(), Some("2"));
}

#[tokio::test]
async fn guild_search_accepts_a_bare_array() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/internal/v1/admin/guilds"))
        .and(query_param("query", "lounge"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([{"id": "1", "name": "Lounge", "member_count": 4}])),
        )
        .mount(&server)
        .await;
    let guilds = ApiAdminClient::new(internal(&server))
        .search_guilds("lounge")
        .await
        .unwrap();
    assert_eq!(
        (guilds[0].name.as_str(), guilds[0].member_count),
        ("Lounge", Some(4))
    );
}

#[tokio::test]
async fn suspension_posts_reason_and_until() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/internal/v1/admin/users/42/suspend"))
        .and(body_json(json!({"reason": "spam", "until": null})))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    let suspension = Suspension {
        reason: "spam".into(),
        until: None,
    };
    ApiAdminClient::new(internal(&server))
        .suspend_user("42", &suspension)
        .await
        .unwrap();
}

#[tokio::test]
async fn upstream_statuses_map_to_port_errors() {
    let server = MockServer::start().await;
    Mock::given(path("/internal/v1/admin/users/404/suspend"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(path("/internal/v1/admin/users/400/suspend"))
        .respond_with(ResponseTemplate::new(400).set_body_string("bad until"))
        .mount(&server)
        .await;
    Mock::given(path("/internal/v1/admin/users/500/suspend"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let client = ApiAdminClient::new(internal(&server));
    let suspension = Suspension {
        reason: "spam".into(),
        until: None,
    };
    assert_eq!(
        client.suspend_user("404", &suspension).await,
        Err(PortError::NotFound)
    );
    assert_eq!(
        client.suspend_user("400", &suspension).await,
        Err(PortError::Rejected {
            status: 400,
            message: "bad until".into()
        })
    );
    assert!(matches!(
        client.suspend_user("500", &suspension).await,
        Err(PortError::Unavailable(_))
    ));
}

#[tokio::test]
async fn an_unreachable_service_is_unavailable() {
    let client = ApiAdminClient::new(InternalClient::new(
        http_client(),
        "http://127.0.0.1:9",
        "t",
    ));
    assert!(matches!(
        client.stats().await,
        Err(PortError::Unavailable(_))
    ));
}

#[tokio::test]
async fn gateway_stats_and_drain_controls() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/internal/v1/admin/stats"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "sessions_online": 5, "sessions_by_node": {"gw-0": 5}, "guild_processes": 2, "voice_allocations": 1
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/internal/v1/voice/nodes/nl-1/drain"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/internal/v1/voice/nodes/nl-1/undrain"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    let gateway = GatewayAdminClient::new(internal(&server));
    assert_eq!(gateway.stats().await.unwrap().sessions_by_node["gw-0"], 5);
    gateway.drain_voice_node("nl-1").await.unwrap();
    gateway.undrain_voice_node("nl-1").await.unwrap();
}

#[tokio::test]
async fn etcd_voice_nodes_are_read_by_prefix_and_malformed_values_skipped() {
    let server = MockServer::start().await;
    let node = json!({"node_id": "nl-1", "region": "nl-home", "ws_url": "wss://discordy-rtc-nl-home.voics.nl", "public_ip": "203.0.113.1",
        "udp_port": 50000, "users": 4, "cpu_pct": 12.5, "egress_bps": 256000, "draining": false, "version": "0.1.0"});
    Mock::given(method("POST"))
        .and(path("/v3/kv/range"))
        .and(body_json(json!({"key": B64.encode("/discordy/voice/nodes/"), "range_end": B64.encode("/discordy/voice/nodes0")})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"kvs": [
            {"key": B64.encode("/discordy/voice/nodes/nl-home/nl-1"), "value": B64.encode(node.to_string())},
            {"key": B64.encode("/discordy/voice/nodes/nl-home/bad"), "value": B64.encode("{not json")}
        ]})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"health": "true"})))
        .mount(&server)
        .await;

    let directory = EtcdVoiceDirectory::new(
        http_client(),
        vec!["http://127.0.0.1:9".into(), server.uri()],
    );
    let nodes = directory.nodes().await.unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(
        (nodes[0].node_id.as_str(), nodes[0].users, nodes[0].cpu_pct),
        ("nl-1", 4, 12.5)
    );
    assert!(directory.is_healthy().await);
}

#[tokio::test]
async fn etcd_without_registrations_has_no_nodes_and_all_endpoints_down_is_unavailable() {
    let server = MockServer::start().await;
    Mock::given(path("/v3/kv/range"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"header": {}})))
        .mount(&server)
        .await;
    assert!(
        EtcdVoiceDirectory::new(http_client(), vec![server.uri()])
            .nodes()
            .await
            .unwrap()
            .is_empty()
    );
    let down = EtcdVoiceDirectory::new(http_client(), vec!["http://127.0.0.1:9".into()]);
    assert!(matches!(down.nodes().await, Err(PortError::Unavailable(_))));
    assert!(!down.is_healthy().await);
}

#[tokio::test]
async fn victoria_target_availability_joins_up_and_total_per_job() {
    let server = MockServer::start().await;
    let vector = |rows: serde_json::Value| json!({"status": "success", "data": {"resultType": "vector", "result": rows}});
    Mock::given(path("/api/v1/query"))
        .and(query_param("query", "sum by (job) (up)"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vector(
            json!([{"metric": {"job": "voics-apps/api"}, "value": [1, "1"]}]),
        )))
        .mount(&server)
        .await;
    Mock::given(path("/api/v1/query"))
        .and(query_param("query", "count by (job) (up)"))
        .respond_with(ResponseTemplate::new(200).set_body_json(vector(
            json!([{"metric": {"job": "voics-apps/api"}, "value": [1, "2"]}]),
        )))
        .mount(&server)
        .await;
    Mock::given(path("/api/v1/query"))
        .and(query_param(
            "query",
            "sum by (redpanda_group) (redpanda_kafka_consumer_group_lag_sum)",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(vector(json!([
            {"metric": {"redpanda_group": "gateway"}, "value": [1, "42"]},
            {"metric": {"redpanda_group": "broken"}, "value": [1, "NaN"]}
        ]))))
        .mount(&server)
        .await;
    let metrics = VictoriaPlatformMetrics::new(http_client(), &server.uri());
    let targets = metrics.target_availability().await.unwrap();
    assert_eq!(
        (targets[0].service.as_str(), targets[0].up, targets[0].total),
        ("api", 1, 2)
    );
    let lag = metrics.snapshot(HealthPanel::ConsumerLag).await.unwrap();
    assert_eq!(lag.len(), 1);
    assert_eq!((lag[0].label.as_str(), lag[0].value), ("gateway", 42.0));
}

#[tokio::test]
async fn victoria_range_queries_return_labelled_points() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/query_range"))
        .and(query_param("step", "60s"))
        .and(query_param("start", "1792152000"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "success", "data": {"resultType": "matrix", "result": [
            {"metric": {"job": "gateway"}, "values": [[1792152000, "0.5"], [1792152060, "0.25"]]}
        ]}})))
        .mount(&server)
        .await;
    let metrics = VictoriaPlatformMetrics::new(http_client(), &server.uri());
    let from = Utc.timestamp_opt(1_792_152_000, 0).unwrap();
    let series = metrics
        .series(
            HealthPanel::ErrorRatio,
            from,
            from + chrono::Duration::hours(1),
            60,
        )
        .await
        .unwrap();
    assert_eq!(series[0].label, "gateway");
    assert_eq!(
        series[0].points,
        vec![(from, 0.5), (from + chrono::Duration::seconds(60), 0.25)]
    );
}

#[tokio::test]
async fn victoria_notifier_panels_query_drop_counts_and_preserve_provider_labels() {
    let server = MockServer::start().await;
    let queries = [
        (
            HealthPanel::PushDrops,
            r#"sum by (provider) (increase(notifier_pushes_total{outcome="dropped"}[5m]))"#,
            "provider",
            "fcm",
            "45000",
        ),
        (
            HealthPanel::PushQueue,
            "sum by (instance) (notifier_queue_depth)",
            "instance",
            "notifier-1",
            "10000",
        ),
        (
            HealthPanel::PushInFlight,
            "sum by (instance) (notifier_in_flight)",
            "instance",
            "notifier-1",
            "100",
        ),
    ];
    let metrics = VictoriaPlatformMetrics::new(http_client(), &server.uri());
    for (panel, query, key, label, value) in queries {
        Mock::given(path("/api/v1/query"))
            .and(query_param("query", query))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"status": "success", "data": {"result": [
                    {"metric": {key: label}, "value": [1, value]}
                ]}}),
            ))
            .expect(1)
            .mount(&server)
            .await;
        let values = metrics.snapshot(panel).await.unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].label, label);
        assert_eq!(values[0].value, value.parse::<f64>().unwrap());
    }
}

#[tokio::test]
async fn victoria_query_errors_are_unavailable() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/query"))
        .respond_with(
            ResponseTemplate::new(422)
                .set_body_json(json!({"status": "error", "error": "parse error"})),
        )
        .mount(&server)
        .await;
    let metrics = VictoriaPlatformMetrics::new(http_client(), &server.uri());
    assert!(matches!(
        metrics.snapshot(HealthPanel::RequestRate).await,
        Err(PortError::Unavailable(_))
    ));
}

fn report_json(status: &str) -> serde_json::Value {
    json!({
        "id": "9", "kind": "message", "category": "spam", "note": null, "status": status,
        "created_at": "2026-11-01T10:00:00.000Z",
        "reporter": {"id": "1", "username": "ben"},
        "reported": {"id": "2", "username": null},
        "channel_id": "3", "guild_id": null, "message_id": "4", "delivery_mode": "ephemeral",
        "content": "text", "content_source": "reporter",
        "resolved_at": null, "resolved_by": null, "resolution_note": null
    })
}

#[tokio::test]
async fn reports_are_listed_with_status_and_cursor() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/internal/v1/admin/reports"))
        .and(query_param("status", "open"))
        .and(query_param("limit", "25"))
        .and(query_param("after", "50"))
        .and(header("X-Internal-Token", "dev-internal-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "reports": [report_json("open")], "next_after": "9"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let page = ApiAdminClient::new(internal(&server))
        .list_reports(&ReportQuery {
            status: ReportStatus::Open,
            limit: 25,
            after: Some("50".into()),
        })
        .await
        .unwrap();
    assert_eq!(page.next_after.as_deref(), Some("9"));
    let report = &page.reports[0];
    assert_eq!(report.reporter.username.as_deref(), Some("ben"));
    assert_eq!(report.reported.username, None);
    assert_eq!(report.content_source.as_deref(), Some("reporter"));
    assert_eq!(report.guild_id, None);
}

#[tokio::test]
async fn resolving_a_report_posts_the_resolution_and_returns_the_report() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/internal/v1/admin/reports/9/resolve"))
        .and(body_json(json!({
            "resolution": "dismissed", "resolved_by": "ops@discordy.dev", "note": "not abuse"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(report_json("dismissed")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/internal/v1/admin/reports/8/resolve"))
        .respond_with(ResponseTemplate::new(409).set_body_string("closed"))
        .mount(&server)
        .await;
    let client = ApiAdminClient::new(internal(&server));
    let report = client
        .resolve_report(
            "9",
            ReportStatus::Dismissed,
            "ops@discordy.dev",
            Some("not abuse"),
        )
        .await
        .unwrap();
    assert_eq!(report.status, "dismissed");
    assert_eq!(
        client
            .resolve_report("8", ReportStatus::Resolved, "ops@discordy.dev", None)
            .await,
        Err(PortError::Rejected {
            status: 409,
            message: "closed".into()
        })
    );
}

#[tokio::test]
async fn a_manual_push_posts_the_target_text_and_actor_with_the_internal_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/internal/v1/manual-push"))
        .and(header("X-Internal-Token", "dev-internal-token"))
        .and(body_json(json!({
            "target": {"user_id": "42"}, "title": "Hi", "body": "There",
            "dry_run": false, "actor": "ops@discordy.dev"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "devices": 2, "queued": 2, "dropped": 0, "truncated": false
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/internal/v1/manual-push"))
        .and(body_json(json!({
            "target": "all", "title": "Hi", "body": "There",
            "dry_run": true, "actor": "ops@discordy.dev"
        })))
        .respond_with(ResponseTemplate::new(409).set_body_string("busy"))
        .mount(&server)
        .await;
    let client = NotifierAdminClient::new(internal(&server));
    let push = |audience, dry_run| ManualPush {
        audience,
        title: "Hi".into(),
        body: "There".into(),
        dry_run,
        actor: "ops@discordy.dev".into(),
    };
    assert_eq!(
        client
            .send_manual(&push(PushAudience::User("42".into()), false))
            .await,
        Ok(PushReport {
            devices: 2,
            queued: 2,
            dropped: 0,
            truncated: false
        })
    );
    assert_eq!(
        client
            .send_manual(&push(PushAudience::Everyone, true))
            .await,
        Err(PortError::Rejected {
            status: 409,
            message: "busy".into()
        })
    );
    assert!(matches!(
        UnconfiguredNotifier
            .send_manual(&push(PushAudience::Everyone, true))
            .await,
        Err(PortError::Unavailable(_))
    ));
}
