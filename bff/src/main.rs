use std::process::ExitCode;

use admin_bff::config::Config;

#[tokio::main]
async fn main() -> ExitCode {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let _telemetry = admin_bff::telemetry::init(&config.log_level, config.otel_endpoint.as_deref());
    match admin_bff::bootstrap::run(config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "admin-bff stopped");
            ExitCode::FAILURE
        }
    }
}
