//! Deterministic Execution Queue Engine
//!
//! Provides deterministic state machine logic for the execution queue,
//! ensuring all transitions are auditable, idempotent, and recoverable.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::execution::{
    ExecutionTask, ExecutionTaskId, ExecutionTaskState, ExecutionTransitionEvent,
    ExecutionTransitionId, IdempotencyRecord, IdempotencyRecordState, OperationKey,
};
use crate::domain::quote::QuoteId;

/// Configuration for the execution engine
#[derive(Clone, Debug)]
pub struct ExecutionEngineConfig {
    /// How long before a claimed task is considered stale
    pub claim_timeout_seconds: i64,
    /// Default max retries for retryable failures
    pub default_max_retries: u32,
    /// Backoff multiplier for retries
    pub retry_backoff_multiplier: u32,
    /// Base delay in seconds between retries
    pub retry_base_delay_seconds: i64,
}

impl Default for ExecutionEngineConfig {
    fn default() -> Self {
        Self {
            claim_timeout_seconds: 300, // 5 minutes
            default_max_retries: 3,
            retry_backoff_multiplier: 2,
            retry_base_delay_seconds: 5,
        }
    }
}

/// Errors that can occur during execution processing
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ExecutionError {
    #[error("invalid state transition from {from:?} to {to:?}: {reason}")]
    InvalidTransition { from: ExecutionTaskState, to: ExecutionTaskState, reason: String },
    #[error("task not found: {0}")]
    TaskNotFound(ExecutionTaskId),
    #[error("idempotency conflict: operation {0} already in state {1}")]
    IdempotencyConflict(OperationKey, String),
    #[error("claim conflict: task {0} already claimed by {1}")]
    ClaimConflict(ExecutionTaskId, String),
    #[error("task not yet available: {0}")]
    TaskNotYetAvailable(ExecutionTaskId),
}

/// Result of claiming a task for execution
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimResult {
    pub task: ExecutionTask,
    pub transition: ExecutionTransitionEvent,
}

/// Result of completing or failing a task
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionResult {
    pub task: ExecutionTask,
    pub transition: ExecutionTransitionEvent,
}

/// Policy for handling failures
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RetryPolicy {
    /// Retry with exponential backoff
    Retry,
    /// Mark as failed terminal, no more retries
    FailTerminal,
}

/// Deterministic execution engine
///
/// This engine provides the core state machine logic for the execution queue,
/// ensuring all transitions follow deterministic rules and maintaining auditability.
#[derive(Clone, Debug)]
pub struct DeterministicExecutionEngine {
    config: ExecutionEngineConfig,
}

impl DeterministicExecutionEngine {
    /// Create a new engine with default configuration
    pub fn new() -> Self {
        Self::with_config(ExecutionEngineConfig::default())
    }

    /// Create a new engine with custom configuration
    pub fn with_config(config: ExecutionEngineConfig) -> Self {
        Self { config }
    }

    /// Create a new task for execution
    ///
    /// This is the entry point for enqueueing work. It creates the initial
    /// task state and idempotency record.
    pub fn create_task(
        &self,
        quote_id: QuoteId,
        operation_kind: impl Into<String>,
        payload_json: impl Into<String>,
        idempotency_key: OperationKey,
        correlation_id: impl Into<String>,
    ) -> (ExecutionTask, IdempotencyRecord) {
        let now = Utc::now();
        let task_id = ExecutionTaskId(Uuid::new_v4().to_string());
        let operation_kind = operation_kind.into();

        let task = ExecutionTask {
            id: task_id.clone(),
            quote_id: quote_id.clone(),
            operation_kind: operation_kind.clone(),
            payload_json: payload_json.into(),
            idempotency_key: idempotency_key.clone(),
            state: ExecutionTaskState::Queued,
            retry_count: 0,
            max_retries: self.config.default_max_retries,
            available_at: now,
            claimed_by: None,
            claimed_at: None,
            last_error: None,
            result_fingerprint: None,
            state_version: 1,
            created_at: now,
            updated_at: now,
        };

        let idempotency_record = IdempotencyRecord {
            operation_key: idempotency_key,
            quote_id,
            operation_kind,
            payload_hash: Self::hash_payload(&task.payload_json),
            state: IdempotencyRecordState::Reserved,
            attempt_count: 1,
            first_seen_at: now,
            last_seen_at: now,
            result_snapshot_json: None,
            error_snapshot_json: None,
            expires_at: None,
            correlation_id: correlation_id.into(),
            created_by_component: "execution_engine".to_string(),
            updated_by_component: "execution_engine".to_string(),
        };

        (task, idempotency_record)
    }

