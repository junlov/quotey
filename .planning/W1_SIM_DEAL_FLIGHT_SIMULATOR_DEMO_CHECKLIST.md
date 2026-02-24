# W1 SIM Deal Flight Simulator Demo Checklist

## Purpose

Provide a reproducible demo and acceptance checklist for Wave 1 SIM (`bd-271.7`).

This artifact satisfies:
- `bd-271.7.5.1` (demo checklist),
- `bd-271.7.5.3` (runbook/demo docs),
- prerequisite evidence for `bd-271.7.5.4` rollout gate.

## Scope Covered

- Deterministic scenario generation and ranking
- Scenario telemetry event taxonomy/counter contracts
- Scenario persistence and promotion lifecycle
- Slack command parsing/comparison card/promotion action handling

## Preconditions

1. Run from repo root: `/data/projects/quotey`.
2. Toolchain installed per `.planning/FOUNDATION_QUICKSTART.md`.
3. No destructive operations required.

## Scripted Demonstration

Run in order.

### Step 1: Core Deterministic Simulator Behavior
```bash
cargo test -p quotey-core cpq::simulator::tests:: -- --nocapture
```
Expected:
- pass for deterministic replay/parity,
- pass for guardrail rejection coverage,
- pass for telemetry emission tests (`usage`, `latency`, `outcome`).

### Step 2: Simulation Domain Contracts (Enums + Counters)
```bash
cargo test -p quotey-core domain::simulation::tests:: -- --nocapture
```
Expected:
- parse/as_str round-trips pass,
- telemetry counter-delta determinism test passes.

### Step 3: Persistence + Promotion Lifecycle
```bash
cargo test -p quotey-db repositories::simulation::tests:: -- --nocapture
```
Expected:
- run lifecycle test passes,
- variant/delta/audit/promotion round-trip test passes,
- missing variant promotion safety test passes.

### Step 4: Slack UX + Command Surface
```bash
cargo test -p quotey-slack --all-targets -- --nocapture
```
Expected:
- `/quote simulate` parse tests pass,
- comparison card render and promote button payload tests pass,
- promotion payload validation/idempotency tests pass.

### Step 5: Formatting Gate
```bash
cargo fmt --all --check
```
Expected:
- no formatting drift.

## Acceptance Mapping (SIM Task 5)

| SIM Task 5 acceptance criterion | Demo evidence | Status |
|---|---|---|
| Unit/integration/scenario tests added | Steps 1-4 | Pass |
| Metrics logs and alertable signals emitted | Step 1 + telemetry contract in SIM spec/runbook | Pass (contract-level) |
| Demo script and operator runbook included | This file + `.planning/W1_SIM_DEAL_FLIGHT_SIMULATOR_RUNBOOK.md` | Pass |
| Follow-up limitations documented | Known gaps section below | Pass |

## Known Gaps / Deferred Work

1. Telemetry events/counter deltas are implemented as deterministic contracts and sink emission paths; external dashboard wiring is still pending.
2. Promotion telemetry emission hooks are specified but require orchestration-layer integration when the end-to-end SIM service wiring lands.
3. Full Slack live-thread integration remains scoped to runtime wiring slices; current validation is command/event contract level.

## Rollout Gate Inputs

A Wave-1 SIM rollout decision is **Go** only if:
1. Steps 1-5 all pass,
2. no deterministic parity regression is observed,
3. no open P0/P1 SIM bugs exist in `br`.

## References

- Spec: `.planning/W1_SIM_DEAL_FLIGHT_SIMULATOR_SPEC.md`
- Runbook: `.planning/W1_SIM_DEAL_FLIGHT_SIMULATOR_RUNBOOK.md`
- Core simulator: `crates/core/src/cpq/simulator.rs`
- SIM domain contracts: `crates/core/src/domain/simulation.rs`
- SIM repository: `crates/db/src/repositories/simulation.rs`
