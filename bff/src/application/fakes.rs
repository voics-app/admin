//! In-memory port implementations for use-case and handler tests.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};

use super::ports::access_verifier::{AccessTokenError, AccessTokenVerifier};
use super::ports::api_admin::{ApiAdmin, ApiStats, GuildSummary, Suspension, UserPage, UserQuery};
use super::ports::audit_log::AuditLog;
use super::ports::clock::Clock;
use super::ports::feature_flags::{FeatureFlagRepository, StoredFlag};
use super::ports::gateway_admin::{GatewayAdmin, GatewayStats};
use super::ports::platform_metrics::{
    HealthPanel, LabelledSeries, LabelledValue, PlatformMetrics, TargetAvailability,
};
use super::ports::rollups::{
    ActiveUsersPoint, DimensionPoint, RollupReader, RollupWriter, StreamCheckpoint,
};
use super::ports::voice_directory::VoiceDirectory;
use super::ports::{PortError, PortResult};
use crate::domain::audit::{AuditEntry, NewAuditEntry};
use crate::domain::feature_flag::{FeatureFlag, FlagKey};
use crate::domain::granularity::Granularity;
use crate::domain::metric::{CounterMetric, GaugeMetric};
use crate::domain::rollup::{QualityTotals, RollupDelta};
use crate::domain::voice_region::VoiceNode;

pub fn fixed_now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 16, 12, 30, 0).unwrap()
}

pub fn unavailable() -> PortError {
    PortError::Unavailable("down".into())
}

pub struct FixedClock(pub DateTime<Utc>);

impl Default for FixedClock {
    fn default() -> Self {
        Self(fixed_now())
    }
}

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

#[derive(Default)]
pub struct FakeRollups {
    pub applied: Mutex<Vec<(RollupDelta, Option<StreamCheckpoint>)>>,
    pub fail_writes: Mutex<bool>,
    pub counters: Mutex<HashMap<CounterMetric, Vec<DimensionPoint>>>,
    pub gauges: Mutex<HashMap<GaugeMetric, Vec<DimensionPoint>>>,
    pub active: Mutex<Vec<ActiveUsersPoint>>,
    pub quality: Mutex<Vec<(String, QualityTotals)>>,
    pub fail_reads: Mutex<bool>,
}

impl FakeRollups {
    pub fn with_counter(self, metric: CounterMetric, points: Vec<DimensionPoint>) -> Self {
        self.counters.lock().unwrap().insert(metric, points);
        self
    }

    pub fn with_gauge(self, metric: GaugeMetric, points: Vec<DimensionPoint>) -> Self {
        self.gauges.lock().unwrap().insert(metric, points);
        self
    }

    fn read_guard(&self) -> PortResult<()> {
        if *self.fail_reads.lock().unwrap() {
            Err(unavailable())
        } else {
            Ok(())
        }
    }
}

