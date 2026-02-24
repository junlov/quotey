# W1 REL Execution Queue Demo Checklist

## Purpose
Provide a reproducible demo + acceptance checklist for Wave 1 REL (`bd-271.1`) execution queue behavior.

This artifact satisfies `bd-271.1.5.1`:
- reproducible demo steps with expected outputs,
- direct mapping to REL acceptance criteria,
- explicit known gaps/deferred work log.

## Scope Covered in This Demo
- Deterministic queue transitions and retry/recovery semantics.
- Slack thread execution cards (queued, running, completed, retryable, terminal failure).
- REL guardrail policy enforcement with explicit allow/deny/degraded outcomes and audit traces.

## Preconditions
1. Run from repo root: `/data/projects/quotey`.
2. Rust toolchain installed per `.planning/FOUNDATION_QUICKSTART.md`.
3. No destructive commands required for this demo.

## Scripted Demonstration
Run these commands in order.

### Step 1: Deterministic Queue Engine State Transitions
```bash
cargo test -p quotey-core execution_engine::tests:: -- --nocapture
```
Expected:
- test module runs for `execution_engine::tests`.
- transitions cover `queued -> running -> completed|retryable_failed|failed_terminal`.
- output includes all passing transition tests (for example: `recover_stale_tasks_finds_only_stale_running_tasks`).
- no test failures.

### Step 2: Slack Execution Thread Lifecycle Cards
```bash
cargo test -p quotey-slack execution_task_progress -- --nocapture
```
Expected:
- tests pass for:
  - queued status card,
  - running status card,
  - completed status card,
  - retryable failed status card with retry action button,
  - terminal failure status card.
- no test failures.

### Step 3: REL Guardrails (Allowed / Denied / Degraded) + Audit Paths
```bash
cargo test -p quotey-agent runtime::tests:: -- --nocapture
```
Expected:
- tests pass for:
  - `allowed_flow_emits_success_audit_event`,
  - `denied_flow_emits_rejected_audit_event_with_fallback_path`,
  - `degraded_flow_emits_failed_audit_event_with_fallback_path`.
- explicit user-safe response behavior is asserted for denied/degraded paths.
- audit metadata includes denial/degraded fallback path markers.

### Step 4: Agent Guardrail Policy Unit Coverage
```bash
cargo test -p quotey-agent guardrails::tests:: -- --nocapture
```
Expected:
- policy decisions verified for:
  - supported queue action allow,
  - price override denial,
  - ambiguous queue intent degrade.
- no test failures.

### Step 5: Quality Gate for Agent REL Slice
```bash
cargo clippy -p quotey-agent --all-targets -- -D warnings
ubs crates/agent
```
Expected:
- clippy exits clean for `quotey-agent`.
- UBS exits `0` with no critical findings.

## Acceptance Mapping (REL Feature)
| REL acceptance criterion | Demo evidence | Status |
|---|---|---|
| Accessible within quote thread context and tied to quote ID | Step 2 covers thread lifecycle cards; Step 3 audit assertions verify quote-id-aware handling path | Partial (component-level validated) |
| Deterministic engines remain source of truth for financial/policy outcomes | Step 1 transition determinism tests; Step 3 denied responses enforce no chat-side price/approval authority | Pass |
| Audit trail records inputs, decisions, outputs, state transitions | Step 1 transition event tests + Step 3 runtime audit metadata assertions | Pass |
| Errors recoverable and user visible with next actions | Step 2 retryable + terminal cards, Step 3 degraded safe response, runbook recovery flow | Pass |
| Telemetry supports adoption/outcome measurement | KPI/query contract documented in `.planning/W1_REL_EXECUTION_QUEUE_SPEC.md` + runbook SQL probes | Partial (ops/query contract ready) |

## Known Gaps / Deferred Work
1. Full end-to-end wiring from live Slack event ingestion to execution queue service orchestration remains a follow-on integration slice.
2. KPI dashboards/alert automation are defined in spec + runbook but not yet wired to an external metrics backend.
3. Workspace-wide compile gate may be temporarily blocked by in-progress non-REL EXP changes on shared db/core paths; REL demo commands above are intentionally scoped to REL-relevant crates.

## Operator Notes
- Canonical REL references:
  - Spec: `.planning/W1_REL_EXECUTION_QUEUE_SPEC.md`
  - Runbook: `.planning/W1_REL_EXECUTION_QUEUE_RUNBOOK.md`
- If Step 1-5 pass, REL Wave-1 demo checklist is reproducible and auditable for current implementation scope.
