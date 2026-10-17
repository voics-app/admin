use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};

use super::usage_range::UsageRange;
use crate::application::error::AppResult;
use crate::application::ports::clock::Clock;
use crate::application::ports::rollups::{ActiveUsersPoint, DimensionPoint, RollupReader};
use crate::domain::granularity::Granularity;
use crate::domain::metric::{CounterMetric, GaugeMetric};
use crate::domain::signups::signups_from_totals;

/// Minimum number of days shown on the DAU/MAU chart.
const MIN_ACTIVE_USER_DAYS: i64 = 30;

#[derive(Debug, Clone, PartialEq)]
pub struct UsagePoint {
    pub bucket: DateTime<Utc>,
    pub messages_standard: i64,
    pub messages_ephemeral: i64,
    pub messages_p2p: i64,
    pub voice_minutes: f64,
    pub sessions_peak: i64,
    pub active_calls_peak: i64,
    pub signups: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UsageReport {
    pub range: UsageRange,
    pub granularity: Granularity,
    pub points: Vec<UsagePoint>,
    pub active_users: Vec<ActiveUsersPoint>,
}

/// Builds the usage time series for a dashboard range, zero-filling empty buckets.
pub struct GetUsageReport {
    rollups: Arc<dyn RollupReader>,
    clock: Arc<dyn Clock>,
}

impl GetUsageReport {
    pub fn new(rollups: Arc<dyn RollupReader>, clock: Arc<dyn Clock>) -> Self {
        Self { rollups, clock }
    }

