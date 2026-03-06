use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::domain::approval::ApprovalStatus;

/// Configurable policy thresholds loaded from org_settings.
/// Defaults match the original hardcoded values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyThresholds {
    /// Discount % above which manager approval is required (default: 20%).
    pub manager_discount_pct: Decimal,
    /// Discount % above which VP/finance approval is required (default: 30%).
    pub vp_discount_pct: Decimal,
    /// Minimum margin % below which finance approval is required (default: 10%).
    pub margin_floor_pct: Decimal,
    /// Deal value in cents above which finance approval is required (default: None = disabled).
    pub finance_deal_value_cents: Option<i64>,
    /// If true, auto-approve quotes with no violations (default: true).
    pub auto_approve_clean: bool,
}

impl Default for PolicyThresholds {
    fn default() -> Self {
        Self {
            manager_discount_pct: Decimal::new(2000, 2), // 20%
            vp_discount_pct: Decimal::new(3000, 2),      // 30%
            margin_floor_pct: Decimal::new(1000, 2),     // 10%
            finance_deal_value_cents: None,
            auto_approve_clean: true,
        }
    }
}

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
    evaluate_policy_with_thresholds(input, &PolicyThresholds::default())
}

pub fn evaluate_policy_with_thresholds(
    input: &PolicyInput,
    thresholds: &PolicyThresholds,
) -> PolicyDecision {
    let mut decision = evaluate_policy();
    let mut reasons = Vec::new();
    let mut violations = Vec::new();

    if input.requested_discount_pct < Decimal::ZERO
        || input.requested_discount_pct > Decimal::new(10000, 2)
    {
        reasons.push("Requested discount out of valid range".to_string());
        violations.push(PolicyViolation {
            policy_id: "invalid-input".to_string(),
            reason: "requested_discount_pct must be between 0 and 100".to_string(),
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

    if input.requested_discount_pct > thresholds.vp_discount_pct {
        reasons.push(format!("Discount exceeds {}% hard threshold", thresholds.vp_discount_pct));
        violations.push(PolicyViolation {
            policy_id: "discount-cap".to_string(),
            reason: format!("Requested discount is above {}%", thresholds.vp_discount_pct),
            required_approval: Some("vp_finance".to_string()),
        });
    } else if input.requested_discount_pct > thresholds.manager_discount_pct {
        reasons.push(format!(
            "Discount exceeds {}% standard approval threshold",
            thresholds.manager_discount_pct
        ));
        violations.push(PolicyViolation {
            policy_id: "discount-cap".to_string(),
            reason: format!("Requested discount is above {}%", thresholds.manager_discount_pct),
            required_approval: Some("sales_manager".to_string()),
        });
    }

    if let Some(finance_cents) = thresholds.finance_deal_value_cents {
        let deal_cents = input.deal_value * Decimal::from(100);
        if deal_cents > Decimal::from(finance_cents) {
            reasons.push(format!(
                "Deal value exceeds finance approval threshold ({}c)",
                finance_cents
            ));
            violations.push(PolicyViolation {
                policy_id: "deal-value-cap".to_string(),
                reason: format!("Deal value exceeds {}c threshold", finance_cents),
                required_approval: Some("finance".to_string()),
            });
        }
    }

    if input.minimum_margin_pct < thresholds.margin_floor_pct {
        reasons.push(format!("Margin floor breached (below {}%)", thresholds.margin_floor_pct));
        violations.push(PolicyViolation {
            policy_id: "margin-floor".to_string(),
            reason: format!("Minimum margin is below {}%", thresholds.margin_floor_pct),
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

    use super::{
        evaluate_policy_input, evaluate_policy_with_thresholds, PolicyInput, PolicyThresholds,
    };

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

    #[test]
    fn custom_thresholds_lower_manager_trigger() {
        // Lower manager threshold to 10% — a 15% discount should now trigger
        let thresholds = PolicyThresholds {
            manager_discount_pct: Decimal::new(1000, 2),
            ..PolicyThresholds::default()
        };
        let result = evaluate_policy_with_thresholds(
            &PolicyInput {
                requested_discount_pct: Decimal::new(1500, 2),
                deal_value: Decimal::new(50_000, 2),
                minimum_margin_pct: Decimal::new(2000, 2),
            },
            &thresholds,
        );
        assert!(result.approval_required);
        assert!(result.violations.iter().any(|v| v.policy_id == "discount-cap"
            && v.required_approval == Some("sales_manager".to_string())));
    }

    #[test]
    fn custom_thresholds_raise_vp_trigger() {
        // Raise VP threshold to 50% — a 35% discount should only trigger manager
        let thresholds = PolicyThresholds {
            vp_discount_pct: Decimal::new(5000, 2),
            ..PolicyThresholds::default()
        };
        let result = evaluate_policy_with_thresholds(
            &PolicyInput {
                requested_discount_pct: Decimal::new(3500, 2),
                deal_value: Decimal::new(50_000, 2),
                minimum_margin_pct: Decimal::new(2000, 2),
            },
            &thresholds,
        );
        assert!(result.approval_required);
        assert!(result
            .violations
            .iter()
            .any(|v| v.required_approval == Some("sales_manager".to_string())));
        assert!(!result
            .violations
            .iter()
            .any(|v| v.required_approval == Some("vp_finance".to_string())));
    }

    #[test]
    fn finance_deal_value_threshold_triggers_when_set() {
        let thresholds = PolicyThresholds {
            finance_deal_value_cents: Some(100_000), // $1,000.00
            ..PolicyThresholds::default()
        };
        let result = evaluate_policy_with_thresholds(
            &PolicyInput {
                requested_discount_pct: Decimal::new(500, 2),
                deal_value: Decimal::new(200_000, 2), // $2,000.00
                minimum_margin_pct: Decimal::new(2000, 2),
            },
            &thresholds,
        );
        assert!(result.approval_required);
        assert!(result.violations.iter().any(|v| v.policy_id == "deal-value-cap"));
    }
}
