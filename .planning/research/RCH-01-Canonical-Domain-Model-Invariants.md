# RCH-01: Canonical Domain Model and Invariants

**Research Task:** `bd-3d8.11.2`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** LilacMountain (Codex)  
**Primary Inputs:** `.planning/PROJECT.md`, `.planning/ARCHITECTURE_DECISION_RESEARCH.md`

---

## Executive Summary

This document defines the canonical domain model for Quotey and the non-negotiable invariants that preserve deterministic pricing, workflow correctness, approval governance, and auditability.

Key outcomes:

- Canonical aggregate boundaries and entity ownership are defined.
- Mutation authority is explicit for every critical entity.
- Lifecycle transitions for quote and approval domains are normalized.
- Deterministic and audit invariants are documented as implementation guardrails.
- Current ambiguities are identified with recommended decisions for upcoming foundation tasks.

This artifact is intended to unblock:

- `bd-3d8.11.10` (decision freeze and phased execution plan)
- `bd-3d8.4` (repository and domain trait contracts)
- `bd-3d8.6` / `bd-3d8.7` (flow and CPQ core scaffolding)

---

## 1. Scope and Domain Boundary

The canonical model covers these runtime domains:

1. Catalog and commercial context (`product`, `price_book`, `account`, `deal`)
2. Quote authoring and pricing (`quote`, `quote_line`, `quote_pricing_snapshot`)
3. Deterministic workflow (`flow_state`)
4. Approval governance (`approval_request`, `approval_chain`)
5. Observability and integrity (`audit_event`, `llm_interaction_log`)
6. Integration linkage (`slack_thread_map`, `crm_sync_state`)

Out of scope for this task:

- concrete sequence diagrams (tracked by `bd-3d8.11.2.1`)
- final crate-level API signatures (tracked by scaffold tasks)
- implementation-specific SQL/index tuning (tracked separately)

---

## 2. Canonical Aggregates and Entity Map

### 2.1 Aggregate Roots

Quotey should treat these as aggregate roots:

1. `QuoteAggregate` (root: `quote`)
2. `ApprovalAggregate` (root: `approval_request`, scoped by `quote_id`)
3. `CatalogAggregate` (roots: `product`, `price_book`)
4. `CommercialAggregate` (roots: `account`, `deal`)
5. `AuditAggregate` (root: append-only `audit_event`)

### 2.2 Entity Ownership and Mutation Authority

| Entity | Primary Owner | Mutation Authority | Notes |
|---|---|---|---|
| `quote` | Flow engine + quote domain service | Domain only (`QuoteService`) | Source of truth for external lifecycle status |
| `quote_line` | Quote domain service | Domain only (`QuoteService`) | No direct adapter writes |
| `quote_pricing_snapshot` | Pricing engine | Pricing engine only | Immutable after insert |
| `flow_state` | Flow engine | Flow engine only | Process state; never user-edited directly |
| `approval_request` | Approval engine | Approval engine + authorized approver action path | Policy outcomes drive creation |
| `approval_chain` | Approval engine | Approval engine only | Chain semantics must be deterministic |
| `audit_event` | Audit logger | Append-only by system services | Never updated/deleted |
| `llm_interaction_log` | LLM adapter via orchestrator | Insert-only | For observability and forensics |
| `slack_thread_map` | Slack adapter + orchestrator | Insert-on-bind only | Immutable binding after creation |
| `crm_sync_state` | CRM adapter | CRM sync engine only | Operational state, not quote truth |
| `product` / `constraint_rule` / pricing/policy tables | Catalog/rules admin workflows | Admin CLI/services only | Runtime evaluation reads only |
| `account` / `deal` / `contact` | CRM adapter + commercial service | Controlled upsert only | Ownership policy required (see ambiguities) |

### 2.3 Domain vs Adapter Contract Boundary

Domain layer owns:

- lifecycle transitions
- pricing/constraint/policy decisions
- approval routing decisions
- audit event semantics and event type taxonomy

Adapters own:

- protocol translation (Slack, CRM, Composio, LLM provider APIs)
- retries/timeouts/network error handling
- serialization/deserialization around domain contracts

Adapters must not:

- write financial values or policy outcomes directly
- bypass domain transition checks
- finalize quotes independently

---

## 3. Canonical Lifecycle Models

### 3.1 Quote Lifecycle (Canonical)

Canonical statuses (from planning schema):

`draft -> validated -> priced -> approval|approved|finalized -> sent`

Additional terminal/supersession states:

`rejected`, `expired`, `cancelled`, `revised`

Allowed transition rules:

