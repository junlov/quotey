# RCH-03: Slack Command Grammar and Thread Lifecycle

**Research Task:** `bd-3d8.11.4`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** LilacMountain (Codex)  
**Primary Inputs:** `.planning/PROJECT.md`, `.planning/ARCHITECTURE_DECISION_RESEARCH.md`

---

## Executive Summary

This artifact defines the deterministic Slack UX contract for Quotey:

1. Slash command grammar with normalized payload contracts.
2. Thread lifecycle model with explicit allowed transitions.
3. Reusable Block Kit interaction patterns for decision-heavy CPQ flows.
4. Error/recovery UX policy and thread hygiene model for high-volume activity.

Decision:

- Keep a hybrid interaction model:
  - slash commands start/route flows,
  - thread replies provide natural language intent updates,
  - interactive components resolve ambiguity and confirm critical transitions.

This aligns with ADR-0011 and preserves deterministic backend authority.

---

## 1. Scope and Acceptance Mapping

Required deliverables from `bd-3d8.11.4`:

1. Command taxonomy and expected payload contracts.
2. Block Kit interaction patterns for decision-heavy flows.
3. Error/recovery UX policy.

Acceptance criteria coverage:

- maps cleanly to deterministic backend actions: Sections 2, 3, and 6
- accessibility/readability considerations: Section 7
- high-volume thread hygiene strategy: Section 8

---

## 2. Command Taxonomy and Payload Contracts

### 2.1 Canonical Command Family

Primary entrypoint:

- `/quote ...`

Supported intent groups:

1. Create: `/quote new ...`
2. Inspect: `/quote status <quote_id>`, `/quote list [filters]`, `/quote audit <quote_id>`
3. Mutate: `/quote edit <quote_id> ...`, `/quote add-line <quote_id> ...`
4. Exception flow: `/quote discount <quote_id> ...`
5. Lifecycle actions: `/quote send <quote_id>`, `/quote clone <quote_id>`

### 2.2 Grammar Contract (deterministic parse target)

The slash payload parser should output a normalized `CommandEnvelope`:

```text
CommandEnvelope {
  command: "quote",
  verb: "new" | "status" | "list" | "discount" | ...,
  quote_id?: string,
  account_hint?: string,
  freeform_args: string,
  channel_id: string,
  user_id: string,
  trigger_ts: string,
  request_id: string
}
```

Deterministic parser rules:

1. Parse `verb` from first token after `/quote`.
2. Parse `quote_id` when token matches quote id regex (`Q-YYYY-NNNN`).
3. Preserve remaining text in `freeform_args` for intent extraction.
4. Reject unknown verb with explicit help response (do not guess).

### 2.3 Example Parse Results

Input:

`/quote new for Acme Corp, Pro Plan, 150 seats, 12 months`

Output:

```text
verb="new"
quote_id=None
account_hint="Acme Corp"
freeform_args="Pro Plan, 150 seats, 12 months"
```

Input:

`/quote discount Q-2026-0040 change to 35% — losing to Vendor X`

Output:

```text
verb="discount"
quote_id="Q-2026-0040"
freeform_args="change to 35% — losing to Vendor X"
```

---

## 3. Thread Lifecycle Contract

### 3.1 Canonical Thread States

Thread state machine (aligned with ADR-0011):

1. `Initialized`
2. `GatheringContext`
3. `DraftConfigured`
4. `Priced`
5. `ApprovalPending`
6. `Approved`
7. `Rejected`
8. `Finalized`
9. `Expired`

### 3.2 State-to-Action Matrix (summary)

| Thread State | Allowed User Inputs | Allowed System Actions |
|---|---|---|
| `Initialized` | provide intent, cancel | create quote shell, ask missing fields |
| `GatheringContext` | provide fields, use edit modal | validate slots, prompt missing values |
| `DraftConfigured` | add/remove/edit lines | run deterministic validation/pricing |
| `Priced` | request discount, generate PDF, edit | evaluate policy, route approval if needed |
| `ApprovalPending` | add justification comment, cancel | send reminders/escalation, process decisions |
| `Approved` | generate PDF, send | finalize and prepare delivery artifacts |
| `Finalized` | send, clone, new version | publish output + CRM writeback |
| `Rejected` | revise quote, cancel | explain rejection reason and next options |
| `Expired` | clone/new version | block direct finalization/send |

Transition guardrails:

1. Critical financial transitions require explicit component action or deterministic confirmation.
2. Free-form thread messages can propose changes but must pass deterministic validation before mutation.
3. Any material edit after pricing returns state to `DraftConfigured`.

### 3.3 Thread Binding Contract

Rules:

1. Each quote maps to one canonical thread (`slack_thread_map`).
2. Status card message in thread is canonical and updated in place.
3. Non-canonical info messages are append-only, but state summary should remain single-source.

---

## 4. Block Kit Pattern Library (Decision-Heavy Flows)

### 4.1 Pattern A: Draft Summary + Missing Fields

Purpose:

- show current quote composition and unresolved required data.

Required controls:

