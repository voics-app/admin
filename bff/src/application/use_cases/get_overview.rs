use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};

use crate::application::ports::api_admin::{ApiAdmin, ApiStats};
use crate::application::ports::clock::Clock;
use crate::application::ports::gateway_admin::{GatewayAdmin, GatewayStats};
use crate::application::ports::rollups::RollupReader;
use crate::application::ports::voice_directory::VoiceDirectory;
use crate::domain::granularity::Granularity;
use crate::domain::metric::CounterMetric;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MessagesByMode {
    pub standard: i64,
    pub ephemeral: i64,
    pub p2p: i64,
}

impl MessagesByMode {
    pub fn total(&self) -> i64 {
        self.standard + self.ephemeral + self.p2p
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Overview {
    pub generated_at: DateTime<Utc>,
    pub api: Option<ApiStats>,
    pub gateway: Option<GatewayStats>,
    pub messages_24h: Option<MessagesByMode>,
    pub voice_minutes_24h: Option<f64>,
    pub dau: Option<u64>,
    pub mau: Option<u64>,
    pub voice_nodes: Option<usize>,
    pub voice_users: Option<u64>,
    /// Sources that failed; their fields are `None`.
    pub degraded: Vec<&'static str>,
}

/// Headline KPIs. Each source is optional: one failing dependency never blanks the page.
pub struct GetOverview {
    api: Arc<dyn ApiAdmin>,
    gateway: Arc<dyn GatewayAdmin>,
    rollups: Arc<dyn RollupReader>,
    voice: Arc<dyn VoiceDirectory>,
    clock: Arc<dyn Clock>,
}

impl GetOverview {
    pub fn new(
        api: Arc<dyn ApiAdmin>,
        gateway: Arc<dyn GatewayAdmin>,
        rollups: Arc<dyn RollupReader>,
        voice: Arc<dyn VoiceDirectory>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            api,
            gateway,
            rollups,
            voice,
            clock,
        }
    }

    pub async fn execute(&self) -> Overview {
        let now = self.clock.now();
        let to = Granularity::Hour.bucket_start(now) + Duration::hours(1);
        let from = to - Duration::hours(24);
        let today = now.date_naive();

        let (api, gateway, messages, voice_seconds, active, nodes) = futures::join!(
            self.api.stats(),
            self.gateway.stats(),
            self.rollups
                .counters(CounterMetric::Messages, Granularity::Hour, from, to),
            self.rollups
                .counters(CounterMetric::VoiceSeconds, Granularity::Hour, from, to),
            self.rollups.active_users(today, today),
            self.voice.nodes(),
        );

        let mut degraded = Vec::new();
        let mut keep = |name: &'static str, ok: bool| {
            if !ok && !degraded.contains(&name) {
                degraded.push(name);
            }
        };
        keep("api", api.is_ok());
        keep("gateway", gateway.is_ok());
        keep(
            "rollups",
            messages.is_ok() && voice_seconds.is_ok() && active.is_ok(),
        );
        keep("voice_directory", nodes.is_ok());

        let messages_24h = messages.ok().map(|points| {
            let mut by_mode = MessagesByMode::default();
            for point in points {
                let value = point.value as i64;
                match point.dimension.as_str() {
                    "standard" => by_mode.standard += value,
                    "ephemeral" => by_mode.ephemeral += value,
                    "p2p" => by_mode.p2p += value,
                    _ => {}
                }
            }
            by_mode
        });
        let active_today = active
            .ok()
            .map(|points| points.first().map(|p| (p.dau, p.mau)).unwrap_or((0, 0)));
        let nodes = nodes.ok();

        Overview {
            generated_at: now,
            api: api.ok(),
            gateway: gateway.ok(),
            messages_24h,
            voice_minutes_24h: voice_seconds
                .ok()
                .map(|points| points.iter().map(|p| p.value).sum::<f64>() / 60.0),
            dau: active_today.map(|(dau, _)| dau),
            mau: active_today.map(|(_, mau)| mau),
            voice_nodes: nodes.as_ref().map(Vec::len),
            voice_users: nodes
                .as_ref()
                .map(|n| n.iter().map(|node| node.users).sum()),
            degraded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{
        FakeApi, FakeGateway, FakeRollups, FakeVoice, FixedClock, api_stats, fixed_now, unavailable,
    };
    use crate::application::ports::rollups::{ActiveUsersPoint, DimensionPoint};

    fn point(dimension: &str, value: f64) -> DimensionPoint {
        DimensionPoint {
            bucket: Granularity::Hour.bucket_start(fixed_now()),
            dimension: dimension.into(),
            value,
        }
    }

    fn overview_with(
        api: FakeApi,
        gateway: FakeGateway,
        rollups: FakeRollups,
        voice: FakeVoice,
    ) -> GetOverview {
        GetOverview::new(
            Arc::new(api),
            Arc::new(gateway),
            Arc::new(rollups),
            Arc::new(voice),
            Arc::new(FixedClock::default()),
        )
    }

    #[tokio::test]
    async fn combines_every_source() {
        let rollups = FakeRollups::default()
            .with_counter(
                CounterMetric::Messages,
                vec![
                    point("standard", 90.0),
                    point("ephemeral", 8.0),
                    point("p2p", 2.0),
                ],
            )
            .with_counter(CounterMetric::VoiceSeconds, vec![point("nl-home", 1200.0)]);
        *rollups.active.lock().unwrap() = vec![ActiveUsersPoint {
            day: fixed_now().date_naive(),
            dau: 33,
            mau: 410,
        }];
        let overview = overview_with(
            FakeApi::default(),
            FakeGateway::default(),
            rollups,
            FakeVoice::default(),
        )
        .execute()
        .await;

        assert_eq!(overview.api, Some(api_stats()));
        assert_eq!(overview.gateway.unwrap().sessions_online, 340);
        let messages = overview.messages_24h.unwrap();
        assert_eq!(
            (
                messages.standard,
                messages.ephemeral,
                messages.p2p,
                messages.total()
            ),
            (90, 8, 2, 100)
        );
        assert_eq!(overview.voice_minutes_24h, Some(20.0));
        assert_eq!((overview.dau, overview.mau), (Some(33), Some(410)));
        assert_eq!(
            (overview.voice_nodes, overview.voice_users),
            (Some(1), Some(12))
        );
        assert!(overview.degraded.is_empty());
    }

    #[tokio::test]
    async fn failing_sources_are_reported_as_degraded_without_failing_the_overview() {
        let api = FakeApi::default();
        *api.stats.lock().unwrap() = Err(unavailable());
        let gateway = FakeGateway::default();
        *gateway.stats.lock().unwrap() = Err(unavailable());
        let overview = overview_with(api, gateway, FakeRollups::default(), FakeVoice::default())
            .execute()
            .await;
        assert_eq!(overview.degraded, vec!["api", "gateway"]);
        assert_eq!(overview.api, None);
        assert_eq!(overview.messages_24h, Some(MessagesByMode::default()));
    }

    #[tokio::test]
    async fn no_activity_today_reads_as_zero_active_users() {
        let overview = overview_with(
            FakeApi::default(),
            FakeGateway::default(),
            FakeRollups::default(),
            FakeVoice::default(),
        )
        .execute()
        .await;
        assert_eq!((overview.dau, overview.mau), (Some(0), Some(0)));
    }
}
