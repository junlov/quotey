# RCH-02: Rules Architecture and Evaluation Order

**Research Task:** `bd-3d8.11.3`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** LilacMountain (Codex)  
**Primary Inputs:** `.planning/PROJECT.md`, `.planning/ARCHITECTURE_DECISION_RESEARCH.md`

---

## Executive Summary

This research finalizes a deterministic rule representation and evaluation order for Quotey's constraint, pricing, and policy engines.

Decision:

- Adopt a **stage + priority + deterministic tie-break** rule model.
- Keep rules **database-native** in SQLite with structured JSON for condition/effect payloads.
- Introduce a **ruleset versioning layer** so every quote evaluation is replayable against the exact rule snapshot used at pricing time.

Why this is the selected model:

1. It is deterministic and explainable.
2. It maps directly to SQLite-backed admin workflows.
3. It supports full replay/audit requirements without introducing runtime DSL complexity too early.

---

## 1. Objective and Acceptance Mapping

Required deliverables from `bd-3d8.11.3`:

1. Rule storage model options and tradeoffs.
2. Deterministic evaluation pipeline and precedence rules.
3. Versioning and migration strategy for rule changes.

Acceptance criteria mapping:

- audit explainability and replay support: Sections 4, 6, and 7.
- evaluation order proven with concrete examples: Section 5.
- blast-radius concerns and mitigations documented: Section 8.

---

## 2. Decision Drivers

1. Deterministic outcomes for the same input context.
2. Explainability at rule/stage level (human-auditable pricing and policy behavior).
3. Local-first operational simplicity (SQLite + CLI management).
4. Safe evolution of rules without invalidating prior quote audits.
5. Clear ownership boundaries (rule authoring vs rule execution).

---

## 3. Rule Storage Model Options and Tradeoffs

### Option A: Stage + Priority Relational Model with Structured JSON (Selected)

Core idea:

- Rules live in typed SQLite tables (`constraint_rule`, `pricing_formula`, `discount_policy`, `approval_threshold`) with common deterministic metadata.
- Condition/effect payload remains structured JSON to avoid excessive schema churn for each new rule flavor.

Pros:

1. Strongly aligned with existing `.planning/PROJECT.md` schema direction.
2. Easy to lint and validate with CLI tooling.
3. Deterministic ordering can be encoded explicitly (`stage`, `priority`, `rule_id` tie-break).
4. Replay is straightforward with ruleset version snapshots.

Cons:

1. Requires a shared metadata contract across rule families.
2. JSON payload validation must be enforced at write-time (not only runtime).

### Option B: Runtime DSL/Expression Engine as Primary Rule Representation

Core idea:

- Store rules as a high-level expression DSL and evaluate via parser/interpreter.

Pros:

1. Flexible rule authoring semantics.
2. Potentially concise rule definitions.

Cons:

1. Higher complexity and larger correctness surface.
2. Harder to guarantee deterministic ordering unless extra metadata is still added.
3. Raises security and validation complexity for expression execution.

### Option C: YAML/Code-Configured Rules Outside SQLite

Core idea:

- Keep rules in files or code and load at runtime.

Pros:

1. Familiar for engineers.
2. Simpler for very small systems.

Cons:

1. Violates project requirement that rules/policies are manageable in SQLite without recompilation.
2. Weaker operational audit trail for non-engineering operators.
3. Higher drift risk between deployed binaries and rule files.

### Selected Model

Option A is selected.

---

## 4. Canonical Deterministic Rule Representation

### 4.1 Common Metadata Contract (all rule families)

Each rule record should expose these deterministic fields (either physically or as a normalized view):

1. `rule_id` (stable unique ID)
2. `rule_family` (`constraint`, `pricing_base`, `pricing_adjustment`, `discount_policy`, `approval_threshold`)
3. `stage` (integer, lower executes earlier)
4. `priority` (integer, higher evaluated first within same stage)
5. `specificity_score` (derived or stored; more specific wins ties)
6. `conflict_strategy` (enum; see 4.3)
7. `active_from`, `active_to`
8. `ruleset_version_id`
9. `condition_json`
10. `effect_json`
11. `enabled` flag

