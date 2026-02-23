mod bootstrap;
mod health;

use anyhow::Result;
use quotey_core::config::{ConfigOverrides, LoadOptions};

#[tokio::main]
async fn main() -> Result<()> {
    run().await
}

pub async fn run() -> Result<()> {
    tracing_subscriber::fmt().with_target(false).compact().init();

    let app = bootstrap::bootstrap(LoadOptions {
        overrides: ConfigOverrides {
            database_url: Some("sqlite://quotey.db".to_string()),
            ..ConfigOverrides::default()
        },
        ..LoadOptions::default()
    })
    .await?;

    health::spawn(
        &app.config.server.bind_address,
        app.config.server.health_check_port,
        app.db_pool.clone(),
    )
    .await?;

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
