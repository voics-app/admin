use std::sync::Arc;

use crate::application::error::{AppError, AppResult};
use crate::application::ports::audit_log::AuditLog;
use crate::application::ports::feature_flags::FeatureFlagRepository;
use crate::domain::audit::{AuditAction, NewAuditEntry};
use crate::domain::feature_flag::FlagKey;

pub struct DeleteFeatureFlag {
    flags: Arc<dyn FeatureFlagRepository>,
    audit: Arc<dyn AuditLog>,
}

impl DeleteFeatureFlag {
    pub fn new(flags: Arc<dyn FeatureFlagRepository>, audit: Arc<dyn AuditLog>) -> Self {
        Self { flags, audit }
    }

    pub async fn execute(&self, actor_email: &str, key: &str) -> AppResult<()> {
        let key = FlagKey::parse(key).map_err(|e| AppError::Validation(e.to_string()))?;
        self.flags.delete(&key).await?;
        let entry = NewAuditEntry {
            actor_email: actor_email.to_owned(),
            action: AuditAction::DeleteFeatureFlag,
            target: key.as_str().to_owned(),
            details: "{}".into(),
        };
        self.audit.record(&entry).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{FakeAudit, FakeFlags};
    use crate::application::use_cases::upsert_feature_flag::{FlagChange, UpsertFeatureFlag};

    #[tokio::test]
    async fn deletes_and_audits() {
        let flags = Arc::new(FakeFlags::default());
        let audit = Arc::new(FakeAudit::default());
        let change = FlagChange {
            key: "p2p_dms".into(),
            description: String::new(),
            enabled: false,
            rollout_percent: 0,
        };
        UpsertFeatureFlag::new(flags.clone(), audit.clone())
            .execute("ops@discordy.dev", &change)
            .await
            .unwrap();
        DeleteFeatureFlag::new(flags.clone(), audit.clone())
            .execute("ops@discordy.dev", "p2p_dms")
            .await
            .unwrap();
        assert!(flags.flags.lock().unwrap().is_empty());
        assert_eq!(
            audit.entries.lock().unwrap()[1].action,
            AuditAction::DeleteFeatureFlag
        );
    }

    #[tokio::test]
    async fn deleting_an_unknown_flag_is_not_found_and_not_audited() {
        let audit = Arc::new(FakeAudit::default());
        let result = DeleteFeatureFlag::new(Arc::new(FakeFlags::default()), audit.clone())
            .execute("ops@discordy.dev", "missing")
            .await;
        assert_eq!(result, Err(AppError::NotFound));
        assert!(audit.entries.lock().unwrap().is_empty());
    }
}
