//! Boundary calculator and walk-away guardrails for NXT negotiation autopilot.
//!
//! Provides margin/discount floor calculators and a stop-reason taxonomy.
//! All evaluations are deterministic given the same inputs.

use serde::{Deserialize, Serialize};

use crate::domain::negotiation::BoundaryEvaluation;

// ---------------------------------------------------------------------------
// Stop reason taxonomy
// ---------------------------------------------------------------------------

/// Categorized stop reasons for blocked or escalated negotiations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReasonCategory {
    /// Pricing floor breached (margin or discount hard limit).
    PricingFloor,
    /// Policy constraint violated (e.g., product bundling rules).
    PolicyViolation,
    /// Approval threshold exceeded — escalation required.
    ApprovalRequired,
    /// Maximum concession budget exhausted.
    ConcessionBudgetExhausted,
    /// Maximum turns reached for this session.
    MaxTurnsReached,
    /// Session expired.
    SessionExpired,
    /// Walk-away triggered (business case no longer viable).
    WalkAway,
}

impl StopReasonCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PricingFloor => "pricing_floor",
            Self::PolicyViolation => "policy_violation",
            Self::ApprovalRequired => "approval_required",
            Self::ConcessionBudgetExhausted => "concession_budget_exhausted",
            Self::MaxTurnsReached => "max_turns_reached",
            Self::SessionExpired => "session_expired",
            Self::WalkAway => "walk_away",
        }
    }

    pub fn parse_label(s: &str) -> Option<Self> {
        match s {
            "pricing_floor" => Some(Self::PricingFloor),
            "policy_violation" => Some(Self::PolicyViolation),
            "approval_required" => Some(Self::ApprovalRequired),
            "concession_budget_exhausted" => Some(Self::ConcessionBudgetExhausted),
            "max_turns_reached" => Some(Self::MaxTurnsReached),
            "session_expired" => Some(Self::SessionExpired),
            "walk_away" => Some(Self::WalkAway),
            _ => None,
        }
    }
}

/// A structured stop reason with category and detail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StopReason {
    pub category: StopReasonCategory,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Floor calculators
// ---------------------------------------------------------------------------

/// Margin floor configuration per product category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarginFloorPolicy {
    pub category: String,
    /// Hard floor: below this, deal is blocked (walk-away).
    pub hard_floor_pct: f64,
    /// Soft floor: below this, approval is required.
    pub soft_floor_pct: f64,
}

/// Discount ceiling configuration per product category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiscountCeilingPolicy {
    pub category: String,
    /// Hard ceiling: above this, deal is blocked.
    pub hard_ceiling_pct: f64,
    /// Soft ceiling: above this, approval is required.
    pub soft_ceiling_pct: f64,
}

/// Input to the boundary calculator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundaryInput {
    pub margin_pct: f64,
    pub discount_pct: f64,
    pub product_category: String,
    pub turn_count: u32,
    pub max_turns: u32,
}

/// Deterministic boundary calculator.
#[derive(Debug, Clone)]
pub struct BoundaryCalculator {
    margin_policies: Vec<MarginFloorPolicy>,
    discount_policies: Vec<DiscountCeilingPolicy>,
}

impl BoundaryCalculator {
    pub fn new(
        margin_policies: Vec<MarginFloorPolicy>,
        discount_policies: Vec<DiscountCeilingPolicy>,
    ) -> Self {
        Self { margin_policies, discount_policies }
    }

    /// Evaluate boundaries for a negotiation position.
    pub fn evaluate(&self, input: &BoundaryInput) -> (BoundaryEvaluation, Vec<StopReason>) {
        let mut stop_reasons = Vec::new();
        let mut floor_breached = false;
        let mut ceiling_breached = false;
        let mut walk_away = false;
        let mut requires_approval = false;

        // Check margin floor
        if let Some(margin_policy) =
            self.margin_policies.iter().find(|p| p.category == input.product_category)
        {
            if input.margin_pct < margin_policy.hard_floor_pct {
                floor_breached = true;
                walk_away = true;
                stop_reasons.push(StopReason {
                    category: StopReasonCategory::PricingFloor,
                    message: format!(
                        "margin {:.1}% below hard floor {:.1}% for category {}",
                        input.margin_pct, margin_policy.hard_floor_pct, input.product_category
                    ),
                });
            } else if input.margin_pct < margin_policy.soft_floor_pct {
                requires_approval = true;
                stop_reasons.push(StopReason {
                    category: StopReasonCategory::ApprovalRequired,
                    message: format!(
                        "margin {:.1}% below soft floor {:.1}% for category {}",
                        input.margin_pct, margin_policy.soft_floor_pct, input.product_category
                    ),
                });
            }
        }

        // Check discount ceiling
        if let Some(discount_policy) =
            self.discount_policies.iter().find(|p| p.category == input.product_category)
        {
            if input.discount_pct > discount_policy.hard_ceiling_pct {
                ceiling_breached = true;
                stop_reasons.push(StopReason {
                    category: StopReasonCategory::PricingFloor,
                    message: format!(
                        "discount {:.1}% exceeds hard ceiling {:.1}% for category {}",
                        input.discount_pct,
                        discount_policy.hard_ceiling_pct,
                        input.product_category
                    ),
                });
            } else if input.discount_pct > discount_policy.soft_ceiling_pct {
                requires_approval = true;
                stop_reasons.push(StopReason {
                    category: StopReasonCategory::ApprovalRequired,
                    message: format!(
                        "discount {:.1}% exceeds soft ceiling {:.1}% for category {}",
                        input.discount_pct,
                        discount_policy.soft_ceiling_pct,
                        input.product_category
                    ),
                });
            }
        }

        // Check max turns
        if input.turn_count >= input.max_turns {
            stop_reasons.push(StopReason {
                category: StopReasonCategory::MaxTurnsReached,
                message: format!(
                    "turn {} of {} maximum reached",
                    input.turn_count, input.max_turns
                ),
            });
        }

        let within_bounds = !floor_breached && !ceiling_breached;
        let boundary_stop_strings = stop_reasons.iter().map(|r| r.message.clone()).collect();

        let eval = BoundaryEvaluation {
            within_bounds,
            floor_breached,
            ceiling_breached,
            walk_away,
            requires_approval,
            stop_reasons: boundary_stop_strings,
        };

        (eval, stop_reasons)
    }
}

