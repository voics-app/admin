use std::time::Duration;

use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::application::ports::{PortError, PortResult};
use crate::infrastructure::contracts::INTERNAL_TOKEN_HEADER;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Builds the shared outbound HTTP client.
pub fn http_client() -> Client {
    Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("discordy-admin-bff/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
}

/// The outbound client for calls that take long, such as a broadcast the notifier paces.
pub fn slow_http_client(timeout: Duration) -> Client {
    Client::builder()
        .timeout(timeout)
        .user_agent(concat!("discordy-admin-bff/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
}

/// Calls a service's `/internal/v1/*` API with the shared `X-Internal-Token` (P4).
#[derive(Clone)]
pub struct InternalClient {
    http: Client,
    base_url: String,
    token: String,
}

impl InternalClient {
    pub fn new(http: Client, base_url: &str, token: &str) -> Self {
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_owned(),
            token: token.to_owned(),
        }
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    pub async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> PortResult<T> {
        let response = self
            .send(self.http.get(self.url(path)).query(query))
            .await?;
        response
            .json()
            .await
            .map_err(|e| PortError::Unavailable(format!("invalid response from {path}: {e}")))
    }

    pub async fn post<B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: Option<&B>,
    ) -> PortResult<()> {
        let mut request = self.http.post(self.url(path));
        if let Some(body) = body {
            request = request.json(body);
        }
        self.send(request).await.map(|_| ())
    }

    pub async fn post_json<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> PortResult<T> {
        let response = self.send(self.http.post(self.url(path)).json(body)).await?;
        response
            .json()
            .await
            .map_err(|e| PortError::Unavailable(format!("invalid response from {path}: {e}")))
    }

    async fn send(&self, request: RequestBuilder) -> PortResult<Response> {
        let response = request
            .header(INTERNAL_TOKEN_HEADER, &self.token)
            .send()
            .await
            .map_err(|e| PortError::Unavailable(e.without_url().to_string()))?;
        match response.status() {
            status if status.is_success() => Ok(response),
            StatusCode::NOT_FOUND => Err(PortError::NotFound),
            status if status.is_server_error() => Err(PortError::Unavailable(format!(
                "upstream returned {status}"
            ))),
            status => {
                let message = response
                    .text()
                    .await
                    .unwrap_or_default()
                    .chars()
                    .take(300)
                    .collect();
                Err(PortError::Rejected {
                    status: status.as_u16(),
                    message,
                })
            }
        }
    }
}
