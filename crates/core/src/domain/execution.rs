use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::quote::QuoteId;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionTaskId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OperationKey(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionTransitionId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionTaskState {
    Queued,
    Running,
    RetryableFailed,
    FailedTerminal,
    Completed,
}

impl ExecutionTaskState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::RetryableFailed => "retryable_failed",
            Self::FailedTerminal => "failed_terminal",
            Self::Completed => "completed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "queued" => Some(Self::Queued),
            "running" => Some(Self::Running),
            "retryable_failed" => Some(Self::RetryableFailed),
            "failed_terminal" => Some(Self::FailedTerminal),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyRecordState {
    Reserved,
    Running,
    Completed,
    FailedRetryable,
    FailedTerminal,
}

impl IdempotencyRecordState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Reserved => "reserved",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::FailedRetryable => "failed_retryable",
            Self::FailedTerminal => "failed_terminal",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "reserved" => Some(Self::Reserved),
            "running" => Some(Self::Running),
            "completed" => Some(Self::Completed),
            "failed_retryable" => Some(Self::FailedRetryable),
            "failed_terminal" => Some(Self::FailedTerminal),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionTask {
    pub id: ExecutionTaskId,
    pub quote_id: QuoteId,
    pub operation_kind: String,
    pub payload_json: String,
    pub idempotency_key: OperationKey,
    pub state: ExecutionTaskState,
    pub retry_count: u32,
    pub max_retries: u32,
    pub available_at: DateTime<Utc>,
    pub claimed_by: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub result_fingerprint: Option<String>,
    pub state_version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdempotencyRecord {
    pub operation_key: OperationKey,
    pub quote_id: QuoteId,
    pub operation_kind: String,
    pub payload_hash: String,
    pub state: IdempotencyRecordState,
    pub attempt_count: u32,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub result_snapshot_json: Option<String>,
    pub error_snapshot_json: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub correlation_id: String,
    pub created_by_component: String,
    pub updated_by_component: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionTransitionEvent {
    pub id: ExecutionTransitionId,
    pub task_id: ExecutionTaskId,
    pub quote_id: QuoteId,
    pub from_state: Option<ExecutionTaskState>,
    pub to_state: ExecutionTaskState,
    pub transition_reason: String,
    pub error_class: Option<String>,
    pub decision_context_json: String,
    pub actor_type: String,
    pub actor_id: String,
    pub idempotency_key: Option<OperationKey>,
    pub correlation_id: String,
    pub state_version: u32,
    pub occurred_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::{ExecutionTaskState, IdempotencyRecordState};

    #[test]
    fn execution_task_state_round_trips_from_storage_encoding() {
        let cases = [
            ExecutionTaskState::Queued,
            ExecutionTaskState::Running,
            ExecutionTaskState::RetryableFailed,
            ExecutionTaskState::FailedTerminal,
            ExecutionTaskState::Completed,
        ];

        for state in cases {
            let decoded = ExecutionTaskState::parse(state.as_str());
            assert_eq!(decoded, Some(state));
        }
    }

    #[test]
    fn idempotency_state_round_trips_from_storage_encoding() {
        let cases = [
            IdempotencyRecordState::Reserved,
            IdempotencyRecordState::Running,
            IdempotencyRecordState::Completed,
            IdempotencyRecordState::FailedRetryable,
            IdempotencyRecordState::FailedTerminal,
        ];

        for state in cases {
            let decoded = IdempotencyRecordState::parse(state.as_str());
            assert_eq!(decoded, Some(state));
        }
    }
}
