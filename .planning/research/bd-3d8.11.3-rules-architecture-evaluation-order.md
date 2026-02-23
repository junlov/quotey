# RCH-02 (Foundation): Rules Architecture and Deterministic Evaluation Order

**Bead:** `bd-3d8.11.3`  
**Date:** 2026-02-23  
**Author:** IvoryBear (Codex)

---

## 1. Objective

Define a deterministic rules architecture for Quotey covering:
- storage model options and tradeoffs,
- evaluation pipeline and precedence,
- versioning and migration strategy,
- replay and audit explainability guarantees,
- blast-radius concerns and mitigations.

This artifact is scoped for foundation and decision-freeze prep.

---

## 2. Decision Summary (Recommended)

1. **Rule storage model:** relational stage-oriented schema in SQLite (`rule_set`, `rule`, `rule_condition`, `rule_action`).
2. **Evaluation ordering:** explicit stage pipeline + deterministic intra-stage ordering (`priority`, then `rule_id` tie-breaker).
3. **Conflict strategy:** per-stage strategy enum (`first_wins`, `max_effect`, `compose_additive`, `deny_overrides`).
4. **Versioning:** immutable ruleset snapshots referenced by quote operations (`ruleset_snapshot_id`).
5. **Migration approach:** additive schema evolution + compatibility views + explicit ruleset activation windows.
6. **Replay contract:** quote pricing/policy results are replayed using pinned `ruleset_snapshot_id` and inputs.

---

## 3. Rule Storage Model Options and Tradeoffs

## Option A: Hard-coded Rust rules

Description:
- Rule logic implemented in code and deployed with binaries.

Pros:
- Compile-time safety and straightforward unit tests.
- Easy local introspection for developers.

Cons:
- Requires deploy for rule edits.
- Poor fit for CPQ operations where policy changes frequently.
- Weak auditability for business-controlled policy evolution.

Verdict:
- Rejected for foundation.

## Option B: Flat JSON rule blobs in SQLite

Description:
- Store entire rule definitions as JSON blobs and evaluate dynamically.

Pros:
- Flexible schema changes.
- Fewer relational tables initially.

Cons:
- Harder SQL-level linting/validation.
- Weaker queryability for governance and audit.
- More runtime parsing failure modes.

Verdict:
- Not preferred for foundational deterministic engine.

## Option C: Relational stage-oriented schema (recommended)

Description:
- Use normalized tables with explicit stage and ordering metadata.

Pros:
- Strong governance/queryability.
- Deterministic execution order easy to enforce.
- Easier incremental validation in CLI/admin tools.
- Supports explainability and operational debugging.

Cons:
- More schema design upfront.
- Requires migration discipline.

Verdict:
- Recommended.

## Option D: Hybrid (relational metadata + expression blob)

Description:
- Stage/order in relational columns; condition/action payloads in structured JSON expressions.

Pros:
- Best balance between strict ordering and flexible expression language.

Cons:
- Requires expression validation layer.

Verdict:
- Recommended as incremental extension to Option C.

---

## 4. Recommended Canonical Data Model

## 4.1 Tables (logical)

1. `rule_set`
- `rule_set_id` (PK)
- `name`
- `domain` (`constraint`, `pricing`, `policy`, `approval`)
- `version`
- `status` (`draft`, `validated`, `active`, `retired`)
- `effective_from`, `effective_to`
- `created_at`, `created_by`

2. `rule`
- `rule_id` (PK)
- `rule_set_id` (FK)
- `stage` (enum/string)
- `priority` (int)
- `conflict_strategy` (nullable, stage override)
- `rule_type`
- `is_active`
- `name`, `description`
- `risk_level`

3. `rule_condition`
- `condition_id` (PK)
- `rule_id` (FK)
- `condition_index`
- `lhs_operand`
- `operator`
- `rhs_operand`
- `condition_group`

4. `rule_action`
- `action_id` (PK)
- `rule_id` (FK)
- `action_index`
- `action_type`
- `action_payload_json`

5. `rule_set_snapshot`
- `ruleset_snapshot_id` (PK)
- `rule_set_id` (FK)
- `snapshot_hash`
- `frozen_at`
- `frozen_by`

6. `rule_evaluation_trace`
- `trace_id` (PK)
- `quote_id`, `quote_version`
- `ruleset_snapshot_id`
- `stage`
- `rule_id`
- `decision` (`matched`, `skipped`, `applied`, `rejected`)
- `reason`
- `created_at`

## 4.2 Why this structure works for CPQ

- Deterministic: stage + priority ordering encoded directly.
- Explainable: each stage and rule decision captured in trace rows.
- Replayable: pinned ruleset snapshot allows exact re-evaluation.
- Governable: SQL queries can detect gaps, conflicts, and risky overrides.

---

## 5. Deterministic Evaluation Pipeline

## 5.1 Stage Pipeline (Canonical)

1. **Eligibility & hard constraints**
- reject impossible configurations early.

2. **Base price selection**
- resolve price book and base unit price.

3. **Contextual adjustments**
- apply segment, term, volume, bundle adjustments.

4. **Discount policy checks**
- enforce caps/floors and policy constraints.

5. **Approval threshold checks**
- decide if approval route is required.

