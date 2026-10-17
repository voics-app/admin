//! Wire shapes pinned by the master plan (P4, P6, P8), kept in one module so they can be
//! swapped for the generated `discordy-contracts` crate once it is published.
//!
//! Every struct deserializes only the fields the admin BFF needs; unknown fields (including any
//! message content) are ignored by serde and never materialized.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Topics consumed by the usage aggregator (P6).
pub const TOPICS: [&str; 6] = [
    "discordy.guild.v1",
    "discordy.message.v1",
    "discordy.user.v1",
    "discordy.interaction.v1",
    "discordy.telemetry.v1",
    "discordy.audit.v1",
];

/// etcd prefix under which voice nodes register (P8).
pub const VOICE_NODES_PREFIX: &str = "/discordy/voice/nodes/";

/// Header carrying the shared service secret on `/internal/*` calls (P2).
pub const INTERNAL_TOKEN_HEADER: &str = "X-Internal-Token";

/// P6 event envelope, without `data`'s content.
#[derive(Debug, Clone, Deserialize)]
pub struct EnvelopeHead {
    pub id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub occurred_at: String,
    #[serde(default)]
    pub target_user_ids: Option<Vec<String>>,
    /// Kept raw so a payload whose shape we do not know never fails the whole envelope.
    #[serde(default)]
    pub data: Option<Box<serde_json::value::RawValue>>,
}

/// The union of id-only fields the aggregator reads from any dispatch payload.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EventData {
    #[serde(default)]
    pub author: Option<AuthorRef>,
    #[serde(default)]
    pub user: Option<AuthorRef>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub member: Option<MemberRef>,
    // USAGE_TICK (telemetry, produced by gateway and voice nodes).
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub interval_s: Option<u32>,
    #[serde(default)]
    pub sessions_online: Option<u64>,
    #[serde(default)]
    pub ephemeral_relayed: Option<u64>,
    #[serde(default)]
    pub p2p_signaled: Option<u64>,
    #[serde(default)]
    pub voice_users: Option<u64>,
    #[serde(default)]
    pub active_calls: Option<u64>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub active_user_ids: Option<Vec<String>>,
    // VOICE_QUALITY (P7 op 23 payload as republished by the gateway).
    #[serde(default)]
    pub jitter_ms: Option<f64>,
    #[serde(default)]
    pub loss_pct: Option<f64>,
    #[serde(default)]
    pub rtt_ms: Option<f64>,
    #[serde(default)]
    pub bitrate_kbps: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthorRef {
    pub id: String,
    #[serde(default)]
    pub bot: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemberRef {
    #[serde(default)]
    pub user: Option<AuthorRef>,
}

/// P4 api `GET /internal/v1/admin/stats`.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiStatsDto {
    pub users_total: u64,
    pub guilds_total: u64,
    pub channels_total: u64,
    pub bots_total: u64,
    pub signups_24h: u64,
}

/// P4 api `GET /internal/v1/admin/users` page. Accepts `{users|items, next_after}` or a bare array.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum UserListDto {
    Page {
        #[serde(alias = "items")]
        users: Vec<UserDto>,
        #[serde(default)]
        next_after: Option<String>,
    },
    Bare(Vec<UserDto>),
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserDto {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub global_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub bot: bool,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub suspended_until: Option<String>,
}

/// P4 api `GET /internal/v1/admin/guilds`. Accepts `{guilds|items}` or a bare array.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum GuildListDto {
    Page {
        #[serde(alias = "items")]
        guilds: Vec<GuildDto>,
    },
    Bare(Vec<GuildDto>),
}

#[derive(Debug, Clone, Deserialize)]
pub struct GuildDto {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub owner_id: Option<String>,
    #[serde(default, alias = "approximate_member_count")]
    pub member_count: Option<u64>,
    #[serde(default)]
    pub voice_region: Option<String>,
}

/// P4 api `POST /internal/v1/admin/users/{id}/suspend` body.
#[derive(Debug, Clone, Serialize)]
pub struct SuspendRequestDto<'a> {
    pub reason: &'a str,
    pub until: Option<&'a str>,
}

/// P4 gateway `GET /internal/v1/admin/stats`.
#[derive(Debug, Clone, Deserialize)]
pub struct GatewayStatsDto {
    pub sessions_online: u64,
    #[serde(default)]
    pub sessions_by_node: BTreeMap<String, u64>,
    #[serde(default)]
    pub guild_processes: u64,
    #[serde(default)]
    pub voice_allocations: u64,
}

/// P8 etcd value at `/discordy/voice/nodes/<region>/<node_id>`.
#[derive(Debug, Clone, Deserialize)]
pub struct VoiceNodeDto {
    pub node_id: String,
    pub region: String,
    pub ws_url: String,
    pub public_ip: String,
    pub udp_port: u16,
    pub users: u64,
    pub cpu_pct: f64,
    pub egress_bps: u64,
    pub draining: bool,
    #[serde(default)]
    pub version: String,
}
