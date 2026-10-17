use std::sync::Arc;

use crate::application::ports::PortResult;
use crate::application::ports::api_admin::ApiAdmin;
use crate::application::ports::clock::Clock;
use crate::application::ports::rollups::RollupWriter;
use crate::domain::metric::GaugeMetric;
use crate::domain::rollup::RollupDelta;
use crate::domain::usage_signal::UsageSignal;

/// Samples the registered-user total from api so signups can be charted over time.
pub struct RecordPlatformTotals {
    api: Arc<dyn ApiAdmin>,
    writer: Arc<dyn RollupWriter>,
    clock: Arc<dyn Clock>,
}

impl RecordPlatformTotals {
    pub fn new(
        api: Arc<dyn ApiAdmin>,
        writer: Arc<dyn RollupWriter>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self { api, writer, clock }
    }

    pub async fn execute(&self) -> PortResult<()> {
        let stats = self.api.stats().await?;
        let mut delta = RollupDelta::default();
        let signal = UsageSignal::Gauge {
            metric: GaugeMetric::UsersTotal,
            dimension: String::new(),
            value: stats.users_total as f64,
        };
        delta.record(self.clock.now(), &signal);
        self.writer.apply(&delta, None).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{FakeApi, FakeRollups, FixedClock, unavailable};

    #[tokio::test]
    async fn records_the_user_total_as_a_gauge() {
        let store = Arc::new(FakeRollups::default());
        let use_case = RecordPlatformTotals::new(
            Arc::new(FakeApi::default()),
            store.clone(),
            Arc::new(FixedClock::default()),
        );
        use_case.execute().await.unwrap();
        let applied = store.applied.lock().unwrap();
        let peaks: Vec<f64> = applied[0]
            .0
            .gauges()
            .filter(|(k, _)| k.metric == "users_total")
            .map(|(_, v)| v)
            .collect();
        assert_eq!(peaks, vec![1200.0, 1200.0, 1200.0]);
        assert_eq!(applied[0].1, None);
    }

    #[tokio::test]
    async fn writes_nothing_when_api_is_down() {
        let store = Arc::new(FakeRollups::default());
        let api = FakeApi::default();
        *api.stats.lock().unwrap() = Err(unavailable());
        let use_case = RecordPlatformTotals::new(
            Arc::new(api),
            store.clone(),
            Arc::new(FixedClock::default()),
        );
        assert!(use_case.execute().await.is_err());
        assert!(store.applied.lock().unwrap().is_empty());
    }
}
