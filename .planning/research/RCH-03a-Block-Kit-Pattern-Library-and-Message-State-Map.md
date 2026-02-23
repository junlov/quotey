# RCH-03a: Block Kit Pattern Library and Message State Map

**Bead:** `bd-3d8.11.4.1`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** IvoryBear (Codex)

## 1. Objective

Define reusable Slack Block Kit templates and deterministic message-state transitions for Quotey.

This artifact provides:

1. template patterns for success, warning, approval, and degraded states,
2. canonical state map for the primary thread status card,
3. idempotency-safe message update protocol.

## 2. Inputs

Primary references:

1. `.planning/research/RCH-03-Slack-Command-Grammar-and-Thread-Lifecycle.md`
2. `.planning/research/RCH-04-Reliability-and-Idempotency-Architecture.md`
3. `.planning/research/RCH-07a-Event-Taxonomy-and-Dashboard-Blueprint.md`

## 3. Block Kit Template Conventions

### 3.1 Block and Action Identity

Use stable identities for replay-safe handling:

1. `block_id = "<screen>.<section>.v<schema_version>"`
2. `action_id = "<domain>.<action>.v<schema_version>"`
3. `private_metadata` includes:
   - `quote_id`
   - `quote_version`
   - `thread_state`
   - `message_revision`
   - `operation_key`

### 3.2 Status Card Structure (Canonical)

Each canonical status card message uses this block order:

1. header block (state + quote id),
2. summary section block (amount/stage/owner),
3. context block (last update ts + correlation id),
4. action block (allowed actions for current state),
5. optional diagnostics block (for warning/degraded only).

### 3.3 Rendering Rule

Only one canonical status message per quote thread is mutable.  
Non-canonical updates are append-only notices.

## 4. Pattern Library

### 4.1 Pattern `PAT-SUCCESS`

Use when state transition completed and no active violation exists.

Required visual/content elements:

1. positive state headline (`Priced`, `Approved`, `Finalized`),
2. key quote metrics (total, currency, quote version),
3. next-step actions.

Allowed actions by sub-state:

1. in `Priced`: `Request Approval`, `Generate PDF`, `Edit`,
2. in `Approved`: `Generate PDF`, `Send to CRM`, `Finalize`,
3. in `Finalized`: `Clone`, `View Audit`.

### 4.2 Pattern `PAT-WARNING`

Use when policy or validation warning exists but user can continue with remediation.

Required elements:

1. warning summary with deterministic rule id references,
2. impacted fields/rules list,
3. clear corrective actions.

Required actions:

1. `Edit Fields`,
2. `Reduce Discount`,
3. `Request Approval` (if policy allows exception path).

### 4.3 Pattern `PAT-APPROVAL`

Use for `ApprovalPending` and approver-facing decision cards.

Required elements:

1. approval reason summary (`threshold_reason`, `policy_violation_ids`),
2. required role set and remaining approvers,
3. quote version binding notice.

Required actions:

1. `Approve`,
2. `Reject`,
3. `Request Changes`.

Required safety fields in action payload:

1. `approval_request_id`,
2. `quote_id`,
3. `quote_version`,
4. `decision_nonce`.

### 4.4 Pattern `PAT-DEGRADED`

Use when external side effects fail but local deterministic state remains valid.

Required elements:

1. degraded banner with reason class (`crm_sync_failed`, `pdf_upload_retrying`, etc.),
2. unaffected local state summary,
3. correlation id and retry status.

Required actions:

1. `Retry`,
2. `Check Status`,
3. `View Diagnostics` (operator-safe view).

Critical rule:

- Never revert business state in UI solely due to adapter-side failure.

## 5. Message State Map

### 5.1 Canonical UI States

| UI State | Backing Domain State | Pattern | Mutable? |
|---|---|---|---|
| `ui.initialized` | `Initialized` | warning | yes |
| `ui.gathering` | `GatheringContext` | warning | yes |
| `ui.configured` | `DraftConfigured` | success | yes |
| `ui.priced` | `Priced` | success | yes |
| `ui.approval_pending` | `ApprovalPending` | approval | yes |
| `ui.approved` | `Approved` | success | yes |
| `ui.rejected` | `Rejected` | warning | yes |
| `ui.finalized` | `Finalized` | success | yes |
| `ui.degraded` | side-effect failure overlay | degraded | yes (overlay mode) |

### 5.2 Transition Matrix

