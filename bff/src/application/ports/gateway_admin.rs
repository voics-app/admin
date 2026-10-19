use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::Serialize;

use super::PortResult;

/// Live gateway counters (P4 gateway `GET /internal/v1/admin/stats`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GatewayStats {
    pub sessions_online: u64,
    pub sessions_by_node: BTreeMap<String, u64>,
    pub guild_processes: u64,
    pub voice_allocations: u64,
}

#[async_trait]
pub trait GatewayAdmin: Send + Sync {
    async fn stats(&self) -> PortResult<GatewayStats>;
    async fn drain_voice_node(&self, node_id: &str) -> PortResult<()>;
    async fn undrain_voice_node(&self, node_id: &str) -> PortResult<()>;
}
