use std::sync::Arc;

use crate::application::error::{AppError, AppResult};
use crate::application::ports::audit_log::AuditLog;
use crate::application::ports::feature_flags::{FeatureFlagRepository, StoredFlag};
use crate::domain::audit::{AuditAction, NewAuditEntry};
use crate::domain::feature_flag::{FeatureFlag, FlagKey, RolloutPercent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagChange {
    pub key: String,
    pub description: String,
    pub enabled: bool,
    pub rollout_percent: i64,
}

/// Creates or updates a feature flag and audits the new state.
pub struct UpsertFeatureFlag {
    flags: Arc<dyn FeatureFlagRepository>,
    audit: Arc<dyn AuditLog>,
}

impl UpsertFeatureFlag {
    pub fn new(flags: Arc<dyn FeatureFlagRepository>, audit: Arc<dyn AuditLog>) -> Self {
        Self { flags, audit }
    }

    pub async fn execute(&self, actor_email: &str, change: &FlagChange) -> AppResult<StoredFlag> {
        let invalid =
            |e: crate::domain::feature_flag::FlagError| AppError::Validation(e.to_string());
        let key = FlagKey::parse(&change.key).map_err(invalid)?;
        let rollout = RolloutPercent::parse(change.rollout_percent).map_err(invalid)?;
        let flag =
            FeatureFlag::new(key, &change.description, change.enabled, rollout).map_err(invalid)?;

        let stored = self.flags.upsert(&flag, actor_email).await?;
        let details = serde_json::json!({ "enabled": flag.enabled, "rollout_percent": flag.rollout_percent.value() }).to_string();
        let entry = NewAuditEntry {
            actor_email: actor_email.to_owned(),
            action: AuditAction::UpsertFeatureFlag,
            target: flag.key.as_str().to_owned(),
            details,
        };
        self.audit.record(&entry).await?;
        Ok(stored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{FakeAudit, FakeFlags};
    use crate::application::use_cases::list_feature_flags::ListFeatureFlags;

    fn change(key: &str, rollout: i64) -> FlagChange {
        FlagChange {
            key: key.into(),
            description: "Sync playback".into(),
            enabled: true,
            rollout_percent: rollout,
        }
    }

    #[tokio::test]
    async fn stores_the_flag_and_audits_its_state() {
        let flags = Arc::new(FakeFlags::default());
        let audit = Arc::new(FakeAudit::default());
        let stored = UpsertFeatureFlag::new(flags.clone(), audit.clone())
            .execute("ops@discordy.dev", &change("watch_together", 25))
            .await
            .unwrap();
        assert_eq!(stored.updated_by, "ops@discordy.dev");
        assert_eq!(stored.flag.rollout_percent.value(), 25);
        let entries = audit.entries.lock().unwrap();
        assert_eq!(entries[0].target, "watch_together");
        assert_eq!(
            entries[0].details,
            r#"{"enabled":true,"rollout_percent":25}"#
        );
    }

    #[tokio::test]
    async fn invalid_flags_are_rejected_and_not_stored() {
        let flags = Arc::new(FakeFlags::default());
        let audit = Arc::new(FakeAudit::default());
        let use_case = UpsertFeatureFlag::new(flags.clone(), audit.clone());
        assert!(matches!(
            use_case
                .execute("ops@discordy.dev", &change("Bad Key", 10))
                .await,
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            use_case
                .execute("ops@discordy.dev", &change("ok_key", 101))
                .await,
            Err(AppError::Validation(_))
        ));
        assert!(flags.flags.lock().unwrap().is_empty());
        assert!(audit.entries.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn listing_returns_flags_ordered_by_key() {
        let flags = Arc::new(FakeFlags::default());
        let use_case = UpsertFeatureFlag::new(flags.clone(), Arc::new(FakeAudit::default()));
        use_case
            .execute("ops@discordy.dev", &change("zeta", 0))
            .await
            .unwrap();
        use_case
            .execute("ops@discordy.dev", &change("alpha", 0))
            .await
            .unwrap();
        let keys: Vec<String> = ListFeatureFlags::new(flags)
            .execute()
            .await
            .unwrap()
            .iter()
            .map(|f| f.flag.key.as_str().to_owned())
            .collect();
        assert_eq!(keys, vec!["alpha", "zeta"]);
    }
}
