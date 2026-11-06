use chrono::{DateTime, Utc};

/// Every privileged action an admin can take; each one is recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    SuspendUser,
    DrainVoiceNode,
    UndrainVoiceNode,
    UpsertFeatureFlag,
    DeleteFeatureFlag,
    ResolveReport,
    DismissReport,
}

impl AuditAction {
    pub fn as_str(self) -> &'static str {
        match self {
            AuditAction::SuspendUser => "user.suspend",
            AuditAction::DrainVoiceNode => "voice_node.drain",
            AuditAction::UndrainVoiceNode => "voice_node.undrain",
            AuditAction::UpsertFeatureFlag => "feature_flag.upsert",
            AuditAction::DeleteFeatureFlag => "feature_flag.delete",
            AuditAction::ResolveReport => "report.resolve",
            AuditAction::DismissReport => "report.dismiss",
        }
    }
}

/// An audit record about to be written.
#[derive(Debug, Clone, PartialEq)]
pub struct NewAuditEntry {
    pub actor_email: String,
    pub action: AuditAction,
    pub target: String,
    /// Small JSON object with action parameters (never message content).
    pub details: String,
}

/// A stored audit record.
#[derive(Debug, Clone, PartialEq)]
pub struct AuditEntry {
    pub id: i64,
    pub at: DateTime<Utc>,
    pub actor_email: String,
    pub action: String,
    pub target: String,
    pub details: String,
}
