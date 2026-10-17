use std::sync::Arc;

use crate::application::error::AppResult;
use crate::application::ports::api_admin::{ApiAdmin, UserPage, UserQuery};

pub const DEFAULT_PAGE_SIZE: u32 = 25;
pub const MAX_PAGE_SIZE: u32 = 100;
const MAX_QUERY_LEN: usize = 100;

/// Trust & safety user lookup, paginated by cursor.
pub struct SearchUsers {
    api: Arc<dyn ApiAdmin>,
}

impl SearchUsers {
    pub fn new(api: Arc<dyn ApiAdmin>) -> Self {
        Self { api }
    }

    pub async fn execute(
        &self,
        query: &str,
        limit: Option<u32>,
        after: Option<String>,
    ) -> AppResult<UserPage> {
        let query = UserQuery {
            query: clean_query(query),
            limit: limit.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE),
            after: after.filter(|a| !a.trim().is_empty()),
        };
        Ok(self.api.search_users(&query).await?)
    }
}

/// Trims and bounds a free-text search query.
pub fn clean_query(query: &str) -> String {
    query.trim().chars().take(MAX_QUERY_LEN).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::FakeApi;

    #[tokio::test]
    async fn trims_the_query_and_clamps_the_page_size() {
        let api = Arc::new(FakeApi::default());
        let use_case = SearchUsers::new(api.clone());
        use_case
            .execute("  alice ", Some(500), Some(" ".into()))
            .await
            .unwrap();
        use_case
            .execute("bob", None, Some("123".into()))
            .await
            .unwrap();
        let queries = api.user_queries.lock().unwrap();
        assert_eq!(
            queries[0],
            UserQuery {
                query: "alice".into(),
                limit: MAX_PAGE_SIZE,
                after: None
            }
        );
        assert_eq!(
            queries[1],
            UserQuery {
                query: "bob".into(),
                limit: DEFAULT_PAGE_SIZE,
                after: Some("123".into())
            }
        );
    }

    #[test]
    fn bounds_overlong_queries() {
        assert_eq!(clean_query(&"q".repeat(300)).len(), 100);
    }
}