    /// Claim a task for execution
    ///
    /// Transitions task from Queued|RetryableFailed -> Running.
    /// Updates idempotency record to Running state.
    pub fn claim_task(
        &self,
        mut task: ExecutionTask,
        worker_id: impl Into<String>,
        idempotency_record: &mut IdempotencyRecord,
    ) -> Result<ClaimResult, ExecutionError> {
        let worker_id = worker_id.into();
        let now = Utc::now();

        // Validate current state allows claiming
        match &task.state {
            ExecutionTaskState::Queued | ExecutionTaskState::RetryableFailed => {
                // Valid states for claiming
            }
            ExecutionTaskState::Running => {
                // Check if current claim is stale
                if let Some(claimed_at) = task.claimed_at {
                    let stale_threshold =
                        claimed_at + Duration::seconds(self.config.claim_timeout_seconds);
                    if now < stale_threshold {
                        return Err(ExecutionError::ClaimConflict(
                            task.id.clone(),
                            task.claimed_by.clone().unwrap_or_default(),
                        ));
                    }
                    // Claim is stale, allow stealing
                }
            }
            ExecutionTaskState::Completed | ExecutionTaskState::FailedTerminal => {
                return Err(ExecutionError::InvalidTransition {
                    from: task.state.clone(),
                    to: ExecutionTaskState::Running,
                    reason: "task already in terminal state".to_string(),
                });
            }
        }

        // Check if task is available (for retry backoff)
        if now < task.available_at {
            return Err(ExecutionError::TaskNotYetAvailable(task.id.clone()));
        }

        let from_state = task.state.clone();

        // Update task
        task.state = ExecutionTaskState::Running;
        task.claimed_by = Some(worker_id);
        task.claimed_at = Some(now);
        task.state_version += 1;
        task.updated_at = now;

        // Update idempotency record
        idempotency_record.state = IdempotencyRecordState::Running;
        idempotency_record.last_seen_at = now;
        idempotency_record.updated_by_component = "execution_engine".to_string();

        // Create transition event
        let transition = ExecutionTransitionEvent {
            id: ExecutionTransitionId(Uuid::new_v4().to_string()),
            task_id: task.id.clone(),
            quote_id: task.quote_id.clone(),
            from_state: Some(from_state),
            to_state: ExecutionTaskState::Running,
            transition_reason: "task_claimed".to_string(),
            error_class: None,
            decision_context_json: serde_json::json!({
                "worker_id": task.claimed_by,
                "claim_timeout_seconds": self.config.claim_timeout_seconds,
            })
            .to_string(),
            actor_type: "worker".to_string(),
            actor_id: task.claimed_by.clone().unwrap_or_default(),
            idempotency_key: Some(task.idempotency_key.clone()),
            correlation_id: idempotency_record.correlation_id.clone(),
            state_version: task.state_version,
            occurred_at: now,
        };

        Ok(ClaimResult { task, transition })
    }

