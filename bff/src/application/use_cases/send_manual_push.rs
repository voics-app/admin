use std::sync::Arc;

use crate::application::error::{AppError, AppResult};
use crate::application::ports::PortError;
use crate::application::ports::audit_log::AuditLog;
use crate::application::ports::notifier_admin::{
    ManualPush, NotifierAdmin, PushAudience, PushReport,
};
use crate::domain::audit::{AuditAction, NewAuditEntry};

pub const MAX_TITLE_CHARS: usize = 64;
pub const MAX_BODY_CHARS: usize = 240;

/// What an admin wants sent, as it arrives from the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// `None` means everyone.
    pub user_id: Option<String>,
    pub title: String,
    pub body: String,
}

/// Sends a notification by hand through the notifier: to one user, or to everyone after the admin
/// has seen how many devices that is. Every real send is audited before it leaves.
pub struct SendManualPush {
    notifier: Arc<dyn NotifierAdmin>,
    audit: Arc<dyn AuditLog>,
}

impl SendManualPush {
    pub fn new(notifier: Arc<dyn NotifierAdmin>, audit: Arc<dyn AuditLog>) -> Self {
        Self { notifier, audit }
    }

    fn check(draft: &Draft) -> AppResult<(PushAudience, String, String)> {
        let title = draft.title.trim();
        let body = draft.body.trim();
        if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
            return Err(AppError::Validation(
                "title must be 1 to 64 characters".into(),
            ));
        }
        if body.is_empty() || body.chars().count() > MAX_BODY_CHARS {
            return Err(AppError::Validation(
                "body must be 1 to 240 characters".into(),
            ));
        }
        if title.chars().any(char::is_control) || body.chars().any(|c| c.is_control() && c != '\n')
        {
            return Err(AppError::Validation(
                "text must not contain control characters".into(),
            ));
        }
        let audience = match draft.user_id.as_deref().map(str::trim) {
            None => PushAudience::Everyone,
            Some(id)
                if !id.is_empty() && id.len() <= 20 && id.bytes().all(|b| b.is_ascii_digit()) =>
            {
                PushAudience::User(id.to_owned())
            }
            Some(_) => return Err(AppError::Validation("user id must be a snowflake".into())),
        };
        Ok((audience, title.to_owned(), body.to_owned()))
    }

    /// How many devices the draft would reach. Sends nothing and writes no audit entry.
    pub async fn preview(&self, actor_email: &str, draft: &Draft) -> AppResult<PushReport> {
        let (audience, title, body) = Self::check(draft)?;
        self.call(ManualPush {
            audience,
            title,
            body,
            dry_run: true,
            actor: actor_email.to_owned(),
        })
        .await
    }

    /// Sends it. A notification to everyone needs `confirmed_everyone`: the page asks the admin
    /// to confirm the device count first, and the API refuses without it.
    pub async fn send(
        &self,
        actor_email: &str,
        draft: &Draft,
        confirmed_everyone: bool,
    ) -> AppResult<PushReport> {
        let (audience, title, body) = Self::check(draft)?;
        if audience == PushAudience::Everyone && !confirmed_everyone {
            return Err(AppError::Validation(
                "sending to everyone needs confirm_everyone: true".into(),
            ));
        }
        let target = match &audience {
            PushAudience::Everyone => "all".to_owned(),
            PushAudience::User(id) => format!("user:{id}"),
        };
        // The record comes first: a send that cannot be audited does not happen.
        self.audit
            .record(&NewAuditEntry {
                actor_email: actor_email.to_owned(),
                action: AuditAction::SendNotification,
                target,
                details: serde_json::json!({ "title": title }).to_string(),
            })
            .await?;
        self.call(ManualPush {
            audience,
            title,
            body,
            dry_run: false,
            actor: actor_email.to_owned(),
        })
        .await
    }

    async fn call(&self, push: ManualPush) -> AppResult<PushReport> {
        self.notifier
            .send_manual(&push)
            .await
            .map_err(|error| match error {
                PortError::Rejected { status: 409, .. } => AppError::Conflict(
                    "another notification to everyone is still being sent".into(),
                ),
                PortError::Rejected { status: 422, .. } => {
                    AppError::Validation("the notifier rejected the text or the target".into())
                }
                other => AppError::Dependency(other),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{FakeAudit, FakeNotifier, unavailable};

    const ADMIN: &str = "ops@discordy.dev";

    fn setup() -> (Arc<FakeNotifier>, Arc<FakeAudit>, SendManualPush) {
        let notifier = Arc::new(FakeNotifier::default());
        let audit = Arc::new(FakeAudit::default());
        (
            notifier.clone(),
            audit.clone(),
            SendManualPush::new(notifier, audit),
        )
    }

    fn draft(user: Option<&str>) -> Draft {
        Draft {
            user_id: user.map(str::to_owned),
            title: " Maintenance ".into(),
            body: "Back at 22:00.".into(),
        }
    }

    #[tokio::test]
    async fn sends_to_one_user_and_audits_who_what_and_to_whom() {
        let (notifier, audit, use_case) = setup();
        let report = use_case
            .send(ADMIN, &draft(Some("42")), false)
            .await
            .unwrap();
        assert_eq!(report.devices, 2);
        let calls = notifier.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].audience, PushAudience::User("42".into()));
        assert_eq!(calls[0].title, "Maintenance");
        assert!(!calls[0].dry_run);
        assert_eq!(calls[0].actor, ADMIN);
        let entries = audit.entries.lock().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, AuditAction::SendNotification);
        assert_eq!(
            (entries[0].actor_email.as_str(), entries[0].target.as_str()),
            (ADMIN, "user:42")
        );
        assert!(entries[0].details.contains("Maintenance"));
        assert!(
            !entries[0].details.contains("22:00"),
            "the body is not audited"
        );
    }

    #[tokio::test]
    async fn everyone_needs_the_confirmation_flag() {
        let (notifier, audit, use_case) = setup();
        let refused = use_case.send(ADMIN, &draft(None), false).await;
        assert!(matches!(refused, Err(AppError::Validation(_))));
        assert!(notifier.calls.lock().unwrap().is_empty());
        assert!(audit.entries.lock().unwrap().is_empty());

        use_case.send(ADMIN, &draft(None), true).await.unwrap();
        assert_eq!(
            notifier.calls.lock().unwrap()[0].audience,
            PushAudience::Everyone
        );
        assert_eq!(audit.entries.lock().unwrap()[0].target, "all");
    }

    #[tokio::test]
    async fn a_preview_counts_devices_without_sending_or_auditing() {
        let (notifier, audit, use_case) = setup();
        use_case.preview(ADMIN, &draft(None)).await.unwrap();
        assert!(notifier.calls.lock().unwrap()[0].dry_run);
        assert!(audit.entries.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn invalid_drafts_never_reach_the_notifier() {
        let (notifier, audit, use_case) = setup();
        let cases = [
            Draft {
                title: " ".into(),
                ..draft(None)
            },
            Draft {
                title: "t".repeat(65),
                ..draft(None)
            },
            Draft {
                body: "".into(),
                ..draft(None)
            },
            Draft {
                body: "b".repeat(241),
                ..draft(None)
            },
            Draft {
                title: "a\u{0}b".into(),
                ..draft(None)
            },
            draft(Some("alice")),
            draft(Some("")),
        ];
        for case in cases {
            assert!(
                matches!(
                    use_case.send(ADMIN, &case, true).await,
                    Err(AppError::Validation(_))
                ),
                "{case:?}"
            );
            assert!(
                matches!(
                    use_case.preview(ADMIN, &case).await,
                    Err(AppError::Validation(_))
                ),
                "{case:?}"
            );
        }
        assert!(notifier.calls.lock().unwrap().is_empty());
        assert!(audit.entries.lock().unwrap().is_empty());
        let limits = Draft {
            title: "t".repeat(64),
            body: "b".repeat(240),
            user_id: Some("1".into()),
        };
        assert!(use_case.send(ADMIN, &limits, false).await.is_ok());
    }

    #[tokio::test]
    async fn a_send_that_cannot_be_audited_does_not_happen() {
        let (notifier, audit, use_case) = setup();
        *audit.fail_record.lock().unwrap() = true;
        assert!(
            use_case
                .send(ADMIN, &draft(Some("42")), false)
                .await
                .is_err()
        );
        assert!(notifier.calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn notifier_answers_map_to_conflict_validation_and_dependency_errors() {
        let (notifier, _, use_case) = setup();
        let cases = [
            (
                PortError::Rejected {
                    status: 409,
                    message: "busy".into(),
                },
                "conflict",
            ),
            (
                PortError::Rejected {
                    status: 422,
                    message: "bad".into(),
                },
                "validation",
            ),
            (unavailable(), "dependency"),
        ];
        for (error, kind) in cases {
            *notifier.result.lock().unwrap() = Err(error);
            let result = use_case.send(ADMIN, &draft(Some("1")), false).await;
            let got = match result {
                Err(AppError::Conflict(_)) => "conflict",
                Err(AppError::Validation(_)) => "validation",
                Err(AppError::Dependency(_)) => "dependency",
                other => panic!("{other:?}"),
            };
            assert_eq!(got, kind);
        }
    }
}
