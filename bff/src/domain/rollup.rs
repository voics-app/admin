use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, NaiveDate, Utc};

use super::granularity::Granularity;
use super::observed_event::VoiceQualitySample;
use super::usage_signal::UsageSignal;

/// Identifies one rollup cell: a metric/dimension pair in one time bucket.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SeriesKey {
    pub metric: &'static str,
    pub dimension: String,
    pub granularity: Granularity,
    pub bucket: DateTime<Utc>,
}

/// Identifies one voice-quality rollup cell.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QualityKey {
    pub region: String,
    pub granularity: Granularity,
    pub bucket: DateTime<Utc>,
}

/// Running sums of voice-quality samples; averages are derived on read.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct QualityTotals {
    pub samples: i64,
    pub jitter_ms: f64,
    pub loss_pct: f64,
    pub rtt_ms: f64,
    pub bitrate_kbps: f64,
}

/// Mean voice quality over a set of samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QualityAverages {
    pub samples: i64,
    pub jitter_ms: f64,
    pub loss_pct: f64,
    pub rtt_ms: f64,
    pub bitrate_kbps: f64,
}

impl QualityTotals {
    pub fn add(&mut self, sample: &VoiceQualitySample) {
        self.merge(&QualityTotals {
            samples: 1,
            jitter_ms: sample.jitter_ms,
            loss_pct: sample.loss_pct,
            rtt_ms: sample.rtt_ms,
            bitrate_kbps: sample.bitrate_kbps,
        });
    }

    pub fn merge(&mut self, other: &QualityTotals) {
        self.samples += other.samples;
        self.jitter_ms += other.jitter_ms;
        self.loss_pct += other.loss_pct;
        self.rtt_ms += other.rtt_ms;
        self.bitrate_kbps += other.bitrate_kbps;
    }

    pub fn averages(&self) -> Option<QualityAverages> {
        if self.samples <= 0 {
            return None;
        }
        let n = self.samples as f64;
        Some(QualityAverages {
            samples: self.samples,
            jitter_ms: self.jitter_ms / n,
            loss_pct: self.loss_pct / n,
            rtt_ms: self.rtt_ms / n,
            bitrate_kbps: self.bitrate_kbps / n,
        })
    }
}

/// The aggregated effect of a batch of events, ready to be upserted atomically.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RollupDelta {
    counters: BTreeMap<SeriesKey, i64>,
    gauges: BTreeMap<SeriesKey, f64>,
    active_users: BTreeSet<(NaiveDate, String)>,
    quality: BTreeMap<QualityKey, QualityTotals>,
}

