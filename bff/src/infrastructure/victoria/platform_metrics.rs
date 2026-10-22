use std::collections::BTreeMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;

use crate::application::ports::platform_metrics::{
    HealthPanel, LabelledSeries, LabelledValue, PlatformMetrics, TargetAvailability,
};
use crate::application::ports::{PortError, PortResult};

/// Labels used to name a series, in order of preference.
const LABEL_KEYS: [&str; 4] = ["job", "group", "node", "instance"];

const UP_BY_JOB: &str = "sum by (job) (up)";
const TARGETS_BY_JOB: &str = "count by (job) (up)";

/// The PromQL behind each health panel. Services follow the platform metric conventions
/// documented in the README (`http_requests_total`, `http_request_duration_seconds`, ...).
pub fn promql(panel: HealthPanel) -> &'static str {
    match panel {
        HealthPanel::RequestRate => {
            "sum by (job) (rate(http_requests_total[5m])) or sum by (job) (rate(messages_http_requests_total[5m]))"
        }
        HealthPanel::ErrorRatio => {
            r#"(sum by (job) (rate(http_requests_total{status=~"5.."}[5m])) / sum by (job) (rate(http_requests_total[5m]))) or (sum by (job) (rate(messages_http_requests_total{status=~"5.."}[5m])) / sum by (job) (rate(messages_http_requests_total[5m])))"#
        }
        HealthPanel::LatencyP95 => {
            "1000 * histogram_quantile(0.95, sum by (job, le) (rate(http_request_duration_seconds_bucket[5m])))"
        }
        HealthPanel::ConsumerLag => "sum by (group) (redpanda_kafka_consumer_group_lag_sum)",
        HealthPanel::DbLatencyP95 => {
            "1000 * histogram_quantile(0.95, sum by (job, le) (rate(db_query_duration_seconds_bucket[5m])))"
        }
        HealthPanel::GatewaySessions => {
            "sum by (instance) (discordy_gateway_gauges_sessions_local)"
        }
    }
}

#[derive(Debug, Deserialize)]
struct PromResponse {
    status: String,
    #[serde(default)]
    data: Option<PromData>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PromData {
    #[serde(default)]
    result: Vec<PromSeries>,
}

#[derive(Debug, Deserialize)]
struct PromSeries {
    #[serde(default)]
    metric: BTreeMap<String, String>,
    #[serde(default)]
    value: Option<(f64, String)>,
    #[serde(default)]
    values: Vec<(f64, String)>,
}

/// [`PlatformMetrics`] over the VictoriaMetrics Prometheus-compatible query API.
pub struct VictoriaPlatformMetrics {
    http: Client,
    base_url: String,
}

impl VictoriaPlatformMetrics {
    pub fn new(http: Client, base_url: &str) -> Self {
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_owned(),
        }
    }

    async fn query(&self, path: &str, params: &[(&str, String)]) -> PortResult<Vec<PromSeries>> {
        let response = self
            .http
            .get(format!("{}{path}", self.base_url))
            .query(params)
            .send()
            .await
            .map_err(|e| PortError::Unavailable(format!("victoriametrics: {}", e.without_url())))?;
        let body: PromResponse = response
            .json()
            .await
            .map_err(|e| PortError::Unavailable(format!("victoriametrics: {e}")))?;
        if body.status != "success" {
            return Err(PortError::Unavailable(format!(
                "victoriametrics: {}",
                body.error.unwrap_or_default()
            )));
        }
        Ok(body.data.map(|d| d.result).unwrap_or_default())
    }

    async fn instant(&self, promql: &str) -> PortResult<Vec<LabelledValue>> {
        let series = self
            .query("/api/v1/query", &[("query", promql.to_owned())])
            .await?;
        Ok(series
            .into_iter()
            .filter_map(|s| {
                let value = s.value.as_ref().and_then(|(_, v)| parse_finite(v))?;
                Some(LabelledValue {
                    label: label_of(&s.metric),
                    value,
                })
            })
            .collect())
    }
}

fn parse_finite(value: &str) -> Option<f64> {
    value.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Picks a readable name for a series; k8s-style `namespace/name` jobs are shortened to `name`.
fn label_of(metric: &BTreeMap<String, String>) -> String {
    LABEL_KEYS
        .iter()
        .find_map(|key| metric.get(*key))
        .map(|v| v.rsplit('/').next().unwrap_or(v).to_owned())
        .unwrap_or_else(|| "total".to_owned())
}

#[async_trait]
impl PlatformMetrics for VictoriaPlatformMetrics {
    async fn target_availability(&self) -> PortResult<Vec<TargetAvailability>> {
        let (up, total) =
            futures::try_join!(self.instant(UP_BY_JOB), self.instant(TARGETS_BY_JOB))?;
        let up: BTreeMap<String, f64> = up.into_iter().map(|v| (v.label, v.value)).collect();
        Ok(total
            .into_iter()
            .map(|t| TargetAvailability {
                up: up.get(&t.label).copied().unwrap_or(0.0) as u32,
                total: t.value as u32,
                service: t.label,
            })
            .collect())
    }

    async fn snapshot(&self, panel: HealthPanel) -> PortResult<Vec<LabelledValue>> {
        self.instant(promql(panel)).await
    }

    async fn series(
        &self,
        panel: HealthPanel,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        step_seconds: u32,
    ) -> PortResult<Vec<LabelledSeries>> {
        let params = [
            ("query", promql(panel).to_owned()),
            ("start", from.timestamp().to_string()),
            ("end", to.timestamp().to_string()),
            ("step", format!("{step_seconds}s")),
        ];
        let series = self.query("/api/v1/query_range", &params).await?;
        Ok(series
            .into_iter()
            .map(|s| LabelledSeries {
                label: label_of(&s.metric),
                points: s
                    .values
                    .iter()
                    .filter_map(|(ts, v)| {
                        Some((DateTime::from_timestamp(*ts as i64, 0)?, parse_finite(v)?))
                    })
                    .collect(),
            })
            .collect())
    }

    async fn is_reachable(&self) -> bool {
        self.http
            .get(format!("{}/health", self.base_url))
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
    }
}
