use async_trait::async_trait;

use super::PortResult;
use crate::domain::audit::{AuditEntry, NewAuditEntry};

#[async_trait]
pub trait AuditLog: Send + Sync {
    async fn record(&self, entry: &NewAuditEntry) -> PortResult<()>;
    /// Newest first, starting strictly before `before_id` when given.
    async fn list(&self, limit: u32, before_id: Option<i64>) -> PortResult<Vec<AuditEntry>>;
}
