//! Outbox domain model for guaranteed side-effect delivery
//!
//! This module extends the execution queue system to provide:
//! - Reliable delivery of side effects (Slack, CRM, Email, etc.)
//! - Automatic retry with exponential backoff
//! - Dead letter queue for manual intervention
//! - Idempotency guarantees

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::execution::{ExecutionTaskId, OperationKey};
use crate::domain::quote::QuoteId;

/// Side effect operation types supported by the outbox
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboxOperation {
    // Slack operations
    SlackPostMessage {
        channel: String,
        text: String,
        thread_ts: Option<String>,
    },
    SlackUpdateBlocks {
        channel: String,
        ts: String,
        blocks: String,
    },
    SlackUploadFile {
        channel: String,
        filename: String,
        content_base64: String,
        initial_comment: Option<String>,
    },

    // CRM operations
    CrmSyncQuote {
        provider: String,
        quote_id: QuoteId,
    },
    CrmCreateDeal {
        provider: String,
        account_id: String,
        deal_name: String,
        amount: String, // Decimal as string for serialization
    },

    // Document operations
    PdfGenerate {
        quote_id: QuoteId,
        template: String,
    },

    // Email operations
    EmailSend {
        to: Vec<String>,
        subject: String,
        body_text: String,
        body_html: Option<String>,
        attachment_path: Option<String>,
    },

    // Custom operations
    WebhookCall {
        url: String,
        method: String,
        headers: HashMap<String, String>,
        body: String,
    },
}

impl OutboxOperation {
    /// Operation kind string for metrics and logging
    pub fn kind(&self) -> &'static str {
        match self {
            Self::SlackPostMessage { .. } => "slack.post_message",
            Self::SlackUpdateBlocks { .. } => "slack.update_blocks",
            Self::SlackUploadFile { .. } => "slack.upload_file",
            Self::CrmSyncQuote { .. } => "crm.sync_quote",
            Self::CrmCreateDeal { .. } => "crm.create_deal",
            Self::PdfGenerate { .. } => "pdf.generate",
            Self::EmailSend { .. } => "email.send",
            Self::WebhookCall { .. } => "webhook.call",
        }
    }

    /// Retry policy specific to this operation type
    pub fn retry_policy(&self) -> RetryPolicy {
        match self {
            // Slack: retry quickly (user-facing)
            Self::SlackPostMessage { .. } | Self::SlackUpdateBlocks { .. } => RetryPolicy {
                max_retries: 3,
                base_delay_secs: 5,
                max_delay_secs: 300, // 5 minutes
                jitter_factor: 0.2,
            },

            // Slack file upload: more retries (can fail due to size)
            Self::SlackUploadFile { .. } => RetryPolicy {
                max_retries: 5,
                base_delay_secs: 10,
                max_delay_secs: 600, // 10 minutes
                jitter_factor: 0.2,
            },

            // CRM: retry aggressively (business-critical)
            Self::CrmSyncQuote { .. } | Self::CrmCreateDeal { .. } => RetryPolicy {
                max_retries: 10,
                base_delay_secs: 30,
                max_delay_secs: 7200, // 2 hours
                jitter_factor: 0.2,
            },

            // PDF: standard retry
            Self::PdfGenerate { .. } => RetryPolicy::default(),

            // Email: standard retry
            Self::EmailSend { .. } => RetryPolicy::default(),

            // Webhook: longer backoff (external dependency)
            Self::WebhookCall { .. } => RetryPolicy {
                max_retries: 8,
                base_delay_secs: 60,
                max_delay_secs: 14400, // 4 hours
                jitter_factor: 0.2,
            },
        }
    }

    /// Generate deterministic idempotency key for this operation
    pub fn idempotency_key(&self, quote_id: &QuoteId) -> OperationKey {
        use blake3::Hasher;

        let mut hasher = Hasher::new();

        // Domain separation
        hasher.update(b"quotey:outbox:v1");

        // Quote scope
        hasher.update(quote_id.0.as_bytes());

        // Operation type
        hasher.update(self.kind().as_bytes());

        // Canonical JSON with object keys sorted recursively.
        let canonical_json = canonical_operation_json(self);
        hasher.update(canonical_json.as_bytes());

        let hash = hasher.finalize();
        let short_hash: String = hash.to_hex().to_string().chars().take(16).collect();

        OperationKey(format!("{}:{short_hash}", quote_id.0))
    }
}

fn canonical_operation_json(operation: &OutboxOperation) -> String {
    match serde_json::to_value(operation).map(canonicalize_json_value) {
        Ok(value) => serde_json::to_string(&value).unwrap_or_else(|_| format!("{operation:?}")),
        Err(_) => format!("{operation:?}"),
    }
}

fn canonicalize_json_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));

            let mut normalized = serde_json::Map::with_capacity(entries.len());
            for (key, nested_value) in entries {
                normalized.insert(key, canonicalize_json_value(nested_value));
            }
            Value::Object(normalized)
        }
        Value::Array(items) => {
            Value::Array(items.into_iter().map(canonicalize_json_value).collect())
        }
        primitive => primitive,
    }
}

/// Retry configuration for outbox operations
#[derive(Clone, Debug, Copy, PartialEq)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay_secs: i64,
    pub max_delay_secs: i64,
    pub jitter_factor: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_delay_secs: 30,
            max_delay_secs: 3600, // 1 hour
            jitter_factor: 0.2,
        }
    }
}