    /// Complete a task successfully
    ///
    /// Transitions task from Running -> Completed.
    pub fn complete_task(
        &self,
        mut task: ExecutionTask,
        result_fingerprint: impl Into<String>,
        idempotency_record: &mut IdempotencyRecord,
    ) -> Result<TransitionResult, ExecutionError> {
        self.validate_transition(&task, &ExecutionTaskState::Completed)?;

        let now = Utc::now();
        let from_state = task.state.clone();

        // Update task
        task.state = ExecutionTaskState::Completed;
        task.result_fingerprint = Some(result_fingerprint.into());
        task.state_version += 1;
        task.updated_at = now;
        task.claimed_by = None;
        task.claimed_at = None;

        // Update idempotency record
        idempotency_record.state = IdempotencyRecordState::Completed;
        idempotency_record.last_seen_at = now;
        idempotency_record.result_snapshot_json = task.result_fingerprint.clone();
        idempotency_record.updated_by_component = "execution_engine".to_string();

        // Create transition event
        let transition = ExecutionTransitionEvent {
            id: ExecutionTransitionId(Uuid::new_v4().to_string()),
            task_id: task.id.clone(),
            quote_id: task.quote_id.clone(),
            from_state: Some(from_state),
            to_state: ExecutionTaskState::Completed,
            transition_reason: "task_completed".to_string(),
            error_class: None,
            decision_context_json: serde_json::json!({
                "result_fingerprint": task.result_fingerprint,
            })
            .to_string(),
            actor_type: "worker".to_string(),
            actor_id: "system".to_string(),
            idempotency_key: Some(task.idempotency_key.clone()),
            correlation_id: idempotency_record.correlation_id.clone(),
            state_version: task.state_version,
            occurred_at: now,
        };

        Ok(TransitionResult { task, transition })
    }

    /// Mark a task as failed
    ///
    /// Depending on retry policy and count, transitions to either:
    /// - RetryableFailed (will be retried)
    /// - FailedTerminal (no more retries)
    pub fn fail_task(
        &self,
        mut task: ExecutionTask,
        error: impl Into<String>,
        error_class: impl Into<String>,
        retry_policy: RetryPolicy,
        idempotency_record: &mut IdempotencyRecord,
    ) -> Result<TransitionResult, ExecutionError> {
        let now = Utc::now();
        let error = error.into();
        let error_class = error_class.into();

        self.validate_transition(&task, &ExecutionTaskState::RetryableFailed)?;

        let from_state = task.state.clone();

        // Determine if we should retry
        let should_retry =
            matches!(retry_policy, RetryPolicy::Retry) && task.retry_count < task.max_retries;

        if should_retry {
            // Calculate next available time with exponential backoff
            let backoff_seconds = self.config.retry_base_delay_seconds
                * (self.config.retry_backoff_multiplier.pow(task.retry_count) as i64);
            let available_at = now + Duration::seconds(backoff_seconds);

            task.state = ExecutionTaskState::RetryableFailed;
            task.retry_count += 1;
            task.available_at = available_at;
            task.last_error = Some(error.clone());
            task.state_version += 1;
            task.updated_at = now;
            task.claimed_by = None;
            task.claimed_at = None;

            idempotency_record.state = IdempotencyRecordState::FailedRetryable;
            idempotency_record.last_seen_at = now;
            idempotency_record.error_snapshot_json = Some(error.clone());
            idempotency_record.updated_by_component = "execution_engine".to_string();

            let transition = ExecutionTransitionEvent {
                id: ExecutionTransitionId(Uuid::new_v4().to_string()),
                task_id: task.id.clone(),
                quote_id: task.quote_id.clone(),
                from_state: Some(from_state),
                to_state: ExecutionTaskState::RetryableFailed,
                transition_reason: "task_failed_retryable".to_string(),
                error_class: Some(error_class.clone()),
                decision_context_json: serde_json::json!({
                    "retry_count": task.retry_count,
                    "max_retries": task.max_retries,
                    "next_available_at": available_at,
                    "error": error,
                    "error_class": error_class,
                })
                .to_string(),
                actor_type: "worker".to_string(),
                actor_id: "system".to_string(),
                idempotency_key: Some(task.idempotency_key.clone()),
                correlation_id: idempotency_record.correlation_id.clone(),
                state_version: task.state_version,
                occurred_at: now,
            };

            Ok(TransitionResult { task, transition })
        } else {
            // Terminal failure
            task.state = ExecutionTaskState::FailedTerminal;
            task.last_error = Some(error.clone());
            task.state_version += 1;
            task.updated_at = now;
            task.claimed_by = None;
            task.claimed_at = None;

            idempotency_record.state = IdempotencyRecordState::FailedTerminal;
            idempotency_record.last_seen_at = now;
            idempotency_record.error_snapshot_json = Some(error.clone());
            idempotency_record.updated_by_component = "execution_engine".to_string();

            let transition = ExecutionTransitionEvent {
                id: ExecutionTransitionId(Uuid::new_v4().to_string()),
                task_id: task.id.clone(),
                quote_id: task.quote_id.clone(),
                from_state: Some(from_state),
                to_state: ExecutionTaskState::FailedTerminal,
                transition_reason: "task_failed_terminal".to_string(),
                error_class: Some(error_class.clone()),
                decision_context_json: serde_json::json!({
                    "retry_count": task.retry_count,
                    "max_retries": task.max_retries,
                    "error": error,
                    "error_class": error_class,
                    "reason": "max_retries_exceeded_or_policy",
                })
                .to_string(),
                actor_type: "worker".to_string(),
                actor_id: "system".to_string(),
                idempotency_key: Some(task.idempotency_key.clone()),
                correlation_id: idempotency_record.correlation_id.clone(),
                state_version: task.state_version,
                occurred_at: now,
            };

            Ok(TransitionResult { task, transition })
        }
    }

