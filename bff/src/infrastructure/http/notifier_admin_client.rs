use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use super::internal_client::InternalClient;
use crate::application::ports::notifier_admin::{
    ManualPush, NotifierAdmin, PushAudience, PushReport,
};
use crate::application::ports::{PortError, PortResult};

/// Adapter for the notifier's `POST /internal/v1/manual-push`. Its HTTP client needs a longer
/// timeout than the others: a broadcast is paced by the notifier's queue.
pub struct NotifierAdminClient {
    client: InternalClient,
}

impl NotifierAdminClient {
    pub fn new(client: InternalClient) -> Self {
        Self { client }
    }
}

#[derive(Deserialize)]
struct ReportDto {
    devices: u64,
    queued: u64,
    dropped: u64,
    #[serde(default)]
    truncated: bool,
}

#[async_trait]
impl NotifierAdmin for NotifierAdminClient {
    async fn send_manual(&self, push: &ManualPush) -> PortResult<PushReport> {
        let target = match &push.audience {
            PushAudience::Everyone => json!("all"),
            PushAudience::User(id) => json!({ "user_id": id }),
        };
        let body = json!({
            "target": target,
            "title": push.title,
            "body": push.body,
            "dry_run": push.dry_run,
            "actor": push.actor,
        });
        let report: ReportDto = self
            .client
            .post_json("/internal/v1/manual-push", &body)
            .await?;
        Ok(PushReport {
            devices: report.devices,
            queued: report.queued,
            dropped: report.dropped,
            truncated: report.truncated,
        })
    }
}

/// Stands in when `NOTIFIER_INTERNAL_URL` is not set: the page answers that sending is not
/// configured instead of the service refusing to start.
pub struct UnconfiguredNotifier;

#[async_trait]
impl NotifierAdmin for UnconfiguredNotifier {
    async fn send_manual(&self, _push: &ManualPush) -> PortResult<PushReport> {
        Err(PortError::Unavailable(
            "NOTIFIER_INTERNAL_URL is not configured".into(),
        ))
    }
}
