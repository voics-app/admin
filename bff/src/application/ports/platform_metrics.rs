use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Serialize;

use super::PortResult;

/// Scrape-target availability of one service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetAvailability {
    pub service: String,
    pub up: u32,
    pub total: u32,
}

/// A labelled instant value, e.g. one service's error ratio.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LabelledValue {
    pub label: String,
    pub value: f64,
}

/// One labelled series of (timestamp, value) points.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LabelledSeries {
    pub label: String,
    pub points: Vec<(DateTime<Utc>, f64)>,
}

/// Named charts on the platform-health page. The adapter owns the PromQL behind each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HealthPanel {
    RequestRate,
    ErrorRatio,
    LatencyP95,
    ConsumerLag,
    DbLatencyP95,
    GatewaySessions,
    PushDrops,
    PushQueue,
    PushInFlight,
}

impl HealthPanel {
    pub const ALL: [HealthPanel; 9] = [
        HealthPanel::RequestRate,
        HealthPanel::ErrorRatio,
        HealthPanel::LatencyP95,
        HealthPanel::ConsumerLag,
        HealthPanel::DbLatencyP95,
        HealthPanel::GatewaySessions,
        HealthPanel::PushDrops,
        HealthPanel::PushQueue,
        HealthPanel::PushInFlight,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            HealthPanel::RequestRate => "request_rate",
            HealthPanel::ErrorRatio => "error_ratio",
            HealthPanel::LatencyP95 => "latency_p95",
            HealthPanel::ConsumerLag => "consumer_lag",
            HealthPanel::DbLatencyP95 => "db_latency_p95",
            HealthPanel::GatewaySessions => "gateway_sessions",
            HealthPanel::PushDrops => "push_drops",
            HealthPanel::PushQueue => "push_queue",
            HealthPanel::PushInFlight => "push_in_flight",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == value)
    }
}

/// Platform telemetry from the metrics backend (VictoriaMetrics).
#[async_trait]
pub trait PlatformMetrics: Send + Sync {
    async fn target_availability(&self) -> PortResult<Vec<TargetAvailability>>;
    /// Current value of a panel, one entry per label (service, consumer group, node...).
    async fn snapshot(&self, panel: HealthPanel) -> PortResult<Vec<LabelledValue>>;
    async fn series(
        &self,
        panel: HealthPanel,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        step_seconds: u32,
    ) -> PortResult<Vec<LabelledSeries>>;
    async fn is_reachable(&self) -> bool;
}