    /// Recover stale tasks that have been claimed for too long
    ///
    /// Returns tasks that should be made available for reprocessing.
    pub fn recover_stale_tasks(
        &self,
        tasks: Vec<ExecutionTask>,
        reference_time: DateTime<Utc>,
    ) -> Vec<ExecutionTask> {
        let stale_threshold = reference_time - Duration::seconds(self.config.claim_timeout_seconds);

        tasks
            .into_iter()
            .filter(|task| {
                matches!(task.state, ExecutionTaskState::Running)
                    && task.claimed_at.is_some_and(|claimed_at| claimed_at < stale_threshold)
            })
            .collect()
    }

    /// Validate that a transition is allowed
    fn validate_transition(
        &self,
        task: &ExecutionTask,
        to_state: &ExecutionTaskState,
    ) -> Result<(), ExecutionError> {
        let valid = match (&task.state, to_state) {
            // Can only complete or fail from Running state
            (ExecutionTaskState::Running, ExecutionTaskState::Completed) => true,
            (ExecutionTaskState::Running, ExecutionTaskState::RetryableFailed) => true,
            (ExecutionTaskState::Running, ExecutionTaskState::FailedTerminal) => true,
            // Can claim from Queued or RetryableFailed
            (ExecutionTaskState::Queued, ExecutionTaskState::Running) => true,
            (ExecutionTaskState::RetryableFailed, ExecutionTaskState::Running) => true,
            // Same state is always valid (idempotent)
            (from, to) if from == to => true,
            // Everything else is invalid
            _ => false,
        };

        if valid {
            Ok(())
        } else {
            Err(ExecutionError::InvalidTransition {
                from: task.state.clone(),
                to: to_state.clone(),
                reason: format!("cannot transition from {:?} to {:?}", task.state, to_state),
            })
        }
    }

