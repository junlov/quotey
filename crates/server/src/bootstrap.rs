use quotey_agent::{guardrails::GuardrailPolicy, runtime::AgentRuntime};
use quotey_core::config::{AppConfig, ConfigError, LoadOptions};
use quotey_db::{connect_with_settings, migrations, DbPool};
use quotey_slack::socket::SocketModeRunner;
use thiserror::Error;
use tracing::info;

pub struct Application {
    pub config: AppConfig,
    pub db_pool: DbPool,
    pub agent_runtime: AgentRuntime,
    pub slack_runner: SocketModeRunner,
}

#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("database connection failed: {0}")]
    DatabaseConnect(#[source] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migration(#[source] sqlx::migrate::MigrateError),
}

pub async fn bootstrap(options: LoadOptions) -> Result<Application, BootstrapError> {
    info!(
        event_name = "system.bootstrap.start",
        correlation_id = "bootstrap",
        quote_id = "unknown",
        thread_id = "unknown",
        "starting application bootstrap"
    );
    let config = AppConfig::load(options)?;

    let db_pool = connect_with_settings(
        &config.database.url,
        config.database.max_connections,
        config.database.timeout_secs,
    )
    .await
    .map_err(BootstrapError::DatabaseConnect)?;
    info!(
        event_name = "system.bootstrap.database_connected",
        correlation_id = "bootstrap",
        quote_id = "unknown",
        thread_id = "unknown",
        "database connection established"
    );

    migrations::run_pending(&db_pool).await.map_err(BootstrapError::Migration)?;
    info!(
        event_name = "system.bootstrap.migrations_applied",
        correlation_id = "bootstrap",
        quote_id = "unknown",
        thread_id = "unknown",
        "database migrations applied"
    );

    Ok(Application {
        config,
        db_pool,
        agent_runtime: AgentRuntime::new(GuardrailPolicy::default()),
        slack_runner: SocketModeRunner::default(),
    })
}

#[cfg(test)]
mod tests {
    use quotey_core::config::{ConfigOverrides, LoadOptions};

    use crate::bootstrap::bootstrap;

    #[tokio::test]
    async fn bootstrap_fails_fast_without_required_slack_tokens() {
        let result = bootstrap(LoadOptions {
            overrides: ConfigOverrides {
                database_url: Some("sqlite::memory:".to_string()),
                slack_app_token: Some("invalid-token".to_string()),
                slack_bot_token: Some("xoxb-valid".to_string()),
                ..ConfigOverrides::default()
            },
            ..LoadOptions::default()
        })
        .await;

        assert!(result.is_err());
        let message = result.err().expect("error").to_string();
        assert!(message.contains("slack.app_token"));
    }
}
