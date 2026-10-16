use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::Duration;

use crate::application::error::AppResult;
use crate::application::ports::clock::Clock;
use crate::application::ports::rollups::RollupReader;
use crate::application::ports::voice_directory::VoiceDirectory;
use crate::domain::granularity::Granularity;
use crate::domain::rollup::{QualityAverages, QualityTotals};
use crate::domain::voice_region::{RegionLoad, VoiceNode, summarize_regions};

/// Client voice-quality reports considered "current".
pub const QUALITY_WINDOW_MINUTES: i64 = 15;

#[derive(Debug, Clone, PartialEq)]
pub struct RegionView {
    pub load: RegionLoad,
    pub quality: Option<QualityAverages>,
    pub nodes: Vec<VoiceNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceRegionsReport {
    pub regions: Vec<RegionView>,
    pub etcd_healthy: bool,
    pub quality_window_minutes: i64,
}

/// Voice regions from etcd joined with recent VOICE_QUALITY telemetry.
pub struct ListVoiceRegions {
    directory: Arc<dyn VoiceDirectory>,
    rollups: Arc<dyn RollupReader>,
    clock: Arc<dyn Clock>,
}

impl ListVoiceRegions {
    pub fn new(directory: Arc<dyn VoiceDirectory>, rollups: Arc<dyn RollupReader>, clock: Arc<dyn Clock>) -> Self {
        Self { directory, rollups, clock }
    }

    pub async fn execute(&self) -> AppResult<VoiceRegionsReport> {
        let now = self.clock.now();
        let to = Granularity::Minute.bucket_start(now) + Duration::minutes(1);
        let from = to - Duration::minutes(QUALITY_WINDOW_MINUTES);
        let (nodes, quality, etcd_healthy) =
            futures::join!(self.directory.nodes(), self.rollups.quality_by_region(from, to), self.directory.is_healthy());
        let nodes = nodes?;
        let quality = merge_quality(quality.unwrap_or_default());

        let regions = summarize_regions(&nodes)
            .into_iter()
            .map(|load| {
                let mut members: Vec<VoiceNode> = nodes.iter().filter(|n| n.region == load.region).cloned().collect();
                members.sort_by(|a, b| a.node_id.cmp(&b.node_id));
                let quality = quality.get(&load.region).and_then(QualityTotals::averages);
                RegionView { load, quality, nodes: members }
            })
            .collect();
        Ok(VoiceRegionsReport { regions, etcd_healthy, quality_window_minutes: QUALITY_WINDOW_MINUTES })
    }
}

fn merge_quality(rows: Vec<(String, QualityTotals)>) -> BTreeMap<String, QualityTotals> {
    let mut merged: BTreeMap<String, QualityTotals> = BTreeMap::new();
    for (region, totals) in rows {
        merged.entry(region).or_default().merge(&totals);
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::error::AppError;
    use crate::application::fakes::{FakeRollups, FakeVoice, FixedClock, unavailable, voice_node};

    fn totals(samples: i64, jitter_sum: f64) -> QualityTotals {
        QualityTotals { samples, jitter_ms: jitter_sum, loss_pct: 0.0, rtt_ms: 0.0, bitrate_kbps: 0.0 }
    }

    #[tokio::test]
    async fn joins_region_load_with_recent_quality() {
        let voice = FakeVoice::default();
        *voice.nodes.lock().unwrap() = Ok(vec![voice_node("nl-2", "nl-home", 3), voice_node("nl-1", "nl-home", 5), voice_node("de-1", "de-fra", 1)]);
        let rollups = FakeRollups::default();
        *rollups.quality.lock().unwrap() = vec![("nl-home".into(), totals(2, 10.0)), ("nl-home".into(), totals(2, 2.0))];
        let report = ListVoiceRegions::new(Arc::new(voice), Arc::new(rollups), Arc::new(FixedClock::default())).execute().await.unwrap();

        assert!(report.etcd_healthy);
        assert_eq!(report.regions.len(), 2);
        let nl = &report.regions[1];
        assert_eq!(nl.load.users, 8);
        assert_eq!(nl.nodes.iter().map(|n| n.node_id.as_str()).collect::<Vec<_>>(), vec!["nl-1", "nl-2"]);
        assert_eq!(nl.quality.unwrap().jitter_ms, 3.0);
        assert_eq!(report.regions[0].quality, None);
    }

    #[tokio::test]
    async fn missing_telemetry_does_not_hide_regions() {
        let rollups = FakeRollups::default();
        *rollups.fail_reads.lock().unwrap() = true;
        let report = ListVoiceRegions::new(Arc::new(FakeVoice::default()), Arc::new(rollups), Arc::new(FixedClock::default())).execute().await.unwrap();
        assert_eq!(report.regions.len(), 1);
    }

    #[tokio::test]
    async fn an_unreachable_directory_is_a_dependency_error() {
        let voice = FakeVoice::default();
        *voice.nodes.lock().unwrap() = Err(unavailable());
        let result = ListVoiceRegions::new(Arc::new(voice), Arc::new(FakeRollups::default()), Arc::new(FixedClock::default())).execute().await;
        assert!(matches!(result, Err(AppError::Dependency(_))));
    }
}
