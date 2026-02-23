# RCH-06: CRM Contract and Sync Boundary Design

**Research Task:** `bd-3d8.11.7`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** LilacMountain (Codex)  
**Primary Inputs:** `.planning/PROJECT.md`, `.planning/ARCHITECTURE_DECISION_RESEARCH.md`

---

## Executive Summary

This document defines the deterministic CRM integration contract for Quotey across:

1. adapter-neutral field mapping for minimum viable CRM sync,
2. source-of-truth ownership by field/domain,
3. conflict, retry, and reconciliation semantics.

Decision:

- Keep domain vendor-agnostic behind a `CrmAdapter` boundary.
- Support dual modes consistently (`StubCrmAdapter`, `ComposioCrmAdapter`).
- Treat local quote lifecycle as authoritative for CPQ execution while CRM sync remains eventually consistent.

This aligns with ADR-0014 and prevents vendor coupling and data drift.

---

## 1. Objective and Acceptance Mapping

Required outputs from `bd-3d8.11.7`:

1. Field mapping contract for minimum viable CRM sync.
2. Source-of-truth ownership by field/domain.
3. Conflict and retry semantics for sync operations.

Acceptance mapping:

- consistent support for stub + composio: Sections 2 and 3.
- error handling and reconciliation documented: Sections 6 and 7.
- offline/demo guardrails included: Section 8.

---

## 2. CRM Adapter Boundary Contract

### 2.1 Domain-Side Adapter Interface

Adapter operations (from planning direction):

1. `lookup_account(name_or_domain)`
2. `get_deal(deal_id)`
3. `create_deal(account_id, deal_data)`
4. `update_deal(deal_id, updates)`
5. `write_quote(deal_id, quote_summary, attachment_ref)`
6. `sync_incremental()`
7. `search_contacts(query)`

Boundary rule:

- Domain/services consume normalized internal DTOs only.
- No Composio/vendor-specific types may cross into core domain.

### 2.2 DTO Normalization Contract

Canonical adapter DTOs:

1. `CrmAccountDto`
2. `CrmContactDto`
3. `CrmDealDto`
4. `CrmQuoteWriteDto`
5. `CrmSyncResultDto`

Each DTO requires:

1. stable internal ids,
2. optional `crm_ref` external id,
3. source metadata (`source_provider`, `source_ts`),
4. mapping diagnostics for partial field coverage.

### 2.3 Stub/Composio Parity Rule

For same domain request, both adapters must return semantically equivalent DTO fields for the contract-required subset.

If Composio has richer fields:

- keep extras in adapter-local metadata,
- do not promote to core domain until field dictionary update is accepted.

---

## 3. Minimum Viable Field Mapping Contract

### 3.1 Entity Mapping Scope (v1)

1. Account
2. Contact
3. Deal
4. Quote writeback summary

### 3.2 Required Field Map

| Domain Field | CRM Field Category | Required | Notes |
|---|---|---|---|
| `account.id` | external account id (`crm_ref`) | yes | stable link key |
| `account.name` | account name | yes | primary lookup field |
| `account.domain` | website/domain | recommended | fuzzy lookup aid |
| `account.segment` | customer segment | yes | pricing/policy context |
| `deal.id` | external deal/opportunity id (`crm_ref`) | yes | sync key |
| `deal.account_id` | parent account id | yes | referential integrity |
| `deal.name` | opportunity name | yes | user-facing context |
| `deal.stage` | pipeline stage | yes | context and reporting |
| `deal.amount` | deal amount | recommended | routing and context |
| `deal.close_date` | close date | recommended | planning context |
| `quote.id` | quote identifier | yes | writeback identity |
| `quote.version` | quote revision | yes | stale-write guard |
| `quote.status` | lifecycle status | yes | CRM visibility |
| `quote.total` | priced total | yes | commercial output |
| `quote.currency` | currency code | yes | monetary correctness |
| `quote.valid_until` | expiry date | recommended | lifecycle signal |
| `quote.attachment_ref` | PDF/file reference | recommended | buyer-facing artifact |

### 3.3 Contract Validation Rule

Mapping validation must fail fast when required fields are absent or malformed.

Failure behavior:

1. keep domain state unchanged,
2. emit explicit mapping error category,
3. record diagnostics in sync state and audit events.

---

## 4. Source-of-Truth Ownership Model

### 4.1 Domain Ownership Principles

1. CPQ execution truth (quote state, pricing, approvals) is local-first in SQLite.
2. CRM is authoritative for core commercial identities unless explicitly overridden by field policy.
3. Ownership is field-level, not table-level.

### 4.2 Ownership Matrix

| Field Group | Primary Owner | Sync Direction | Conflict Policy |
|---|---|---|---|
| account identity (`crm_ref`, canonical external IDs) | CRM | inbound to local | `crm_wins` |
| account enrichment (`segment`, region if policy-managed locally) | policy-configurable | bidirectional | explicit per field |
| deal stage/close date from sales process | CRM | inbound preferred | `crm_wins` unless lock window |
| local CPQ quote lifecycle/status | Quotey | outbound to CRM | `local_wins` |
| local pricing totals and traces | Quotey | outbound summary only | `local_wins` |
| approval artifacts | Quotey | outbound summary optional | `local_wins` |
| sync cursors/operational metadata | Quotey | local only | n/a |

