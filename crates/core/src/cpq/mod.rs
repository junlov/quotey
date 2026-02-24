pub mod catalog;
pub mod constraints;
pub mod policy;
pub mod precedent;
pub mod pricing;
pub mod simulator;

use crate::domain::quote::Quote;
use serde::{Deserialize, Serialize};

use self::{
    constraints::{
        ConstraintEngine, ConstraintInput, ConstraintResult, DeterministicConstraintEngine,
    },
    policy::{DeterministicPolicyEngine, PolicyDecision, PolicyEngine, PolicyInput},
    pricing::{DeterministicPricingEngine, PricingEngine, PricingResult},
};

#[derive(Clone, Debug)]
pub struct CpqEvaluationInput<'a> {
    pub quote: &'a Quote,
    pub currency: &'a str,
    pub policy_input: PolicyInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpqEvaluation {
    pub constraints: ConstraintResult,
    pub pricing: PricingResult,
    pub policy: PolicyDecision,
}

pub trait CpqRuntime: Send + Sync {
    fn evaluate_quote(&self, input: CpqEvaluationInput<'_>) -> CpqEvaluation;
}

pub struct DeterministicCpqRuntime<C, P, O> {
    constraint_engine: C,
    pricing_engine: P,
    policy_engine: O,
}

impl<C, P, O> DeterministicCpqRuntime<C, P, O> {
    pub fn new(constraint_engine: C, pricing_engine: P, policy_engine: O) -> Self {
        Self { constraint_engine, pricing_engine, policy_engine }
    }
}

impl Default
    for DeterministicCpqRuntime<
        DeterministicConstraintEngine,
        DeterministicPricingEngine,
        DeterministicPolicyEngine,
    >
{
    fn default() -> Self {
        Self::new(
            DeterministicConstraintEngine,
            DeterministicPricingEngine,
            DeterministicPolicyEngine,
        )
    }
}

impl<C, P, O> CpqRuntime for DeterministicCpqRuntime<C, P, O>
where
    C: ConstraintEngine,
    P: PricingEngine,
    O: PolicyEngine,
{
    fn evaluate_quote(&self, input: CpqEvaluationInput<'_>) -> CpqEvaluation {
        let constraints = self
            .constraint_engine
            .validate(&ConstraintInput { quote_lines: input.quote.lines.clone() });
        let pricing = self.pricing_engine.price(input.quote, input.currency);
        let policy = self.policy_engine.evaluate(&input.policy_input);

        CpqEvaluation { constraints, pricing, policy }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;

    use crate::{
        cpq::{
            constraints::{ConstraintEngine, ConstraintResult, DeterministicConstraintEngine},
            policy::{DeterministicPolicyEngine, PolicyDecision, PolicyEngine, PolicyInput},
            pricing::{DeterministicPricingEngine, PricingEngine, PricingResult},
            CpqEvaluationInput, CpqRuntime, DeterministicCpqRuntime,
        },
        domain::{
            approval::ApprovalStatus,
            product::ProductId,
            quote::{Quote, QuoteId, QuoteLine, QuoteStatus},
        },
    };

    #[test]
    fn deterministic_cpq_runtime_returns_all_three_engine_outputs() {
        let runtime = DeterministicCpqRuntime::new(
            DeterministicConstraintEngine,
            DeterministicPricingEngine,
            DeterministicPolicyEngine,
        );

        let quote = quote_fixture();
        let result = runtime.evaluate_quote(CpqEvaluationInput {
            quote: &quote,
            currency: "USD",
            policy_input: PolicyInput {
                requested_discount_pct: Decimal::new(500, 2),
                deal_value: Decimal::new(100_000, 2),
                minimum_margin_pct: Decimal::new(4000, 2),
            },
        });

        assert!(result.constraints.valid);
        assert!(result.pricing.total > Decimal::ZERO);
        assert_eq!(result.policy.approval_status, ApprovalStatus::Approved);
    }

    #[test]
    fn runtime_supports_explicit_engine_interfaces() {
        #[derive(Default)]
        struct TestConstraintEngine;

        impl ConstraintEngine for TestConstraintEngine {
            fn validate(
                &self,
                _input: &crate::cpq::constraints::ConstraintInput,
            ) -> ConstraintResult {
                ConstraintResult { valid: false, violations: Vec::new() }
            }
        }

        #[derive(Default)]
        struct TestPricingEngine;

        impl PricingEngine for TestPricingEngine {
            fn price(&self, quote: &Quote, _currency: &str) -> PricingResult {
                crate::cpq::pricing::price_quote_with_trace(quote, "USD")
            }
        }

        #[derive(Default)]
        struct TestPolicyEngine;

        impl PolicyEngine for TestPolicyEngine {
            fn evaluate(&self, _input: &PolicyInput) -> PolicyDecision {
                crate::cpq::policy::evaluate_policy()
            }
        }

        let runtime =
            DeterministicCpqRuntime::new(TestConstraintEngine, TestPricingEngine, TestPolicyEngine);

        let quote = quote_fixture();
        let result = runtime.evaluate_quote(CpqEvaluationInput {
            quote: &quote,
            currency: "USD",
            policy_input: PolicyInput {
                requested_discount_pct: Decimal::ZERO,
                deal_value: Decimal::new(100_000, 2),
                minimum_margin_pct: Decimal::new(4000, 2),
            },
        });

        assert!(!result.constraints.valid);
    }

    fn quote_fixture() -> Quote {
        Quote {
            id: QuoteId("Q-2026-0001".to_owned()),
            status: QuoteStatus::Draft,
            lines: vec![QuoteLine {
                product_id: ProductId("plan-pro".to_owned()),
                quantity: 10,
                unit_price: Decimal::new(9_999, 2),
            }],
            created_at: Utc::now(),
        }
    }
}
