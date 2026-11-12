//! Ports: the interfaces use cases depend on. Adapters in `infrastructure` implement them.

pub mod access_verifier;
pub mod api_admin;
pub mod audit_log;
pub mod clock;
pub mod feature_flags;
pub mod gateway_admin;
pub mod notifier_admin;
pub mod platform_metrics;
pub mod rollups;
pub mod voice_directory;

use thiserror::Error;

/// Failure of an outbound dependency, independent of transport.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PortError {
    #[error("dependency unavailable: {0}")]
    Unavailable(String),
    #[error("not found")]
    NotFound,
    #[error("rejected by dependency ({status}): {message}")]
    Rejected { status: u16, message: String },
}

pub type PortResult<T> = Result<T, PortError>;
