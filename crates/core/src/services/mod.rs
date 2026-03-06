//! Application services that orchestrate domain logic

pub mod integration_adapter;
pub mod outbox_service;

pub use integration_adapter::{
    AdapterError, AdapterPayload, AdapterRegistry, AdapterResult, IntegrationAdapter, NoopAdapter,
};
pub use outbox_service::{OutboxService, OutboxServiceError, OutboxServiceExt};