1. `draft -> validated` only when required fields are complete and config is valid.
2. `validated -> priced` only through deterministic pricing run.
3. `priced -> finalized` only when no approval is required.
4. `priced -> approval` when policy evaluation requires approval.
5. `approval -> approved|rejected` only via approval decision record.
6. `approved -> finalized` only when approved scope still matches active quote version.
7. `finalized -> sent` only after document generation succeeds.
8. Any active quote may move to `expired` on validity timeout.
9. Any non-terminal quote may move to `cancelled` via explicit user action.
10. Any superseded quote moves to `revised` when a newer version is created.

### 3.2 Approval Lifecycle (Canonical)

`pending -> approved|rejected|escalated|delegated|expired`

Rules:

1. Approval decisions are per request id and tied to quote version.
2. Delegation must preserve original authority requirement in metadata.
3. Escalation must preserve chain-of-custody in audit.
4. Expired approvals cannot be reused; a new request must be created.

### 3.3 Flow Lifecycle (Canonical)

`flow_state` is the process view and must remain derivable from quote/approval state.

`flow_state.current_step` is authoritative for "what happens next" decisions.
`quote.status` is authoritative for external lifecycle stage.

Consistency invariant:

- quote status and flow step must never conflict (defined in invariant set below).

---

## 4. Non-Negotiable Invariants

### 4.1 Determinism and Financial Correctness

1. **LLM non-authority:** LLM output may suggest intent/summary text only; it cannot set prices, approvals, or compliance outcomes.
2. **Deterministic replay:** Given identical quote inputs + rules snapshot, pricing and policy outputs must be identical.
3. **Money precision:** Monetary arithmetic must use fixed decimal representation, never floating-point.
4. **Snapshot immutability:** `quote_pricing_snapshot` rows are immutable after insert.
5. **No implicit prices:** Missing price data is an error; system must not synthesize fallback prices.
6. **Trace completeness:** Every priced line in quote must be represented in pricing trace.
7. **Total integrity:** Snapshot totals must satisfy deterministic formula consistency (`subtotal - discount + tax = total`).

### 4.2 Workflow and State Integrity

8. **Transition legality:** quote status changes only through allowed transitions.
9. **Step/status coherence:** `flow_state.current_step` must be coherent with `quote.status`.
10. **Version isolation:** approval and pricing artifacts must reference specific quote version.
11. **No finalize-before-policy:** quote cannot reach `finalized` if unresolved policy violations require approval.
12. **Thread binding immutability:** quote-to-Slack thread mapping is immutable after first bind.

### 4.3 Approval Governance Integrity

13. **Authority enforcement:** approval decisions must be made by user/role satisfying policy threshold requirements.
14. **Decision provenance:** every approval decision records who, when, outcome, and optional comment.
15. **Chain integrity:** sequential approval chains must not skip required steps.
16. **Post-approval drift protection:** material quote changes after approval invalidate approval and require re-evaluation.

### 4.4 Audit and Observability Integrity

17. **Append-only audit:** `audit_event` is insert-only.
18. **Coverage:** every lifecycle mutation, policy decision, and pricing run emits an audit event.
19. **Actor attribution:** each audit event must have actor and actor_type.
20. **LLM traceability:** each LLM call must record provider/model/purpose and success/failure.

### 4.5 Adapter Boundary Integrity

21. **Domain-authoritative writes:** adapter-originated operations mutate canonical entities only via domain services.
22. **Idempotent command handling:** repeated external events (Slack retries/websocket reconnects) must not duplicate business mutations.
23. **CRM eventual sync boundary:** CRM failures cannot invalidate local quote lifecycle truth.

---

## 5. Data Ownership and Mutation Authority Matrix

### 5.1 Human Actor vs System Actor

| Concern | Human can request | System decides | Final authority |
|---|---|---|---|
| Product configuration intent | Yes | Yes (validation outcome) | Constraint engine |
| Discount request | Yes | Yes (policy evaluation + routing) | Policy + approval engine |
| Approval decision | Yes (authorized approver only) | No | Approval engine validates authority |
| Pricing execution | Triggers only | Yes | Pricing engine |
| Quote finalization | Triggers only | Yes | Flow engine |

### 5.2 Service-Level Mutation Rules

1. `QuoteService`:
- creates/updates quote draft fields and line items
- never writes policy approval outcomes directly

2. `PricingService`:
- reads quote + catalog/rules
- writes pricing snapshot
- updates quote status from `validated` to `priced` via flow orchestration

3. `PolicyService`:
- evaluates policy result
- emits violations/approval requirements
- does not mutate approval request directly; requests `ApprovalService`

