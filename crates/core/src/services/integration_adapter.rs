//! Pluggable integration adapter trait.
//!
//! Every concrete adapter (Salesforce CRM, builtin PDF, Slack notifications…)
//! implements `IntegrationAdapter` so the outbox worker can dispatch generically.

use async_trait::async_trait;

use crate::domain::integration::{AdapterTestResult, IntegrationConfig};

/// Errors returned by adapter implementations.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("adapter not configured: {0}")]
    NotConfigured(String),
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("operation failed: {0}")]
    OperationFailed(String),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
}

/// Payload for an adapter operation dispatched by the outbox worker.
#[derive(Debug, Clone)]
pub struct AdapterPayload {
    /// JSON-serialised operation-specific data.
    pub data_json: String,
    /// Idempotency key for dedup.
    pub idempotency_key: Option<String>,
}

/// Result of a successful adapter operation.
#[derive(Debug, Clone)]
pub struct AdapterResult {
    /// JSON-serialised result data returned by the adapter.
    pub result_json: String,
}

/// The pluggable adapter interface.
///
/// Implementations are registered in an `AdapterRegistry` and looked up at
/// dispatch time by `(integration_type, adapter_type)`.
#[async_trait]
pub trait IntegrationAdapter: Send + Sync {
    /// Human-readable adapter name for logs (e.g. "salesforce-crm").
    fn name(&self) -> &str;

    /// Send an outbound payload through this adapter.
    async fn send(
        &self,
        config: &IntegrationConfig,
        payload: &AdapterPayload,
    ) -> Result<AdapterResult, AdapterError>;

    /// Receive / poll for inbound data (pull-based adapters).
    /// Returns a list of JSON payloads.  Adapters that are push-only should
    /// return `Ok(vec![])`.
    async fn receive(
        &self,
        config: &IntegrationConfig,
        limit: usize,
    ) -> Result<Vec<String>, AdapterError>;

    /// Connectivity / health check.
    async fn test(&self, config: &IntegrationConfig) -> Result<AdapterTestResult, AdapterError>;
}

/// Registry that maps `(integration_type, adapter_type)` pairs to concrete
/// adapter implementations.
pub struct AdapterRegistry {
    adapters: std::collections::HashMap<(String, String), std::sync::Arc<dyn IntegrationAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self { adapters: std::collections::HashMap::new() }
    }

    /// Register a concrete adapter for a given `(integration_type, adapter_type)` pair.
    pub fn register(
        &mut self,
        integration_type: &str,
        adapter_type: &str,
        adapter: std::sync::Arc<dyn IntegrationAdapter>,
    ) {
        self.adapters.insert((integration_type.to_string(), adapter_type.to_string()), adapter);
    }

    /// Look up an adapter by its integration+adapter type pair.
    pub fn get(
        &self,
        integration_type: &str,
        adapter_type: &str,
    ) -> Option<&std::sync::Arc<dyn IntegrationAdapter>> {
        self.adapters.get(&(integration_type.to_string(), adapter_type.to_string()))
    }

    /// List all registered `(integration_type, adapter_type)` pairs.
    pub fn registered_pairs(&self) -> Vec<(String, String)> {
        let mut pairs: Vec<_> = self.adapters.keys().cloned().collect();
        pairs.sort();
        pairs
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Builtin no-op adapter (used for "none" adapter_type and testing)
// ---------------------------------------------------------------------------

pub struct NoopAdapter;

#[async_trait]
impl IntegrationAdapter for NoopAdapter {
    fn name(&self) -> &str {
        "noop"
    }

    async fn send(
        &self,
        _config: &IntegrationConfig,
        _payload: &AdapterPayload,
    ) -> Result<AdapterResult, AdapterError> {
        Ok(AdapterResult { result_json: r#"{"status":"noop"}"#.to_string() })
    }

    async fn receive(
        &self,
        _config: &IntegrationConfig,
        _limit: usize,
    ) -> Result<Vec<String>, AdapterError> {
        Ok(vec![])
    }

    async fn test(&self, _config: &IntegrationConfig) -> Result<AdapterTestResult, AdapterError> {
        Ok(AdapterTestResult {
            ok: true,
            latency_ms: 0,
            message: "noop adapter always healthy".to_string(),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::domain::integration::{
        AdapterStatus, AdapterType, IntegrationConfig, IntegrationType,
    };

    fn test_config() -> IntegrationConfig {
        IntegrationConfig {
            id: "INT-0001".to_string(),
            integration_type: IntegrationType::Notification,
            adapter_type: AdapterType::None,
            name: "test noop".to_string(),
            adapter_config: "{}".to_string(),
            status: AdapterStatus::Active,
            status_message: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn noop_adapter_send() {
        let adapter = NoopAdapter;
        let cfg = test_config();
        let payload = AdapterPayload { data_json: "{}".to_string(), idempotency_key: None };
        let result = adapter.send(&cfg, &payload).await.unwrap();
        assert!(result.result_json.contains("noop"));
    }

    #[tokio::test]
    async fn noop_adapter_receive() {
        let adapter = NoopAdapter;
        let cfg = test_config();
        let received = adapter.receive(&cfg, 10).await.unwrap();
        assert!(received.is_empty());
    }

    #[tokio::test]
    async fn noop_adapter_test() {
        let adapter = NoopAdapter;
        let cfg = test_config();
        let result = adapter.test(&cfg).await.unwrap();
        assert!(result.ok);
    }

    #[tokio::test]
    async fn registry_lookup() {
        let mut registry = AdapterRegistry::new();
        registry.register("notification", "none", std::sync::Arc::new(NoopAdapter));

        assert!(registry.get("notification", "none").is_some());
        assert!(registry.get("crm", "salesforce").is_none());
    }

    #[tokio::test]
    async fn registry_registered_pairs() {
        let mut registry = AdapterRegistry::new();
        registry.register("notification", "none", std::sync::Arc::new(NoopAdapter));
        registry.register("pdf", "builtin", std::sync::Arc::new(NoopAdapter));

        let pairs = registry.registered_pairs();
        assert_eq!(pairs.len(), 2);
        assert!(pairs.contains(&("notification".to_string(), "none".to_string())));
        assert!(pairs.contains(&("pdf".to_string(), "builtin".to_string())));
    }
}
