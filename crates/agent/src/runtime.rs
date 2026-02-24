use anyhow::Result;
use std::sync::Mutex;

use crate::guardrails::{GuardrailDecision, GuardrailIntent, GuardrailPolicy, QueueAction};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeOutcome {
    Success,
    Rejected,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeResponse {
    pub quote_id: String,
    pub action_key: String,
    pub outcome: RuntimeOutcome,
    pub user_message: String,
    pub fallback_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeAuditEvent {
    pub quote_id: String,
    pub action_key: String,
    pub outcome: RuntimeOutcome,
    pub user_message: String,
    pub fallback_path: Option<String>,
}

pub struct AgentRuntime {
    guardrails: GuardrailPolicy,
    audit_events: Mutex<Vec<RuntimeAuditEvent>>,
}

impl Default for AgentRuntime {
    fn default() -> Self {
        Self::new(GuardrailPolicy::default())
    }
}

impl AgentRuntime {
    pub fn new(guardrails: GuardrailPolicy) -> Self {
        Self { guardrails, audit_events: Mutex::new(Vec::new()) }
    }

    pub fn apply_guardrails(&self, intent: GuardrailIntent) -> RuntimeResponse {
        let quote_id = intent.quote_id().to_string();
        let action_key = intent.action_key();
        let decision = self.guardrails.evaluate(&intent);

        let response = match decision {
            GuardrailDecision::Allow => RuntimeResponse {
                quote_id: quote_id.clone(),
                action_key: action_key.clone(),
                outcome: RuntimeOutcome::Success,
                user_message: format!(
                    "Execution request accepted for quote {quote_id}. Continuing deterministic queue processing."
                ),
                fallback_path: None,
            },
            GuardrailDecision::Deny { user_message, fallback_path, .. } => RuntimeResponse {
                quote_id: quote_id.clone(),
                action_key: action_key.clone(),
                outcome: RuntimeOutcome::Rejected,
                user_message,
                fallback_path: Some(fallback_path.to_string()),
            },
            GuardrailDecision::Degrade { user_message, fallback_path, .. } => RuntimeResponse {
                quote_id: quote_id.clone(),
                action_key: action_key.clone(),
                outcome: RuntimeOutcome::Failed,
                user_message,
                fallback_path: Some(fallback_path.to_string()),
            },
        };

        self.record_audit_event(RuntimeAuditEvent {
            quote_id,
            action_key,
            outcome: response.outcome.clone(),
            user_message: response.user_message.clone(),
            fallback_path: response.fallback_path.clone(),
        });

        response
    }

    pub fn audit_events(&self) -> Vec<RuntimeAuditEvent> {
        self.audit_events.lock().expect("audit event lock should not be poisoned").clone()
    }

    pub async fn handle_thread_message(&self, text: &str) -> Result<String> {
        let intent = classify_thread_intent(text);
        let response = self.apply_guardrails(intent);
        Ok(response.user_message)
    }

    fn record_audit_event(&self, event: RuntimeAuditEvent) {
        self.audit_events.lock().expect("audit event lock should not be poisoned").push(event);
    }
}

fn classify_thread_intent(text: &str) -> GuardrailIntent {
    let lowered = text.to_ascii_lowercase();
    let quote_id = extract_quote_id(text).unwrap_or_else(|| "Q-UNKNOWN".to_string());
    let task_id = extract_task_id(text).unwrap_or_else(|| "task-unknown".to_string());

    if lowered.contains("price override") || lowered.contains("set price") {
        return GuardrailIntent::PriceOverride { quote_id, requested_price_cents: 0 };
    }

    if lowered.contains("approve discount") || lowered.contains("discount approval") {
        return GuardrailIntent::DiscountApproval { quote_id, requested_discount_pct: 0 };
    }

    if lowered.contains("retry") {
        return GuardrailIntent::QueueAction { quote_id, task_id, action: QueueAction::RetryNow };
    }

    if lowered.contains("cancel") {
        return GuardrailIntent::QueueAction { quote_id, task_id, action: QueueAction::Cancel };
    }

    if lowered.contains("view result") {
        return GuardrailIntent::QueueAction { quote_id, task_id, action: QueueAction::ViewResult };
    }

    if lowered.contains("refresh") || lowered.contains("status") || lowered.contains("check") {
        return GuardrailIntent::QueueAction {
            quote_id,
            task_id,
            action: QueueAction::RefreshStatus,
        };
    }

    GuardrailIntent::AmbiguousQueueIntent { quote_id, raw_text: text.to_string() }
}

fn extract_quote_id(text: &str) -> Option<String> {
    extract_prefixed_token(text, "q-").map(|token| token.to_ascii_uppercase())
}

fn extract_task_id(text: &str) -> Option<String> {
    extract_prefixed_token(text, "task-")
}

fn extract_prefixed_token(text: &str, prefix: &str) -> Option<String> {
    text.split_whitespace().find_map(|part| {
        let cleaned = part.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-');
        if cleaned.to_ascii_lowercase().starts_with(prefix) {
            Some(cleaned.to_string())
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{AgentRuntime, RuntimeOutcome};
    use crate::guardrails::{GuardrailIntent, GuardrailPolicy, QueueAction};

    #[test]
    fn allowed_flow_emits_success_audit_event() {
        let runtime = AgentRuntime::new(GuardrailPolicy::default());
        let response = runtime.apply_guardrails(GuardrailIntent::QueueAction {
            quote_id: "Q-REL-200".to_string(),
            task_id: "task-12".to_string(),
            action: QueueAction::RetryNow,
        });

        assert_eq!(response.outcome, RuntimeOutcome::Success);
        assert!(response.fallback_path.is_none());
        assert!(response.user_message.contains("accepted"));

        let events = runtime.audit_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].quote_id, "Q-REL-200");
        assert_eq!(events[0].action_key, "queue.retry_now");
        assert_eq!(events[0].outcome, RuntimeOutcome::Success);
        assert!(events[0].fallback_path.is_none());
    }

    #[test]
    fn denied_flow_emits_rejected_audit_event_with_fallback_path() {
        let runtime = AgentRuntime::new(GuardrailPolicy::default());
        let response = runtime.apply_guardrails(GuardrailIntent::PriceOverride {
            quote_id: "Q-REL-201".to_string(),
            requested_price_cents: 99_000,
        });

        assert_eq!(response.outcome, RuntimeOutcome::Rejected);
        assert!(response.user_message.contains("cannot set or override prices"));
        assert_eq!(response.fallback_path.as_deref(), Some("deterministic_pricing_workflow"));

        let events = runtime.audit_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].quote_id, "Q-REL-201");
        assert_eq!(events[0].action_key, "policy.price_override");
        assert_eq!(events[0].outcome, RuntimeOutcome::Rejected);
        assert_eq!(events[0].fallback_path.as_deref(), Some("deterministic_pricing_workflow"));
    }

    #[test]
    fn degraded_flow_emits_failed_audit_event_with_fallback_path() {
        let runtime = AgentRuntime::new(GuardrailPolicy::default());
        let response = runtime.apply_guardrails(GuardrailIntent::AmbiguousQueueIntent {
            quote_id: "Q-REL-202".to_string(),
            raw_text: "can you do it".to_string(),
        });

        assert_eq!(response.outcome, RuntimeOutcome::Failed);
        assert!(response.user_message.contains("could not safely determine"));
        assert_eq!(response.fallback_path.as_deref(), Some("request_explicit_queue_action"));

        let events = runtime.audit_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].quote_id, "Q-REL-202");
        assert_eq!(events[0].action_key, "queue.ambiguous_intent");
        assert_eq!(events[0].outcome, RuntimeOutcome::Failed);
        assert_eq!(events[0].fallback_path.as_deref(), Some("request_explicit_queue_action"));
    }
}