impl RetryPolicy {
    /// Calculate next retry time based on attempt count
    pub fn next_retry_at(&self, attempt: u32) -> Option<DateTime<Utc>> {
        if attempt >= self.max_retries {
            return None;
        }

        // Exponential backoff: 30s, 60s, 120s, 240s, 480s...
        let exponential = self.base_delay_secs * 2_i64.pow(attempt);
        let delay_secs = std::cmp::min(exponential, self.max_delay_secs);

        // Add jitter (±20%)
        let jitter_range = delay_secs as f64 * self.jitter_factor;
        let jitter = (rand::random::<f64>() * 2.0 - 1.0) * jitter_range;
        let final_delay = (delay_secs as f64 + jitter) as i64;

        Some(Utc::now() + Duration::seconds(final_delay))
    }
}

/// Status of an outbox entry
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutboxStatus {
    pub task_id: ExecutionTaskId,
    pub state: OutboxState,
    pub operation_kind: String,
    pub retry_count: u32,
    pub max_retries: u32,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

/// Simplified state for outbox operations
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutboxState {
    Pending,
    Claimed,
    Completed,
    Failed,
}

impl std::fmt::Display for OutboxState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Claimed => write!(f, "claimed"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Entry in the dead letter queue (manual intervention required)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeadLetterEntry {
    pub id: String,
    pub quote_id: QuoteId,
    pub operation_kind: String,
    pub payload_json: String,
    pub idempotency_key: OperationKey,
    pub failed_at: DateTime<Utc>,
    pub failure_reason: String,
    pub error_class: Option<String>,
    pub retry_count: u32,
    pub max_retries: u32,
    pub resolution_status: ResolutionStatus,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution_notes: Option<String>,
}

/// Resolution status for dead letter entries
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStatus {
    Pending,
    Replayed,
    Abandoned,
}

/// Request to manually replay a dead letter entry
#[derive(Clone, Debug)]
pub struct ReplayRequest {
    pub dead_letter_id: String,
    pub requested_by: String,
}

/// Result of a replay attempt
#[derive(Clone, Debug)]
pub struct ReplayResult {
    pub success: bool,
    pub new_task_id: Option<ExecutionTaskId>,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_policy_exponential_backoff() {
        let policy = RetryPolicy::default();

        // First retry: ~30s
        let t1 = policy.next_retry_at(0).unwrap();
        let diff1 = (t1 - Utc::now()).num_seconds();
        assert!(diff1 >= 24 && diff1 <= 36, "First retry should be ~30s");

        // Second retry: ~60s
        let t2 = policy.next_retry_at(1).unwrap();
        let diff2 = (t2 - Utc::now()).num_seconds();
        assert!(diff2 >= 48 && diff2 <= 72, "Second retry should be ~60s");
    }

    #[test]
    fn retry_policy_max_retries() {
        let policy = RetryPolicy { max_retries: 3, ..Default::default() };

        assert!(policy.next_retry_at(0).is_some());
        assert!(policy.next_retry_at(1).is_some());
        assert!(policy.next_retry_at(2).is_some());
        assert!(policy.next_retry_at(3).is_none()); // Max exceeded
        assert!(policy.next_retry_at(4).is_none());
    }

    #[test]
    fn idempotency_key_is_deterministic() {
        let quote_id = QuoteId("Q-TEST-001".to_string());
        let op = OutboxOperation::SlackPostMessage {
            channel: "#sales".to_string(),
            text: "Hello".to_string(),
            thread_ts: None,
        };

        let key1 = op.idempotency_key(&quote_id);
        let key2 = op.idempotency_key(&quote_id);

        assert_eq!(key1.0, key2.0, "Idempotency key should be deterministic");
    }

    #[test]
    fn idempotency_key_differs_by_content() {
        let quote_id = QuoteId("Q-TEST-001".to_string());

        let op1 = OutboxOperation::SlackPostMessage {
            channel: "#sales".to_string(),
            text: "Hello".to_string(),
            thread_ts: None,
        };

        let op2 = OutboxOperation::SlackPostMessage {
            channel: "#sales".to_string(),
            text: "World".to_string(),
            thread_ts: None,
        };

        let key1 = op1.idempotency_key(&quote_id);
        let key2 = op2.idempotency_key(&quote_id);

        assert_ne!(key1.0, key2.0, "Different content should produce different keys");
    }

    #[test]
    fn operation_kind_mapping() {
        assert_eq!(
            OutboxOperation::SlackPostMessage {
                channel: "#test".to_string(),
                text: "test".to_string(),
                thread_ts: None,
            }
            .kind(),
            "slack.post_message"
        );

        assert_eq!(
            OutboxOperation::CrmSyncQuote {
                provider: "salesforce".to_string(),
                quote_id: QuoteId("Q-001".to_string()),
            }
            .kind(),
            "crm.sync_quote"
        );
    }

    #[test]
    fn idempotency_key_is_stable_for_equivalent_map_payloads() {
        let quote_id = QuoteId("Q-TEST-001".to_string());

        let mut headers_a = HashMap::new();
        headers_a.insert("x-customer".to_string(), "acme".to_string());
        headers_a.insert("x-request-id".to_string(), "REQ-123".to_string());

        let mut headers_b = HashMap::new();
        headers_b.insert("x-request-id".to_string(), "REQ-123".to_string());
        headers_b.insert("x-customer".to_string(), "acme".to_string());

        let op_a = OutboxOperation::WebhookCall {
            url: "https://example.com/hook".to_string(),
            method: "POST".to_string(),
            headers: headers_a,
            body: "{\"event\":\"quote.updated\"}".to_string(),
        };
        let op_b = OutboxOperation::WebhookCall {
            url: "https://example.com/hook".to_string(),
            method: "POST".to_string(),
            headers: headers_b,
            body: "{\"event\":\"quote.updated\"}".to_string(),
        };

        assert_eq!(op_a.idempotency_key(&quote_id), op_b.idempotency_key(&quote_id));
    }
}