### 4.3 Field Policy Modes

Per mapped field, policy must be one of:

1. `crm_wins`
2. `local_wins`
3. `merge_with_audit`
4. `immutable_after_bind`

Default recommendation:

- IDs and external refs: `immutable_after_bind`
- quote financial outputs: `local_wins`
- process fields (stage/date): `crm_wins` unless explicit local lock

---

## 5. Conflict and Sync Semantics

### 5.1 Sync Model

1. Inbound incremental sync updates account/deal/contact mirrors.
2. Outbound writeback publishes quote summary and artifact references.
3. Sync is asynchronous by default; user request path should not block on full sync.

### 5.2 Conflict Detection

Use deterministic conflict keys:

1. `entity_id`
2. `field_name`
3. `local_version`
4. `remote_version_or_timestamp`

Conflict classes:

1. stale outbound write,
2. inbound overwrite of locally locked field,
3. schema mapping mismatch.

### 5.3 Conflict Resolution Rules

1. Apply field policy mode deterministically.
2. If `merge_with_audit`, keep winner + record loser value in audit payload.
3. If unresolved by policy, mark conflict state and require operator reconciliation.

### 5.4 Quote Version Guard

Outbound quote writeback must include quote version.

If CRM writeback target already has newer version marker:

- reject stale write as non-retryable conflict,
- emit reconciliation item.

---

## 6. Retry and Error Handling Semantics

### 6.1 Retry Policy Classes

| Error Class | Retry Policy | Notes |
|---|---|---|
| network timeout / transient 5xx | retry with exponential backoff + jitter | bounded attempts |
| auth/token invalid | no blind retry | require re-auth workflow |
| mapping/schema error | no retry until mapping fixed | terminal for given payload |
| rate limit | retry with provider-aware delay | preserve operation correlation |

### 6.2 `crm_sync_state` Lifecycle

States:

1. `idle`
2. `syncing`
3. `error`

Required transitions:

1. `idle -> syncing` on sync start.
2. `syncing -> idle` on success with cursor/count update.
3. `syncing -> error` on terminal failure with classified error.
4. `error -> syncing` on manual/scheduled retry after remediation.

### 6.3 Partial Success Handling

If remote update succeeds but local finalize fails:

1. emit compensating audit event,
2. persist reconciliation task with full correlation metadata,
3. avoid silent success reporting.

---

## 7. Reconciliation and Operator Workflow

### 7.1 Reconciliation Record Minimum Fields

1. `entity_type`
2. `entity_id`
3. `conflict_type`
4. `local_value`
5. `remote_value`
6. `field_policy_mode`
7. `first_seen_ts`
8. `last_attempt_ts`
9. `correlation_id`

### 7.2 Operator Commands (planned)

1. `quotey crm status`
2. `quotey crm sync`
3. `quotey crm reconcile list`
4. `quotey crm reconcile apply <id> --policy ...`

### 7.3 User-Facing Failure Messaging

When sync/writeback fails:

1. keep quote lifecycle intact,
2. show concise warning with correlation id,
3. allow continued local progress where safe.

---

## 8. Offline/Demo Mode Guardrails

1. Stub mode must implement same adapter interface and required field contract.
2. Fixture data must include deterministic IDs and stable timestamps where needed.
3. Offline mode must not mimic successful remote sync when mapping validation failed.
4. Output messaging must clearly indicate stub/offline provider context for operators.

---

## 9. Implementation Handoff Notes

### For `bd-3d8.11.7.1` (field-mapping matrix)

1. Expand Section 3 mapping table into provider-specific matrix (HubSpot/Salesforce via Composio).
2. Add field policy mode per mapped attribute.
3. Add unresolved-field backlog for non-v1 fields.

### For adapter implementation tasks

1. enforce adapter-local vendor DTO to normalized DTO mapping.
2. add strict required-field validation before domain mutation.
3. add conflict classification and reconciliation persistence path.

### For `bd-3d8.11.10` (decision freeze)

Freeze these defaults:

1. field-level ownership model,
2. asynchronous sync boundary with local CPQ authority,
3. deterministic conflict class/resolution rules,
4. no vendor DTO leakage into core domain.

---

## 10. Done Criteria Mapping

Deliverable: Field mapping contract  
Completed: Section 3.

Deliverable: Source-of-truth ownership by field/domain  
Completed: Section 4.

Deliverable: Conflict and retry semantics  
Completed: Sections 5 and 6.

Acceptance: Stub and composio consistency  
Completed: Sections 2 and 8.

Acceptance: Error handling and reconciliation strategy  
Completed: Sections 6 and 7.

Acceptance: Offline/demo guardrails  
Completed: Section 8.

