use std::collections::BTreeMap;
use std::sync::Arc;

use crate::application::ports::gateway_admin::{GatewayAdmin, GatewayStats};
use crate::application::ports::platform_metrics::{HealthPanel, LabelledValue, PlatformMetrics, TargetAvailability};
use crate::domain::service_health::{PLATFORM_SERVICES, ServiceStatus};

#[derive(Debug, Clone, PartialEq)]
pub struct ServiceHealthView {
    pub service: String,
    pub status: ServiceStatus,
    pub instances_up: u32,
    pub instances_total: u32,
    pub request_rate: Option<f64>,
    pub error_ratio: Option<f64>,
    pub latency_p95_ms: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlatformHealth {
    pub services: Vec<ServiceHealthView>,
    pub consumer_lag: Vec<LabelledValue>,
    pub db_latency_p95_ms: Vec<LabelledValue>,
    pub gateway: Option<GatewayStats>,
    pub metrics_reachable: bool,
}

/// Current health of every platform service from VictoriaMetrics plus live gateway counters.
pub struct GetPlatformHealth {
    metrics: Arc<dyn PlatformMetrics>,
    gateway: Arc<dyn GatewayAdmin>,
}

impl GetPlatformHealth {
    pub fn new(metrics: Arc<dyn PlatformMetrics>, gateway: Arc<dyn GatewayAdmin>) -> Self {
        Self { metrics, gateway }
    }

    pub async fn execute(&self) -> PlatformHealth {
        let (targets, rates, errors, latency, lag, db, gateway) = futures::join!(
            self.metrics.target_availability(),
            self.metrics.snapshot(HealthPanel::RequestRate),
            self.metrics.snapshot(HealthPanel::ErrorRatio),
            self.metrics.snapshot(HealthPanel::LatencyP95),
            self.metrics.snapshot(HealthPanel::ConsumerLag),
            self.metrics.snapshot(HealthPanel::DbLatencyP95),
            self.gateway.stats(),
        );
        let metrics_reachable = targets.is_ok();
        let targets = targets.unwrap_or_default();
        let rates = by_label(rates.unwrap_or_default());
        let errors = by_label(errors.unwrap_or_default());
        let latency = by_label(latency.unwrap_or_default());

        let services = service_names(&targets)
            .into_iter()
            .map(|service| {
                let (up, total) = targets.iter().find(|t| t.service == service).map_or((0, 0), |t| (t.up, t.total));
                let error_ratio = errors.get(&service).copied();
                ServiceHealthView {
                    status: ServiceStatus::classify(up, total, error_ratio),
                    instances_up: up,
                    instances_total: total,
                    request_rate: rates.get(&service).copied(),
                    error_ratio,
                    latency_p95_ms: latency.get(&service).copied(),
                    service,
                }
            })
            .collect();

        PlatformHealth {
            services,
            consumer_lag: lag.unwrap_or_default(),
            db_latency_p95_ms: db.unwrap_or_default(),
            gateway: gateway.ok(),
            metrics_reachable,
        }
    }
}

/// Known platform services first (in a stable order), then anything else being scraped.
fn service_names(targets: &[TargetAvailability]) -> Vec<String> {
    let mut names: Vec<String> = PLATFORM_SERVICES.iter().map(|s| (*s).to_owned()).collect();
    let mut extra: Vec<String> = targets.iter().map(|t| t.service.clone()).filter(|s| !names.contains(s)).collect();
    extra.sort();
    extra.dedup();
    names.extend(extra);
    names
}

fn by_label(values: Vec<LabelledValue>) -> BTreeMap<String, f64> {
    values.into_iter().map(|v| (v.label, v.value)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{FakeGateway, FakeMetrics, unavailable};

    fn target(service: &str, up: u32, total: u32) -> TargetAvailability {
        TargetAvailability { service: service.into(), up, total }
    }

    fn value(label: &str, value: f64) -> LabelledValue {
        LabelledValue { label: label.into(), value }
    }

    #[tokio::test]
    async fn classifies_each_service_from_targets_and_error_ratio() {
        let metrics = FakeMetrics::default();
        *metrics.availability.lock().unwrap() = Ok(vec![target("api", 2, 2), target("gateway", 1, 2), target("redpanda", 1, 1)]);
        metrics.snapshots.lock().unwrap().insert(HealthPanel::ErrorRatio, vec![value("api", 0.2)]);
        metrics.snapshots.lock().unwrap().insert(HealthPanel::LatencyP95, vec![value("api", 42.0)]);
        metrics.snapshots.lock().unwrap().insert(HealthPanel::ConsumerLag, vec![value("admin-bff", 12.0)]);
        let health = GetPlatformHealth::new(Arc::new(metrics), Arc::new(FakeGateway::default())).execute().await;

        let status = |name: &str| health.services.iter().find(|s| s.service == name).unwrap().status;
        assert_eq!(status("api"), ServiceStatus::Degraded);
        assert_eq!(status("gateway"), ServiceStatus::Degraded);
        assert_eq!(status("messages"), ServiceStatus::Unknown);
        assert_eq!(status("redpanda"), ServiceStatus::Healthy);
        assert_eq!(health.services.last().unwrap().service, "redpanda");
        assert_eq!(health.services[0].latency_p95_ms, Some(42.0));
        assert_eq!(health.consumer_lag, vec![value("admin-bff", 12.0)]);
        assert!(health.metrics_reachable);
        assert_eq!(health.gateway.unwrap().sessions_online, 340);
    }

    #[tokio::test]
    async fn unreachable_metrics_mark_every_service_unknown() {
        let metrics = FakeMetrics { reachable: false, ..FakeMetrics::default() };
        *metrics.availability.lock().unwrap() = Err(unavailable());
        let health = GetPlatformHealth::new(Arc::new(metrics), Arc::new(FakeGateway::default())).execute().await;
        assert!(!health.metrics_reachable);
        assert!(health.services.iter().all(|s| s.status == ServiceStatus::Unknown));
        assert_eq!(health.services.len(), PLATFORM_SERVICES.len());
    }
}