4. `ApprovalService`:
- creates and advances approval requests/chains
- transitions quote from `approval` to `approved`/`rejected` through flow orchestration

5. `FlowEngine`:
- owns next-step logic
- updates `flow_state` and permitted `quote.status` transitions

6. `AuditService`:
- logs all material actions
- has no authority to mutate non-audit domain state

---

## 6. Critical Ambiguities and Resolution Recommendations

### A1. Source of truth overlap: `quote.status` vs `flow_state.current_step`

Ambiguity:

- Both fields can encode progression, creating possible divergence.

Recommendation:

- Treat `quote.status` as externally-visible lifecycle phase.
- Treat `flow_state.current_step` as operational step pointer.
- Add invariant check utility at transaction boundary: `assert_step_status_compatibility`.

Priority: High (must resolve before `bd-3d8.6`).

### A2. Quote line mutable pricing fields vs immutable snapshot

Ambiguity:

- `quote_line` includes `unit_price`/`subtotal` while snapshots are immutable record.

Recommendation:

- Keep `quote_line` financial columns as "latest computed view".
- Treat snapshot as audit truth; always generate snapshot id and stamp quote with last snapshot ref.
- Disallow direct editing of line financial fields outside pricing service.

Priority: High (must resolve before `bd-3d8.7`).

### A3. Approval invalidation criteria after quote edits

Ambiguity:

- What edits require reapproval?

Recommendation:

- Define material-change predicate:
  - any price-affecting field change
  - discount change
  - line add/remove
  - term/currency/billing_country change
- If predicate true and status is `approval` or `approved`, invalidate current approval artifacts and reroute.

Priority: High.

### A4. Account/deal field ownership (CRM vs local)

Ambiguity:

- Conflicts between local edits and CRM sync updates are not explicitly resolved.

Recommendation:

- Define per-field ownership map:
  - CRM-owned immutable fields in local store by default
  - locally-owned quoting metadata fields kept local
- Add conflict policy per field: `crm_wins`, `local_wins`, `merge_with_audit`.

Priority: Medium (before full CRM adapter rollout).

### A5. Time and timezone normalization

Ambiguity:

- Date/time fields across approvals, expiries, and audits risk timezone drift.

Recommendation:

- Persist all timestamps in UTC RFC3339 with offset `Z`.
- Store date-only commercial fields (`start_date`, `end_date`, `valid_until`) explicitly as date semantics.

Priority: Medium.

---

## 7. Implementation Handoff (for Foundation Tasks)

### 7.1 For `bd-3d8.4` (Repository and Domain Trait Contracts)

Implement these first-class domain types/interfaces:

1. `QuoteStatus`, `FlowStep`, and compatibility validator.
2. `QuoteVersionRef` enforced on pricing and approval records.
3. `PricingSnapshot` immutable type with deterministic total check.
4. `PolicyDecision` and `ApprovalRequirement` value objects.
5. Repository traits split by aggregate (`QuoteRepo`, `ApprovalRepo`, `CatalogRepo`, `AuditRepo`).

### 7.2 For `bd-3d8.6` (Flow Engine Skeleton)

Flow engine must:

1. own transition table centrally (compile-time match or transition map)
2. prevent illegal quote state changes
3. persist coherent `flow_state` and `quote.status` together transactionally

### 7.3 For `bd-3d8.7` (CPQ Core Service Stubs)

CPQ service contracts must:

1. expose deterministic pricing input/output structs
2. return explicit missing-data errors (no fallback guessing)
3. produce pricing trace payloads from the first stub implementation

### 7.4 For Audit/Observability Tasks

Minimum required event categories at scaffold stage:

- quote lifecycle mutation events
- pricing calculation events
- policy evaluation events
- approval route/decision events
- external adapter failures and retries

---

## 8. Definition of Done Mapping to Bead Acceptance Criteria

Bead requirement: Entity map and lifecycle transitions  
Status: Completed in Sections 2 and 3.

Bead requirement: Invariant list  
Status: Completed in Section 4.

Bead requirement: Contract boundaries  
Status: Completed in Sections 2.3 and 5.

Bead requirement: Ambiguities + resolution recommendation  
Status: Completed in Section 6.

Bead requirement: Data ownership/mutation authority  
Status: Completed in Sections 2.2 and 5.

Bead requirement: Implementation implications handoff  
Status: Completed in Section 7.

---

## 9. Next Recommended Follow-On

1. Execute `bd-3d8.11.2.1` to convert this model into sequence diagrams with retry/failure branches.
2. Feed Section 6 resolutions into `bd-3d8.11.10` decision freeze checklist.
3. Use Sections 4 and 7 as contract input for `bd-3d8.4` trait/type definitions.

