//! JSON response/request shapes of `/admin-api`, mapped from use-case results.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::application::ports::api_admin::{ApiStats, GuildSummary, UserSummary};
use crate::application::ports::feature_flags::StoredFlag;
use crate::application::ports::gateway_admin::GatewayStats;
use crate::application::ports::platform_metrics::{LabelledSeries, LabelledValue};
use crate::application::use_cases::get_overview::Overview;
use crate::application::use_cases::get_platform_health::PlatformHealth;
use crate::application::use_cases::get_usage_report::UsageReport;
use crate::application::use_cases::list_voice_regions::VoiceRegionsReport;
use crate::domain::audit::AuditEntry;

#[derive(Serialize)]
pub struct MeDto {
    pub email: String,
}

#[derive(Serialize)]
pub struct MessagesDto {
    pub standard: i64,
    pub ephemeral: i64,
    pub p2p: i64,
    pub total: i64,
}

#[derive(Serialize)]
pub struct OverviewDto {
    pub generated_at: DateTime<Utc>,
    pub platform: Option<ApiStats>,
    pub gateway: Option<GatewayStats>,
    pub messages_24h: Option<MessagesDto>,
    pub voice_minutes_24h: Option<f64>,
    pub dau: Option<u64>,
    pub mau: Option<u64>,
    pub voice_nodes: Option<usize>,
    pub voice_users: Option<u64>,
    pub degraded: Vec<&'static str>,
}

impl From<Overview> for OverviewDto {
    fn from(o: Overview) -> Self {
        Self {
            generated_at: o.generated_at,
            platform: o.api,
            gateway: o.gateway,
            messages_24h: o.messages_24h.map(|m| MessagesDto {
                standard: m.standard,
                ephemeral: m.ephemeral,
                p2p: m.p2p,
                total: m.total(),
            }),
            voice_minutes_24h: o.voice_minutes_24h,
            dau: o.dau,
            mau: o.mau,
            voice_nodes: o.voice_nodes,
            voice_users: o.voice_users,
            degraded: o.degraded,
        }
    }
}

#[derive(Serialize)]
pub struct UsagePointDto {
    pub t: DateTime<Utc>,
    pub messages_standard: i64,
    pub messages_ephemeral: i64,
    pub messages_p2p: i64,
    pub voice_minutes: f64,
    pub sessions_peak: i64,
    pub active_calls_peak: i64,
    pub signups: i64,
}

#[derive(Serialize)]
pub struct ActiveUsersDto {
    pub day: NaiveDate,
    pub dau: u64,
    pub mau: u64,
}

#[derive(Serialize)]
pub struct UsageDto {
    pub range: &'static str,
    pub granularity: &'static str,
    pub points: Vec<UsagePointDto>,
    pub active_users: Vec<ActiveUsersDto>,
}