    /// Hash payload for idempotency checking
    fn hash_payload(payload: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(payload.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

impl Default for DeterministicExecutionEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// In-memory implementation for testing
#[derive(Clone, Debug, Default)]
pub struct InMemoryExecutionEngine {
    engine: DeterministicExecutionEngine,
    tasks: HashMap<String, ExecutionTask>,
    idempotency_records: HashMap<String, IdempotencyRecord>,
    transitions: Vec<ExecutionTransitionEvent>,
}

impl InMemoryExecutionEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: ExecutionEngineConfig) -> Self {
        Self { engine: DeterministicExecutionEngine::with_config(config), ..Default::default() }
    }

    pub fn enqueue(
        &mut self,
        quote_id: QuoteId,
        operation_kind: impl Into<String>,
        payload_json: impl Into<String>,
        idempotency_key: OperationKey,
        correlation_id: impl Into<String>,
    ) -> ExecutionTaskId {
        let (task, idempotency_record) = self.engine.create_task(
            quote_id,
            operation_kind,
            payload_json,
            idempotency_key,
            correlation_id,
        );

        let task_id = task.id.clone();
        self.tasks.insert(task_id.0.clone(), task);
        self.idempotency_records
            .insert(idempotency_record.operation_key.0.clone(), idempotency_record);

        task_id
    }

    pub fn claim(
        &mut self,
        task_id: &ExecutionTaskId,
        worker_id: impl Into<String>,
    ) -> Result<ClaimResult, ExecutionError> {
        let task = self
            .tasks
            .get(&task_id.0)
            .cloned()
            .ok_or_else(|| ExecutionError::TaskNotFound(task_id.clone()))?;

        let idempotency_key = task.idempotency_key.clone();
        let idempotency_record =
            self.idempotency_records.get_mut(&idempotency_key.0).ok_or_else(|| {
                ExecutionError::IdempotencyConflict(idempotency_key.clone(), "missing".to_string())
            })?;

        let result = self.engine.claim_task(task, worker_id, idempotency_record)?;

        self.tasks.insert(task_id.0.clone(), result.task.clone());
        self.transitions.push(result.transition.clone());

        Ok(result)
    }

    pub fn complete(
        &mut self,
        task_id: &ExecutionTaskId,
        result_fingerprint: impl Into<String>,
    ) -> Result<TransitionResult, ExecutionError> {
        let task = self
            .tasks
            .get(&task_id.0)
            .cloned()
            .ok_or_else(|| ExecutionError::TaskNotFound(task_id.clone()))?;

        let idempotency_key = task.idempotency_key.clone();
        let idempotency_record =
            self.idempotency_records.get_mut(&idempotency_key.0).ok_or_else(|| {
                ExecutionError::IdempotencyConflict(idempotency_key.clone(), "missing".to_string())
            })?;

        let result = self.engine.complete_task(task, result_fingerprint, idempotency_record)?;

        self.tasks.insert(task_id.0.clone(), result.task.clone());
        self.transitions.push(result.transition.clone());

        Ok(result)
    }

    pub fn fail(
        &mut self,
        task_id: &ExecutionTaskId,
        error: impl Into<String>,
        error_class: impl Into<String>,
        retry_policy: RetryPolicy,
    ) -> Result<TransitionResult, ExecutionError> {
        let task = self
            .tasks
            .get(&task_id.0)
            .cloned()
            .ok_or_else(|| ExecutionError::TaskNotFound(task_id.clone()))?;

        let idempotency_key = task.idempotency_key.clone();
        let idempotency_record =
            self.idempotency_records.get_mut(&idempotency_key.0).ok_or_else(|| {
                ExecutionError::IdempotencyConflict(idempotency_key.clone(), "missing".to_string())
            })?;

        let result =
            self.engine.fail_task(task, error, error_class, retry_policy, idempotency_record)?;

        self.tasks.insert(task_id.0.clone(), result.task.clone());
        self.transitions.push(result.transition.clone());

        Ok(result)
    }

    pub fn get_task(&self, task_id: &ExecutionTaskId) -> Option<&ExecutionTask> {
        self.tasks.get(&task_id.0)
    }

    pub fn get_transitions(&self) -> &[ExecutionTransitionEvent] {
        &self.transitions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_quote_id() -> QuoteId {
        QuoteId("Q-TEST-001".to_string())
    }

    fn test_operation_key() -> OperationKey {
        OperationKey("op-key-001".to_string())
    }

    #[test]
    fn create_task_initializes_queued_state() {
        let engine = DeterministicExecutionEngine::new();
        let (task, idempotency) = engine.create_task(
            test_quote_id(),
            "send_slack_message",
            "{\"channel\": \"#general\"}",
            test_operation_key(),
            "corr-001",
        );

        assert_eq!(task.state, ExecutionTaskState::Queued);
        assert_eq!(task.retry_count, 0);
        assert_eq!(idempotency.state, IdempotencyRecordState::Reserved);
        assert_eq!(idempotency.operation_key, test_operation_key());
    }

    #[test]
    fn claim_task_transitions_to_running() {
        let engine = DeterministicExecutionEngine::new();
        let (task, mut idempotency) = engine.create_task(
            test_quote_id(),
            "send_slack_message",
            "{\"channel\": \"#general\"}",
            test_operation_key(),
            "corr-001",
        );

        let result = engine.claim_task(task, "worker-001", &mut idempotency).unwrap();

        assert_eq!(result.task.state, ExecutionTaskState::Running);
        assert_eq!(result.task.claimed_by, Some("worker-001".to_string()));
        assert!(result.task.claimed_at.is_some());
        assert_eq!(idempotency.state, IdempotencyRecordState::Running);
    }

    #[test]
    fn complete_task_transitions_to_completed() {
        let engine = DeterministicExecutionEngine::new();
        let (task, mut idempotency) = engine.create_task(
            test_quote_id(),
            "send_slack_message",
            "{\"channel\": \"#general\"}",
            test_operation_key(),
            "corr-001",
        );

        let claimed = engine.claim_task(task, "worker-001", &mut idempotency).unwrap();
        let completed =
            engine.complete_task(claimed.task, "result-hash-abc", &mut idempotency).unwrap();

        assert_eq!(completed.task.state, ExecutionTaskState::Completed);
        assert_eq!(completed.task.result_fingerprint, Some("result-hash-abc".to_string()));
        assert_eq!(idempotency.state, IdempotencyRecordState::Completed);
    }

    #[test]
    fn fail_task_with_retry_policy_retries_until_max() {
        let config = ExecutionEngineConfig {
            default_max_retries: 2,
            retry_base_delay_seconds: 0, // No delay for tests
            ..Default::default()
        };
        let engine = DeterministicExecutionEngine::with_config(config);
        let (task, mut idempotency) = engine.create_task(
            test_quote_id(),
            "send_slack_message",
            "{\"channel\": \"#general\"}",
            test_operation_key(),
            "corr-001",
        );

        // First failure - should be retryable
        let claimed = engine.claim_task(task, "worker-001", &mut idempotency).unwrap();
        let failed1 = engine
            .fail_task(
                claimed.task,
                "network timeout",
                "NetworkError",
                RetryPolicy::Retry,
                &mut idempotency,
            )
            .unwrap();

        assert_eq!(failed1.task.state, ExecutionTaskState::RetryableFailed);
        assert_eq!(failed1.task.retry_count, 1);

        // Second failure - should be retryable
        let claimed2 = engine.claim_task(failed1.task, "worker-002", &mut idempotency).unwrap();
        let failed2 = engine
            .fail_task(
                claimed2.task,
                "network timeout",
                "NetworkError",
                RetryPolicy::Retry,
                &mut idempotency,
            )
            .unwrap();

        assert_eq!(failed2.task.state, ExecutionTaskState::RetryableFailed);
        assert_eq!(failed2.task.retry_count, 2);

        // Third failure - should be terminal
        let claimed3 = engine.claim_task(failed2.task, "worker-003", &mut idempotency).unwrap();
        let failed3 = engine
            .fail_task(
                claimed3.task,
                "network timeout",
                "NetworkError",
                RetryPolicy::Retry,
                &mut idempotency,
            )
            .unwrap();

        assert_eq!(failed3.task.state, ExecutionTaskState::FailedTerminal);
        assert_eq!(failed3.task.retry_count, 2); // Didn't increase past max
    }

    #[test]
    fn fail_task_with_fail_terminal_policy_goes_directly_to_terminal() {
        let engine = DeterministicExecutionEngine::new();
        let (task, mut idempotency) = engine.create_task(
            test_quote_id(),
            "send_slack_message",
            "{\"channel\": \"#general\"}",
            test_operation_key(),
            "corr-001",
        );

        let claimed = engine.claim_task(task, "worker-001", &mut idempotency).unwrap();
        let failed = engine
            .fail_task(
                claimed.task,
                "invalid payload",
                "ValidationError",
                RetryPolicy::FailTerminal,
                &mut idempotency,
            )
            .unwrap();

        assert_eq!(failed.task.state, ExecutionTaskState::FailedTerminal);
    }

    #[test]
    fn cannot_claim_completed_task() {
        let engine = DeterministicExecutionEngine::new();
        let (task, mut idempotency) = engine.create_task(
            test_quote_id(),
            "send_slack_message",
            "{\"channel\": \"#general\"}",
            test_operation_key(),
            "corr-001",
        );

        let claimed = engine.claim_task(task, "worker-001", &mut idempotency).unwrap();
        let completed =
            engine.complete_task(claimed.task, "result-hash-abc", &mut idempotency).unwrap();

        let result = engine.claim_task(completed.task, "worker-002", &mut idempotency);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExecutionError::InvalidTransition { from: ExecutionTaskState::Completed, .. }
        ));
    }