- `Confirm`
- `Edit`
- `Add Line`
- `Set Missing Fields`

### 4.2 Pattern B: Policy Alert and Approval Routing

Purpose:

- explain policy violation and available deterministic options.

Required controls:

- `Request Approval`
- `Reduce Discount`
- `Edit`
- `Cancel`

### 4.3 Pattern C: Approval Decision Card

Purpose:

- present approver with concise, actionable context.

Required controls:

- `Approve`
- `Reject`
- `Request Changes`

### 4.4 Pattern D: Post-Approval Finalization Actions

Purpose:

- route final user actions after approval.

Required controls:

- `Generate PDF`
- `Send to CRM`
- `Done`

### 4.5 Component Naming Convention (idempotent-safe)

Use stable IDs:

- `action_id = "<domain>.<action>.<version>"`
- `block_id = "<screen>.<section>.<version>"`

Each action payload must include:

1. `quote_id`
2. `quote_version`
3. `thread_state`
4. `operation_nonce`

This supports duplicate-click dedupe and replay safety.

---

## 5. Error and Recovery UX Policy

### 5.1 Error Categories

1. Parse/grammar errors
2. Missing required fields
3. Constraint validation failures
4. Pricing/policy engine hard errors
5. Approval routing failures
6. External adapter failures (Slack API, CRM, PDF generation)

### 5.2 Response Policy by Category

1. Parse error:
- Return concise help with recognized verbs and examples.

2. Missing field:
- Return targeted missing-field prompt, not generic failure.

3. Constraint/policy violation:
- Explain exact violated rule and offer corrective actions.

4. External adapter failure:
- Preserve domain state; show "operation pending/retry" status with correlation id.

5. Duplicate event/action:
- Return idempotent confirmation ("already applied") instead of reapplying side effect.

### 5.3 Recovery Interaction Rules

1. Always provide next valid action buttons where possible.
2. Never ask user to manually repair internal consistency.
3. Include `/quote status <id>` fast-path in failure responses for recovery.

---

## 6. Deterministic Backend Mapping Contract

### 6.1 Slash and Interaction Mapping

| Slack Input | Domain Command |
|---|---|
| `/quote new ...` | `CreateQuoteDraft` |
| `/quote status <id>` | `GetQuoteStatus` |
| `/quote discount <id> ...` | `RequestDiscountException` |
| `Approve` button | `RecordApprovalDecision(approved=true)` |
| `Reject` button | `RecordApprovalDecision(approved=false)` |
| `Generate PDF` button | `GenerateQuoteDocument` |

Rule:

- Slack layer only translates and validates shape; it does not decide pricing/approval outcomes.

### 6.2 Idempotency Boundary

Operation key source:

- Slack envelope/request id + action id + quote id + quote version.

The command executor must treat repeated operation keys as safe replays.

---

## 7. Accessibility and Readability Policy

1. Do not rely on emoji/color alone for critical state meaning.
2. Lead with plain-language status summary before detailed numbers.
3. Keep action labels concise and verb-first.
4. Keep block content scannable (short sections, consistent ordering).
5. Provide text fallback when components cannot render.

Numeric readability:

- format money consistently with currency and separators.
- show key deltas in renewal/discount flows (`old -> new`).

---

## 8. High-Volume Thread Hygiene Strategy

1. Maintain one canonical "status card" message updated in place.
2. Append only materially new events (approval decision, finalization, delivery).
3. Collapse repetitive system chatter into periodic summaries.
4. Include `quote_id` and current state in all significant bot messages.
5. Use reminder throttling for long-running approvals (avoid spam).

Operational benefit:

- reduces cognitive load in active enterprise channels.
- preserves clean audit narrative in thread history.

---

## 9. Verification Plan

1. Parser tests for supported/unsupported verb patterns.
2. Thread state transition simulation for net-new, renewal, and discount exception paths.
3. Block action replay tests with duplicate payloads.
4. Snapshot tests for key Block Kit cards.
5. Manual UX pass for readability/accessibility across desktop/mobile layouts.

---

## 10. Ambiguities and Recommended Resolution

1. Ambiguity: should `/quote list` support rich filters in v1?
- Recommendation: support minimal deterministic filters (`status`, `account`, `owner`) now; defer advanced search syntax.

2. Ambiguity: where should long-form justification edits happen?
- Recommendation: modal for edit + thread summary echo, to preserve thread readability.

3. Ambiguity: how to handle thread drift with unrelated chatter?
- Recommendation: ignore non-command chatter unless message is explicitly targeted with quote context marker or detected stateful slot response.

---

## 11. Done Criteria Mapping

Deliverable: Command taxonomy and payload contracts  
Completed: Section 2.

Deliverable: Block Kit interaction patterns  
Completed: Section 4.

Deliverable: Error/recovery UX policy  
Completed: Section 5.

Acceptance: deterministic backend mapping  
Completed: Sections 3 and 6.

Acceptance: accessibility/readability coverage  
Completed: Section 7.

Acceptance: high-volume thread hygiene strategy  
Completed: Section 8.

