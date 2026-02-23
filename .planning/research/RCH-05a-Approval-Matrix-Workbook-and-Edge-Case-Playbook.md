# RCH-05a: Approval Matrix Workbook and Edge-Case Playbook

**Bead:** `bd-3d8.11.6.1`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** IvoryBear (Codex)

## 1. Objective

Turn the approval-governance model into fixture-ready artifacts for deterministic implementation.

This workbook delivers:

1. an explicit authority matrix,
2. deterministic tie-break and escalation rules,
3. edge-case scenarios with expected outcomes,
4. audit and logging consequences for each case.

## 2. Inputs and Dependencies

Primary references:

1. `.planning/research/RCH-05-Approval-Governance-and-Delegation-Model.md`
2. `.planning/research/RCH-04-Reliability-and-Idempotency-Architecture.md`
3. `.planning/research/RCH-05-Security-and-Compliance-Baseline.md`
4. `.planning/research/RCH-08a-Threat-Model-Worksheet-and-Control-Map.md`

Parent bead dependency:

- `bd-3d8.11.6` (closed): establishes baseline governance and delegation model.

## 3. Canonical Approval Policy Fixture Shape

Recommended canonical record for matrix rows:

| Field | Type | Notes |
|---|---|---|
| `rule_id` | string | Stable identifier (`APR-*`). |
| `priority` | integer | Lower value evaluates first for deterministic ordering. |
| `quote_type` | enum | `net_new`, `renewal`, `amendment`, `exception`. |
| `discount_pct_min` / `discount_pct_max` | decimal nullable | Inclusive thresholds. |
| `deal_value_min` / `deal_value_max` | decimal nullable | Inclusive thresholds. |
| `margin_pct_min` / `margin_pct_max` | decimal nullable | Inclusive thresholds. |
| `legal_trigger` | bool nullable | `true` means legal role required. |
| `product_risk_tier` | enum nullable | `standard`, `regulated`, `export_controlled`, etc. |
| `required_role_set` | set<role> | Deterministic required approver roles. |
| `approval_mode` | enum | `sequential` or `parallel`. |
| `sla_hours` | integer | Time to first decision for this rule class. |
| `escalation_hours` | integer | Escalation threshold from request creation. |
| `allow_delegation` | bool | Delegation eligibility for this rule. |
| `emergency_override_policy` | enum | `forbid`, `limited`, `requires_dual_control`. |
| `effective_from` / `effective_to` | timestamp | Policy versioning window. |
| `policy_snapshot_id` | string | Links quote approval to immutable policy snapshot. |

## 4. Deterministic Authority Matrix Workbook

### 4.1 Baseline Role Vocabulary

Roles:

1. `sales_manager`
2. `deal_desk`
3. `vp_sales`
4. `cfo`
5. `legal` (orthogonal mandatory role when legal conditions match)

### 4.2 Matrix Rows (Alpha Baseline)

| Rule ID | Trigger Conditions | Required Role Set | Mode | SLA/Escalation | Delegation | Emergency Override |
|---|---|---|---|---|---|---|
| `APR-001` | `discount_pct <= 10` AND no other violations | none (auto-approve) | n/a | n/a | n/a | `forbid` |
| `APR-002` | `10 < discount_pct <= 20` | `sales_manager` | sequential | `2h / 4h` | yes | `forbid` |
| `APR-003` | `deal_value > 100000` | `deal_desk` | sequential | `2h / 4h` | yes | `forbid` |
| `APR-004` | `deal_value > 500000` | `vp_sales` | sequential | `2h / 4h` | yes | `limited` |
| `APR-005` | `discount_pct > 30` OR `margin_pct < floor_exception_band` | `cfo` | sequential | `1h / 2h` | restricted | `requires_dual_control` |
| `APR-006` | `legal_trigger = true` | `legal` + max(commercial role) | parallel | `2h / 4h` | yes | `forbid` |
| `APR-007` | `product_risk_tier = export_controlled` | `legal`, `cfo` | parallel | `1h / 2h` | restricted | `requires_dual_control` |
| `APR-008` | `quote_type = exception` with any policy breach | role set from matched breaches | parallel | `1h / 2h` | restricted | `requires_dual_control` |

## 5. Tie-Break and Conflict Resolution Rules

When multiple rules match, apply this deterministic sequence:

1. Compute full matched rule set.
2. Resolve highest commercial authority role required.
3. Add orthogonal roles (`legal`) if any matching rule requires them.
4. Merge to `required_role_set` with stable lexical role ordering for reproducibility.
5. Resolve approval mode:
   - if any matched rule requires `parallel`, final mode is `parallel`,
   - else `sequential`.
6. Resolve SLA/escalation:
   - choose the strictest (lowest) SLA and escalation window among matched rules.
7. Resolve delegation:
   - `restricted` dominates `yes`,
   - `forbid` dominates all.
8. Resolve emergency override:
   - `requires_dual_control` dominates `limited`, which dominates `forbid`.

## 6. Delegation Policy Workbook

### 6.1 Delegation Preconditions

Delegation allowed only when:

1. delegation record is active in time window,
2. delegate role scope is equal or higher than required role,
3. policy row does not set `allow_delegation = false` or `restricted` violation applies,
4. delegate identity is mapped and not suspended.

### 6.2 Delegation Outcomes

