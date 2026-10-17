use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use crate::application::ports::voice_directory::VoiceDirectory;
use crate::application::ports::{PortError, PortResult};
use crate::domain::voice_region::VoiceNode;
use crate::infrastructure::contracts::{VOICE_NODES_PREFIX, VoiceNodeDto};

#[derive(Debug, Deserialize)]
struct RangeResponse {
    #[serde(default)]
    kvs: Vec<KeyValue>,
}

#[derive(Debug, Deserialize)]
struct KeyValue {
    #[serde(default)]
    value: String,
}

#[derive(Debug, Deserialize)]
struct HealthResponse {
    health: String,
}

/// Reads voice-node leases from etcd through its v3 JSON gateway (`/v3/kv/range`), so no gRPC
/// toolchain is needed. Endpoints are tried in order until one answers.
pub struct EtcdVoiceDirectory {
    http: Client,
    endpoints: Vec<String>,
}

impl EtcdVoiceDirectory {
    pub fn new(http: Client, endpoints: Vec<String>) -> Self {
        let endpoints = endpoints
            .into_iter()
            .map(|e| e.trim_end_matches('/').to_owned())
            .collect();
        Self { http, endpoints }
    }

    async fn range(&self, endpoint: &str) -> Result<RangeResponse, String> {
        let body = json!({ "key": B64.encode(VOICE_NODES_PREFIX), "range_end": B64.encode(prefix_end(VOICE_NODES_PREFIX)) });
        let response = self
            .http
            .post(format!("{endpoint}/v3/kv/range"))
            .json(&body)
            .send()
            .await
            .map_err(|e| e.without_url().to_string())?;
        if !response.status().is_success() {
            return Err(format!("etcd returned {}", response.status()));
        }
        response.json().await.map_err(|e| e.to_string())
    }
}

/// The smallest key greater than every key with this prefix.
fn prefix_end(prefix: &str) -> Vec<u8> {
    let mut end = prefix.as_bytes().to_vec();
    while let Some(last) = end.pop() {
        if last < u8::MAX {
            end.push(last + 1);
            return end;
        }
    }
    vec![0]
}

fn decode_node(kv: &KeyValue) -> Option<VoiceNode> {
    let bytes = B64.decode(&kv.value).ok()?;
    let dto: VoiceNodeDto = serde_json::from_slice(&bytes).ok()?;
    Some(VoiceNode {
        node_id: dto.node_id,
        region: dto.region,
        ws_url: dto.ws_url,
        public_ip: dto.public_ip,
        udp_port: dto.udp_port,
        users: dto.users,
        cpu_pct: dto.cpu_pct,
        egress_bps: dto.egress_bps,
        draining: dto.draining,
        version: dto.version,
    })
}

#[async_trait]
impl VoiceDirectory for EtcdVoiceDirectory {
    async fn nodes(&self) -> PortResult<Vec<VoiceNode>> {
        let mut last_error = String::from("no etcd endpoints configured");
        for endpoint in &self.endpoints {
            match self.range(endpoint).await {
                Ok(range) => {
                    let nodes: Vec<VoiceNode> = range.kvs.iter().filter_map(decode_node).collect();
                    let skipped = range.kvs.len() - nodes.len();
                    if skipped > 0 {
                        tracing::warn!(skipped, "ignoring malformed voice node registrations");
                    }
                    return Ok(nodes);
                }
                Err(error) => last_error = error,
            }
        }
        Err(PortError::Unavailable(format!("etcd: {last_error}")))
    }

    async fn is_healthy(&self) -> bool {
        for endpoint in &self.endpoints {
            if let Ok(response) = self.http.get(format!("{endpoint}/health")).send().await
                && let Ok(health) = response.json::<HealthResponse>().await
                && health.health == "true"
            {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_end_increments_the_last_byte() {
        assert_eq!(
            prefix_end("/discordy/voice/nodes/"),
            b"/discordy/voice/nodes0".to_vec()
        );
    }
}
