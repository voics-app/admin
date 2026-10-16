use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::PortResult;
use crate::domain::feature_flag::{FeatureFlag, FlagKey};

/// A flag as stored, with its change metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFlag {
    pub flag: FeatureFlag,
    pub updated_at: DateTime<Utc>,
    pub updated_by: String,
}

#[async_trait]
pub trait FeatureFlagRepository: Send + Sync {
    async fn list(&self) -> PortResult<Vec<StoredFlag>>;
    async fn upsert(&self, flag: &FeatureFlag, updated_by: &str) -> PortResult<StoredFlag>;
    /// Returns `PortError::NotFound` when the flag does not exist.
    async fn delete(&self, key: &FlagKey) -> PortResult<()>;
}
