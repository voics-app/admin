use async_trait::async_trait;

use super::internal_client::InternalClient;
use crate::application::ports::PortResult;
use crate::application::ports::gateway_admin::{GatewayAdmin, GatewayStats};
use crate::infrastructure::contracts::GatewayStatsDto;

/// Adapter for the gateway's admin and voice-drain endpoints (P4).
pub struct GatewayAdminClient {
    client: InternalClient,
}

impl GatewayAdminClient {
    pub fn new(client: InternalClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl GatewayAdmin for GatewayAdminClient {
    async fn stats(&self) -> PortResult<GatewayStats> {
        let dto: GatewayStatsDto = self.client.get_json("/internal/v1/admin/stats", &[]).await?;
        Ok(GatewayStats {
            sessions_online: dto.sessions_online,
            sessions_by_node: dto.sessions_by_node,
            guild_processes: dto.guild_processes,
            voice_allocations: dto.voice_allocations,
        })
    }

    async fn drain_voice_node(&self, node_id: &str) -> PortResult<()> {
        self.client.post::<()>(&format!("/internal/v1/voice/nodes/{node_id}/drain"), None).await
    }

    async fn undrain_voice_node(&self, node_id: &str) -> PortResult<()> {
        self.client.post::<()>(&format!("/internal/v1/voice/nodes/{node_id}/undrain"), None).await
    }
}
