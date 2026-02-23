use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::domain::approval::ApprovalStatus;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyInput {
    pub requested_discount_pct: Decimal,
    pub deal_value: Decimal,
    pub minimum_margin_pct: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyViolation {
    pub policy_id: String,
    pub reason: String,
    pub required_approval: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub approval_required: bool,
    pub approval_status: ApprovalStatus,
    pub reasons: Vec<String>,
    pub violations: Vec<PolicyViolation>,
}

pub trait PolicyEngine: Send + Sync {
    fn evaluate(&self, input: &PolicyInput) -> PolicyDecision;
}

#[derive(Default)]
pub struct DeterministicPolicyEngine;

impl PolicyEngine for DeterministicPolicyEngine {
    fn evaluate(&self, input: &PolicyInput) -> PolicyDecision {
        evaluate_policy_input(input)
    }
}

pub fn evaluate_policy() -> PolicyDecision {
    PolicyDecision {
        approval_required: false,
        approval_status: ApprovalStatus::Approved,
        reasons: Vec::new(),
        violations: Vec::new(),
    }
}

pub fn evaluate_policy_input(input: &PolicyInput) -> PolicyDecision {
    if input.requested_discount_pct > Decimal::new(2000, 2) {
        return PolicyDecision {
            approval_required: true,
            approval_status: ApprovalStatus::Pending,
            reasons: vec!["Discount exceeds automatic threshold".to_string()],
            violations: vec![PolicyViolation {
                policy_id: "discount-cap".to_string(),
                reason: "Requested discount is above 20%".to_string(),
                required_approval: Some("sales_manager".to_string()),
            }],
        };
    }

    evaluate_policy()
}
