use std::sync::Arc;

use super::usage_range::UsageRange;
use crate::application::error::AppResult;
use crate::application::ports::clock::Clock;
use crate::application::ports::platform_metrics::{HealthPanel, LabelledSeries, PlatformMetrics};

/// Target number of points per chart line.
const POINTS_PER_SERIES: i64 = 120;
const MIN_STEP_SECONDS: i64 = 15;

/// Time series behind one platform-health chart.
pub struct GetHealthSeries {
    metrics: Arc<dyn PlatformMetrics>,
    clock: Arc<dyn Clock>,
}

impl GetHealthSeries {
    pub fn new(metrics: Arc<dyn PlatformMetrics>, clock: Arc<dyn Clock>) -> Self {
        Self { metrics, clock }
    }

    pub async fn execute(&self, panel: HealthPanel, range: UsageRange) -> AppResult<Vec<LabelledSeries>> {
        let to = self.clock.now();
        let from = to - range.duration();
        let step = (range.duration().num_seconds() / POINTS_PER_SERIES).max(MIN_STEP_SECONDS);
        let step = u32::try_from(step).unwrap_or(u32::MAX);
        Ok(self.metrics.series(panel, from, to, step).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{FakeMetrics, FixedClock, fixed_now};
    use chrono::Duration;

    #[tokio::test]
    async fn spreads_the_range_over_about_120_points() {
        let metrics = Arc::new(FakeMetrics::default());
        let use_case = GetHealthSeries::new(metrics.clone(), Arc::new(FixedClock::default()));
        use_case.execute(HealthPanel::ErrorRatio, UsageRange::Day).await.unwrap();
        let calls = metrics.series_calls.lock().unwrap();
        assert_eq!(calls[0], (HealthPanel::ErrorRatio, fixed_now() - Duration::hours(24), fixed_now(), 720));
    }

    #[tokio::test]
    async fn never_steps_finer_than_fifteen_seconds() {
        let metrics = Arc::new(FakeMetrics::default());
        let use_case = GetHealthSeries::new(metrics.clone(), Arc::new(FixedClock::default()));
        use_case.execute(HealthPanel::RequestRate, UsageRange::Hour).await.unwrap();
        assert_eq!(metrics.series_calls.lock().unwrap()[0].3, 30);
    }
}
