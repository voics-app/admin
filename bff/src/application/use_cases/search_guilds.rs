use std::sync::Arc;

use super::search_users::clean_query;
use crate::application::error::AppResult;
use crate::application::ports::api_admin::{ApiAdmin, GuildSummary};

/// Trust & safety guild lookup.
pub struct SearchGuilds {
    api: Arc<dyn ApiAdmin>,
}

impl SearchGuilds {
    pub fn new(api: Arc<dyn ApiAdmin>) -> Self {
        Self { api }
    }

    pub async fn execute(&self, query: &str) -> AppResult<Vec<GuildSummary>> {
        Ok(self.api.search_guilds(&clean_query(query)).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::FakeApi;

    #[tokio::test]
    async fn returns_the_guilds_api_found() {
        let api = FakeApi::default();
        let guild = GuildSummary { id: "1".into(), name: "Lounge".into(), owner_id: None, member_count: Some(3), voice_region: None };
        *api.guilds.lock().unwrap() = Ok(vec![guild.clone()]);
        assert_eq!(SearchGuilds::new(Arc::new(api)).execute(" lou ").await.unwrap(), vec![guild]);
    }
}
