# W2 Anomaly Engine Implementation (quotey-006-2)

## Objective
Implement executable deterministic anomaly rule evaluation that can be called by higher-level
quote workflows before finalization.

## Implementation
Updated:
- `crates/core/src/cpq/anomaly.rs`

Added engine input contract:
- `AnomalyRuleEvaluationInput`
  - discount context (requested/average/std dev)
  - margin context (actual/floor)
  - quantity context (requested/average)
  - price context (quote total/similar-deal average)

Added detector API:
- `AnomalyDetector::evaluate_rules(&AnomalyRuleEvaluationInput) -> Vec<AnomalyRuleHit>`

Behavior:
1. Executes deterministic discount/margin/quantity/price checks via `AnomalyRuleSet`.
2. Returns typed hits (`AnomalyRuleHit`) for each triggered rule.
3. No side effects, no external dependencies, no LLM interaction.

## Validation
Added unit test:
- `detector_evaluate_rules_returns_typed_hits`
  - verifies all four rule kinds trigger on extreme input.
