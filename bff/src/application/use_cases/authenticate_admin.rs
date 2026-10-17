use std::sync::Arc;

use thiserror::Error;

use crate::application::ports::access_verifier::{AccessTokenError, AccessTokenVerifier};
use crate::domain::admin_allowlist::AdminAllowlist;

/// What a request presented to prove who it is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdminCredentials {
    /// `Cf-Access-Jwt-Assertion` header.
    pub access_assertion: Option<String>,
    /// `X-Dev-Admin-Email` header, honoured only in local development.
    pub dev_email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminIdentity {
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AuthenticationError {
    #[error("no admin credentials presented")]
    MissingCredentials,
    #[error("invalid access assertion: {0}")]
    InvalidAssertion(String),
    #[error("identity provider keys unavailable")]
    KeysUnavailable,
    #[error("email is not an allowed admin")]
    NotAllowed,
}

/// Two gates: a verified Cloudflare Access identity (or the local dev header) AND the email allowlist.
pub struct AuthenticateAdmin {
    verifier: Option<Arc<dyn AccessTokenVerifier>>,
    allowlist: AdminAllowlist,
    accept_dev_header: bool,
}

impl AuthenticateAdmin {
    pub fn new(
        verifier: Option<Arc<dyn AccessTokenVerifier>>,
        allowlist: AdminAllowlist,
        accept_dev_header: bool,
    ) -> Self {
        Self {
            verifier,
            allowlist,
            accept_dev_header,
        }
    }

    pub async fn execute(
        &self,
        credentials: &AdminCredentials,
    ) -> Result<AdminIdentity, AuthenticationError> {
        let email = self.identify(credentials).await?;
        if !self.allowlist.permits(&email) {
            return Err(AuthenticationError::NotAllowed);
        }
        Ok(AdminIdentity {
            email: email.trim().to_lowercase(),
        })
    }

    async fn identify(
        &self,
        credentials: &AdminCredentials,
    ) -> Result<String, AuthenticationError> {
        if self.accept_dev_header
            && let Some(email) = credentials
                .dev_email
                .as_deref()
                .filter(|e| !e.trim().is_empty())
        {
            return Ok(email.to_owned());
        }
        let (Some(verifier), Some(token)) =
            (&self.verifier, credentials.access_assertion.as_deref())
        else {
            return Err(AuthenticationError::MissingCredentials);
        };
        verifier.verify(token).await.map_err(|error| match error {
            AccessTokenError::Invalid(reason) => AuthenticationError::InvalidAssertion(reason),
            AccessTokenError::KeysUnavailable(_) => AuthenticationError::KeysUnavailable,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::FakeVerifier;

    fn auth(accept_dev_header: bool) -> AuthenticateAdmin {
        AuthenticateAdmin::new(
            Some(Arc::new(FakeVerifier)),
            AdminAllowlist::parse("ops@discordy.dev"),
            accept_dev_header,
        )
    }

    fn assertion(token: &str) -> AdminCredentials {
        AdminCredentials {
            access_assertion: Some(token.into()),
            dev_email: None,
        }
    }

    fn dev(email: &str) -> AdminCredentials {
        AdminCredentials {
            access_assertion: None,
            dev_email: Some(email.into()),
        }
    }

    #[tokio::test]
    async fn a_verified_allowlisted_identity_is_admitted() {
        let identity = auth(false)
            .execute(&assertion("valid:Ops@Discordy.dev"))
            .await
            .unwrap();
        assert_eq!(identity.email, "ops@discordy.dev");
    }

    #[tokio::test]
    async fn a_verified_identity_outside_the_allowlist_is_refused() {
        assert_eq!(
            auth(false)
                .execute(&assertion("valid:eve@discordy.dev"))
                .await,
            Err(AuthenticationError::NotAllowed)
        );
    }

    #[tokio::test]
    async fn an_invalid_assertion_is_refused() {
        assert!(matches!(
            auth(false).execute(&assertion("forged")).await,
            Err(AuthenticationError::InvalidAssertion(_))
        ));
    }

    #[tokio::test]
    async fn a_key_outage_is_reported_distinctly() {
        assert_eq!(
            auth(false).execute(&assertion("keys-down")).await,
            Err(AuthenticationError::KeysUnavailable)
        );
    }

    #[tokio::test]
    async fn no_credentials_are_refused() {
        assert_eq!(
            auth(true).execute(&AdminCredentials::default()).await,
            Err(AuthenticationError::MissingCredentials)
        );
    }

    #[tokio::test]
    async fn the_dev_header_works_only_when_enabled() {
        assert!(auth(true).execute(&dev("ops@discordy.dev")).await.is_ok());
        assert_eq!(
            auth(false).execute(&dev("ops@discordy.dev")).await,
            Err(AuthenticationError::MissingCredentials)
        );
    }

    #[tokio::test]
    async fn the_dev_header_is_still_subject_to_the_allowlist() {
        assert_eq!(
            auth(true).execute(&dev("eve@discordy.dev")).await,
            Err(AuthenticationError::NotAllowed)
        );
    }

    #[tokio::test]
    async fn without_a_verifier_assertions_cannot_be_used() {
        let auth = AuthenticateAdmin::new(None, AdminAllowlist::parse("ops@discordy.dev"), true);
        assert_eq!(
            auth.execute(&assertion("valid:ops@discordy.dev")).await,
            Err(AuthenticationError::MissingCredentials)
        );
    }
}
