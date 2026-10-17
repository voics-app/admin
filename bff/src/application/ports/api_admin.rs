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

/// The admin surface of the api service's internal HTTP API (P4).
#[async_trait]
pub trait ApiAdmin: Send + Sync {
    async fn stats(&self) -> PortResult<ApiStats>;
    async fn search_users(&self, query: &UserQuery) -> PortResult<UserPage>;
    async fn search_guilds(&self, query: &str) -> PortResult<Vec<GuildSummary>>;
    async fn suspend_user(&self, user_id: &str, suspension: &Suspension) -> PortResult<()>;
}