impl RollupDelta {
    /// Folds one signal observed at `at` into every granularity.
    pub fn record(&mut self, at: DateTime<Utc>, signal: &UsageSignal) {
        match signal {
            UsageSignal::Count { metric, dimension, amount } => {
                for key in series_keys(metric.as_str(), dimension, at) {
                    *self.counters.entry(key).or_insert(0) += amount;
                }
            }
            UsageSignal::Gauge { metric, dimension, value } => {
                for key in series_keys(metric.as_str(), dimension, at) {
                    let peak = self.gauges.entry(key).or_insert(*value);
                    *peak = peak.max(*value);
                }
            }
            UsageSignal::ActiveUser { user_id } => {
                self.active_users.insert((at.date_naive(), user_id.clone()));
            }
            UsageSignal::Quality(sample) => {
                for granularity in Granularity::ALL {
                    let key = QualityKey {
                        region: sample.region.clone(),
                        granularity,
                        bucket: granularity.bucket_start(at),
                    };
                    self.quality.entry(key).or_default().add(sample);
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.counters.is_empty() && self.gauges.is_empty() && self.active_users.is_empty() && self.quality.is_empty()
    }

    /// Increments to add to counter cells.
    pub fn counters(&self) -> impl Iterator<Item = (&SeriesKey, i64)> {
        self.counters.iter().map(|(k, v)| (k, *v))
    }

    /// Peak values to merge (max) into gauge cells.
    pub fn gauges(&self) -> impl Iterator<Item = (&SeriesKey, f64)> {
        self.gauges.iter().map(|(k, v)| (k, *v))
    }

    /// Distinct (UTC day, user) activity pairs.
    pub fn active_users(&self) -> impl Iterator<Item = &(NaiveDate, String)> {
        self.active_users.iter()
    }

    /// Voice-quality sums to add to quality cells.
    pub fn quality(&self) -> impl Iterator<Item = (&QualityKey, &QualityTotals)> {
        self.quality.iter()
    }
}

fn series_keys<'a>(metric: &'static str, dimension: &'a str, at: DateTime<Utc>) -> impl Iterator<Item = SeriesKey> + 'a {
    Granularity::ALL.into_iter().map(move |granularity| SeriesKey {
        metric,
        dimension: dimension.to_owned(),
        granularity,
        bucket: granularity.bucket_start(at),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::metric::{CounterMetric, GaugeMetric};
    use chrono::TimeZone;

    fn at(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 16, h, m, 15).unwrap()
    }

    fn key(metric: &'static str, dimension: &str, granularity: Granularity, bucket: DateTime<Utc>) -> SeriesKey {
        SeriesKey { metric, dimension: dimension.into(), granularity, bucket: granularity.bucket_start(bucket) }
    }

    fn messages(amount: i64) -> UsageSignal {
        UsageSignal::Count { metric: CounterMetric::Messages, dimension: "standard".into(), amount }
    }

    fn sessions(node: &str, value: f64) -> UsageSignal {
        UsageSignal::Gauge { metric: GaugeMetric::SessionsOnline, dimension: node.into(), value }
    }

    fn sample(jitter: f64, loss: f64) -> VoiceQualitySample {
        VoiceQualitySample { region: "nl-home".into(), jitter_ms: jitter, loss_pct: loss, rtt_ms: 20.0, bitrate_kbps: 64.0 }
    }

    fn counter(delta: &RollupDelta, k: &SeriesKey) -> Option<i64> {
        delta.counters().find(|(key, _)| *key == k).map(|(_, v)| v)
    }

    fn gauge(delta: &RollupDelta, k: &SeriesKey) -> Option<f64> {
        delta.gauges().find(|(key, _)| *key == k).map(|(_, v)| v)
    }

    #[test]
    fn new_delta_is_empty() {
        assert!(RollupDelta::default().is_empty());
    }

    #[test]
    fn a_count_lands_in_every_granularity() {
        let mut delta = RollupDelta::default();
        delta.record(at(13, 37), &messages(1));
        for g in Granularity::ALL {
            assert_eq!(counter(&delta, &key("messages", "standard", g, at(13, 37))), Some(1), "{g:?}");
        }
        assert!(!delta.is_empty());
    }

    #[test]
    fn counts_in_the_same_bucket_are_summed() {
        let mut delta = RollupDelta::default();
        delta.record(at(13, 37), &messages(2));
        delta.record(at(13, 37), &messages(3));
        assert_eq!(counter(&delta, &key("messages", "standard", Granularity::Minute, at(13, 37))), Some(5));
    }

    #[test]
    fn counts_in_different_minutes_share_the_hour_bucket() {
        let mut delta = RollupDelta::default();
        delta.record(at(13, 1), &messages(1));
        delta.record(at(13, 59), &messages(1));
        assert_eq!(counter(&delta, &key("messages", "standard", Granularity::Minute, at(13, 1))), Some(1));
        assert_eq!(counter(&delta, &key("messages", "standard", Granularity::Minute, at(13, 59))), Some(1));
        assert_eq!(counter(&delta, &key("messages", "standard", Granularity::Hour, at(13, 0))), Some(2));
    }

    #[test]
    fn gauges_keep_the_peak_per_bucket() {
        let mut delta = RollupDelta::default();
        delta.record(at(13, 1), &sessions("gw-0", 10.0));
        delta.record(at(13, 1), &sessions("gw-0", 30.0));
        delta.record(at(13, 1), &sessions("gw-0", 20.0));
        assert_eq!(gauge(&delta, &key("sessions_online", "gw-0", Granularity::Minute, at(13, 1))), Some(30.0));
    }

    #[test]
    fn gauges_are_tracked_per_dimension() {
        let mut delta = RollupDelta::default();
        delta.record(at(13, 1), &sessions("gw-0", 10.0));
        delta.record(at(13, 1), &sessions("gw-1", 5.0));
        assert_eq!(gauge(&delta, &key("sessions_online", "gw-1", Granularity::Hour, at(13, 1))), Some(5.0));
    }

    #[test]
    fn active_users_are_distinct_per_utc_day() {
        let mut delta = RollupDelta::default();
        let user = UsageSignal::ActiveUser { user_id: "42".into() };
        delta.record(at(1, 0), &user);
        delta.record(at(23, 0), &user);
        delta.record(Utc.with_ymd_and_hms(2026, 10, 17, 0, 5, 0).unwrap(), &user);
        let days: Vec<_> = delta.active_users().map(|(day, _)| day.to_string()).collect();
        assert_eq!(days, vec!["2026-10-16", "2026-10-17"]);
    }

    #[test]
    fn quality_samples_are_summed_per_region_bucket() {
        let mut delta = RollupDelta::default();
        delta.record(at(13, 1), &UsageSignal::Quality(sample(4.0, 1.0)));
        delta.record(at(13, 1), &UsageSignal::Quality(sample(6.0, 3.0)));
        let quality_key = QualityKey { region: "nl-home".into(), granularity: Granularity::Minute, bucket: Granularity::Minute.bucket_start(at(13, 1)) };
        let totals = delta.quality().find(|(k, _)| **k == quality_key).map(|(_, t)| *t).unwrap();
        assert_eq!(totals.samples, 2);
        let averages = totals.averages().unwrap();
        assert_eq!(averages.jitter_ms, 5.0);
        assert_eq!(averages.loss_pct, 2.0);
        assert_eq!(averages.rtt_ms, 20.0);
    }

    #[test]
    fn empty_quality_totals_have_no_averages() {
        assert_eq!(QualityTotals::default().averages(), None);
    }

    #[test]
    fn merging_quality_totals_adds_sums() {
        let mut left = QualityTotals::default();
        left.add(&sample(2.0, 0.0));
        let mut right = QualityTotals::default();
        right.add(&sample(4.0, 2.0));
        left.merge(&right);
        assert_eq!(left.samples, 2);
        assert_eq!(left.averages().unwrap().jitter_ms, 3.0);
    }
}
