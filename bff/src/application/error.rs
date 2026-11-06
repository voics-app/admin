use thiserror::Error;

use super::ports::PortError;

/// Failure of a use case, mapped to HTTP by the interface layer.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Dependency(PortError),
}

impl From<PortError> for AppError {
    fn from(error: PortError) -> Self {
        match error {
            PortError::NotFound => AppError::NotFound,
            other => AppError::Dependency(other),
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