| Delegation State | Expected Outcome | Required Audit Events |
|---|---|---|
| valid delegate, valid scope | action accepted as delegated decision | `approval.delegated`, `approval.decision_recorded` |
| delegate below authority scope | action rejected | `approval.delegation_denied_scope`, `security.authz_deny` |
| expired delegation window | action rejected as stale | `approval.delegation_expired`, `approval.decision_rejected` |
| delegation revoked after request issued | re-resolve assignee using fallback/escalation | `approval.delegation_revoked`, `approval.reassigned` |

## 7. Emergency Override Policy

Emergency overrides are not a bypass; they are a controlled exception path.

Rules:

1. Override never reduces required role set.
2. Override requires explicit rationale and incident ticket reference.
3. `requires_dual_control` means two distinct approvers with no shared actor id.
4. Override decisions are version-bound to `quote_version` and `policy_snapshot_id`.
5. Override path must emit dedicated audit records and security events.

## 8. Edge-Case Playbook (Fixture-Ready)

| Case ID | Scenario | Deterministic Outcome | Tie-Break Rule Applied | Audit Consequence |
|---|---|---|---|---|
| `EC-01` | Discount 18%, value 120k | `sales_manager` + `deal_desk` resolved to `deal_desk` | highest authority | `approval.rule_resolved`, `approval.request_created` |
| `EC-02` | Value 600k and legal trigger true | required roles = `vp_sales`, `legal` | orthogonal role merge | `approval.parallel_chain_created` |
| `EC-03` | CFO rule + legal trigger | required roles = `cfo`, `legal`, parallel | highest + orthogonal + parallel dominance | `approval.parallel_chain_created` |
| `EC-04` | Two callbacks: approve then reject within ms | first valid decision accepted; second marked stale-conflict | operation ordering | `approval.decision_recorded`, `approval.conflict_rejected` |
| `EC-05` | Duplicate approve callback replay | no second mutation | idempotency dedupe | `approval.replay_blocked` |
| `EC-06` | Delegate attempts CFO-only decision with deal_desk scope | deny | delegation scope check | `approval.delegation_denied_scope`, `security.authz_deny` |
| `EC-07` | Delegate valid but delegation expires before action | deny/stale | active-window check | `approval.delegation_expired` |
| `EC-08` | Escalation occurs; original approver responds late | late decision rejected unless slot still pending | stale-slot check | `approval.escalated`, `approval.decision_rejected_stale` |
| `EC-09` | Emergency override requested on `requires_dual_control` rule | require 2 distinct approvers + rationale | override dominance | `approval.override_requested`, `approval.override_completed` |
| `EC-10` | Override attempted without incident reference | reject | override validation | `approval.override_rejected_missing_context` |
| `EC-11` | Quote version increments after approval request | invalidate open approvals; require re-approval | version binding | `approval.invalidated_version_change`, `approval.request_created` |
| `EC-12` | Legal required but legal approver unavailable | escalate legal queue; finalization blocked | orthogonal role cannot be dropped | `approval.escalated`, `approval.blocked_missing_role` |

## 9. Audit and Logging Contract for Approval Engine

### 9.1 Mandatory Approval Event Set

For every approval request lifecycle:

1. `approval.rule_resolved`
2. `approval.request_created`
3. `approval.reminder_sent` (optional by timing)
4. `approval.escalated` (optional by timing)
5. `approval.decision_recorded` or `approval.decision_rejected`
6. `approval.chain_completed` or `approval.chain_failed`

### 9.2 Mandatory Event Fields

Each event requires:

1. `approval_request_id`
2. `quote_id`
3. `quote_version`
4. `policy_snapshot_id`
5. `required_role_set`
6. `actor_id` and `actor_role_at_time` (when actor exists)
7. `decision` and `reason_code`
8. `operation_key`
9. `correlation_id`
10. `event_ts_utc`

### 9.3 Security Event Hooks

Emit security events when:

1. role mismatch or unauthorized delegate action,
2. stale/replay decision attempts exceed threshold,
3. override misuse or missing required context,
4. approval chain progresses without required role completion (should be impossible; treat as critical).

## 10. Policy Fixture Starter Examples

Illustrative JSON fixture rows (for fixture generation and tests):

```json
[
  {
    "rule_id": "APR-002",
    "priority": 20,
    "quote_type": "net_new",
    "discount_pct_min": 10.0001,
    "discount_pct_max": 20.0,
    "required_role_set": ["sales_manager"],
    "approval_mode": "sequential",
    "sla_hours": 2,
    "escalation_hours": 4,
    "allow_delegation": true,
    "emergency_override_policy": "forbid"
  },
  {
    "rule_id": "APR-006",
    "priority": 60,
    "legal_trigger": true,
    "required_role_set": ["legal"],
    "approval_mode": "parallel",
    "sla_hours": 2,
    "escalation_hours": 4,
    "allow_delegation": true,
    "emergency_override_policy": "forbid"
  }
]
```

## 11. Verification Checklist for Implementation Beads

Before converting this workbook into runtime policy fixtures:

1. Add table-driven tests for `EC-01` through `EC-12`.
2. Validate that tie-break resolution is deterministic for randomized input order.
3. Validate delegation scope and validity-window enforcement.
4. Validate quote-version invalidation semantics.
5. Validate all required audit and security events exist per scenario.

## 12. Acceptance Criteria Mapping

`bd-3d8.11.6.1` requirements:

1. **Includes tie-break and audit consequences**: Sections 5, 8, and 9.
2. **Ready to convert into policy engine fixtures**: Sections 3, 4, 10, and 11.

This workbook is implementation-ready for policy-fixture and approval-engine tasks.
