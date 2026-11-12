use async_trait::async_trait;
use serde::Serialize;

use super::PortResult;

/// Who a hand-written notification goes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushAudience {
    User(String),
    Everyone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManualPush {
    pub audience: PushAudience,
    pub title: String,
    pub body: String,
    /// Count the devices only; nothing is sent.
    pub dry_run: bool,
    /// The admin's email, for the notifier's log.
    pub actor: String,
}

/// What the notifier did (or, for a dry run, would do).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PushReport {
    pub devices: u64,
    pub queued: u64,
    pub dropped: u64,
    pub truncated: bool,
}

/// The notifier's internal `POST /internal/v1/manual-push`.
#[async_trait]
pub trait NotifierAdmin: Send + Sync {
    async fn send_manual(&self, push: &ManualPush) -> PortResult<PushReport>;
}
