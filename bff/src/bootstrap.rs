//! Composition root: wires adapters into ports and starts the HTTP server and background workers.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::application::ports::access_verifier::AccessTokenVerifier;
use crate::application::ports::clock::{Clock, SystemClock};
use crate::application::use_cases::authenticate_admin::AuthenticateAdmin;
use crate::application::use_cases::ingest_usage_events::IngestUsageEvents;
use crate::application::use_cases::record_platform_totals::RecordPlatformTotals;
use crate::config::Config;
use crate::infrastructure::bus::usage_consumer::UsageConsumer;
use crate::infrastructure::cloudflare::access_verifier::CloudflareAccessVerifier;
use crate::infrastructure::etcd::voice_directory::EtcdVoiceDirectory;
use crate::infrastructure::http::api_admin_client::ApiAdminClient;
use crate::infrastructure::http::gateway_admin_client::GatewayAdminClient;
use crate::infrastructure::http::internal_client::{InternalClient, http_client};
use crate::infrastructure::postgres::audit_log::PgAuditLog;
use crate::infrastructure::postgres::connect_and_migrate;
use crate::infrastructure::postgres::feature_flags::PgFeatureFlags;
use crate::infrastructure::postgres::rollup_store::PgRollupStore;
use crate::infrastructure::victoria::platform_metrics::VictoriaPlatformMetrics;
use crate::interface::http::router::router;
use crate::interface::http::state::{AppState, Ports};

/// Fixed by P1.
pub const LISTEN_ADDR: ([u8; 4], u16) = ([0, 0, 0, 0], 8090);
const TOTALS_SAMPLE_INTERVAL: Duration = Duration::from_secs(60);
const PRUNE_INTERVAL: Duration = Duration::from_secs(3600);

pub async fn run(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let prometheus = crate::telemetry::install_metrics_recorder();
    let pool = connect_and_migrate(&config.database_url).await?;
    let http = http_client();
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);

    let rollups = Arc::new(PgRollupStore::new(pool.clone()));
    let api = Arc::new(ApiAdminClient::new(InternalClient::new(
        http.clone(),
        &config.api_internal_url,
        &config.internal_service_token,
    )));
    let gateway = Arc::new(GatewayAdminClient::new(InternalClient::new(
        http.clone(),
        &config.gateway_internal_url,
        &config.internal_service_token,
    )));
    let ports = Ports {
        api: api.clone(),
        gateway,
        rollups: rollups.clone(),
        voice: Arc::new(EtcdVoiceDirectory::new(
            http.clone(),
            config.etcd_endpoints.clone(),
        )),
        metrics: Arc::new(VictoriaPlatformMetrics::new(
            http.clone(),
            &config.victoriametrics_url,
        )),
        flags: Arc::new(PgFeatureFlags::new(pool.clone())),
        audit: Arc::new(PgAuditLog::new(pool)),
        clock: clock.clone(),
    };

    let verifier: Option<Arc<dyn AccessTokenVerifier>> =
        config.cloudflare_access.as_ref().map(|cf| {
            Arc::new(CloudflareAccessVerifier::new(
                http.clone(),
                &cf.team_domain,
                &cf.audience,
            )) as Arc<dyn AccessTokenVerifier>
        });
    let auth = AuthenticateAdmin::new(
        verifier,
        config.allowlist.clone(),
        config.accepts_dev_header(),
    );
    if config.accepts_dev_header() {
        tracing::warn!(
            "DISCORDY_ENV=local: X-Dev-Admin-Email is accepted in place of Cloudflare Access"
        );
    }

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut workers = tokio::task::JoinSet::new();

    let ingest = Arc::new(IngestUsageEvents::new(rollups.clone()));
    workers.spawn(
        UsageConsumer::new(config.kafka_brokers.clone(), ingest, rollups.clone())
            .run(shutdown_rx.clone()),
    );

    let totals = Arc::new(RecordPlatformTotals::new(
        api,
        rollups.clone(),
        clock.clone(),
    ));
    workers.spawn(every(
        TOTALS_SAMPLE_INTERVAL,
        shutdown_rx.clone(),
        move || {
            let totals = totals.clone();
            async move {
                if let Err(error) = totals.execute().await {
                    tracing::warn!(%error, "sampling platform totals failed");
                }
            }
        },
    ));

    let pruner = rollups.clone();
    let prune_clock = clock.clone();
    workers.spawn(every(PRUNE_INTERVAL, shutdown_rx.clone(), move || {
        let pruner = pruner.clone();
        let now = prune_clock.now();
        async move {
            if let Err(error) = pruner.prune(now).await {
                tracing::warn!(%error, "pruning rollups failed");
            }
        }
    }));

    let app = router(AppState::new(ports, auth, prometheus));
    let address = SocketAddr::from(LISTEN_ADDR);
    let listener = TcpListener::bind(address).await?;
    tracing::info!(%address, "admin-bff listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            let _ = shutdown_tx.send(true);
        })
        .await?;

    while workers.join_next().await.is_some() {}
    Ok(())
}

async fn every<F, Fut>(period: Duration, mut shutdown: watch::Receiver<bool>, mut job: F)
where
    F: FnMut() -> Fut + Send,
    Fut: Future<Output = ()> + Send,
{
    let mut ticker = tokio::time::interval(period);
    loop {
        tokio::select! {
            _ = shutdown.changed() => return,
            _ = ticker.tick() => job().await,
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutting down");
}
