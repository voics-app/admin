use std::sync::Arc;

use crate::application::error::{AppError, AppResult};
use crate::application::ports::PortError;
use crate::application::ports::api_admin::{ApiAdmin, ReportStatus, ReportSummary};
use crate::application::ports::audit_log::AuditLog;
use crate::domain::audit::{AuditAction, NewAuditEntry};

const MAX_NOTE_LEN: usize = 500;

/// Closes a report as resolved or dismissed and records who did it. The audit entry names the
/// report, its category and the reported account, never the message content.
pub struct ResolveReport {
    api: Arc<dyn ApiAdmin>,
    audit: Arc<dyn AuditLog>,
}

impl ResolveReport {
    pub fn new(api: Arc<dyn ApiAdmin>, audit: Arc<dyn AuditLog>) -> Self {
        Self { api, audit }
    }

    pub async fn execute(
        &self,
        actor_email: &str,
        report_id: &str,
        resolution: &str,
        note: Option<&str>,
    ) -> AppResult<ReportSummary> {
        if report_id.is_empty() || !report_id.chars().all(|c| c.is_ascii_digit()) {
            return Err(AppError::Validation("report id must be a snowflake".into()));
        }
        let (status, action) = match resolution {
            "resolved" => (ReportStatus::Resolved, AuditAction::ResolveReport),
            "dismissed" => (ReportStatus::Dismissed, AuditAction::DismissReport),
            _ => {
                return Err(AppError::Validation(
                    "resolution must be resolved or dismissed".into(),
                ));
            }
        };
        let note = note.map(str::trim).filter(|n| !n.is_empty());
        if note.is_some_and(|n| n.chars().count() > MAX_NOTE_LEN) {
            return Err(AppError::Validation(
                "note must be at most 500 characters".into(),
            ));
        }
        let report = self
            .api
            .resolve_report(report_id, status, actor_email, note)
            .await
            .map_err(|error| match error {
                PortError::Rejected { status: 409, .. } => {
                    AppError::Conflict("the report is already closed".into())
                }
                other => other.into(),
            })?;

        let details = serde_json::json!({
            "kind": report.kind,
            "category": report.category,
            "reported_user_id": report.reported.id,
            "note": note,
        })
        .to_string();
        self.audit
            .record(&NewAuditEntry {
                actor_email: actor_email.to_owned(),
                action,
                target: report_id.to_owned(),
                details,
            })
            .await?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{FakeApi, FakeAudit, unavailable};

    fn setup() -> (Arc<FakeApi>, Arc<FakeAudit>, ResolveReport) {
        let api = Arc::new(FakeApi::default());
        let audit = Arc::new(FakeAudit::default());
        let use_case = ResolveReport::new(api.clone(), audit.clone());
        (api, audit, use_case)
    }

    #[tokio::test]
    async fn resolving_calls_api_and_audits_without_the_content() {
        let (api, audit, use_case) = setup();
        let report = use_case
            .execute("ops@discordy.dev", "1", "resolved", Some(" warned "))
            .await
            .unwrap();
        assert_eq!(report.id, "1");
        assert_eq!(
            api.report_closures.lock().unwrap()[0],
            (
                "1".to_owned(),
                ReportStatus::Resolved,
                "ops@discordy.dev".to_owned(),
                Some("warned".to_owned())
            )
        );
        let entries = audit.entries.lock().unwrap();
        assert_eq!(entries[0].action, AuditAction::ResolveReport);
        assert_eq!(entries[0].target, "1");
        assert!(entries[0].details.contains("spam"));
        assert!(entries[0].details.contains("22"));
        assert!(
            !entries[0].details.contains("buy my coins"),
            "message content never reaches the audit log"
        );
    }

    #[tokio::test]
    async fn dismissing_has_its_own_audit_action() {
        let (_, audit, use_case) = setup();
        use_case
            .execute("ops@discordy.dev", "1", "dismissed", None)
            .await
            .unwrap();
        assert_eq!(
            audit.entries.lock().unwrap()[0].action,
            AuditAction::DismissReport
        );
    }

    #[tokio::test]
    async fn rejects_invalid_input_before_calling_api() {
        let (api, _, use_case) = setup();
        let long = "n".repeat(501);
        let cases = [
            ("abc", "resolved", None),
            ("", "resolved", None),
            ("1", "open", None),
            ("1", "resolved", Some(long.as_str())),
        ];
        for (id, resolution, note) in cases {
            let result = use_case
                .execute("ops@discordy.dev", id, resolution, note)
                .await;
            assert!(
                matches!(result, Err(AppError::Validation(_))),
                "{id} {resolution}"
            );
        }
        assert!(api.report_closures.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_already_closed_report_is_a_conflict_and_not_audited() {
        let (api, audit, use_case) = setup();
        *api.resolve_result.lock().unwrap() = Err(PortError::Rejected {
            status: 409,
            message: "closed".into(),
        });
        assert_eq!(
            use_case
                .execute("ops@discordy.dev", "1", "resolved", None)
                .await,
            Err(AppError::Conflict("the report is already closed".into()))
        );
        assert!(audit.entries.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_unknown_report_is_not_found_and_a_failing_api_is_not_audited() {
        let (api, audit, use_case) = setup();
        *api.resolve_result.lock().unwrap() = Err(PortError::NotFound);
        assert_eq!(
            use_case
                .execute("ops@discordy.dev", "9", "resolved", None)
                .await,
            Err(AppError::NotFound)
        );
        *api.resolve_result.lock().unwrap() = Err(unavailable());
        assert!(
            use_case
                .execute("ops@discordy.dev", "1", "resolved", None)
                .await
                .is_err()
        );
        assert!(audit.entries.lock().unwrap().is_empty());
    }
}
