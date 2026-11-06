use std::sync::Arc;

use crate::application::error::{AppError, AppResult};
use crate::application::ports::api_admin::{ApiAdmin, ReportPage, ReportQuery, ReportStatus};

pub const DEFAULT_PAGE_SIZE: u32 = 25;
pub const MAX_PAGE_SIZE: u32 = 100;

/// The Trust and Safety review queue, newest first, paginated by cursor.
pub struct ListReports {
    api: Arc<dyn ApiAdmin>,
}

impl ListReports {
    pub fn new(api: Arc<dyn ApiAdmin>) -> Self {
        Self { api }
    }

    /// `status` defaults to `open`.
    pub async fn execute(
        &self,
        status: Option<&str>,
        limit: Option<u32>,
        after: Option<String>,
    ) -> AppResult<ReportPage> {
        let status = match status {
            None => ReportStatus::Open,
            Some(value) => ReportStatus::parse(value).ok_or_else(|| {
                AppError::Validation("status must be open, resolved or dismissed".into())
            })?,
        };
        let after = after.filter(|a| !a.trim().is_empty());
        if after
            .as_deref()
            .is_some_and(|a| !a.chars().all(|c| c.is_ascii_digit()))
        {
            return Err(AppError::Validation("after must be a snowflake".into()));
        }
        let query = ReportQuery {
            status,
            limit: limit.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE),
            after,
        };
        Ok(self.api.list_reports(&query).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{FakeApi, report_summary, unavailable};

    #[tokio::test]
    async fn defaults_to_open_reports_and_clamps_the_page_size() {
        let api = Arc::new(FakeApi::default());
        let use_case = ListReports::new(api.clone());
        use_case
            .execute(None, Some(500), Some(" ".into()))
            .await
            .unwrap();
        use_case
            .execute(Some("dismissed"), None, Some("123".into()))
            .await
            .unwrap();
        let queries = api.report_queries.lock().unwrap();
        assert_eq!(
            queries[0],
            ReportQuery {
                status: ReportStatus::Open,
                limit: MAX_PAGE_SIZE,
                after: None
            }
        );
        assert_eq!(
            queries[1],
            ReportQuery {
                status: ReportStatus::Dismissed,
                limit: DEFAULT_PAGE_SIZE,
                after: Some("123".into())
            }
        );
    }

    #[tokio::test]
    async fn returns_what_the_api_serves() {
        let api = Arc::new(FakeApi::default());
        *api.reports.lock().unwrap() = Ok(ReportPage {
            reports: vec![report_summary("9")],
            next_after: Some("9".into()),
        });
        let page = ListReports::new(api)
            .execute(None, None, None)
            .await
            .unwrap();
        assert_eq!(page.reports[0].content.as_deref(), Some("buy my coins"));
        assert_eq!(page.next_after.as_deref(), Some("9"));
    }

    #[tokio::test]
    async fn rejects_unknown_statuses_and_cursors_before_calling_api() {
        let api = Arc::new(FakeApi::default());
        let use_case = ListReports::new(api.clone());
        assert!(matches!(
            use_case.execute(Some("all"), None, None).await,
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            use_case.execute(None, None, Some("1;drop".into())).await,
            Err(AppError::Validation(_))
        ));
        assert!(api.report_queries.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_failing_api_is_a_dependency_error() {
        let api = Arc::new(FakeApi::default());
        *api.reports.lock().unwrap() = Err(unavailable());
        assert!(matches!(
            ListReports::new(api).execute(None, None, None).await,
            Err(AppError::Dependency(_))
        ));
    }
}
