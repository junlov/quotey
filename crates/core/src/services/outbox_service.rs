//! Outbox service trait for guaranteed side-effect delivery
//!
//! This service extends the execution queue to provide:
//! - Type-safe enqueueing of side effects
//! - Status tracking and querying
//! - Manual replay capabilities

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::domain::execution::ExecutionTaskId;
use crate::domain::outbox::{
    DeadLetterEntry, OutboxOperation, OutboxState, OutboxStatus, ReplayRequest, ReplayResult,
};
use crate::domain::quote::QuoteId;

/// Errors that can occur in outbox operations
#[derive(Error, Debug)]
pub enum OutboxServiceError {
    #[error("Repository error: {0}")]
    Repository(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Operation not found: {0}")]
    NotFound(String),

    #[error("Duplicate operation: existing task {0}")]
    Duplicate(String),

    #[error("Invalid state transition from {from} to {to}")]
    InvalidStateTransition { from: OutboxState, to: OutboxState },

    #[error("Max retries exceeded for task {0}")]
    MaxRetriesExceeded(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),
}

/// Result type for outbox operations
pub type Result<T> = std::result::Result<T, OutboxServiceError>;

/// Core outbox service trait
#[async_trait]
pub trait OutboxService: Send + Sync {
    /// Enqueue a side effect for guaranteed delivery
    ///
    /// Returns the newly created task ID.
    ///
    /// Implementations should return `OutboxServiceError::Duplicate(existing_task_id)`
    /// when an identical operation already exists.
    async fn enqueue(
        &self,
        quote_id: &QuoteId,
        operation: OutboxOperation,
    ) -> Result<ExecutionTaskId>;

    /// Get the current status of an operation
    async fn get_status(&self, task_id: &ExecutionTaskId) -> Result<Option<OutboxStatus>>;

    /// List pending operations for a quote
    async fn list_pending(&self, quote_id: &QuoteId) -> Result<Vec<OutboxStatus>>;

    /// List all pending operations (for worker processing)
    async fn claim_pending(
        &self,
        limit: usize,
        worker_id: &str,
    ) -> Result<Vec<(ExecutionTaskId, OutboxOperation)>>;

    /// Mark an operation as completed successfully
    async fn complete(
        &self,
        task_id: &ExecutionTaskId,
        result_json: Option<String>,
    ) -> Result<()>;

    /// Mark an operation as failed (may trigger retry or dead letter)
    async fn fail(&self, task_id: &ExecutionTaskId, error: &OutboxServiceError) -> Result<()>;

    /// List failed operations awaiting manual intervention
    async fn list_failed(&self, limit: usize) -> Result<Vec<DeadLetterEntry>>;

    /// Manually replay a failed operation
    async fn replay(&self, request: ReplayRequest) -> Result<ReplayResult>;

    /// Abandon a failed operation (manual override)
    async fn abandon(
        &self,
        dead_letter_id: &str,
        reason: &str,
        abandoned_by: &str,
    ) -> Result<()>;

    /// Get statistics for monitoring
    async fn get_stats(&self) -> Result<OutboxStats>;
}

/// Statistics for outbox monitoring
#[derive(Clone, Debug, Default)]
pub struct OutboxStats {
    pub pending_count: i64,
    pub claimed_count: i64,
    pub completed_count: i64,
    pub failed_count: i64,
    pub dead_letter_count: i64,
    pub oldest_pending_age_secs: Option<i64>,
}

/// Configuration for the outbox service
#[derive(Clone, Debug)]
pub struct OutboxConfig {
    /// How long a worker can hold a claim before auto-release
    pub claim_timeout_secs: i64,
    /// Batch size for worker polling
    pub poll_batch_size: usize,
    /// Enable automatic retry processing
    pub auto_retry_enabled: bool,
}

impl Default for OutboxConfig {
    fn default() -> Self {
        Self {
            claim_timeout_secs: 300, // 5 minutes
            poll_batch_size: 10,
            auto_retry_enabled: true,
        }
    }
}

/// Request to enqueue multiple operations atomically
#[derive(Clone, Debug)]
pub struct BatchEnqueueRequest {
    pub quote_id: QuoteId,
    pub operations: Vec<OutboxOperation>,
}

/// Result of a batch enqueue operation
#[derive(Clone, Debug)]
pub struct BatchEnqueueResult {
    pub task_ids: Vec<ExecutionTaskId>,
    pub duplicates: Vec<(OutboxOperation, ExecutionTaskId)>,
}

/// Extension trait for batch operations
#[async_trait]
pub trait OutboxServiceExt: OutboxService {
    /// Enqueue multiple operations atomically
    async fn enqueue_batch(&self, request: BatchEnqueueRequest) -> Result<BatchEnqueueResult>;

    /// Cancel all pending operations for a quote
    async fn cancel_pending(&self, quote_id: &QuoteId, reason: &str) -> Result<usize>;

    /// Get timeline of all operations for a quote
    async fn get_quote_timeline(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Vec<(DateTime<Utc>, String, OutboxState, Option<String>)>>;
}

// blanket implementation for any type that implements OutboxService
#[async_trait]
impl<T: OutboxService + ?Sized> OutboxServiceExt for T {
    async fn enqueue_batch(&self, request: BatchEnqueueRequest) -> Result<BatchEnqueueResult> {
        let mut task_ids = Vec::new();
        let mut duplicates = Vec::new();

        for op in request.operations {
            match self.enqueue(&request.quote_id, op.clone()).await {
                Ok(task_id) => task_ids.push(task_id),
                Err(OutboxServiceError::Duplicate(existing_task_id)) => {
                    duplicates.push((op, ExecutionTaskId(existing_task_id)));
                }
                Err(e) => return Err(e),
            }
        }

        Ok(BatchEnqueueResult {
            task_ids,
            duplicates,
        })
    }

    async fn cancel_pending(&self, quote_id: &QuoteId, reason: &str) -> Result<usize> {
        let pending = self.list_pending(quote_id).await?;
        let mut cancelled = 0;

        for status in pending {
            if status.state == OutboxState::Pending {
                // Mark as failed with cancellation reason
                self.fail(
                    &status.task_id,
                    &OutboxServiceError::Repository(format!("Cancelled: {reason}")),
                )
                .await?;
                cancelled += 1;
            }
        }

        Ok(cancelled)
    }

    async fn get_quote_timeline(
        &self,
        _quote_id: &QuoteId,
    ) -> Result<Vec<(DateTime<Utc>, String, OutboxState, Option<String>)>> {
        // This would need a proper repository method
        // For now, return empty - implementation would fetch from audit log
        Ok(Vec::new())
    }
}
