use async_trait::async_trait;

use super::PortResult;
use crate::domain::voice_region::VoiceNode;

/// Voice-server discovery (etcd, P8).
#[async_trait]
pub trait VoiceDirectory: Send + Sync {
    async fn nodes(&self) -> PortResult<Vec<VoiceNode>>;
    async fn is_healthy(&self) -> bool;
}
