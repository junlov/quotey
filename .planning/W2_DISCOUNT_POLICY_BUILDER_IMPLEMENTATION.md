# W2 Discount Policy Builder Implementation (quotey-004-4)

## Objective
Implement deterministic translation from visual discount-policy definitions into typed
policy drafts for approval routing and policy persistence layers.

## Implementation
Added:
- `crates/core/src/cpq/discount_policy_builder.rs`

Core API:
- `build_discount_policy(visual_rule: &VisualRuleDefinition) -> Result<DiscountPolicyDraft, DiscountPolicyBuilderError>`

Derived policy fields:
- `customer_segment` from `customer_segment` condition
- `product_category` from `product_category` condition
- `min_deal_value` from `deal_value` condition
- `max_discount_auto_approve_pct` from `apply_discount_cap(max_discount_pct)` action
- `max_discount_with_approval_pct` from:
  - `route_approval_role(max_discount_with_approval_pct)` OR
  - `set_approval_threshold(threshold_pct)`
- `required_approver_role` from `route_approval_role(approver_role)`

## Deterministic Guarantees
1. Fail-closed if schema validation fails or `rule_type != discount_policy`.
2. Required auto-approve cap must exist; missing cap returns typed error.
3. Decimal values are parsed strictly from numeric/string JSON values.
4. Builder is pure deterministic Rust with no LLM dependency.

## Follow-On
1. Persist `DiscountPolicyDraft` into `approval_authorities`/`routing_rules` mapping layer.
2. Wire Slack modal save flow to call the builder and return validation errors inline.
3. Add preview simulation for policy outcomes against sample discount/deal inputs.
