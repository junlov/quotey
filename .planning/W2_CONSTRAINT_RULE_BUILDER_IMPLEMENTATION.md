# W2 Constraint Rule Builder Implementation (quotey-004-3)

## Objective
Implement deterministic translation from visual constraint-rule payloads into typed
constraint rule drafts for runtime/storage integration.

## Implementation
Added:
- `crates/core/src/cpq/constraint_rule_builder.rs`

Core API:
- `build_constraint_rule(visual_rule: &VisualRuleDefinition) -> Result<ConstraintRuleDraft, ConstraintRuleBuilderError>`

Supported constraint actions:
1. `require_product`
   - required parameter: `required_product_id`
2. `exclude_product`
   - required parameter: `excluded_product_id`

Supported condition operators:
- `equals`
- `not_equals`
- `contains`
- `in`

Unsupported operators fail closed with typed errors.

## Output Shape
`ConstraintRuleDraft` includes:
- identity and ordering (`id`, `name`, `enabled`, `priority`)
- normalized condition list (`ConstraintRuleCondition`)
- typed action (`ConstraintRuleAction`)

## Deterministic Guarantees
1. Pure translation path with no LLM dependency.
2. Visual schema validation required before conversion.
3. Required parameter and operator checks fail closed.
4. Condition values normalized to canonical string transport form.

## Follow-On
1. Route Slack modal submit payloads into `build_constraint_rule`.
2. Persist drafts into constraint rule storage tables.
3. Add preview validation against `DeterministicConstraintEngine`.