impl From<UsageReport> for UsageDto {
    fn from(r: UsageReport) -> Self {
        Self {
            range: r.range.as_str(),
            granularity: r.granularity.as_str(),
            points: r
                .points
                .into_iter()
                .map(|p| UsagePointDto {
                    t: p.bucket,
                    messages_standard: p.messages_standard,
                    messages_ephemeral: p.messages_ephemeral,
                    messages_p2p: p.messages_p2p,
                    voice_minutes: (p.voice_minutes * 10.0).round() / 10.0,
                    sessions_peak: p.sessions_peak,
                    active_calls_peak: p.active_calls_peak,
                    signups: p.signups,
                })
                .collect(),
            active_users: r
                .active_users
                .into_iter()
                .map(|a| ActiveUsersDto {
                    day: a.day,
                    dau: a.dau,
                    mau: a.mau,
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
pub struct QualityDto {
    pub samples: i64,
    pub jitter_ms: f64,
    pub loss_pct: f64,
    pub rtt_ms: f64,
    pub bitrate_kbps: f64,
}

#[derive(Serialize)]
pub struct VoiceNodeDto {
    pub node_id: String,
    pub ws_url: String,
    pub public_ip: String,
    pub udp_port: u16,
    pub users: u64,
    pub cpu_pct: f64,
    pub egress_bps: u64,
    pub draining: bool,
    pub version: String,
}

#[derive(Serialize)]
pub struct RegionDto {
    pub region: String,
    pub nodes: usize,
    pub accepting_nodes: usize,
    pub draining_nodes: usize,
    pub users: u64,
    pub egress_bps: u64,
    pub avg_cpu_pct: f64,
    pub max_cpu_pct: f64,
    pub quality: Option<QualityDto>,
    pub node_list: Vec<VoiceNodeDto>,
}

#[derive(Serialize)]
pub struct VoiceRegionsDto {
    pub etcd_healthy: bool,
    pub quality_window_minutes: i64,
    pub regions: Vec<RegionDto>,
}

impl From<VoiceRegionsReport> for VoiceRegionsDto {
    fn from(r: VoiceRegionsReport) -> Self {
        Self {
            etcd_healthy: r.etcd_healthy,
            quality_window_minutes: r.quality_window_minutes,
            regions: r
                .regions
                .into_iter()
                .map(|v| RegionDto {
                    region: v.load.region,
                    nodes: v.load.nodes,
                    accepting_nodes: v.load.accepting_nodes,
                    draining_nodes: v.load.draining_nodes,
                    users: v.load.users,
                    egress_bps: v.load.egress_bps,
                    avg_cpu_pct: v.load.avg_cpu_pct,
                    max_cpu_pct: v.load.max_cpu_pct,
                    quality: v.quality.map(|q| QualityDto {
                        samples: q.samples,
                        jitter_ms: q.jitter_ms,
                        loss_pct: q.loss_pct,
                        rtt_ms: q.rtt_ms,
                        bitrate_kbps: q.bitrate_kbps,
                    }),
                    node_list: v
                        .nodes
                        .into_iter()
                        .map(|n| VoiceNodeDto {
                            node_id: n.node_id,
                            ws_url: n.ws_url,
                            public_ip: n.public_ip,
                            udp_port: n.udp_port,
                            users: n.users,
                            cpu_pct: n.cpu_pct,
                            egress_bps: n.egress_bps,
                            draining: n.draining,
                            version: n.version,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
pub struct ServiceDto {
    pub service: String,
    pub status: &'static str,
    pub instances_up: u32,
    pub instances_total: u32,
    pub request_rate: Option<f64>,
    pub error_ratio: Option<f64>,
    pub latency_p95_ms: Option<f64>,
}

#[derive(Serialize)]
pub struct PlatformHealthDto {
    pub metrics_reachable: bool,
    pub services: Vec<ServiceDto>,
    pub consumer_lag: Vec<LabelledValue>,
    pub db_latency_p95_ms: Vec<LabelledValue>,
    pub gateway: Option<GatewayStats>,
}

impl From<PlatformHealth> for PlatformHealthDto {
    fn from(h: PlatformHealth) -> Self {
        Self {
            metrics_reachable: h.metrics_reachable,
            services: h
                .services
                .into_iter()
                .map(|s| ServiceDto {
                    status: s.status.as_str(),
                    service: s.service,
                    instances_up: s.instances_up,
                    instances_total: s.instances_total,
                    request_rate: s.request_rate,
                    error_ratio: s.error_ratio,
                    latency_p95_ms: s.latency_p95_ms,
                })
                .collect(),
            consumer_lag: h.consumer_lag,
            db_latency_p95_ms: h.db_latency_p95_ms,
            gateway: h.gateway,
        }
    }
}

#[derive(Serialize)]
pub struct HealthSeriesDto {
    pub panel: &'static str,
    pub range: &'static str,
    pub series: Vec<LabelledSeries>,
}

#[derive(Serialize)]
pub struct UserPageDto {
    pub users: Vec<UserSummary>,
    pub next_after: Option<String>,
}

#[derive(Serialize)]
pub struct GuildListDto {
    pub guilds: Vec<GuildSummary>,
}

#[derive(Deserialize)]
pub struct SuspendRequest {
    pub reason: String,
    #[serde(default)]
    pub until: Option<String>,
}

#[derive(Serialize)]
pub struct FlagDto {
    pub key: String,
    pub description: String,
    pub enabled: bool,
    pub rollout_percent: u8,
    pub updated_at: DateTime<Utc>,
    pub updated_by: String,
}

impl From<StoredFlag> for FlagDto {
    fn from(s: StoredFlag) -> Self {
        Self {
            key: s.flag.key.as_str().to_owned(),
            description: s.flag.description,
            enabled: s.flag.enabled,
            rollout_percent: s.flag.rollout_percent.value(),
            updated_at: s.updated_at,
            updated_by: s.updated_by,
        }
    }
}

#[derive(Serialize)]
pub struct FlagListDto {
    pub flags: Vec<FlagDto>,
}

#[derive(Deserialize)]
pub struct FlagRequest {
    #[serde(default)]
    pub description: String,
    pub enabled: bool,
    #[serde(default)]
    pub rollout_percent: i64,
}

#[derive(Serialize)]
pub struct AuditEntryDto {
    pub id: i64,
    pub at: DateTime<Utc>,
    pub actor_email: String,
    pub action: String,
    pub target: String,
    pub details: serde_json::Value,
}

impl From<AuditEntry> for AuditEntryDto {
    fn from(e: AuditEntry) -> Self {
        Self {
            id: e.id,
            at: e.at,
            actor_email: e.actor_email,
            action: e.action,
            target: e.target,
            details: serde_json::from_str(&e.details).unwrap_or(serde_json::Value::Null),
        }
    }
}

#[derive(Serialize)]
pub struct AuditPageDto {
    pub entries: Vec<AuditEntryDto>,
    pub next_before: Option<i64>,
}
