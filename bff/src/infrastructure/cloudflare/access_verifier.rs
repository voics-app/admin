use std::time::{Duration, Instant};

use async_trait::async_trait;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::application::ports::access_verifier::{AccessTokenError, AccessTokenVerifier};

/// How long fetched signing keys are trusted before refetching.
const KEYS_TTL: Duration = Duration::from_secs(600);
/// Minimum gap between refetches triggered by an unknown `kid` (protects against key-id spraying).
const MIN_REFRESH_GAP: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
struct AccessClaims {
    #[serde(default)]
    email: Option<String>,
}

struct CachedKeys {
    keys: JwkSet,
    fetched_at: Instant,
}

/// Verifies `Cf-Access-Jwt-Assertion` tokens against the team's published certs (RS256),
/// checking signature, expiry, issuer and the application audience tag.
pub struct CloudflareAccessVerifier {
    http: Client,
    certs_url: String,
    issuer: String,
    audience: String,
    host_audiences: std::collections::HashMap<String, String>,
    cache: RwLock<Option<CachedKeys>>,
}

impl CloudflareAccessVerifier {
    /// `team_domain` may be the bare team name (`acme`) or the full host (`acme.cloudflareaccess.com`).
    pub fn new(http: Client, team_domain: &str, audience: &str) -> Self {
        let issuer = team_issuer(team_domain);
        let certs_url = format!("{issuer}/cdn-cgi/access/certs");
        Self::with_certs_url(http, &issuer, &certs_url, audience)
    }

    pub fn with_certs_url(http: Client, issuer: &str, certs_url: &str, audience: &str) -> Self {
        Self {
            http,
            certs_url: certs_url.to_owned(),
            issuer: issuer.to_owned(),
            audience: audience.to_owned(),
            host_audiences: Default::default(),
            cache: RwLock::new(None),
        }
    }

    pub fn with_host_audiences(
        mut self,
        host_audiences: std::collections::HashMap<String, String>,
    ) -> Self {
        self.host_audiences = host_audiences;
        self
    }

    async fn decoding_key(&self, kid: &str) -> Result<DecodingKey, AccessTokenError> {
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.as_ref().filter(|c| c.fetched_at.elapsed() < KEYS_TTL)
                && let Some(jwk) = cached.keys.find(kid)
            {
                return DecodingKey::from_jwk(jwk)
                    .map_err(|e| AccessTokenError::Invalid(e.to_string()));
            }
        }
        let mut cache = self.cache.write().await;
        let recently_fetched = cache
            .as_ref()
            .is_some_and(|c| c.fetched_at.elapsed() < MIN_REFRESH_GAP);
        if !recently_fetched {
            let keys = self.fetch_keys().await?;
            *cache = Some(CachedKeys {
                keys,
                fetched_at: Instant::now(),
            });
        }
        let jwk = cache
            .as_ref()
            .and_then(|c| c.keys.find(kid))
            .ok_or_else(|| AccessTokenError::Invalid(format!("unknown signing key {kid}")))?;
        DecodingKey::from_jwk(jwk).map_err(|e| AccessTokenError::Invalid(e.to_string()))
    }

    async fn fetch_keys(&self) -> Result<JwkSet, AccessTokenError> {
        let unavailable =
            |e: reqwest::Error| AccessTokenError::KeysUnavailable(e.without_url().to_string());
        let response = self
            .http
            .get(&self.certs_url)
            .send()
            .await
            .map_err(unavailable)?;
        let response = response.error_for_status().map_err(unavailable)?;
        response.json::<JwkSet>().await.map_err(unavailable)
    }
}

fn team_issuer(team_domain: &str) -> String {
    let host = team_domain
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    if host.contains('.') {
        format!("https://{host}")
    } else {
        format!("https://{host}.cloudflareaccess.com")
    }
}

#[async_trait]
impl AccessTokenVerifier for CloudflareAccessVerifier {
    async fn verify(&self, token: &str, host: Option<&str>) -> Result<String, AccessTokenError> {
        let audience = if self.host_audiences.is_empty() {
            &self.audience
        } else {
            let host = host
                .and_then(|h| h.parse::<axum::http::uri::Authority>().ok())
                .map(|h| h.host().to_ascii_lowercase());
            host.as_ref()
                .and_then(|h| self.host_audiences.get(h))
                .ok_or_else(|| AccessTokenError::Invalid("unknown admin host".into()))?
        };
        let header = decode_header(token).map_err(|e| AccessTokenError::Invalid(e.to_string()))?;
        if header.alg != Algorithm::RS256 {
            return Err(AccessTokenError::Invalid(format!(
                "unexpected algorithm {:?}",
                header.alg
            )));
        }
        let kid = header
            .kid
            .ok_or_else(|| AccessTokenError::Invalid("missing kid".into()))?;
        let key = self.decoding_key(&kid).await?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[audience]);
        validation.set_issuer(&[&self.issuer]);
        validation.set_required_spec_claims(&["exp", "iss", "aud"]);
        let data = decode::<AccessClaims>(token, &key, &validation)
            .map_err(|e| AccessTokenError::Invalid(e.to_string()))?;
        data.claims
            .email
            .filter(|e| !e.trim().is_empty())
            .ok_or_else(|| AccessTokenError::Invalid("token has no email claim".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_names_expand_to_the_cloudflareaccess_host() {
        assert_eq!(team_issuer("acme"), "https://acme.cloudflareaccess.com");
        assert_eq!(
            team_issuer("https://acme.cloudflareaccess.com/"),
            "https://acme.cloudflareaccess.com"
        );
    }
}