6. **Final normalization and trace assembly**
- apply final rounding and write trace summary.

## 5.2 Intra-stage ordering

Deterministic order within each stage:
1. ascending `priority` (lower number runs first),
2. ascending `rule_id` as stable tie-breaker.

No implicit DB iteration order is allowed.

## 5.3 Conflict strategy hierarchy

Resolution precedence:
1. rule-level conflict strategy (if set),
2. stage default strategy,
3. global fallback strategy.

Recommended stage defaults:
- constraints: `deny_overrides`
- base pricing: `first_wins`
- adjustments: `compose_additive`
- discounts: `max_effect` with caps
- approvals: `deny_overrides`

## 5.4 Pseudocode (reference)

```text
for stage in STAGE_ORDER:
  candidates = load_active_rules(stage, ruleset_snapshot_id)
  ordered = sort(candidates, by=[priority asc, rule_id asc])

  decisions = []
  for rule in ordered:
    match = eval_conditions(rule, context)
    if !match:
      trace(stage, rule, "skipped", reason="conditions_not_met")
      continue

    decision = resolve_conflict(rule, decisions, stage_defaults)
    if decision == "reject":
      trace(stage, rule, "rejected", reason=decision.reason)
      continue

    context = apply_action(rule, context)
    trace(stage, rule, "applied", reason=decision.reason)

  validate_stage_invariants(stage, context)

return finalize_trace_and_result(context)
```

---

## 6. Concrete Precedence Examples

## Example 1: conflicting discounts

Inputs:
- Rule A (priority 20): 10% segment discount.
- Rule B (priority 20): 15% partner discount.

Deterministic result:
- tie broken by `rule_id`; if discount stage strategy is `max_effect`, both evaluated and max eligible effect retained subject to cap.

Trace output:
- Rule A matched, applied/rejected (based on strategy).
- Rule B matched, applied/rejected.
- final discount rationale recorded.

## Example 2: constraint vs pricing

Inputs:
- constraint rule denies bundle option combination.
- pricing rule would otherwise apply bundle discount.

Deterministic result:
- Stage 1 constraint rejection prevents stage 3 adjustment from applying.

Trace output:
- Stage 1 rule applied with denial reason.
- Stage 3 rule skipped due invalid configuration state.

## Example 3: approval threshold overrides

Inputs:
- pricing computed net margin below floor.
- approval policy rule says require VP approval.

Deterministic result:
- quote cannot transition to finalized without approval state.

Trace output:
- approval stage records route requirement and threshold evidence.

---

## 7. Versioning and Migration Strategy

## 7.1 Versioning model

- Each ruleset change increments `rule_set.version`.
- Activation creates immutable `rule_set_snapshot`.
- Quote operations pin `ruleset_snapshot_id` at evaluation time.

## 7.2 Migration rules

1. Use additive migrations for new columns/tables.
2. Avoid destructive migrations on active rule tables in foundation.
3. Introduce compatibility views for renamed fields where needed.
4. Run rule validation/lint after migration before activation.

## 7.3 Backward compatibility policy (foundation)

- Existing quote replays use their pinned snapshots.
- New operations use currently active snapshot.
- Re-price with latest policy is explicit operation, not implicit mutation.

---

## 8. Explainability and Replay Guarantees

Required outputs per evaluation:
- `ruleset_snapshot_id`
- ordered list of evaluated rule IDs
- matched/skipped/applied/rejected reason codes
- stage-wise intermediate totals/context decisions
- final result hash

Replay guarantee:
- Given identical input payload and snapshot ID, result hash must match.

If mismatch occurs:
- mark as determinism incident,
- block finalization for affected operations,
- require operator investigation.

---

## 9. Blast Radius Risks and Mitigations

| Risk | Blast Radius | Mitigation |
|---|---|---|
| Stage order change without migration controls | all active quotes using latest snapshot | freeze stage ordering schema + decision gate |
| Priority collisions with ambiguous strategies | subset of deals with overlapping rules | tie-break by `rule_id` + conflict lint |
| Silent rule deactivation | policy/compliance drift | immutable activation audit + approval workflow |
| Snapshot pinning not enforced | replay nondeterminism | mandatory snapshot FK on pricing/policy artifacts |
| Expression parser regression | wide runtime failures | validation pipeline + canary activation |
| Missing trace rows | explainability gap, audit risk | invariant check requiring complete trace per stage |

---

## 10. Operational Validation Checklist

1. Rule lint command catches duplicate stage/priority collisions.
2. Dry-run evaluation command shows stage-by-stage decisions.
3. Replay test corpus passes for frozen snapshots.
4. Migration smoke tests preserve snapshot references.
5. Approval threshold decisions always produce trace + route artifact.

---

## 11. Acceptance Criteria Traceability (`bd-3d8.11.3`)

- **Rule storage model options/tradeoffs:** Section 3 complete.
- **Deterministic evaluation pipeline + precedence rules:** Sections 5 and 6 complete.
- **Versioning and migration strategy:** Section 7 complete.
- **Supports explainability and replay:** Section 8 complete.
- **Concrete examples:** Section 6 complete.
- **Blast-radius concerns and mitigations:** Section 9 complete.

Result: `bd-3d8.11.3` acceptance criteria are satisfied by this artifact.

