//! Application services that orchestrate domain logic

pub mod outbox_service;

pub use outbox_service::{OutboxService, OutboxServiceError, OutboxServiceExt};
