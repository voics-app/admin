//! Structured JSON logs, optional OpenTelemetry traces, Prometheus recorder.

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

const SERVICE_NAME: &str = "admin-bff";

/// Keeps the tracer provider alive; flushes spans on drop.
pub struct TelemetryGuard {
    provider: Option<SdkTracerProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take()
            && let Err(error) = provider.shutdown()
        {
            eprintln!("failed to flush traces: {error}");
        }
    }
}

pub fn init(log_level: &str, otel_endpoint: Option<&str>) -> TelemetryGuard {
    let filter = EnvFilter::try_new(log_level).unwrap_or_else(|_| EnvFilter::new("info"));
    let json_logs = tracing_subscriber::fmt::layer()
        .json()
        .flatten_event(true)
        .with_current_span(false)
        .boxed();

    let provider = otel_endpoint.and_then(|endpoint| {
        let exporter = SpanExporter::builder()
            .with_http()
            .with_endpoint(format!("{}/v1/traces", endpoint.trim_end_matches('/')))
            .build();
        match exporter {
            Ok(exporter) => Some(
                SdkTracerProvider::builder()
                    .with_batch_exporter(exporter)
                    .with_resource(Resource::builder().with_service_name(SERVICE_NAME).build())
                    .build(),
            ),
            Err(error) => {
                eprintln!("OpenTelemetry disabled: {error}");
                None
            }
        }
    });
    let otel_layer = provider.as_ref().map(|p| {
        tracing_opentelemetry::layer()
            .with_tracer(p.tracer(SERVICE_NAME))
            .boxed()
    });

    tracing_subscriber::registry()
        .with(filter)
        .with(json_logs)
        .with(otel_layer)
        .init();
    TelemetryGuard { provider }
}

pub fn install_metrics_recorder() -> Option<PrometheusHandle> {
    match PrometheusBuilder::new().install_recorder() {
        Ok(handle) => Some(handle),
        Err(error) => {
            tracing::warn!(%error, "prometheus recorder unavailable");
            None
        }
    }
}
