# PC-1.3-A Contract: Rep Profile Entity

Bead: `bd-y1dd.1`  
Parent: `bd-y1dd` (Rep Profile Entity — Replace bare `actor_id` with structured `sales_rep`)  
Date: 2026-03-06

## Objective

Define the deterministic contract, schema boundary, migration/backfill order, and rollback plan for introducing a first-class `sales_rep` entity while preserving existing audit and actor semantics.

## Scope

In scope for contract design:

1. Canonical `sales_rep` schema and ownership boundaries.
2. Which existing identity fields migrate to `sales_rep_id` vs remain generic `actor_id`.
3. Deterministic migration and backfill sequence.
4. Failure modes, invariants, audit requirements, and security/governance implications.

Out of scope for this bead:

1. Full implementation across repositories/handlers (`bd-y1dd.2`).
2. Verification/doc rollout hardening (`bd-y1dd.3`).
3. Multi-tenant org scoping (`bd-2jvw.1`) as a hard requirement.

## Current State Inventory

Current schema mixes human rep identity with generic system identity:

1. Human-oriented fields:
`quote.created_by`, `quote_sessions.created_by`, `dialogue_sessions.user_id`, `session_participants.user_id`, `session_operations.user_id`, `approval_request.requested_by`, `approval_authorities.user_id`, `org_hierarchy.user_id`.
2. Generic actor fields that intentionally include system/integration actors:
`audit_event.actor`, `quote_ledger.actor_id`, `execution_queue_transition_audit.actor_id`, `negotiation_session.actor_id`, `deal_flight_* .actor_id`, optimizer/audit actor columns.

Key repository/code touchpoints:

1. `crates/core/src/domain/quote.rs` (`Quote.created_by: String`)
2. `crates/core/src/domain/approval.rs` (`ApprovalRequest.requested_by: String`)
3. `crates/db/src/repositories/quote.rs`, `approval.rs`, `dialogue.rs`, `analytics.rs`
4. `crates/server/src/crm.rs`, `pdf.rs`, `health.rs`
5. `crates/mcp/src/server.rs` (quote create/get payload shape and actor handling)

## Contract Decision

Use a split identity model:

1. `sales_rep_id` for **human sales ownership and authority decisions**.
2. Keep `actor_id`/`actor` strings for **generic provenance and non-human actors**.

This avoids forcing machine/service identities (`agent:mcp`, `system`, worker IDs) into the rep table.

## Proposed Schema Contract

### New Table

`sales_rep` (authoritative rep profile):

1. `id TEXT PRIMARY KEY` (stable rep key; not display name)
2. `external_user_ref TEXT UNIQUE` (Slack user ID or external identity)
3. `name TEXT NOT NULL`
4. `email TEXT`
5. `role TEXT NOT NULL` (`ae|se|manager|vp|cro|ops`)
6. `title TEXT`
7. `team_id TEXT`
8. `reports_to TEXT NULL REFERENCES sales_rep(id) ON DELETE SET NULL`
9. `status TEXT NOT NULL` (`active|inactive|disabled`)
10. `max_discount_pct REAL`
11. `auto_approve_threshold_cents INTEGER`
12. `capabilities_json TEXT NOT NULL DEFAULT '[]'`
13. `config_json TEXT NOT NULL DEFAULT '{}'`
14. `created_at TEXT NOT NULL`
15. `updated_at TEXT NOT NULL`

Indexes:

1. `idx_sales_rep_external_ref`
2. `idx_sales_rep_role`
3. `idx_sales_rep_reports_to`
4. `idx_sales_rep_status`

### Additive Columns (Dual-Write Bridge)

Add nullable `*_sales_rep_id` FKs first; keep legacy string columns during transition:

1. `quote.created_by_sales_rep_id`
2. `approval_request.requested_by_sales_rep_id`
3. `quote_sessions.created_by_sales_rep_id`
4. `dialogue_sessions.sales_rep_id` (parallel to legacy `user_id`)
5. `session_participants.sales_rep_id` (parallel to legacy `user_id`)
6. `session_operations.sales_rep_id` (parallel to legacy `user_id`)
7. `approval_authorities.sales_rep_id` (replace `user_id` as canonical key)
8. `org_hierarchy.sales_rep_id`, `org_hierarchy.manager_sales_rep_id`

No forced migration for generic actor columns (`audit_event.actor`, `quote_ledger.actor_id`, etc).

## Ownership Boundaries

1. `quotey-core`:
Domain types for `SalesRep`, `SalesRepId`, authority/capability value objects, and strict transition/validation rules.
2. `quotey-db`:
DDL migrations, backfill routines, repository dual-read/dual-write behavior, deterministic query semantics.
3. `quotey-server` + `quotey-slack` + `quotey-mcp`:
Identity resolution inputs (`Slack user_id`, MCP actor context) mapped into `sales_rep_id` where the workflow is rep-owned.
4. `audit`/`optimizer`/`simulation` paths:
Keep generic actor contract; optionally enrich with `sales_rep_id` metadata when resolvable.