fn in_window(
    points: &[DimensionPoint],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Vec<DimensionPoint> {
    points
        .iter()
        .filter(|p| p.bucket >= from && p.bucket < to)
        .cloned()
        .collect()
}

#[async_trait]
impl RollupWriter for FakeRollups {
    async fn apply(
        &self,
        delta: &RollupDelta,
        checkpoint: Option<&StreamCheckpoint>,
    ) -> PortResult<()> {
        if *self.fail_writes.lock().unwrap() {
            return Err(unavailable());
        }
        self.applied
            .lock()
            .unwrap()
            .push((delta.clone(), checkpoint.cloned()));
        Ok(())
    }

    async fn checkpoint(&self, topic: &str, partition: i32) -> PortResult<Option<i64>> {
        let applied = self.applied.lock().unwrap();
        Ok(applied
            .iter()
            .filter_map(|(_, c)| c.as_ref())
            .filter(|c| c.topic == topic && c.partition == partition)
            .map(|c| c.next_offset)
            .next_back())
    }
}

#[async_trait]
impl RollupReader for FakeRollups {
    async fn counters(
        &self,
        metric: CounterMetric,
        _: Granularity,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> PortResult<Vec<DimensionPoint>> {
        self.read_guard()?;
        Ok(in_window(
            self.counters
                .lock()
                .unwrap()
                .get(&metric)
                .map(Vec::as_slice)
                .unwrap_or_default(),
            from,
            to,
        ))
    }

    async fn gauges(
        &self,
        metric: GaugeMetric,
        _: Granularity,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> PortResult<Vec<DimensionPoint>> {
        self.read_guard()?;
        Ok(in_window(
            self.gauges
                .lock()
                .unwrap()
                .get(&metric)
                .map(Vec::as_slice)
                .unwrap_or_default(),
            from,
            to,
        ))
    }

    async fn active_users(
        &self,
        from: NaiveDate,
        to: NaiveDate,
    ) -> PortResult<Vec<ActiveUsersPoint>> {
        self.read_guard()?;
        Ok(self
            .active
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.day >= from && p.day <= to)
            .copied()
            .collect())
    }

    async fn quality_by_region(
        &self,
        _: DateTime<Utc>,
        _: DateTime<Utc>,
    ) -> PortResult<Vec<(String, QualityTotals)>> {
        self.read_guard()?;
        Ok(self.quality.lock().unwrap().clone())
    }

    async fn is_reachable(&self) -> bool {
        !*self.fail_reads.lock().unwrap()
    }
}

pub fn api_stats() -> ApiStats {
    ApiStats {
        users_total: 1200,
        guilds_total: 80,
        channels_total: 640,
        bots_total: 4,
        signups_24h: 12,
    }
}

pub struct FakeApi {
    pub stats: Mutex<PortResult<ApiStats>>,
    pub users: Mutex<PortResult<UserPage>>,
    pub guilds: Mutex<PortResult<Vec<GuildSummary>>>,
    pub user_queries: Mutex<Vec<UserQuery>>,
    pub suspensions: Mutex<Vec<(String, Suspension)>>,
    pub suspend_result: Mutex<PortResult<()>>,
}

impl Default for FakeApi {
    fn default() -> Self {
        Self {
            stats: Mutex::new(Ok(api_stats())),
            users: Mutex::new(Ok(UserPage {
                users: vec![],
                next_after: None,
            })),
            guilds: Mutex::new(Ok(vec![])),
            user_queries: Mutex::new(vec![]),
            suspensions: Mutex::new(vec![]),
            suspend_result: Mutex::new(Ok(())),
        }
    }
}

#[async_trait]
impl ApiAdmin for FakeApi {
    async fn stats(&self) -> PortResult<ApiStats> {
        self.stats.lock().unwrap().clone()
    }

    async fn search_users(&self, query: &UserQuery) -> PortResult<UserPage> {
        self.user_queries.lock().unwrap().push(query.clone());
        self.users.lock().unwrap().clone()
    }

    async fn search_guilds(&self, _: &str) -> PortResult<Vec<GuildSummary>> {
        self.guilds.lock().unwrap().clone()
    }

    async fn suspend_user(&self, user_id: &str, suspension: &Suspension) -> PortResult<()> {
        let result = self.suspend_result.lock().unwrap().clone();
        if result.is_ok() {
            self.suspensions
                .lock()
                .unwrap()
                .push((user_id.to_owned(), suspension.clone()));
        }
        result
    }
}

pub fn gateway_stats() -> GatewayStats {
    GatewayStats {
        sessions_online: 340,
        sessions_by_node: BTreeMap::from([("gw-0".to_owned(), 200), ("gw-1".to_owned(), 140)]),
        guild_processes: 75,
        voice_allocations: 6,
    }
}

pub struct FakeGateway {
    pub stats: Mutex<PortResult<GatewayStats>>,
    pub drain_calls: Mutex<Vec<(String, bool)>>,
    pub drain_result: Mutex<PortResult<()>>,
}

impl Default for FakeGateway {
    fn default() -> Self {
        Self {
            stats: Mutex::new(Ok(gateway_stats())),
            drain_calls: Mutex::new(vec![]),
            drain_result: Mutex::new(Ok(())),
        }
    }
}

impl FakeGateway {
    fn record_drain(&self, node_id: &str, draining: bool) -> PortResult<()> {
        let result = self.drain_result.lock().unwrap().clone();
        if result.is_ok() {
            self.drain_calls
                .lock()
                .unwrap()
                .push((node_id.to_owned(), draining));
        }
        result
    }
}

#[async_trait]
impl GatewayAdmin for FakeGateway {
    async fn stats(&self) -> PortResult<GatewayStats> {
        self.stats.lock().unwrap().clone()
    }

    async fn drain_voice_node(&self, node_id: &str) -> PortResult<()> {
        self.record_drain(node_id, true)
    }

    async fn undrain_voice_node(&self, node_id: &str) -> PortResult<()> {
        self.record_drain(node_id, false)
    }
}

pub fn voice_node(id: &str, region: &str, users: u64) -> VoiceNode {
    VoiceNode {
        node_id: id.into(),
        region: region.into(),
        ws_url: format!("wss://discordy-rtc-{region}.voics.nl"),
        public_ip: "203.0.113.7".into(),
        udp_port: 50000,
        users,
        cpu_pct: 25.0,
        egress_bps: users * 64_000,
        draining: false,
        version: "0.1.0".into(),
    }
}

pub struct FakeVoice {
    pub nodes: Mutex<PortResult<Vec<VoiceNode>>>,
    pub healthy: bool,
}

impl Default for FakeVoice {
    fn default() -> Self {
        Self {
            nodes: Mutex::new(Ok(vec![voice_node("nl-1", "nl-home", 12)])),
            healthy: true,
        }
    }
}

#[async_trait]
impl VoiceDirectory for FakeVoice {
    async fn nodes(&self) -> PortResult<Vec<VoiceNode>> {
        self.nodes.lock().unwrap().clone()
    }

    async fn is_healthy(&self) -> bool {
        self.healthy
    }
}

pub type SeriesCall = (HealthPanel, DateTime<Utc>, DateTime<Utc>, u32);

pub struct FakeMetrics {
    pub availability: Mutex<PortResult<Vec<TargetAvailability>>>,
    pub snapshots: Mutex<HashMap<HealthPanel, Vec<LabelledValue>>>,
    pub series_calls: Mutex<Vec<SeriesCall>>,
    pub reachable: bool,
}

impl Default for FakeMetrics {
    fn default() -> Self {
        Self {
            availability: Mutex::new(Ok(vec![])),
            snapshots: Mutex::new(HashMap::new()),
            series_calls: Mutex::new(vec![]),
            reachable: true,
        }
    }
}

#[async_trait]
impl PlatformMetrics for FakeMetrics {
    async fn target_availability(&self) -> PortResult<Vec<TargetAvailability>> {
        self.availability.lock().unwrap().clone()
    }

    async fn snapshot(&self, panel: HealthPanel) -> PortResult<Vec<LabelledValue>> {
        if !self.reachable {
            return Err(unavailable());
        }
        Ok(self
            .snapshots
            .lock()
            .unwrap()
            .get(&panel)
            .cloned()
            .unwrap_or_default())
    }

    async fn series(
        &self,
        panel: HealthPanel,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        step: u32,
    ) -> PortResult<Vec<LabelledSeries>> {
        self.series_calls
            .lock()
            .unwrap()
            .push((panel, from, to, step));
        Ok(vec![LabelledSeries {
            label: "api".into(),
            points: vec![(from, 1.0)],
        }])
    }

    async fn is_reachable(&self) -> bool {
        self.reachable
    }
}

#[derive(Default)]
pub struct FakeFlags {
    pub flags: Mutex<BTreeMap<String, StoredFlag>>,
}

#[async_trait]
impl FeatureFlagRepository for FakeFlags {
    async fn list(&self) -> PortResult<Vec<StoredFlag>> {
        Ok(self.flags.lock().unwrap().values().cloned().collect())
    }

    async fn upsert(&self, flag: &FeatureFlag, updated_by: &str) -> PortResult<StoredFlag> {
        let stored = StoredFlag {
            flag: flag.clone(),
            updated_at: fixed_now(),
            updated_by: updated_by.to_owned(),
        };
        self.flags
            .lock()
            .unwrap()
            .insert(flag.key.as_str().to_owned(), stored.clone());
        Ok(stored)
    }

    async fn delete(&self, key: &FlagKey) -> PortResult<()> {
        self.flags
            .lock()
            .unwrap()
            .remove(key.as_str())
            .map(|_| ())
            .ok_or(PortError::NotFound)
    }
}

#[derive(Default)]
pub struct FakeAudit {
    pub entries: Mutex<Vec<NewAuditEntry>>,
}

#[async_trait]
impl AuditLog for FakeAudit {
    async fn record(&self, entry: &NewAuditEntry) -> PortResult<()> {
        self.entries.lock().unwrap().push(entry.clone());
        Ok(())
    }

    async fn list(&self, limit: u32, before_id: Option<i64>) -> PortResult<Vec<AuditEntry>> {
        let entries = self.entries.lock().unwrap();
        let mut stored: Vec<AuditEntry> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| AuditEntry {
                id: i as i64 + 1,
                at: fixed_now(),
                actor_email: e.actor_email.clone(),
                action: e.action.as_str().to_owned(),
                target: e.target.clone(),
                details: e.details.clone(),
            })
            .filter(|e| before_id.is_none_or(|b| e.id < b))
            .collect();
        stored.reverse();
        stored.truncate(limit as usize);
        Ok(stored)
    }
}

/// Accepts tokens of the form `valid:<email>`, reports `keys-down` as a key outage, rejects anything else.
pub struct FakeVerifier;

#[async_trait]
impl AccessTokenVerifier for FakeVerifier {
    async fn verify(&self, token: &str) -> Result<String, AccessTokenError> {
        if token == "keys-down" {
            return Err(AccessTokenError::KeysUnavailable(
                "jwks fetch failed".into(),
            ));
        }
        token
            .strip_prefix("valid:")
            .map(str::to_owned)
            .ok_or_else(|| AccessTokenError::Invalid("bad signature".into()))
    }
}