impl Default for BoundaryCalculator {
    fn default() -> Self {
        Self::new(
            vec![
                MarginFloorPolicy {
                    category: "software".to_string(),
                    hard_floor_pct: 15.0,
                    soft_floor_pct: 25.0,
                },
                MarginFloorPolicy {
                    category: "services".to_string(),
                    hard_floor_pct: 20.0,
                    soft_floor_pct: 30.0,
                },
            ],
            vec![
                DiscountCeilingPolicy {
                    category: "software".to_string(),
                    hard_ceiling_pct: 40.0,
                    soft_ceiling_pct: 25.0,
                },
                DiscountCeilingPolicy {
                    category: "services".to_string(),
                    hard_ceiling_pct: 30.0,
                    soft_ceiling_pct: 20.0,
                },
            ],
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn calc() -> BoundaryCalculator {
        BoundaryCalculator::default()
    }

    fn input(margin: f64, discount: f64, category: &str) -> BoundaryInput {
        BoundaryInput {
            margin_pct: margin,
            discount_pct: discount,
            product_category: category.to_string(),
            turn_count: 1,
            max_turns: 20,
        }
    }

    #[test]
    fn within_bounds_passes() {
        let (eval, stops) = calc().evaluate(&input(40.0, 10.0, "software"));
        assert!(eval.within_bounds);
        assert!(!eval.walk_away);
        assert!(!eval.requires_approval);
        assert!(stops.is_empty());
    }

    #[test]
    fn margin_hard_floor_breach_triggers_walk_away() {
        let (eval, stops) = calc().evaluate(&input(10.0, 10.0, "software"));
        assert!(!eval.within_bounds);
        assert!(eval.floor_breached);
        assert!(eval.walk_away);
        assert_eq!(stops[0].category, StopReasonCategory::PricingFloor);
    }

    #[test]
    fn margin_soft_floor_requires_approval() {
        let (eval, stops) = calc().evaluate(&input(20.0, 10.0, "software"));
        assert!(eval.within_bounds);
        assert!(eval.requires_approval);
        assert_eq!(stops[0].category, StopReasonCategory::ApprovalRequired);
    }

    #[test]
    fn discount_hard_ceiling_breach() {
        let (eval, stops) = calc().evaluate(&input(40.0, 50.0, "software"));
        assert!(!eval.within_bounds);
        assert!(eval.ceiling_breached);
        assert!(stops.iter().any(|s| s.category == StopReasonCategory::PricingFloor));
    }

    #[test]
    fn discount_soft_ceiling_requires_approval() {
        let (eval, stops) = calc().evaluate(&input(40.0, 30.0, "software"));
        assert!(eval.within_bounds);
        assert!(eval.requires_approval);
        assert!(stops.iter().any(|s| s.category == StopReasonCategory::ApprovalRequired));
    }

    #[test]
    fn max_turns_reached_generates_stop_reason() {
        let mut inp = input(40.0, 10.0, "software");
        inp.turn_count = 20;
        inp.max_turns = 20;

        let (eval, stops) = calc().evaluate(&inp);
        assert!(eval.within_bounds); // turns don't affect bounds
        assert!(stops.iter().any(|s| s.category == StopReasonCategory::MaxTurnsReached));
    }

    #[test]
    fn unknown_category_passes_without_policy_check() {
        let (eval, stops) = calc().evaluate(&input(5.0, 90.0, "unknown"));
        assert!(eval.within_bounds);
        assert!(stops.is_empty());
    }

    #[test]
    fn services_category_uses_different_thresholds() {
        // Services has hard margin floor at 20%
        let (eval, _) = calc().evaluate(&input(18.0, 10.0, "services"));
        assert!(!eval.within_bounds);
        assert!(eval.walk_away);

        // Services has hard discount ceiling at 30%
        let (eval, _) = calc().evaluate(&input(40.0, 35.0, "services"));
        assert!(!eval.within_bounds);
        assert!(eval.ceiling_breached);
    }

    #[test]
    fn stop_reason_category_roundtrip() {
        let categories = [
            StopReasonCategory::PricingFloor,
            StopReasonCategory::PolicyViolation,
            StopReasonCategory::ApprovalRequired,
            StopReasonCategory::ConcessionBudgetExhausted,
            StopReasonCategory::MaxTurnsReached,
            StopReasonCategory::SessionExpired,
            StopReasonCategory::WalkAway,
        ];
        for c in &categories {
            let parsed = StopReasonCategory::parse_label(c.as_str()).unwrap();
            assert_eq!(&parsed, c);
        }
        assert!(StopReasonCategory::parse_label("nope").is_none());
    }

    #[test]
    fn deterministic_same_inputs_same_outputs() {
        let inp = input(22.0, 28.0, "software");
        let (e1, s1) = calc().evaluate(&inp);
        let (e2, s2) = calc().evaluate(&inp);
        assert_eq!(e1.within_bounds, e2.within_bounds);
        assert_eq!(e1.requires_approval, e2.requires_approval);
        assert_eq!(e1.walk_away, e2.walk_away);
        assert_eq!(s1.len(), s2.len());
    }
}
