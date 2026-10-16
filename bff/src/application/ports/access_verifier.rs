use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AccessTokenError {
    #[error("access token is invalid: {0}")]
    Invalid(String),
    #[error("signing keys unavailable: {0}")]
    KeysUnavailable(String),
}

/// Verifies an identity-provider assertion (Cloudflare Access JWT) and returns the email it vouches for.
#[async_trait]
pub trait AccessTokenVerifier: Send + Sync {
    async fn verify(&self, token: &str) -> Result<String, AccessTokenError>;
}
