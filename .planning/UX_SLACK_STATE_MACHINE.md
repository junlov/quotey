# Slack Quote Journey State Machine (Deterministic UX Contract)

## Purpose
Define deterministic quote journey states for Slack surfaces and bind transitions to existing handlers so users always get explicit next-step feedback.

## State Set

1. `intent_capture`
2. `context_collection`
3. `assumption_review`
4. `pricing_ready`
5. `priced_review`
6. `approval_required`
7. `approved`
8. `finalized`
9. `sent`
10. `blocked_error`

## Transition Contract

| From | Event | To | Guard | User feedback | Next best action |
|------|-------|----|-------|---------------|------------------|
| `intent_capture` | `/quote new` parsed (`QuoteCommand::New`) | `context_collection` | Valid slash command envelope | `preview_mode_message`/summary card | provide required fields |
| `context_collection` | required fields complete | `assumption_review` | required context present | status card + assumption panel | confirm assumptions |
| `context_collection` | pricing requested | `blocked_error` | missing required context | warning/error message with correction text | fill missing context |
| `assumption_review` | user confirms assumptions | `pricing_ready` | assumptions explicit or accepted | confirmation message | run pricing |
| `pricing_ready` | pricing succeeds | `priced_review` | deterministic pricing completed | quote status message (`priced`) | confirm / request approval / edit |
| `priced_review` | policy violation detected | `approval_required` | violation threshold crossed | approval-required card and rationale | request approval |
| `priced_review` | policy clean + finalize | `finalized` | no approval required | finalized status card | generate/send artifacts |
| `approval_required` | approve action (`approval.approve.*`) | `approved` | approver action captured | approval status message | finalize quote |
| `approval_required` | reject action (`approval.reject.*`) | `blocked_error` | rejection captured | rejection status message | revise quote |
| `approved` | finalize action | `finalized` | approval persisted | finalized message | send to customer |
| `finalized` | send action (`/quote send`) | `sent` | delivery action accepted | sent message | monitor status / clone |
| `*` | unsupported command/action | `blocked_error` | no valid route | explicit unsupported-action error | `/quote help` or explicit command |

## Handler Binding (Current Code)

| Concern | File | Binding |
|--------|------|---------|
| Slash command normalization and verb routing | `crates/slack/src/commands.rs` | `normalize_quote_command`, `classify_quote_command`, `CommandRouter::route` |
| Thread/interaction event dispatch | `crates/slack/src/events.rs` | `SlashCommandHandler`, `BlockActionHandler`, `ThreadMessageHandler`, `ReactionAddedHandler` |
| Deterministic status + next action rendering | `crates/slack/src/blocks.rs` | `quote_status_message`, action blocks, help/shortcut blocks |
| Underlying flow progression proof points | `crates/server/src/bootstrap.rs` | transition expectations `Draft -> Validated -> Priced -> Finalized -> Sent` |

## Mandatory UX Invariants

1. Pricing cannot run from Slack until required context exists.
2. Every transition must emit a status, warning, or confirmation message.
3. Every emitted state must include a next best action (`Refresh status`, `Command help`, approval action, edit/retry path).
4. Unsupported actions must fail loudly with deterministic recovery guidance.
5. No silent state changes: if state changed, user sees it in thread.

## Implementation Notes for Future Code Changes

1. Treat `context_collection -> pricing_ready` as a hard guard in command services (not just copy).
2. Keep status vocabulary aligned with `.planning/UX_COPY_SYSTEM.md`.
3. When adding new actions, update this state table and the UX gate checklist in the same change.

## Bead Linkage

- Bead: `quotey-ux-001-4`
- Depends on: `quotey-ux-001-3`
- Unblocks: `quotey-ux-001-5`
