# RCH-05: Approval Governance and Delegation Model

**Research Task:** `bd-3d8.11.6`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** LilacMountain (Codex)  
**Primary Inputs:** `.planning/PROJECT.md`, `.planning/ARCHITECTURE_DECISION_RESEARCH.md`

---

## Executive Summary

This document specifies a deterministic approval governance model for Quotey covering:

1. authority matrix and threshold boundaries,
2. delegation and out-of-office (OOO) fallback semantics,
3. escalation timing and conflict resolution policy.

Decision:

- Use a DB-managed approval matrix with explicit threshold dimensions and role resolution.
- Treat approval actions as idempotent, version-bound decisions.
- Enforce escalation and delegation through deterministic state transitions with full audit traces.

This aligns with ADR-0013 and prevents stuck or ambiguous approval outcomes.

---

## 1. Objective and Acceptance Mapping

Required outputs from `bd-3d8.11.6`:

1. Approval authority matrix and threshold boundaries.
2. Delegation/OOO fallback semantics.
3. Escalation timing and conflict resolution policy.

Acceptance mapping:

- deterministic enforceability: Sections 3, 4, and 5.
- audit/legal alignment: Sections 6 and 7.
- ambiguous authority scenarios resolved: Section 8.

---

## 2. Governance Principles

1. Approval authority is policy-derived, never ad hoc.
2. All approval decisions are bound to a specific quote version and policy snapshot.
3. Delegation may reassign actor identity, but not required authority level.
4. Escalation cannot reduce required authority.
5. Finalization is blocked until all required approvals are satisfied.
6. Every approval lifecycle event is auditable and append-only.

---

## 3. Deterministic Authority Matrix

### 3.1 Role Hierarchy (minimum)

Ordered authority ladder:

1. `sales_manager`
2. `deal_desk`
3. `vp_sales`
4. `cfo`
5. `legal` (orthogonal mandatory role for legal-trigger conditions)

Notes:

- `legal` is not strictly "higher" in commercial authority; it is mandatory when legal-trigger conditions match.
- Multi-role requirements are represented as a deterministic set, not a single winner.

### 3.2 Threshold Dimensions

Supported threshold dimensions (from schema direction):

1. `discount_pct`
2. `deal_value`
3. `margin_pct`
4. `product_specific`
5. temporal/compliance flags (e.g., end-of-quarter scrutiny, custom terms)

### 3.3 Baseline Matrix (alpha default policy)

| Condition | Required Authority |
|---|---|
| discount <= auto cap and no other violations | auto-approve (no human) |
| discount > auto cap and <= approval cap | `sales_manager` |
| deal_value > 100k | `deal_desk` |
| deal_value > 500k | `vp_sales` |
| discount > 30% OR margin below floor exception band | `cfo` |
| custom SLA/legal clause/product-specific legal trigger | `legal` required in chain |

Deterministic resolution rule:

1. Resolve all matched threshold requirements.
2. Take maximum commercial authority role required.
3. Add orthogonal mandatory roles (e.g., `legal`) if triggered.
4. Build required approval chain/set from the resolved role set.

---

## 4. Delegation and OOO Fallback Semantics

### 4.1 Delegation Model

Delegation record fields (conceptual):

1. `principal_approver_id`
2. `delegate_approver_id`
3. `role_scope`
4. `valid_from`, `valid_to`
5. `reason` (`ooo`, `workload`, `temporary_assignment`)
6. `enabled`

Delegation rules:

1. Delegate is valid only within window.
2. Delegate must be authorized for equal-or-higher role scope than required action.
3. Delegation never upgrades policy authority requirements.
4. Delegation changes actor identity only; required role metadata remains original.

### 4.2 OOO Resolution Order

When primary approver unavailable:

1. active explicit delegate for required role scope,
2. role queue fallback (configured roster),
3. escalation to next authority level after timeout.

### 4.3 Delegation Audit Requirements

Each delegated decision must record:

1. principal approver id,
2. acting delegate id,
3. delegation record id/window,
4. decision timestamp and comment.

---

## 5. Escalation Timing and Routing Policy

### 5.1 Timing Defaults

Use current planning defaults:

1. reminder at `2h`
2. auto-escalation at `4h`
3. max chain length `5`

### 5.2 Escalation Algorithm

For each pending step:

