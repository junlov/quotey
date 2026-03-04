# W2 Pricing Rule Builder Implementation (quotey-004-2)

## Objective
Implement deterministic translation from visual rule-builder payloads into typed pricing-rule
drafts that runtime/storage layers can persist and evaluate.

## Implementation
Added:
- `crates/core/src/cpq/rule_builder.rs`

Core API:
- `build_pricing_rule(visual_rule: &VisualRuleDefinition) -> Result<PricingRuleDraft, PricingRuleBuilderError>`

Supported pricing actions:
1. `set_unit_price`:
   - required parameter: `amount`
   - optional parameter: `currency` (defaults to `USD`, normalized uppercase)
2. `apply_discount_cap`:
   - required parameter: `max_discount_pct`

The translator enforces:
- valid visual schema (`visual_rule.v1`) via `VisualRuleDefinition::validate()`
- `rule_type == pricing`
- action parameter presence and decimal parsing
- explicit rejection of unsupported actions

## Output Shape
`PricingRuleDraft` includes:
- identity and ordering: `id`, `name`, `enabled`, `priority`
- normalized conditions (`PricingRuleCondition`)
- typed action (`PricingRuleAction`)

## Deterministic Guarantees
1. No LLM dependency:
   translation is pure deterministic Rust.
2. Stable value handling:
   JSON values are canonicalized to string form for condition transport.
3. Fail-closed on invalid input:
   missing/invalid parameters become typed errors.

## Follow-On
1. Hook builder into Slack modal submit handling (`/quotey rules` flow).
2. Persist `PricingRuleDraft` into SQLite rule tables.
3. Add preview simulation endpoint to evaluate a draft against sample quote payloads.
