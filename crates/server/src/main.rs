mod bootstrap;
mod health;
pub mod portal;

use anyhow::Result;
use quotey_core::config::{AppConfig, LoadOptions};

fn init_logging(config: &AppConfig) {
    use quotey_core::config::LogFormat::*;
    use tracing::Level;

    let log_level = config.logging.level.parse::<Level>().unwrap_or(Level::INFO);

    match config.logging.format {
        Compact => {
            tracing_subscriber::fmt().with_target(false).with_max_level(log_level).compact().init();
        }
        Pretty => {
            tracing_subscriber::fmt().with_target(false).with_max_level(log_level).pretty().init();
        }
        Json => {
            tracing_subscriber::fmt().with_target(false).with_max_level(log_level).json().init();
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    run().await
}

pub async fn run() -> Result<()> {
    // Load config and initialize logging before any other operations
    let config = AppConfig::load(LoadOptions::default())?;
    init_logging(&config);

    // Now bootstrap using the same config we already loaded
    let app = bootstrap::bootstrap_with_config(config).await?;

    health::spawn(
        &app.config.server.bind_address,
        app.config.server.health_check_port,
        app.db_pool.clone(),
    )
    .await?;

    tracing::info!(
        event_name = "system.server.slack_transport_mode",
        transport_mode = if app.slack_runner.is_noop_transport() { "noop" } else { "socket" },
        correlation_id = "bootstrap",
        quote_id = "unknown",
        thread_id = "unknown",
        "slack runner transport mode initialized"
    );

    let _ = &app.config;
    let _ = &app.db_pool;
    let _ = &app.agent_runtime;
    app.slack_runner.start().await?;

    tracing::info!(
        event_name = "system.server.started",
        correlation_id = "bootstrap",
        quote_id = "unknown",
        thread_id = "unknown",
        "quotey-server scaffold started"
    );
    wait_for_shutdown().await?;
    tracing::info!(
        event_name = "system.server.stopping",
        correlation_id = "shutdown",
        quote_id = "unknown",
        thread_id = "unknown",
        "quotey-server scaffold stopping"
    );

    Ok(())
}

async fn wait_for_shutdown() -> Result<()> {
    tokio::signal::ctrl_c().await?;
    Ok(())
}
