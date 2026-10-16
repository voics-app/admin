use std::sync::Arc;

use crate::application::error::{AppError, AppResult};
use crate::application::ports::audit_log::AuditLog;
use crate::application::ports::gateway_admin::GatewayAdmin;
use crate::domain::audit::{AuditAction, NewAuditEntry};

/// Drains (stops new allocations to) or undrains a voice node via the gateway, and audits it.
pub struct SetVoiceNodeDraining {
    gateway: Arc<dyn GatewayAdmin>,
    audit: Arc<dyn AuditLog>,
}

impl SetVoiceNodeDraining {
    pub fn new(gateway: Arc<dyn GatewayAdmin>, audit: Arc<dyn AuditLog>) -> Self {
        Self { gateway, audit }
    }

    pub async fn execute(&self, actor_email: &str, node_id: &str, draining: bool) -> AppResult<()> {
        if !is_valid_node_id(node_id) {
            return Err(AppError::Validation("node id must be 1-128 chars of letters, digits, '.', '_' or '-'".into()));
        }
        let action = if draining {
            self.gateway.drain_voice_node(node_id).await?;
            AuditAction::DrainVoiceNode
        } else {
            self.gateway.undrain_voice_node(node_id).await?;
            AuditAction::UndrainVoiceNode
        };
        let entry = NewAuditEntry { actor_email: actor_email.to_owned(), action, target: node_id.to_owned(), details: "{}".into() };
        self.audit.record(&entry).await?;
        Ok(())
    }
}

fn is_valid_node_id(node_id: &str) -> bool {
    (1..=128).contains(&node_id.len()) && node_id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{FakeAudit, FakeGateway, unavailable};

    fn setup() -> (Arc<FakeGateway>, Arc<FakeAudit>, SetVoiceNodeDraining) {
        let gateway = Arc::new(FakeGateway::default());
        let audit = Arc::new(FakeAudit::default());
        let use_case = SetVoiceNodeDraining::new(gateway.clone(), audit.clone());
        (gateway, audit, use_case)
    }

    #[tokio::test]
    async fn draining_calls_the_gateway_and_audits() {
        let (gateway, audit, use_case) = setup();
        use_case.execute("ops@discordy.dev", "voice-nl-1", true).await.unwrap();
        assert_eq!(*gateway.drain_calls.lock().unwrap(), vec![("voice-nl-1".to_owned(), true)]);
        let entries = audit.entries.lock().unwrap();
        assert_eq!(entries[0].action, AuditAction::DrainVoiceNode);
        assert_eq!(entries[0].actor_email, "ops@discordy.dev");
        assert_eq!(entries[0].target, "voice-nl-1");
    }

    #[tokio::test]
    async fn undraining_is_audited_separately() {
        let (gateway, audit, use_case) = setup();
        use_case.execute("ops@discordy.dev", "voice-nl-1", false).await.unwrap();
        assert_eq!(*gateway.drain_calls.lock().unwrap(), vec![("voice-nl-1".to_owned(), false)]);
        assert_eq!(audit.entries.lock().unwrap()[0].action, AuditAction::UndrainVoiceNode);
    }

    #[tokio::test]
    async fn rejects_node_ids_that_could_escape_the_url_path() {
        let (gateway, _, use_case) = setup();
        for bad in ["", "../admin", "a/b", "node id"] {
            assert!(matches!(use_case.execute("ops@discordy.dev", bad, true).await, Err(AppError::Validation(_))), "{bad}");
        }
        assert!(gateway.drain_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_failed_drain_is_not_audited() {
        let (gateway, audit, use_case) = setup();
        *gateway.drain_result.lock().unwrap() = Err(unavailable());
        assert!(use_case.execute("ops@discordy.dev", "voice-nl-1", true).await.is_err());
        assert!(audit.entries.lock().unwrap().is_empty());
    }
}