1. if decision arrives before deadline: advance chain.
2. if reminder threshold reached and still pending: send reminder event.
3. if escalation deadline reached: route to fallback delegate or next authority role.
4. increment `escalation_count` and emit audit event.
5. preserve original request lineage and policy references.

### 5.3 Escalation Guardrails

1. No escalation path may reduce authority requirement.
2. No silent auto-approval on timeout for high-risk thresholds.
3. Escalated request remains linked to original approval request id/chain.

---

## 6. Enforceability and Deterministic Execution Contract

### 6.1 Approval Request Creation Contract

Approval request creation must include:

1. `quote_id`
2. `quote_version`
3. triggering policy violations
4. resolved required role set
5. chain mode (`sequential` or `parallel`)
6. expiry/escalation timestamps

### 6.2 Approval Action Idempotency

Action key dimensions:

1. `approval_request_id`
2. `quote_version`
3. `actor_id`
4. `decision_type`
5. `source_request_id`

Idempotency outcomes:

1. duplicate approve/reject action with same key returns stored decision outcome.
2. stale action against superseded quote version is rejected with explicit reason.

### 6.3 Completion Rule

Quote can transition `approval -> approved` only when:

1. all required commercial authority approvals are satisfied,
2. all required orthogonal approvals (e.g., legal) are satisfied,
3. no pending conflict state remains.

---

## 7. Audit and Legal Alignment Requirements

Minimum fields per approval event:

1. `approval_request_id`
2. `quote_id`, `quote_version`
3. `required_role`
4. `actor_id` + `actor_role_at_time`
5. `decision` (`approved`, `rejected`, `delegated`, `escalated`, `expired`)
6. `timestamp` (UTC)
7. `rationale/comment`
8. `policy_violation_ids`

Legal/compliance alignment:

1. Preserve immutable history of who authorized which exception and why.
2. Preserve separation between automated policy evaluation and human exception approval.
3. Ensure no approval action can be orphaned from quote version context.

---

## 8. Ambiguous Scenarios and Explicit Resolution Policy

### Scenario A: Conflicting thresholds imply different approver roles

Policy:

1. resolve highest commercial authority role required,
2. include orthogonal mandatory roles,
3. build deterministic chain from resulting role set.

### Scenario B: Delegate has lower authority than required

Policy:

1. delegate action is invalid,
2. keep request pending,
3. escalate per timeout policy.

### Scenario C: Simultaneous approve and reject callbacks (race)

Policy:

1. apply first valid decision by operation ordering timestamp/idempotency key,
2. subsequent conflicting action marked as stale-conflict and audited.

### Scenario D: Approval arrives after escalation already reassigned

Policy:

1. late decision accepted only if request step still pending for same actor slot,
2. otherwise reject as stale and log event.

### Scenario E: Quote materially changed during pending approval

Policy:

1. invalidate pending approvals for old version,
2. create new approval request for new version if still required.

### Scenario F: Timeout reached and no fallback delegate configured

Policy:

1. escalate to next authority role roster,
2. if roster missing, move to explicit `stuck_pending` operational alert state and notify operators.

---

## 9. Implementation Handoff Notes

### For `bd-3d8.11.6.1` (approval matrix workbook)

1. Expand Section 3 matrix into scenario workbook by segment/product/deal class.
2. Add concrete edge-case playbook entries from Section 8.

### For approval engine implementation tasks

1. model approval role set resolution as pure deterministic function.
2. enforce quote-version binding in all approval APIs.
3. require idempotency wrapper for all approval actions.

### For `bd-3d8.11.10` (decision freeze)

Freeze these governance defaults:

1. authority resolution algorithm,
2. delegation eligibility constraints,
3. escalation timing and no-downgrade guardrails,
4. stale action conflict handling.

---

## 10. Done Criteria Mapping

Deliverable: Approval authority matrix and thresholds  
Completed: Section 3.

Deliverable: Delegation/OOO semantics  
Completed: Section 4.

Deliverable: Escalation timing and conflict resolution  
Completed: Sections 5 and 8.

Acceptance: Deterministically enforceable model  
Completed: Sections 6 and 8.

Acceptance: Audit/legal alignment  
Completed: Section 7.

Acceptance: Ambiguous scenarios resolved explicitly  
Completed: Section 8.

