use async_trait::async_trait;
use serde::Serialize;

use super::PortResult;

/// Platform totals reported by the api service (P4 `GET /internal/v1/admin/stats`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ApiStats {
    pub users_total: u64,
    pub guilds_total: u64,
    pub channels_total: u64,
    pub bots_total: u64,
    pub signups_24h: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UserSummary {
    pub id: String,
    pub username: String,
    pub global_name: Option<String>,
    pub email: Option<String>,
    pub bot: bool,
    pub created_at: Option<String>,
    pub suspended_until: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UserPage {
    pub users: Vec<UserSummary>,
    /// Cursor for the next page; `None` on the last page.
    pub next_after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuildSummary {
    pub id: String,
    pub name: String,
    pub owner_id: Option<String>,
    pub member_count: Option<u64>,
    pub voice_region: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserQuery {
    pub query: String,
    pub limit: u32,
    pub after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suspension {
    pub reason: String,
    /// RFC 3339 end of the suspension; `None` suspends indefinitely.
    pub until: Option<String>,
}

/// A user named in a report. The name is `None` once the account no longer resolves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReportParty {
    pub id: String,
    pub username: Option<String>,
}

/// A report in the review queue. `content` is the reported message text: copied by the api at
/// report time (`content_source` `server`) or submitted by the reporter for ephemeral and P2P
/// chats the server never stores (`reporter`, unverified). The BFF only passes it through.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReportSummary {
    pub id: String,
    pub kind: String,
    pub category: String,
    pub note: Option<String>,
    pub status: String,
    pub created_at: String,
    pub reporter: ReportParty,
    pub reported: ReportParty,
    pub channel_id: Option<String>,
    pub guild_id: Option<String>,
    pub message_id: Option<String>,
    pub delivery_mode: Option<String>,
    pub content: Option<String>,
    pub content_source: Option<String>,
    pub resolved_at: Option<String>,
    pub resolved_by: Option<String>,
    pub resolution_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReportPage {
    pub reports: Vec<ReportSummary>,
    /// Cursor for the next page; `None` on the last page.
    pub next_after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportQuery {
    pub status: ReportStatus,
    pub limit: u32,
    pub after: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportStatus {
    Open,
    Resolved,
    Dismissed,
}

impl ReportStatus {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "open" => Some(Self::Open),
            "resolved" => Some(Self::Resolved),
            "dismissed" => Some(Self::Dismissed),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Resolved => "resolved",
            Self::Dismissed => "dismissed",
        }
    }
}

/// The admin surface of the api service's internal HTTP API (P4).
#[async_trait]
pub trait ApiAdmin: Send + Sync {
    async fn stats(&self) -> PortResult<ApiStats>;
    async fn search_users(&self, query: &UserQuery) -> PortResult<UserPage>;
    async fn search_guilds(&self, query: &str) -> PortResult<Vec<GuildSummary>>;
    async fn suspend_user(&self, user_id: &str, suspension: &Suspension) -> PortResult<()>;
    async fn list_reports(&self, query: &ReportQuery) -> PortResult<ReportPage>;
    /// Closes an open report as resolved or dismissed (`status` is one of those two).
    async fn resolve_report(
        &self,
        report_id: &str,
        status: ReportStatus,
        resolved_by: &str,
        note: Option<&str>,
    ) -> PortResult<ReportSummary>;
}
