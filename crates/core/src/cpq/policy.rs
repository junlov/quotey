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
    let mut decision = evaluate_policy();
    let mut reasons = Vec::new();
    let mut violations = Vec::new();

    if input.requested_discount_pct < Decimal::ZERO {
        reasons.push("Requested discount cannot be negative".to_string());
        violations.push(PolicyViolation {
            policy_id: "invalid-input".to_string(),
            reason: "requested_discount_pct must be >= 0".to_string(),
            required_approval: Some("risk".to_string()),
        });
    }

    if input.minimum_margin_pct < Decimal::ZERO {
        reasons.push("Minimum margin cannot be negative".to_string());
        violations.push(PolicyViolation {
            policy_id: "invalid-input".to_string(),
            reason: "minimum_margin_pct must be >= 0".to_string(),
            required_approval: Some("risk".to_string()),
        });
    }

    if input.deal_value < Decimal::ZERO {
        reasons.push("Deal value cannot be negative".to_string());
        violations.push(PolicyViolation {
            policy_id: "invalid-input".to_string(),
            reason: "deal_value must be >= 0".to_string(),
            required_approval: Some("risk".to_string()),
        });
    }

    if input.requested_discount_pct > Decimal::new(3000, 2) {
        reasons.push("Discount exceeds 30% hard threshold".to_string());
        violations.push(PolicyViolation {
            policy_id: "discount-cap".to_string(),
            reason: "Requested discount is above 30%".to_string(),
            required_approval: Some("vp_finance".to_string()),
        });
    } else if input.requested_discount_pct > Decimal::new(2000, 2) {
        reasons.push("Discount exceeds standard approval threshold".to_string());
        violations.push(PolicyViolation {
            policy_id: "discount-cap".to_string(),
            reason: "Requested discount is above 20%".to_string(),
            required_approval: Some("sales_manager".to_string()),
        });
    }

    if input.minimum_margin_pct < Decimal::new(1000, 2) {
        reasons.push("Margin floor breached".to_string());
        violations.push(PolicyViolation {
            policy_id: "margin-floor".to_string(),
            reason: "Minimum margin is below 10%".to_string(),
            required_approval: Some("finance".to_string()),
        });
    }

    if !violations.is_empty() {
        decision.approval_required = true;
        decision.approval_status = ApprovalStatus::Pending;
        decision.reasons = reasons;
        decision.violations = violations;
        return decision;
    }

    decision
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::{evaluate_policy_input, PolicyInput};

    #[test]
    fn policy_requires_approver_above_thresholds() {
        let high_discount = evaluate_policy_input(&PolicyInput {
            requested_discount_pct: Decimal::new(3001, 2),
            deal_value: Decimal::new(50_000, 2),
            minimum_margin_pct: Decimal::new(1200, 2),
        });
        assert!(high_discount.approval_required);
        assert!(high_discount.violations.iter().any(|v| v.policy_id == "discount-cap"
            && v.required_approval == Some("vp_finance".to_string())));

        let low_margin = evaluate_policy_input(&PolicyInput {
            requested_discount_pct: Decimal::new(1000, 2),
            deal_value: Decimal::new(50_000, 2),
            minimum_margin_pct: Decimal::new(50, 2),
        });
        assert!(low_margin.approval_required);
        assert!(low_margin.violations.iter().any(|v| v.policy_id == "margin-floor"));
    }

    #[test]
    fn policy_allows_standard_quote_with_clean_inputs() {
        let normal = evaluate_policy_input(&PolicyInput {
            requested_discount_pct: Decimal::new(1500, 2),
            deal_value: Decimal::new(50_000, 2),
            minimum_margin_pct: Decimal::new(2000, 2),
        });
        assert!(!normal.approval_required);
    }
}
