use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};

use super::PortResult;
use crate::domain::granularity::Granularity;
use crate::domain::metric::{CounterMetric, GaugeMetric};
use crate::domain::rollup::{QualityTotals, RollupDelta};

/// Position of the aggregator in one topic partition, committed with the rollups it produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamCheckpoint {
    pub topic: String,
    pub partition: i32,
    /// Offset of the next record to consume.
    pub next_offset: i64,
}

/// One value of a metric dimension in one bucket.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionPoint {
    pub bucket: DateTime<Utc>,
    pub dimension: String,
    pub value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveUsersPoint {
    pub day: NaiveDate,
    pub dau: u64,
    /// Distinct users over the 30 days ending on `day`.
    pub mau: u64,
}

/// Persists rollups. `apply` must be atomic: rollups and checkpoint commit together.
#[async_trait]
pub trait RollupWriter: Send + Sync {
    async fn apply(
        &self,
        delta: &RollupDelta,
        checkpoint: Option<&StreamCheckpoint>,
    ) -> PortResult<()>;
    async fn checkpoint(&self, topic: &str, partition: i32) -> PortResult<Option<i64>>;
}

/// Reads rollups for dashboards.
#[async_trait]
pub trait RollupReader: Send + Sync {
    /// Counter values per dimension for buckets in `[from, to)`.
    async fn counters(
        &self,
        metric: CounterMetric,
        granularity: Granularity,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> PortResult<Vec<DimensionPoint>>;
    /// Peak gauge values per dimension for buckets in `[from, to)`.
    async fn gauges(
        &self,
        metric: GaugeMetric,
        granularity: Granularity,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> PortResult<Vec<DimensionPoint>>;
    /// DAU and rolling MAU for each day in `[from, to]`.
    async fn active_users(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> PortResult<Vec<ActiveUsersPoint>>;
    /// Voice-quality sums per region over minute buckets in `[from, to)`.
    async fn quality_by_region(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> PortResult<Vec<(String, QualityTotals)>>;
    async fn is_reachable(&self) -> bool;
}
