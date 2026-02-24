#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueAction {
    RefreshStatus,
    RetryNow,
    Cancel,
    ViewResult,
}

impl QueueAction {
    pub fn action_key(&self) -> &'static str {
        match self {
            Self::RefreshStatus => "queue.refresh_status",
            Self::RetryNow => "queue.retry_now",
            Self::Cancel => "queue.cancel",
            Self::ViewResult => "queue.view_result",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuardrailIntent {
    QueueAction { quote_id: String, task_id: String, action: QueueAction },
    PriceOverride { quote_id: String, requested_price_cents: i64 },
    DiscountApproval { quote_id: String, requested_discount_pct: u8 },
    AmbiguousQueueIntent { quote_id: String, raw_text: String },
}

impl GuardrailIntent {
    pub fn quote_id(&self) -> &str {
        match self {
            Self::QueueAction { quote_id, .. }
            | Self::PriceOverride { quote_id, .. }
            | Self::DiscountApproval { quote_id, .. }
            | Self::AmbiguousQueueIntent { quote_id, .. } => quote_id,
        }
    }

    pub fn action_key(&self) -> String {
        match self {
            Self::QueueAction { action, .. } => action.action_key().to_string(),
            Self::PriceOverride { .. } => "policy.price_override".to_string(),
            Self::DiscountApproval { .. } => "policy.discount_approval".to_string(),
            Self::AmbiguousQueueIntent { .. } => "queue.ambiguous_intent".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuardrailDecision {
    Allow,
    Deny { reason_code: &'static str, user_message: String, fallback_path: &'static str },
    Degrade { reason_code: &'static str, user_message: String, fallback_path: &'static str },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardrailPolicy {
    pub llm_can_set_prices: bool,
    pub llm_can_approve_discounts: bool,
    pub queue_actions_enabled: bool,
}

impl Default for GuardrailPolicy {
    fn default() -> Self {
        Self {
            llm_can_set_prices: false,
            llm_can_approve_discounts: false,
            queue_actions_enabled: true,
        }
    }
}

impl GuardrailPolicy {
    pub fn evaluate(&self, intent: &GuardrailIntent) -> GuardrailDecision {
        match intent {
            GuardrailIntent::QueueAction { .. } if self.queue_actions_enabled => {
                GuardrailDecision::Allow
            }
            GuardrailIntent::QueueAction { .. } => GuardrailDecision::Degrade {
                reason_code: "queue_actions_disabled",
                user_message:
                    "Queue controls are temporarily unavailable. Please use status-only mode."
                        .to_string(),
                fallback_path: "queue_status_only",
            },
            GuardrailIntent::PriceOverride { .. } => GuardrailDecision::Deny {
                reason_code: if self.llm_can_set_prices {
                    "price_override_policy_conflict"
                } else {
                    "price_override_disallowed"
                },
                user_message:
                    "I cannot set or override prices from chat. Please use deterministic pricing workflow."
                        .to_string(),
                fallback_path: "deterministic_pricing_workflow",
            },
            GuardrailIntent::DiscountApproval { .. } => GuardrailDecision::Deny {
                reason_code: if self.llm_can_approve_discounts {
                    "discount_policy_conflict"
                } else {
                    "discount_approval_disallowed"
                },
                user_message:
                    "I cannot approve discounts in chat. Please route this through the approval workflow."
                        .to_string(),
                fallback_path: "approval_workflow",
            },
            GuardrailIntent::AmbiguousQueueIntent { .. } => GuardrailDecision::Degrade {
                reason_code: "ambiguous_queue_intent",
                user_message: "I could not safely determine the queue action from that request."
                    .to_string(),
                fallback_path: "request_explicit_queue_action",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GuardrailDecision, GuardrailIntent, GuardrailPolicy, QueueAction};

    #[test]
    fn supported_queue_action_allow() {
        let policy = GuardrailPolicy::default();
        let decision = policy.evaluate(&GuardrailIntent::QueueAction {
            quote_id: "Q-REL-100".to_string(),
            task_id: "task-1".to_string(),
            action: QueueAction::RetryNow,
        });
        assert_eq!(decision, GuardrailDecision::Allow);
    }

    #[test]
    fn price_override_denial() {
        let policy = GuardrailPolicy::default();
        let decision = policy.evaluate(&GuardrailIntent::PriceOverride {
            quote_id: "Q-REL-101".to_string(),
            requested_price_cents: 250_000,
        });

        let (reason_code, user_message, fallback_path) = match decision {
            GuardrailDecision::Deny { reason_code, user_message, fallback_path } => {
                (reason_code, user_message, fallback_path)
            }
            _ => ("", String::new(), ""),
        };

        assert_eq!(reason_code, "price_override_disallowed");
        assert!(user_message.contains("cannot set or override prices"));
        assert_eq!(fallback_path, "deterministic_pricing_workflow");
    }

    #[test]
    fn ambiguous_queue_intent_degrade() {
        let policy = GuardrailPolicy::default();
        let decision = policy.evaluate(&GuardrailIntent::AmbiguousQueueIntent {
            quote_id: "Q-REL-102".to_string(),
            raw_text: "can you do the thing from before".to_string(),
        });

        let (reason_code, user_message, fallback_path) = match decision {
            GuardrailDecision::Degrade { reason_code, user_message, fallback_path } => {
                (reason_code, user_message, fallback_path)
            }
            _ => ("", String::new(), ""),
        };

        assert_eq!(reason_code, "ambiguous_queue_intent");
        assert!(user_message.contains("could not safely determine"));
        assert_eq!(fallback_path, "request_explicit_queue_action");
    }
}
