use std::sync::Arc;

use chrono::{DateTime, Utc};

use crate::application::error::{AppError, AppResult};
use crate::application::ports::api_admin::{ApiAdmin, Suspension};
use crate::application::ports::audit_log::AuditLog;
use crate::application::ports::clock::Clock;
use crate::domain::audit::{AuditAction, NewAuditEntry};

const MAX_REASON_LEN: usize = 512;

/// Suspends a user account through api and records who did it and why.
pub struct SuspendUser {
    api: Arc<dyn ApiAdmin>,
    audit: Arc<dyn AuditLog>,
    clock: Arc<dyn Clock>,
}

impl SuspendUser {
    pub fn new(api: Arc<dyn ApiAdmin>, audit: Arc<dyn AuditLog>, clock: Arc<dyn Clock>) -> Self {
        Self { api, audit, clock }
    }

    pub async fn execute(&self, actor_email: &str, user_id: &str, reason: &str, until: Option<&str>) -> AppResult<()> {
        if user_id.is_empty() || !user_id.chars().all(|c| c.is_ascii_digit()) {
            return Err(AppError::Validation("user id must be a snowflake".into()));
        }
        let reason = reason.trim();
        if reason.is_empty() || reason.chars().count() > MAX_REASON_LEN {
            return Err(AppError::Validation("reason is required and must be at most 512 characters".into()));
        }
        let until = until.map(|value| self.parse_future(value)).transpose()?;
        let suspension = Suspension { reason: reason.to_owned(), until: until.map(|u| u.to_rfc3339()) };
        self.api.suspend_user(user_id, &suspension).await?;

        let details = serde_json::json!({ "reason": suspension.reason, "until": suspension.until }).to_string();
        let entry = NewAuditEntry { actor_email: actor_email.to_owned(), action: AuditAction::SuspendUser, target: user_id.to_owned(), details };
        self.audit.record(&entry).await?;
        Ok(())
    }

    fn parse_future(&self, value: &str) -> AppResult<DateTime<Utc>> {
        let until = DateTime::parse_from_rfc3339(value)
            .map_err(|_| AppError::Validation("until must be an RFC 3339 timestamp".into()))?
            .with_timezone(&Utc);
        if until <= self.clock.now() {
            return Err(AppError::Validation("until must be in the future".into()));
        }
        Ok(until)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{FakeApi, FakeAudit, FixedClock, unavailable};

    fn setup() -> (Arc<FakeApi>, Arc<FakeAudit>, SuspendUser) {
        let api = Arc::new(FakeApi::default());
        let audit = Arc::new(FakeAudit::default());
        let use_case = SuspendUser::new(api.clone(), audit.clone(), Arc::new(FixedClock::default()));
        (api, audit, use_case)
    }

    #[tokio::test]
    async fn suspends_through_api_and_audits_the_reason() {
        let (api, audit, use_case) = setup();
        use_case.execute("ops@discordy.dev", "1234", " spam ", Some("2026-11-01T00:00:00Z")).await.unwrap();
        let suspensions = api.suspensions.lock().unwrap();
        assert_eq!(suspensions[0].0, "1234");
        assert_eq!(suspensions[0].1, Suspension { reason: "spam".into(), until: Some("2026-11-01T00:00:00+00:00".into()) });
        let entries = audit.entries.lock().unwrap();
        assert_eq!(entries[0].action, AuditAction::SuspendUser);
        assert!(entries[0].details.contains("spam"));
    }

    #[tokio::test]
    async fn indefinite_suspensions_have_no_end() {
        let (api, _, use_case) = setup();
        use_case.execute("ops@discordy.dev", "1234", "abuse", None).await.unwrap();
        assert_eq!(api.suspensions.lock().unwrap()[0].1.until, None);
    }

    #[tokio::test]
    async fn rejects_invalid_input_before_calling_api() {
        let (api, _, use_case) = setup();
        let cases = [
            ("abc", "spam", None),
            ("1234", "  ", None),
            ("1234", "spam", Some("tomorrow")),
            ("1234", "spam", Some("2026-01-01T00:00:00Z")),
        ];
        for (user, reason, until) in cases {
            let result = use_case.execute("ops@discordy.dev", user, reason, until).await;
            assert!(matches!(result, Err(AppError::Validation(_))), "{user} {reason} {until:?}");
        }
        assert!(api.suspensions.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_failed_suspension_is_not_audited() {
        let (api, audit, use_case) = setup();
        *api.suspend_result.lock().unwrap() = Err(unavailable());
        assert!(use_case.execute("ops@discordy.dev", "1234", "spam", None).await.is_err());
        assert!(audit.entries.lock().unwrap().is_empty());
    }
}
