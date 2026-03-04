# W2 Analytics Metrics and Dimensions (quotey-009-1)

## Objective
Define deterministic analytics contracts for metric and dimension selection before
query/dashboard implementation beads.

## Contract
Implemented in:
- `crates/core/src/domain/analytics.rs`

Schema version:
- `analytics_contract.v1`

### Metrics
- `quote_count`
- `win_rate_pct`
- `avg_discount_pct`
- `avg_deal_value`
- `approval_cycle_hours`
- `time_to_finalize_hours`
- `anomaly_rate_pct`

### Dimensions
- time: `day`, `week`, `month`, `quarter`
- business: `customer_segment`, `industry`, `region`, `sales_rep`, `product_family`, `approval_role`

### Query Spec
`AnalyticsQuerySpec`:
- `schema_version`
- `metrics[]` (required, at least one)
- `dimensions[]`
- `lookback_days` (1..=3650)
- `include_only_finalized`

Validation is fail-closed via `AnalyticsQuerySpec::validate()`.

## Follow-On
1. `quotey-009-2`: map these contracts to SQL query builders.
2. `quotey-009-3`: dashboard UI consumes this contract for metric/dimension picker options.