### 4.2 Global Ordering Function

Evaluation order key:

`(stage ASC, priority DESC, specificity_score DESC, rule_id ASC)`

This key must be the same in every execution path (runtime, preview tool, replay verifier).

### 4.3 Conflict Strategies by Rule Family

1. `constraint`: `collect_all_violations` (no silent override)
2. `pricing_base`: `single_winner_required` (exactly one winning base-price rule per line/context)
3. `pricing_adjustment`: `ordered_compose` (apply all matched adjustments in deterministic order)
4. `discount_policy`: `most_restrictive_wins` (lower allowed cap dominates)
5. `approval_threshold`: `highest_authority_wins` (max required authority among matched thresholds)

Conflict strategy must be explicit and persisted, never inferred implicitly.

---

## 5. Deterministic Evaluation Pipeline and Concrete Examples

### 5.1 Unified Stage Pipeline

Recommended stage map:

1. `S10 Context normalization`  
Normalize quote context (segment, region, currency, term, catalog snapshot refs).

2. `S20 Hard constraints`  
Evaluate `requires/excludes/attribute/quantity/cross-product` constraints.

3. `S30 Base price selection`  
Resolve price book + base price per line.

4. `S40 Pricing adjustments`  
Apply volume tiers, bundles, and formula adjustments.

5. `S50 Requested discount application`  
Apply requested discounts in normalized form.

6. `S60 Policy enforcement`  
Evaluate discount caps, margin floors, product policies.

7. `S70 Approval threshold routing`  
Determine approval requirement and required role(s).

8. `S80 Trace finalization`  
Persist explainability payload and deterministic summary outputs.

Short-circuit rule:

- Fail fast only for hard blocking errors (e.g., unresolved hard constraint, missing base price).
- Otherwise continue and collect complete policy/approval output.

### 5.2 Example A: Deterministic Base Price + Tier Selection

Input:

- Product: `plan_pro_v2`
- Segment: `enterprise`
- Region: `us`
- Quantity: `150`

Matching rules:

1. Base price rule `pb_enterprise_us` (`stage=30`, `priority=100`)
2. Base price fallback `pb_global_default` (`stage=30`, `priority=10`)
3. Volume tier `100+ => $6.00` (`stage=40`, `priority=80`)

Deterministic result:

1. `pb_enterprise_us` wins at S30 due to higher priority + higher specificity.
2. S40 applies 100+ tier rule.
3. Final unit price is deterministic regardless of insertion order in DB.

### 5.3 Example B: Conflicting Discount Policies

Input:

- Requested discount: `18%`
- Policies matched:
  - `enterprise_general_cap = 20%` (`stage=60`, priority=50)
  - `security_bundle_cap = 12%` (`stage=60`, priority=70)

Conflict strategy:

- `most_restrictive_wins` for `discount_policy`.

Deterministic result:

- Effective auto-approval cap is `12%`.
- Requested `18%` triggers approval requirement.
- Trace records both matched policies and selected effective cap.

### 5.4 Example C: Approval Threshold Resolution

Input:

- Discount over auto-cap: yes
- Deal total: `$620,000`

Matched thresholds:

1. `deal_value > 100k => deal_desk`
2. `deal_value > 500k => vp_sales`
3. `discount > 30% => cfo` (not matched in this example)

Conflict strategy:

- `highest_authority_wins`.

Deterministic result:

- Required approver role = `vp_sales`.
- If multiple chains are configured, route resolution is deterministic by configured policy mode (`sequential` or `parallel`) and trace captures selection.

---

## 6. Explainability and Replay Contract

### 6.1 Minimum Trace Fields Per Rule Decision

Each rule decision in trace should include:

1. `stage`
2. `rule_family`
3. `rule_id`
4. `ruleset_version_id`
5. `match_result` (`matched`, `skipped`, `blocked`, `error`)
6. `inputs_excerpt`
7. `effect_applied`
8. `conflict_resolution_note` (if applicable)
9. `timestamp`