| From | To | Trigger | Allowed | Notes |
|---|---|---|---|---|
| `ui.initialized` | `ui.gathering` | quote shell created | yes | default first transition |
| `ui.gathering` | `ui.configured` | required fields complete | yes | deterministic validation gate |
| `ui.configured` | `ui.priced` | pricing success | yes | includes trace reference |
| `ui.priced` | `ui.approval_pending` | policy threshold exceeded | yes | approval request id required |
| `ui.priced` | `ui.finalized` | no approval required and finalize action | yes | version-bound finalize |
| `ui.approval_pending` | `ui.approved` | approval chain satisfied | yes | role set complete |
| `ui.approval_pending` | `ui.rejected` | rejection decision recorded | yes | include rationale |
| `ui.approved` | `ui.finalized` | finalize completed | yes | emit finalization event |
| any | `ui.degraded` | side-effect failure | yes | overlay; does not rewrite domain state |
| `ui.degraded` | previous steady state | retry success | yes | remove degraded overlay |

Illegal transitions must be ignored and logged as:

- `ingress.command.rejected` or `quote.lifecycle.transition_rejected`.

## 6. Idempotency-Safe Message Update Protocol

### 6.1 Message Update Key

Canonical update key:

`message_update_key = hash(quote_id | thread_ts | ui_state | message_revision | operation_key)`

### 6.2 Update Algorithm

1. Resolve canonical status message by `quote_id -> status_message_ts`.
2. Build next payload with incremented `message_revision`.
3. Reserve idempotency entry keyed by `message_update_key`.
4. Perform `chat.update` using canonical `status_message_ts`.
5. On duplicate/replay key:
   - return stored result,
   - do not emit additional `chat.update`.
6. On `message_not_found`:
   - recreate canonical message once,
   - update thread mapping,
   - log reconciliation event.

### 6.3 Concurrency Guardrails

1. Reject stale updates where incoming `quote_version < current quote_version`.
2. Reject conflicting update when same revision already applied with different payload hash.
3. Record conflict as `reliability.idempotency.hit` + `status=conflict`.

## 7. Example Template Skeletons

### 7.1 Success Template Skeleton

```json
{
  "blocks": [
    {"type":"header","block_id":"status.header.v1","text":{"type":"plain_text","text":"Quote Q-2026-0042: Priced"}},
    {"type":"section","block_id":"status.summary.v1","text":{"type":"mrkdwn","text":"*Total:* $125,000\\n*Version:* 3\\n*Stage:* Priced"}},
    {"type":"actions","block_id":"status.actions.v1","elements":[
      {"type":"button","action_id":"quote.request_approval.v1","text":{"type":"plain_text","text":"Request Approval"}},
      {"type":"button","action_id":"quote.generate_pdf.v1","text":{"type":"plain_text","text":"Generate PDF"}}
    ]}
  ]
}
```

### 7.2 Degraded Template Skeleton

```json
{
  "blocks": [
    {"type":"header","block_id":"status.header.v1","text":{"type":"plain_text","text":"Quote Q-2026-0042: Degraded (CRM Sync)"}},
    {"type":"section","block_id":"status.degraded.v1","text":{"type":"mrkdwn","text":"Local quote state is safe. CRM writeback failed with `timeout`."}},
    {"type":"context","block_id":"status.context.v1","elements":[{"type":"mrkdwn","text":"Correlation: cr-7f91 | Retry #2"}]},
    {"type":"actions","block_id":"status.actions.v1","elements":[
      {"type":"button","action_id":"integration.retry_crm.v1","text":{"type":"plain_text","text":"Retry"}},
      {"type":"button","action_id":"quote.check_status.v1","text":{"type":"plain_text","text":"Check Status"}}
    ]}
  ]
}
```

## 8. Instrumentation Hooks

Each canonical update emits:

1. `quote.ui.status_card_update_requested`
2. `quote.ui.status_card_update_applied` or `quote.ui.status_card_update_replayed`
3. `quote.ui.status_card_update_rejected` for stale/conflict cases

Dimensions:

1. `quote_id`
2. `quote_version`
3. `ui_state`
4. `message_revision`
5. `operation_id`
6. `correlation_id`

## 9. Acceptance Criteria Mapping

`bd-3d8.11.4.1` requirements:

1. **Pattern list includes success, warning, approval, and degraded states**: Sections 4 and 7.
2. **Message updates are idempotency-safe**: Section 6.

This artifact is ready for Slack adapter implementation and test conversion.
