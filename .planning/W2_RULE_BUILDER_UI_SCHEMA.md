# W2 Rule Builder UI Schema (quotey-004-1)

## Scope
Define a deterministic schema contract for visual no-code rule authoring across:
- pricing rules,
- constraint rules,
- discount policies,
- approval thresholds.

This artifact is the contract for Slack modal payloads and for storage adapters that persist
rule definitions to SQLite-backed CPQ/routing models.

## Deterministic Contract
Schema version: `visual_rule.v1`

Top-level shape (`VisualRuleDefinition`):
- `schema_version`: must equal `visual_rule.v1`
- `id`: stable rule identifier
- `name`: user-facing rule label
- `description`: optional long-form explanation
- `rule_type`: `pricing | constraint | discount_policy | approval_threshold`
- `enabled`: whether the rule is active
- `priority`: deterministic ordering key (higher priority wins)
- `conditions[]`: one or more `VisualRuleCondition`
- `actions[]`: one or more `VisualRuleAction`
- `metadata`: actor/tags/rationale context

Condition shape (`VisualRuleCondition`):
- `field_key`: canonical field name (for example `account_tier`, `product_category`)
- `operator`: `equals | not_equals | greater_than | greater_or_equal | less_than | less_or_equal | in | not_in | contains`
- `value`: JSON literal (string/number/boolean/array)
- `connector`: optional `and | or`; first condition must not set connector

Action shape (`VisualRuleAction`):
- `action_type`: `set_unit_price | apply_discount_cap | require_product | exclude_product | route_approval_role | set_approval_threshold`
- `parameters`: key/value payload needed by the action

Metadata shape (`VisualRuleMetadata`):
- `created_by`
- `updated_by`
- `tags[]`
- `rationale` (optional)

## Validation Rules
`VisualRuleDefinition::validate()` enforces:
1. schema version match (`visual_rule.v1`)
2. non-empty `id` and `name`
3. non-empty `conditions` and `actions`
4. first condition has no logical connector

Canonical serialization:
- `canonical_json()` emits deterministic JSON for hashing/signing and audit diffing.
- Action `parameters` uses `BTreeMap` for stable key ordering.

## Mapping to Current Runtime
This schema intentionally maps to existing deterministic engines and tables:

- `approval_threshold` + `route_approval_role`
  - maps into approval/routing primitives in:
    - `crates/core/src/approvals/mod.rs`
    - `migrations/0010_approval_routing.up.sql` (`approval_authorities`, `routing_rules`)
- `constraint` actions (`require_product`, `exclude_product`)
  - maps into deterministic constraint evaluation surfaces in:
    - `crates/core/src/cpq/constraints.rs`
- `pricing` + `discount_policy`
  - maps into deterministic pricing/policy execution surfaces in:
    - `crates/core/src/cpq/pricing.rs`
    - `crates/core/src/cpq/policy.rs`

## Follow-On Work
Subsequent beads should build on this contract:
1. Slack modal field schema and server-side payload parser.
2. Storage adapter translating visual actions into persisted rule records.
3. Preview/test endpoint that executes rules in simulation mode before save.
4. Audit event payloads that include canonical schema hash + actor metadata.