### 6.2 Replay Guarantees

Replay requirement for any priced quote:

1. Quote references immutable pricing snapshot.
2. Snapshot includes rule decision trace with `ruleset_version_id`.
3. Replayer loads same ruleset version and input context.
4. Replayed output must match stored totals and stage decisions byte-for-byte for canonical fields.

If mismatch occurs, classify as high-severity determinism regression.

---

## 7. Versioning and Migration Strategy for Rule Changes

### 7.1 Ruleset Versioning Model

Add a top-level ruleset version registry:

`ruleset_version(id, label, created_at, activation_ts, status[draft|active|retired], notes)`

All rule families reference `ruleset_version_id`.

Operational policy:

1. Rules edited in `draft` ruleset.
2. Validation/lint + preview evaluation required before activation.
3. Activation is atomic and timestamped.
4. Existing quotes keep historical rule references through stored snapshots.

### 7.2 Migration Strategy

Phase 1 (non-breaking):

1. Add nullable deterministic metadata fields (`stage`, `conflict_strategy`, `ruleset_version_id`) to relevant rule tables.
2. Backfill defaults from current behavior assumptions.
3. Add indexes supporting deterministic queries.

Phase 2 (behavior freeze):

1. Enforce not-null constraints for required metadata.
2. Enable runtime checks rejecting rules missing stage/conflict config.
3. Require trace payload to include rule decision metadata.

Phase 3 (operator tooling):

1. Add CLI lint command for stage gaps/conflicts.
2. Add preview command to run sample contexts before activation.

### 7.3 Compatibility and Rollout Guardrails

1. Never mutate existing active ruleset in place; create new version.
2. Do not delete historical ruleset versions referenced by snapshots.
3. Activation requires dry-run against golden scenario corpus.

---

## 8. Blast Radius Concerns and Mitigations

| Risk | Failure Mode | Impact | Mitigation |
|---|---|---|---|
| Stage misconfiguration | Incorrect stage ordering | Wrong price/policy decisions | Lint checks + explicit stage enum + tests |
| Hidden tie collisions | Nondeterministic winner selection | Replay mismatch, approval drift | Global ordering key with deterministic tie-break |
| In-place policy edits | Historical quote reinterpretation | Audit non-reproducibility | Immutable ruleset versions and snapshot binding |
| Missing trace details | Opaque outcomes | Support/legal/audit friction | Mandatory per-stage rule decision trace |
| Partial rollout | Different workers use different rule sets | Inconsistent behavior | Atomic activation marker + cache invalidation by version id |
| Over-broad rule scope | Unintended policy enforcement | Revenue/compliance errors | Scope validation + preview command + canary test contexts |

---

## 9. Implementation Handoff Notes

### For `bd-3d8.4` (Domain contracts)

Define domain primitives:

1. `RuleStage`
2. `RuleFamily`
3. `ConflictStrategy`
4. `RuleDecisionTrace`
5. `RulesetVersionId`

### For `bd-3d8.7` (CPQ core stubs)

Evaluator interfaces should explicitly accept:

1. normalized context
2. ruleset version
3. deterministic ordering strategy

and return:

1. computed outputs
2. rule decision trace
3. blocking/non-blocking violation set

### For `bd-3d8.11.10` (Decision freeze)

Freeze these as accepted defaults unless explicitly superseded:

1. stage+priority+tie-break ordering key
2. conflict strategy by rule family
3. immutable ruleset versioning for replay

---

## 10. Done Criteria Mapping

Deliverable: Rule storage options and tradeoffs  
Completed: Section 3.

Deliverable: Deterministic pipeline and precedence rules  
Completed: Sections 4 and 5.

Deliverable: Versioning and migration strategy  
Completed: Section 7.

Acceptance: Explainability and replay support  
Completed: Section 6.

Acceptance: Evaluation order proven with concrete examples  
Completed: Section 5.2, 5.3, 5.4.

Acceptance: Blast-radius concerns and mitigations  
Completed: Section 8.

