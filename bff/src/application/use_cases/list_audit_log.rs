use std::sync::Arc;

use crate::application::error::AppResult;
use crate::application::ports::audit_log::AuditLog;
use crate::domain::audit::AuditEntry;

pub const DEFAULT_AUDIT_PAGE: u32 = 50;
pub const MAX_AUDIT_PAGE: u32 = 200;

pub struct ListAuditLog {
    audit: Arc<dyn AuditLog>,
}

impl ListAuditLog {
    pub fn new(audit: Arc<dyn AuditLog>) -> Self {
        Self { audit }
    }

    /// Newest entries first; page backwards with `before_id`.
    pub async fn execute(&self, limit: Option<u32>, before_id: Option<i64>) -> AppResult<Vec<AuditEntry>> {
        let limit = limit.unwrap_or(DEFAULT_AUDIT_PAGE).clamp(1, MAX_AUDIT_PAGE);
        Ok(self.audit.list(limit, before_id).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::FakeAudit;
    use crate::domain::audit::{AuditAction, NewAuditEntry};

    #[tokio::test]
    async fn pages_newest_first() {
        let audit = Arc::new(FakeAudit::default());
        for target in ["a", "b", "c"] {
            let entry = NewAuditEntry { actor_email: "ops@discordy.dev".into(), action: AuditAction::DrainVoiceNode, target: target.into(), details: "{}".into() };
            audit.record(&entry).await.unwrap();
        }
        let use_case = ListAuditLog::new(audit);
        let first = use_case.execute(Some(2), None).await.unwrap();
        assert_eq!(first.iter().map(|e| e.target.as_str()).collect::<Vec<_>>(), vec!["c", "b"]);
        let next = use_case.execute(Some(2), Some(first[1].id)).await.unwrap();
        assert_eq!(next.iter().map(|e| e.target.as_str()).collect::<Vec<_>>(), vec!["a"]);
    }

    #[tokio::test]
    async fn zero_limit_still_returns_one_entry() {
        let audit = Arc::new(FakeAudit::default());
        let entry = NewAuditEntry { actor_email: "x@y.z".into(), action: AuditAction::SuspendUser, target: "1".into(), details: "{}".into() };
        audit.record(&entry).await.unwrap();
        assert_eq!(ListAuditLog::new(audit).execute(Some(0), None).await.unwrap().len(), 1);
    }
}
