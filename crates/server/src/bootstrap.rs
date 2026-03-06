use std::sync::Arc;

use chrono::Utc;
use quotey_agent::{guardrails::GuardrailPolicy, runtime::AgentRuntime};
use quotey_core::config::{AppConfig, ConfigError, LoadOptions};
use quotey_core::suggestions::{SuggestionFeedback, SuggestionFeedbackEvent};
use quotey_db::repositories::{SqlSuggestionFeedbackRepository, SuggestionFeedbackRepository};
use quotey_db::{connect_with_settings, migrations, DbPool};
use quotey_slack::commands::NoopQuoteCommandService;
use quotey_slack::events::{
    BlockActionHandler, EventDispatcher, EventHandlerError, NoopBlockActionService,
    NoopReactionApprovalService, NoopThreadMessageService, ReactionAddedHandler,
    SlashCommandHandler, SuggestionFeedbackRecorder, SuggestionShownRecord,
    SuggestionShownRecorder, ThreadMessageHandler,
};
use quotey_slack::socket::{NoopSocketTransport, ReconnectPolicy, SocketModeRunner};
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

#[derive(Clone)]
struct DbSuggestionFeedbackRecorder {
    pool: DbPool,
}

#[async_trait::async_trait]
impl SuggestionFeedbackRecorder for DbSuggestionFeedbackRecorder {
    async fn record_feedback(
        &self,
        event: SuggestionFeedbackEvent,
    ) -> Result<(), EventHandlerError> {
        let repo = SqlSuggestionFeedbackRepository::new(self.pool.clone());
        match event {
            SuggestionFeedbackEvent::Added { request_id, product_id, .. } => {
                repo.record_added(&request_id, &product_id).await.map_err(|error| {
                    EventHandlerError::BlockAction(format!(
                        "failed to persist suggestion-added feedback: {error}"
                    ))
                })?;
            }
            SuggestionFeedbackEvent::Clicked { request_id, product_id } => {
                repo.record_clicked(&request_id, &product_id).await.map_err(|error| {
                    EventHandlerError::BlockAction(format!(
                        "failed to persist suggestion-clicked feedback: {error}"
                    ))
                })?;
            }
            SuggestionFeedbackEvent::Hidden { request_id, product_id } => {
                repo.record_hidden(&request_id, &product_id).await.map_err(|error| {
                    EventHandlerError::BlockAction(format!(
                        "failed to persist suggestion-hidden feedback: {error}"
                    ))
                })?;
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl SuggestionShownRecorder for DbSuggestionFeedbackRecorder {
    async fn record_shown(
        &self,
        records: Vec<SuggestionShownRecord>,
    ) -> Result<(), EventHandlerError> {
        if records.is_empty() {
            return Ok(());
        }

        let feedbacks = records
            .into_iter()
            .map(|record| SuggestionFeedback {
                id: format!("{}:{}", record.request_id, record.product_id),
                request_id: record.request_id,
                customer_id: record.customer_hint,
                product_id: record.product_id,
                product_sku: record.product_sku,
                score: record.score.unwrap_or(0.0),
                confidence: record.confidence.unwrap_or_else(|| "Unknown".to_owned()),
                category: record.category_description.unwrap_or_else(|| "Suggestion".to_owned()),
                quote_id: record.quote_id,
                suggested_at: Utc::now(),
                was_shown: true,
                was_clicked: false,
                was_added_to_quote: false,
                was_hidden: false,
                context: None,
            })
            .collect();

        let repo = SqlSuggestionFeedbackRepository::new(self.pool.clone());
        repo.record_shown(feedbacks).await.map_err(|error| {
            EventHandlerError::BlockAction(format!(
                "failed to persist suggestion-shown feedback: {error}"
            ))
        })
    }
}

/// Bootstrap with a pre-loaded config - avoids double config loading
pub async fn bootstrap_with_config(config: AppConfig) -> Result<Application, BootstrapError> {
    bootstrap_from_config(config).await
}

/// Bootstrap by loading config from options (legacy - loads config internally)
/// Kept for backward compatibility with tests
#[allow(dead_code)]
pub async fn bootstrap(options: LoadOptions) -> Result<Application, BootstrapError> {
    let config = AppConfig::load(options)?;
    bootstrap_from_config(config).await
}

async fn bootstrap_from_config(config: AppConfig) -> Result<Application, BootstrapError> {
    info!(
        event_name = "system.bootstrap.start",
        correlation_id = "bootstrap",
        quote_id = "unknown",
        thread_id = "unknown",
        "starting application bootstrap"
    );

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

    let feedback_recorder = DbSuggestionFeedbackRecorder { pool: db_pool.clone() };
    let dispatcher = build_slack_dispatcher(feedback_recorder);
    let slack_runner = SocketModeRunner::new(
        Arc::new(NoopSocketTransport),
        dispatcher,
        ReconnectPolicy::default(),
    );

    Ok(Application {
        config,
        db_pool,
        agent_runtime: AgentRuntime::new(GuardrailPolicy::default()),
        slack_runner,
    })
}

fn build_slack_dispatcher(feedback_recorder: DbSuggestionFeedbackRecorder) -> EventDispatcher {
    let mut dispatcher = EventDispatcher::new();
    dispatcher.register(SlashCommandHandler::with_shown_recorder(
        NoopQuoteCommandService,
        feedback_recorder.clone(),
    ));
    dispatcher.register(ThreadMessageHandler::new(NoopThreadMessageService::new()));
    dispatcher.register(ReactionAddedHandler::new(NoopReactionApprovalService));
    dispatcher.register(BlockActionHandler::new(NoopBlockActionService::with_feedback_recorder(
        feedback_recorder,
    )));
    dispatcher
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
        let now = Utc::now();
        Quote {
            id: QuoteId("Q-INT-0001".to_string()),
            version: 1,
            status: QuoteStatus::Draft,
            account_id: None,
            deal_id: None,
            currency: "USD".to_string(),
            term_months: None,
            start_date: None,
            end_date: None,
            valid_until: None,
            notes: None,
            created_by: "system".to_string(),
            lines: vec![QuoteLine {
                product_id: ProductId("plan-pro".to_string()),
                quantity: 2,
                unit_price: Decimal::new(25_000, 2),
                discount_pct: 0.0,
                notes: None,
            }],
            created_at: now,
            updated_at: now,
        }
    }
}
