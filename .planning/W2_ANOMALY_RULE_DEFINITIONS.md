# W2 Anomaly Rule Definitions (quotey-006-1)

## Objective
Define deterministic anomaly detection rules and thresholds that can be explained in plain
language and enforced consistently before quote finalization.

## Rule Thresholds
Implemented defaults (`AnomalyRuleThresholds`) in `crates/core/src/cpq/anomaly.rs`:

1. Discount anomaly
   - warning: `requested_discount > avg_discount * 1.5`
   - flag: `requested_discount > avg_discount * 2.0 + std_dev`
   - critical: `requested_discount > avg_discount * 3.0`
2. Margin anomaly
   - warning: `margin < floor + 5%`
   - critical: `margin < floor`
3. Quantity anomaly
   - flag: `requested_quantity > customer_avg_quantity * 3.0`
4. Price anomaly
   - flag: `quote_total > similar_deals_avg_total * 1.5`

## Deterministic Contract
Added threshold-rule evaluator:
- `AnomalyRuleSet`
  - `evaluate_discount(...)`
  - `evaluate_margin(...)`
  - `evaluate_quantity(...)`
  - `evaluate_price(...)`

Result contract:
- `AnomalyRuleHit { rule, severity, reason }`
- `AnomalyRuleKind { Discount, Margin, Quantity, Price }`

## Relationship to Existing Detector
The existing z-score `AnomalyDetector` remains intact for statistical scoring.
The new `AnomalyRuleSet` adds explicit threshold policy definitions for:
- clear business-facing rules,
- deterministic alert rationale,
- easier policy tuning/review.

## Validation
Unit tests added to verify:
1. discount/margin rule severities map to expected thresholds.
2. quantity/price outlier thresholds trigger correctly.
