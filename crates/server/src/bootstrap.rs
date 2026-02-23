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
    use chrono::Utc;
    use quotey_core::config::{ConfigOverrides, LoadOptions};
    use quotey_core::{
        cpq::{policy::PolicyInput, DeterministicCpqRuntime},
        domain::{
            product::ProductId,
            quote::{Quote, QuoteId, QuoteLine, QuoteStatus},
        },
        flows::{
            engine::FlowEngine,
            states::{FlowAction, FlowContext, FlowEvent, FlowState},
        },
    };
    use rust_decimal::Decimal;

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

    #[tokio::test]
    async fn integration_smoke_covers_startup_data_path_and_quote_checkpoints() {
        let app = bootstrap(valid_overrides("sqlite::memory:?cache=shared"))
            .await
            .expect("bootstrap should succeed with valid overrides");

        let (table_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type = 'table' AND name IN ('quote', 'quote_line', 'flow_state', 'audit_event')",
        )
        .fetch_one(&app.db_pool)
        .await
        .expect("expected foundation tables to be available after bootstrap");
        assert_eq!(table_count, 4, "bootstrap should expose baseline quote-path tables");

        let flow_engine = FlowEngine::default();
        let quote = quote_fixture();
        let context = FlowContext::default();

        let validated = flow_engine
            .apply(&FlowState::Draft, &FlowEvent::RequiredFieldsCollected, &context)
            .expect("draft -> validated should succeed");
        assert_eq!(validated.to, FlowState::Validated);

        let priced = flow_engine
            .apply(&validated.to, &FlowEvent::PricingCalculated, &context)
            .expect("validated -> priced should succeed");
        assert_eq!(priced.to, FlowState::Priced);

        let cpq_evaluation = flow_engine.evaluate_cpq(
            &DeterministicCpqRuntime::default(),
            &quote,
            "USD",
            PolicyInput {
                requested_discount_pct: Decimal::new(1500, 2),
                deal_value: Decimal::new(250_000, 2),
                minimum_margin_pct: Decimal::new(4000, 2),
            },
        );
        assert!(cpq_evaluation.constraints.valid, "quote fixture should pass constraints");
        assert!(
            cpq_evaluation.pricing.total > Decimal::ZERO,
            "pricing checkpoint should produce positive total"
        );
        assert!(
            !cpq_evaluation.policy.approval_required,
            "policy checkpoint should allow auto-approval for baseline discount"
        );

        let finalized = flow_engine
            .apply(&priced.to, &FlowEvent::PolicyClear, &context)
            .expect("priced -> finalized should succeed");
        assert_eq!(finalized.to, FlowState::Finalized);
        assert!(
            finalized.actions.contains(&FlowAction::GenerateDeliveryArtifacts),
            "finalization should include delivery artifact generation"
        );

        let sent = flow_engine
            .apply(&finalized.to, &FlowEvent::QuoteDelivered, &context)
            .expect("finalized -> sent should succeed");
        assert_eq!(sent.to, FlowState::Sent);

        app.db_pool.close().await;
    }

    fn valid_overrides(database_url: &str) -> LoadOptions {
        LoadOptions {
            overrides: ConfigOverrides {
                database_url: Some(database_url.to_string()),
                slack_app_token: Some("xapp-test".to_string()),
                slack_bot_token: Some("xoxb-test".to_string()),
                ..ConfigOverrides::default()
            },
            ..LoadOptions::default()
        }
    }

    fn quote_fixture() -> Quote {
        Quote {
            id: QuoteId("Q-INT-0001".to_string()),
            status: QuoteStatus::Draft,
            lines: vec![QuoteLine {
                product_id: ProductId("plan-pro".to_string()),
                quantity: 2,
                unit_price: Decimal::new(25_000, 2),
            }],
            created_at: Utc::now(),
        }
    }
}