    #[test]
    fn recover_stale_tasks_finds_only_stale_running_tasks() {
        let config = ExecutionEngineConfig {
            claim_timeout_seconds: 300, // 5 minutes
            ..Default::default()
        };
        let engine = DeterministicExecutionEngine::with_config(config);

        let now = Utc::now();
        let stale_time = now - Duration::seconds(400); // Older than 5 minutes

        let stale_task = ExecutionTask {
            id: ExecutionTaskId("stale-001".to_string()),
            quote_id: test_quote_id(),
            operation_kind: "test".to_string(),
            payload_json: "{}".to_string(),
            idempotency_key: OperationKey("op-001".to_string()),
            state: ExecutionTaskState::Running,
            retry_count: 0,
            max_retries: 3,
            available_at: now,
            claimed_by: Some("worker-001".to_string()),
            claimed_at: Some(stale_time),
            last_error: None,
            result_fingerprint: None,
            state_version: 1,
            created_at: now,
            updated_at: now,
        };

        let fresh_task = ExecutionTask {
            id: ExecutionTaskId("fresh-001".to_string()),
            quote_id: test_quote_id(),
            operation_kind: "test".to_string(),
            payload_json: "{}".to_string(),
            idempotency_key: OperationKey("op-002".to_string()),
            state: ExecutionTaskState::Running,
            retry_count: 0,
            max_retries: 3,
            available_at: now,
            claimed_by: Some("worker-002".to_string()),
            claimed_at: Some(now - Duration::seconds(60)), // 1 minute ago
            last_error: None,
            result_fingerprint: None,
            state_version: 1,
            created_at: now,
            updated_at: now,
        };

        let queued_task = ExecutionTask {
            id: ExecutionTaskId("queued-001".to_string()),
            quote_id: test_quote_id(),
            operation_kind: "test".to_string(),
            payload_json: "{}".to_string(),
            idempotency_key: OperationKey("op-003".to_string()),
            state: ExecutionTaskState::Queued,
            retry_count: 0,
            max_retries: 3,
            available_at: now,
            claimed_by: None,
            claimed_at: None,
            last_error: None,
            result_fingerprint: None,
            state_version: 1,
            created_at: now,
            updated_at: now,
        };

        let tasks = vec![stale_task.clone(), fresh_task.clone(), queued_task.clone()];
        let stale = engine.recover_stale_tasks(tasks, now);

        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].id, stale_task.id);
    }

    #[test]
    fn in_memory_engine_tracks_tasks_and_transitions() {
        let mut engine = InMemoryExecutionEngine::new();

        let task_id = engine.enqueue(
            test_quote_id(),
            "send_slack_message",
            "{\"channel\": \"#general\"}",
            test_operation_key(),
            "corr-001",
        );

        assert!(engine.get_task(&task_id).is_some());

        let claimed = engine.claim(&task_id, "worker-001").unwrap();
        assert_eq!(claimed.task.state, ExecutionTaskState::Running);

        let completed = engine.complete(&task_id, "result-hash-abc").unwrap();
        assert_eq!(completed.task.state, ExecutionTaskState::Completed);

        assert_eq!(engine.get_transitions().len(), 2);
    }
}