    pub async fn execute(&self, range: UsageRange) -> AppResult<UsageReport> {
        let now = self.clock.now();
        let granularity = Granularity::for_range(range.duration());
        let to = granularity.bucket_start(now) + granularity.step();
        let from = to - range.duration();
        let buckets = bucket_starts(from, to, granularity.step());

        let messages = self
            .rollups
            .counters(CounterMetric::Messages, granularity, from, to)
            .await?;
        let voice = self
            .rollups
            .counters(CounterMetric::VoiceSeconds, granularity, from, to)
            .await?;
        let sessions = self
            .rollups
            .gauges(GaugeMetric::SessionsOnline, granularity, from, to)
            .await?;
        let calls = self
            .rollups
            .gauges(GaugeMetric::ActiveCalls, granularity, from, to)
            .await?;
        // One extra bucket before the window gives the first signup delta a baseline.
        let totals = self
            .rollups
            .gauges(
                GaugeMetric::UsersTotal,
                granularity,
                from - granularity.step(),
                to,
            )
            .await?;

        let signups: BTreeMap<_, _> = signups_from_totals(&dense_totals(&totals))
            .into_iter()
            .collect();
        let messages_by_mode = by_bucket_and_dimension(&messages);
        let voice_seconds = sum_by_bucket(&voice);
        let sessions_peak = sum_by_bucket(&sessions);
        let calls_peak = sum_by_bucket(&calls);

        let points = buckets
            .into_iter()
            .map(|bucket| {
                let mode = |name: &str| {
                    messages_by_mode
                        .get(&(bucket, name.to_owned()))
                        .copied()
                        .unwrap_or(0.0) as i64
                };
                UsagePoint {
                    bucket,
                    messages_standard: mode("standard"),
                    messages_ephemeral: mode("ephemeral"),
                    messages_p2p: mode("p2p"),
                    voice_minutes: voice_seconds.get(&bucket).copied().unwrap_or(0.0) / 60.0,
                    sessions_peak: sessions_peak.get(&bucket).copied().unwrap_or(0.0) as i64,
                    active_calls_peak: calls_peak.get(&bucket).copied().unwrap_or(0.0) as i64,
                    signups: signups.get(&bucket).copied().unwrap_or(0),
                }
            })
            .collect();

        let days = range.duration().num_days().max(MIN_ACTIVE_USER_DAYS);
        let today = now.date_naive();
        let active_users = self
            .rollups
            .active_users(today - Duration::days(days - 1), today)
            .await?;

        Ok(UsageReport {
            range,
            granularity,
            points,
            active_users,
        })
    }
}

fn bucket_starts(from: DateTime<Utc>, to: DateTime<Utc>, step: Duration) -> Vec<DateTime<Utc>> {
    std::iter::successors(Some(from), |b| Some(*b + step))
        .take_while(|b| *b < to)
        .collect()
}

fn sum_by_bucket(points: &[DimensionPoint]) -> BTreeMap<DateTime<Utc>, f64> {
    let mut sums = BTreeMap::new();
    for point in points {
        *sums.entry(point.bucket).or_insert(0.0) += point.value;
    }
    sums
}

fn by_bucket_and_dimension(points: &[DimensionPoint]) -> BTreeMap<(DateTime<Utc>, String), f64> {
    let mut sums = BTreeMap::new();
    for point in points {
        *sums
            .entry((point.bucket, point.dimension.clone()))
            .or_insert(0.0) += point.value;
    }
    sums
}

fn dense_totals(points: &[DimensionPoint]) -> Vec<(DateTime<Utc>, f64)> {
    sum_by_bucket(points).into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{FakeRollups, FixedClock, fixed_now};
    use chrono::NaiveDate;

    fn point(bucket: DateTime<Utc>, dimension: &str, value: f64) -> DimensionPoint {
        DimensionPoint {
            bucket,
            dimension: dimension.into(),
            value,
        }
    }

    fn hour(offset: i64) -> DateTime<Utc> {
        Granularity::Hour.bucket_start(fixed_now()) - Duration::hours(offset)
    }

    async fn report(store: FakeRollups, range: UsageRange) -> UsageReport {
        GetUsageReport::new(Arc::new(store), Arc::new(FixedClock::default()))
            .execute(range)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn a_day_has_24_hourly_zero_filled_buckets_ending_now() {
        let report = report(FakeRollups::default(), UsageRange::Day).await;
        assert_eq!(report.granularity, Granularity::Hour);
        assert_eq!(report.points.len(), 24);
        assert_eq!(report.points.last().unwrap().bucket, hour(0));
        assert!(
            report
                .points
                .iter()
                .all(|p| p.messages_standard == 0 && p.signups == 0)
        );
    }

    #[tokio::test]
    async fn an_hour_uses_minute_buckets() {
        let report = report(FakeRollups::default(), UsageRange::Hour).await;
        assert_eq!(report.granularity, Granularity::Minute);
        assert_eq!(report.points.len(), 60);
    }

    #[tokio::test]
    async fn messages_are_split_by_delivery_mode() {
        let store = FakeRollups::default().with_counter(
            CounterMetric::Messages,
            vec![
                point(hour(1), "standard", 40.0),
                point(hour(1), "ephemeral", 7.0),
                point(hour(1), "p2p", 2.0),
            ],
        );
        let report = report(store, UsageRange::Day).await;
        let p = report.points.iter().find(|p| p.bucket == hour(1)).unwrap();
        assert_eq!(
            (p.messages_standard, p.messages_ephemeral, p.messages_p2p),
            (40, 7, 2)
        );
    }

    #[tokio::test]
    async fn voice_seconds_are_reported_in_minutes_across_regions() {
        let store = FakeRollups::default().with_counter(
            CounterMetric::VoiceSeconds,
            vec![
                point(hour(2), "nl-home", 600.0),
                point(hour(2), "de-fra", 300.0),
            ],
        );
        let report = report(store, UsageRange::Day).await;
        assert_eq!(
            report
                .points
                .iter()
                .find(|p| p.bucket == hour(2))
                .unwrap()
                .voice_minutes,
            15.0
        );
    }

    #[tokio::test]
    async fn concurrent_sessions_sum_node_peaks() {
        let store = FakeRollups::default().with_gauge(
            GaugeMetric::SessionsOnline,
            vec![point(hour(0), "gw-0", 200.0), point(hour(0), "gw-1", 140.0)],
        );
        let report = report(store, UsageRange::Day).await;
        assert_eq!(report.points.last().unwrap().sessions_peak, 340);
    }

    #[tokio::test]
    async fn signups_use_the_bucket_before_the_window_as_baseline() {
        let first = hour(23);
        let store = FakeRollups::default().with_gauge(
            GaugeMetric::UsersTotal,
            vec![
                point(first - Duration::hours(1), "", 100.0),
                point(first, "", 103.0),
                point(hour(22), "", 110.0),
            ],
        );
        let report = report(store, UsageRange::Day).await;
        assert_eq!(report.points[0].signups, 3);
        assert_eq!(report.points[1].signups, 7);
    }

    #[tokio::test]
    async fn active_users_cover_at_least_thirty_days() {
        let store = FakeRollups::default();
        let today = fixed_now().date_naive();
        *store.active.lock().unwrap() = vec![
            ActiveUsersPoint {
                day: today,
                dau: 10,
                mau: 50,
            },
            ActiveUsersPoint {
                day: today - Duration::days(29),
                dau: 4,
                mau: 20,
            },
            ActiveUsersPoint {
                day: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                dau: 1,
                mau: 1,
            },
        ];
        let report = report(store, UsageRange::Day).await;
        assert_eq!(report.active_users.len(), 2);
    }
}
