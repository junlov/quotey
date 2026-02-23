# W1 REL Execution Queue Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-271.1`
(`Resilient Execution Queue`) so implementation remains auditable and deterministic.

## Scope
### In Scope
- Durable execution queue state for quote-scoped actions (Slack, CRM, PDF).
- Idempotency-key reservation and completion semantics.
- Retry and recoverability model with user-visible thread status.
- Structured audit trail of transitions and decision context.
- Telemetry for failure rate and recovery outcomes.

### Out of Scope (for Wave 1)
- Cross-quote global scheduling optimization.
- Non-quote tasks (general workflow orchestration).
- ML-based retry policy selection.
- Full multi-region replication semantics.

## Rollout Slices
- `Slice A` (foundation): deterministic state machine and queue status model.
- `Slice B` (durability): SQLite persistence, idempotency keys, recovery bootstrap.
- `Slice C` (UX): Slack thread progress/retry/recovery cards.
- `Slice D` (ops): metrics, runbook, failure drills.

`Slice A/B` are required before `Slice C/D` move to production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Failed action rate (transient faults) | 8.0% | <= 2.0% | Reliability owner | `queue_actions_failed / queue_actions_total` |
| Recovery success within 5 minutes | 65% | >= 95% | Runtime owner | `% stuck actions transitioned to completed/failed_terminal <=5m` |
| Duplicate side-effect rate | 1.5% | <= 0.1% | Determinism owner | `duplicate_mutations_detected / queue_actions_total` |
| Mean user-visible status latency | 6.0s | <= 1.5s | Slack integration owner | transition timestamp to thread update timestamp |

## Deterministic Safety Constraints
- Deterministic engines remain source of truth for pricing, policy, and approval routing.
- Queue retries must never alter business outcomes for identical idempotency keys.
- Each state transition must be explicitly recorded (`queued -> running -> completed|retryable_failed|failed_terminal`).
- Financial or policy outcomes cannot be inferred by LLM output.
- Queue processing must be resumable from persisted state only.

## Interface Boundaries (Draft)
### Domain Contracts
- `ExecutionTask`: quote-scoped action request and typed payload.
- `ExecutionState`: `queued`, `running`, `retryable_failed`, `failed_terminal`, `completed`.
- `IdempotencyRecord`: key, task hash, last state, result fingerprint.

### Service Contracts
- `ExecutionQueueService::enqueue(task) -> task_id`
- `ExecutionQueueService::claim(task_id, worker_id) -> ClaimResult`
- `ExecutionQueueService::complete(task_id, outcome) -> ()`
- `ExecutionQueueService::fail(task_id, error, retry_policy) -> TransitionResult`
- `ExecutionQueueService::recover_stale(now) -> Vec<RecoveredTask>`

### Persistence Contracts
- `ExecutionQueueRepo`: append/read/update queue entries by quote and state.
- `IdempotencyRepo`: reserve/check/complete key lifecycle.
- `ExecutionAuditRepo`: append-only transition and decision events.

### Slack Contract
- Thread-scoped progress card keyed by `quote_id` and `task_id`.
- Explicit recovery message when `retryable_failed` transitions after retry.
- User action buttons map deterministically to backend state transitions.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Duplicate side effects under retry storms | High | Medium | strict idempotency key reservation + result fingerprint checks | Runtime owner |
| Queue entry stuck in `running` after worker crash | High | Medium | stale-claim timeout + deterministic recovery sweep | Reliability owner |
| Silent failure not visible in Slack | High | Medium | mandatory transition-to-thread status emitter | Slack owner |
| Non-deterministic fallback in failure handling | High | Low | typed error taxonomy + explicit deterministic transitions only | Determinism owner |
| Migration rollback gaps | Medium | Low | migration reversibility tests + runbook rollback steps | Data owner |

## Guardrail Checklist (Pre-implementation Exit)
- Scope and non-goals agreed.
- KPI owner and metric formula explicitly documented.
- Deterministic constraints copied into implementation task templates.
- Interface contracts reviewed against existing crate boundaries.
- Risks and mitigations acknowledged by feature owner.