## Deterministic Migration Plan

### Phase 1: Additive DDL (No Behavior Change)

1. Create `sales_rep`.
2. Add nullable `*_sales_rep_id` columns and indexes.
3. Add FKs with `ON DELETE SET NULL` during transition.

### Phase 2: Deterministic Backfill

Source identity set (de-duplicated, sorted):

1. `quote.created_by`
2. `approval_request.requested_by`
3. `quote_sessions.created_by`
4. `dialogue_sessions.user_id`
5. `session_participants.user_id`
6. `session_operations.user_id`
7. `approval_authorities.user_id`
8. `org_hierarchy.user_id`, `org_hierarchy.manager_id`

Backfill classification:

1. Rep-candidate IDs: Slack-style IDs (`U...`), known people IDs/emails.
2. Non-rep/system IDs: `agent:*`, `system`, worker IDs, service principals.

Backfill behavior:

1. Create deterministic placeholder rep rows for resolvable human IDs.
2. Write `*_sales_rep_id` for rows with resolved rep mapping.
3. Leave `*_sales_rep_id` null for non-rep/system actors.
4. Emit summary metrics: total scanned, mapped, unresolved-human, skipped-system.

### Phase 3: Dual-Read / Dual-Write Cutover

1. Writes to rep-owned workflows populate both legacy string field and `*_sales_rep_id`.
2. Reads prefer `*_sales_rep_id` joins; fallback to legacy strings until full rollout.

### Phase 4: Constraint Tightening (Post-Adoption)

1. Make selected `*_sales_rep_id` columns `NOT NULL` where semantically required.
2. Retire or freeze legacy string columns after downstream consumers migrate.

## Rollback Expectations

Rollback must preserve existing behavior and data:

1. Feature flag rollback path: disable reads from `*_sales_rep_id`, continue legacy string fields.
2. Schema rollback for additive columns only after export/snapshot.
3. Never drop legacy columns in same rollout as first backfill.
4. Backfill operations idempotent and replay-safe.

## Invariants

1. Rep-owned operations always have deterministic owner semantics.
2. Generic system actors remain representable without forced rep records.
3. No quote/approval lifecycle break during dual-write period.
4. `reports_to` graph must remain acyclic at write time.
5. Authority resolution must be deterministic for the same rep + policy snapshot.

## Failure Modes and Mitigations

1. Ambiguous legacy IDs (same token representing different humans):
resolve via explicit mapping table before strict constraints.
2. Missing rep profile during request handling:
fail closed for governance-sensitive actions, fallback for read-only surfaces.
3. Cyclic manager chain:
reject write; log audit with validation error code.
4. Deleted/inactive manager:
`reports_to` nullable with escalation fallback to role-based routing.
5. Partial backfill:
startup health check blocks strict mode until completeness threshold met.

## Security and Governance Implications

1. Rep profile updates become governance events (authority and hierarchy mutations).
2. PII fields (`name`, `email`) require log redaction discipline.
3. Authorization gates should consume canonical rep role/authority, not free-form actor strings.
4. Emergency override paths must capture both acting principal and impacted rep ID.

## Required Audit Events

Minimum new audit events:

1. `sales_rep.created`
2. `sales_rep.updated`
3. `sales_rep.deactivated`
4. `sales_rep.authority_changed`
5. `sales_rep.hierarchy_changed`
6. `sales_rep.backfill_mapped`
7. `sales_rep.backfill_unresolved`
8. `quote.rep_assignment_changed`

Each event must include:

1. `actor` + `actor_type`
2. `sales_rep_id` (when applicable)
3. before/after payloads for mutable governance fields
4. correlation/idempotency keys

## Cross-Feature Assumptions

1. `bd-2jvw.1` (org scoping) will later add `org_id` constraints; this contract keeps room for it.
2. `bd-3ncv` (sales org hierarchy) should reuse `sales_rep.reports_to` instead of parallel hierarchy models.
3. Approval/governance beads (`bd-3vxx`, `bd-xj93`, `bd-17ei`) should key policy authority on `sales_rep_id`.

## Implementation Hand-off Checklist for `bd-y1dd.2`

1. Add core/domain `SalesRep` types and validation.
2. Add migration with additive columns/FKs/indexes.
3. Implement deterministic backfill command and dry-run mode.
4. Add dual-write repository behavior and compatibility reads.
5. Add resolver for Slack/MCP actor → `sales_rep_id`.

## Verification Checklist for `bd-y1dd.3`

1. Unit tests for resolver, hierarchy cycle checks, authority resolution.
2. Migration tests for up/down and idempotent backfill replay.
3. Integration tests for quote create/list, approval request/list, analytics `SalesRep` dimension.
4. Audit assertions for all new rep-profile governance events.

