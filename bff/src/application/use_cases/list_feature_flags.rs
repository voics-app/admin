use std::sync::Arc;

use crate::application::error::AppResult;
use crate::application::ports::feature_flags::{FeatureFlagRepository, StoredFlag};

pub struct ListFeatureFlags {
    flags: Arc<dyn FeatureFlagRepository>,
}

impl ListFeatureFlags {
    pub fn new(flags: Arc<dyn FeatureFlagRepository>) -> Self {
        Self { flags }
    }

    /// All flags, ordered by key.
    pub async fn execute(&self) -> AppResult<Vec<StoredFlag>> {
        let mut flags = self.flags.list().await?;
        flags.sort_by(|a, b| a.flag.key.cmp(&b.flag.key));
        Ok(flags)
    }
}
