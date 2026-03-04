# W2 Analytics Query Implementation (quotey-009-2)

## Objective
Implement deterministic analytics SQL query generation from the `analytics_contract.v1`
metric/dimension spec.

## Implementation
Added:
- `crates/db/src/repositories/analytics.rs`

Core builder:
- `SqlAnalyticsQueryBuilder::build_query(spec: &AnalyticsQuerySpec) -> Result<String, AnalyticsQueryError>`

## Query Behavior
1. Validates `AnalyticsQuerySpec` before generating SQL.
2. Maps each dimension to deterministic SQLite expressions and stable aliases.
3. Maps each metric to explicit aggregate expressions.
4. Applies lookback filter:
   - `q.created_at >= datetime('now', '-{lookback_days} days')`
5. Applies finalized-only filter when requested.
6. Adds `GROUP BY`/`ORDER BY` when dimensions are present.

## Validation
Unit tests cover:
1. metrics+dimensions+filters are rendered into SQL,
2. no-dimension queries skip grouping clauses.
