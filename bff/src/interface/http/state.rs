use std::sync::Arc;

use metrics_exporter_prometheus::PrometheusHandle;

use crate::application::ports::api_admin::ApiAdmin;
use crate::application::ports::audit_log::AuditLog;
use crate::application::ports::clock::Clock;
use crate::application::ports::feature_flags::FeatureFlagRepository;
use crate::application::ports::gateway_admin::GatewayAdmin;
use crate::application::ports::notifier_admin::NotifierAdmin;
use crate::application::ports::platform_metrics::PlatformMetrics;
use crate::application::ports::rollups::RollupReader;
use crate::application::ports::voice_directory::VoiceDirectory;
use crate::application::use_cases::authenticate_admin::AuthenticateAdmin;
use crate::application::use_cases::delete_feature_flag::DeleteFeatureFlag;
use crate::application::use_cases::get_health_series::GetHealthSeries;
use crate::application::use_cases::get_overview::GetOverview;
use crate::application::use_cases::get_platform_health::GetPlatformHealth;
use crate::application::use_cases::get_usage_report::GetUsageReport;
use crate::application::use_cases::list_audit_log::ListAuditLog;
use crate::application::use_cases::list_feature_flags::ListFeatureFlags;
use crate::application::use_cases::list_reports::ListReports;
use crate::application::use_cases::list_voice_regions::ListVoiceRegions;
use crate::application::use_cases::resolve_report::ResolveReport;
use crate::application::use_cases::search_guilds::SearchGuilds;
use crate::application::use_cases::search_users::SearchUsers;
use crate::application::use_cases::send_manual_push::SendManualPush;
use crate::application::use_cases::set_voice_node_draining::SetVoiceNodeDraining;
use crate::application::use_cases::suspend_user::SuspendUser;
use crate::application::use_cases::upsert_feature_flag::UpsertFeatureFlag;

/// Outbound ports the HTTP interface needs, wired at the composition root.
#[derive(Clone)]
pub struct Ports {
    pub api: Arc<dyn ApiAdmin>,
    pub gateway: Arc<dyn GatewayAdmin>,
    pub notifier: Arc<dyn NotifierAdmin>,
    pub rollups: Arc<dyn RollupReader>,
    pub voice: Arc<dyn VoiceDirectory>,
    pub metrics: Arc<dyn PlatformMetrics>,
    pub flags: Arc<dyn FeatureFlagRepository>,
    pub audit: Arc<dyn AuditLog>,
    pub clock: Arc<dyn Clock>,
}

/// Use cases shared by all handlers.
#[derive(Clone)]
pub struct AppState {
    pub auth: Arc<AuthenticateAdmin>,
    pub overview: Arc<GetOverview>,
    pub usage: Arc<GetUsageReport>,
    pub voice_regions: Arc<ListVoiceRegions>,
    pub voice_draining: Arc<SetVoiceNodeDraining>,
    pub platform_health: Arc<GetPlatformHealth>,
    pub health_series: Arc<GetHealthSeries>,
    pub search_users: Arc<SearchUsers>,
    pub search_guilds: Arc<SearchGuilds>,
    pub suspend_user: Arc<SuspendUser>,
    pub send_push: Arc<SendManualPush>,
    pub list_reports: Arc<ListReports>,
    pub resolve_report: Arc<ResolveReport>,
    pub list_flags: Arc<ListFeatureFlags>,
    pub upsert_flag: Arc<UpsertFeatureFlag>,
    pub delete_flag: Arc<DeleteFeatureFlag>,
    pub audit_log: Arc<ListAuditLog>,
    pub readiness: Arc<dyn RollupReader>,
    pub prometheus: Option<PrometheusHandle>,
}

impl AppState {
    pub fn new(
        ports: Ports,
        auth: AuthenticateAdmin,
        prometheus: Option<PrometheusHandle>,
    ) -> Self {
        let Ports {
            api,
            gateway,
            notifier,
            rollups,
            voice,
            metrics,
            flags,
            audit,
            clock,
        } = ports;
        Self {
            auth: Arc::new(auth),
            overview: Arc::new(GetOverview::new(
                api.clone(),
                gateway.clone(),
                rollups.clone(),
                voice.clone(),
                clock.clone(),
            )),
            usage: Arc::new(GetUsageReport::new(rollups.clone(), clock.clone())),
            voice_regions: Arc::new(ListVoiceRegions::new(voice, rollups.clone(), clock.clone())),
            voice_draining: Arc::new(SetVoiceNodeDraining::new(gateway.clone(), audit.clone())),
            platform_health: Arc::new(GetPlatformHealth::new(metrics.clone(), gateway)),
            health_series: Arc::new(GetHealthSeries::new(metrics, clock.clone())),
            search_users: Arc::new(SearchUsers::new(api.clone())),
            search_guilds: Arc::new(SearchGuilds::new(api.clone())),
            list_reports: Arc::new(ListReports::new(api.clone())),
            resolve_report: Arc::new(ResolveReport::new(api.clone(), audit.clone())),
            suspend_user: Arc::new(SuspendUser::new(api, audit.clone(), clock)),
            send_push: Arc::new(SendManualPush::new(notifier, audit.clone())),
            list_flags: Arc::new(ListFeatureFlags::new(flags.clone())),
            upsert_flag: Arc::new(UpsertFeatureFlag::new(flags.clone(), audit.clone())),
            delete_flag: Arc::new(DeleteFeatureFlag::new(flags, audit.clone())),
            audit_log: Arc::new(ListAuditLog::new(audit)),
            readiness: rollups,
            prometheus,
        }
    }
}
