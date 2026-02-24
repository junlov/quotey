# FEAT-06 Multi-User Sessions Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-70d.6`
(`Live Multi-User Quote Sessions`) so multiple users can collaborate on quotes in real-time with conflict-free editing.

## Scope
### In Scope
- Real-time quote editing sessions with presence tracking.
- Operational Transform (OT) for conflict-free concurrent edits.
- Cursor/selection awareness for active collaborators.
- Session lifecycle: create, join, leave, timeout, persist.
- Slack thread integration for session notifications and joins.
- Session audit trail with actor attribution for all changes.

### Out of Scope (for Wave 1)
- Offline editing with later sync (online-only in Wave 1).
- Video/voice collaboration features.
- Fine-grained field locking (row-level only).
- Session recording and replay.

## Rollout Slices
- `Slice A` (contracts): session schema, OT operation model, presence structure.
- `Slice B` (sync): WebSocket/Socket Mode transport, OT engine, cursor tracking.
- `Slice C` (runtime): session service, join/leave management, conflict resolution.
- `Slice D` (integration): Slack thread cards, presence UI, session audit, metrics.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Concurrent user support | N/A | >= 5 users | Platform owner | max simultaneous editors per quote |
| Edit synchronization latency | N/A | <= 200ms | Platform owner | local edit to remote visibility |
| Conflict resolution success | N/A | >= 99% | Runtime owner | auto-resolved conflicts / total conflicts |
| Session stability | N/A | >= 99.5% | Reliability owner | sessions without disconnect / total sessions |
| User adoption rate | N/A | >= 40% | Product owner | multi-user sessions / total quote edits |

## Deterministic Safety Constraints
- All quote mutations flow through OT engine; no direct edits bypassing transform.
- Pricing and policy decisions remain deterministic; OT handles only data structure changes.
- Session state is append-only log of operations; replayable for audit.
- Last-write-wins only for user metadata (cursors, presence); quote data uses OT.
- Conflict resolution never alters business logic outcomes.

## Interface Boundaries (Draft)
### Domain Contracts
- `Session`: session_id, quote_id, participants, created_at, last_activity.
- `SessionParticipant`: user_id, cursor_position, selection_range, joined_at.
- `Operation`: op_type, path, value, timestamp, client_id, parent_op_id.
- `PresenceUpdate`: user_id, status, cursor, selection.

### Service Contracts
- `SessionService::create_session(quote_id, creator) -> Session`
- `SessionService::join_session(session_id, user_id) -> SessionJoinResult`
- `SessionService::leave_session(session_id, user_id) -> ()`
- `SessionService::apply_operation(session_id, operation) -> OperationResult`
- `SessionService::get_participants(session_id) -> Vec<SessionParticipant>`
- `SessionService::close_inactive_sessions(timeout) -> Vec<ClosedSession>`

### Persistence Contracts
- `SessionRepo`: session metadata and lifecycle.
- `SessionOperationRepo`: append-only operation log per session.
- `SessionPresenceRepo`: ephemeral presence and cursor state.

### Slack Contract
- Session creation posts card to quote thread with join link.
- Active participants shown as "typing" indicators in thread.
- Edit notifications batched to avoid channel spam.
- Session timeout warning posted before automatic close.

### Crate Boundaries
- `quotey-core`: OT engine, operation transform logic.
- `quotey-db`: session state, operation log, presence storage.
- `quotey-slack`: real-time transport, presence UI, thread integration.
- `quotey-agent`: session lifecycle, user routing, audit logging.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Concurrent edit conflicts corrupt quote | High | Low | OT engine + deterministic replay | Runtime owner |
| Session state loss on disconnect | High | Low | server-side operation log + reconnection replay | Reliability owner |
| Race condition in session join | Medium | Low | atomic session state updates | Data owner |
| Performance degradation at scale | Medium | Medium | operation batching + cursor throttling | Platform owner |
| User confusion about who changed what | Medium | Medium | operation attribution in UI + audit trail | UX owner |

## Guardrail Checklist (Pre-implementation Exit)
- [ ] Scope and non-goals agreed.
- [ ] KPI owner and metric formula explicitly documented.
- [ ] Deterministic constraints copied into implementation task templates.
- [ ] Interface contracts reviewed against existing crate boundaries.
- [ ] Risks and mitigations acknowledged by feature owner.

## Migration Contract
### Schema Additions (`0023_multi_user_sessions`)
- `quote_sessions`: session metadata and lifecycle state.
- `session_operations`: append-only operation log.
- `session_participants`: presence and cursor state.
- `session_audit`: session lifecycle events for compliance.

### Version and Audit Semantics
- Operations stored with vector clock for ordering.
- Session timeout closes session but preserves operation log.
- All edits attributed to actor via session participation.

### Migration Behavior and Rollback
- Migration adds session tables; no changes to quote schema.
- Sessions disabled by default; enable via feature flag.
- Rollback removes session tables; quotes retain last state.
