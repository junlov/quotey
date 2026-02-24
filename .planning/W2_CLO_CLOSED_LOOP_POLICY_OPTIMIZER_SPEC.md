# W2 CLO Closed-Loop Policy Optimizer Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, interfaces, and execution backlog for `bd-lmuc`
(`W2 [CLO] Closed-Loop Policy Optimizer`) so Quotey can safely evolve pricing/policy rules from
real outcomes without violating deterministic CPQ guarantees.

## Problem Statement
Quotey can currently evaluate and enforce policy deterministically, but policy evolution is still
manual and episodic. We need a controlled feedback loop that:
- learns from won/lost outcomes and approval outcomes,
- proposes policy improvements with evidence,
- proves blast-radius impact deterministically before promotion,
- requires explicit human approval and signed apply,
- supports auditable rollback when real-world outcomes diverge.

## Product Goal
Increase revenue policy quality over time while preserving deterministic and auditable behavior.
The optimizer must be accretive (net positive impact), reversible, and human-governed.

## Scope
### In Scope
- Candidate policy proposal lifecycle (`draft -> replayed -> approved -> applied -> monitored -> rolled_back?`).
- Deterministic historical replay and impact scoring for each candidate.
- Human review packet in Slack/CLI with explicit approve/reject/change-request actions.
- Signed policy apply + rollback pipeline with immutable audit events.
- Outcome telemetry for projected-vs-realized impact and safety guardrail monitoring.

### Out of Scope (Wave 2)
- Fully autonomous policy application without human approval.
- Multi-armed bandit or online experimentation on live deals.
- Cross-tenant learning or external benchmark-driven policy mutation.
- Non-deterministic policy evaluation paths.
- Direct LLM authority over pricing, approval routing, or compliance decisions.

## Rollout Slices
- `Slice A` (contracts/spec): candidate schema, KPI contract, hard guardrails, risk thresholds.
- `Slice B` (data): persistence schema and repositories for lifecycle state + replay artifacts.
- `Slice C` (engine): deterministic replay, impact scoring, and candidate gating.
- `Slice D` (review/apply): approval packets + signed apply/rollback workflow.
- `Slice E` (ops): telemetry, runbook, drills, and end-to-end rollout gate.

`Slice A` must be complete before `Slice B/C` execution.
`Slice C` must pass determinism/safety gates before `Slice D` apply capability is enabled.
`Slice D` and `Slice E` must complete before production rollout.

## KPI Contract
| KPI | Baseline | Wave-2 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Candidate acceptance rate | N/A | 20-40% | Product owner | `approved_candidates / replayed_candidates` |
| Projected-vs-realized margin delta error | N/A | <= 5% absolute | Determinism owner | `abs(projected_margin_delta - realized_margin_delta)` |
| Unsafe candidate block rate | N/A | 100% for hard violations | Safety owner | `% hard-violation candidates prevented from approval` |
| Rollback MTTR | N/A | <= 15m | Runtime owner | time from rollback trigger to prior-policy restoration |
| Policy improvement realization | N/A | >= +2% weighted outcome score/quarter | Revenue ops owner | weighted blend of win rate, margin, approval latency |
| Replay determinism rate | N/A | 100% | Core owner | `identical_replay_inputs_with_identical_outputs / total_repeats` |

## Deterministic Safety Constraints
- LLMs can suggest candidate diffs and narrative explanations only; they cannot apply policy.
- Every candidate must include canonical policy diff + replay checksum before approval actions are enabled.
- Replay must execute against immutable historical snapshots and pinned engine/version references.
- Hard safety thresholds (for example margin floor violations) must auto-block candidate promotion.
- Apply must require signed approval artifact matching candidate ID + replay checksum + policy base version.
- Rollback must be deterministic, idempotent, and restore prior policy state exactly.

## Candidate Lifecycle Contract
1. `draft`: candidate created from outcome analysis.
2. `replayed`: deterministic replay completed with impact report.
3. `review_ready`: packet generated with evidence + risk summary.
4. `approved` or `rejected` or `changes_requested`.
5. `applied`: signed migration executed and audited.
6. `monitoring`: realized impact tracked against forecast.
7. `rolled_back` (optional): signed rollback to prior version.

## Interface Boundaries (Draft)
### Domain Contracts
- `PolicyCandidate`: id, base_policy_version, diff, provenance, confidence, cohort_scope.
- `ReplayEvaluation`: deterministic checksum, cohort metrics, blast radius, risk flags.
- `ApprovalPacket`: candidate snapshot + replay evidence + apply/rollback plan.
- `PolicyApplyRecord`: signature metadata, actor, checksum, applied version.
- `PolicyRollbackRecord`: rollback reason, target version, verification checksum.

### Service Contracts
- `CandidateService::propose(from_outcomes) -> PolicyCandidate`
- `ReplayService::evaluate(candidate_id, scope) -> ReplayEvaluation`
- `PacketService::build(candidate_id) -> ApprovalPacket`
- `ApprovalService::decide(packet_id, decision) -> DecisionResult`
- `PolicyLifecycleService::apply(candidate_id, signature) -> ApplyResult`
- `PolicyLifecycleService::rollback(apply_id, reason, signature) -> RollbackResult`
- `MonitoringService::compute_realized_delta(candidate_id, window) -> RealizedImpact`

### Persistence Contracts
- `PolicyCandidateRepo`
- `ReplayEvaluationRepo`
- `ApprovalDecisionRepo`
- `PolicyApplyRepo`
- `PolicyRollbackRepo`
- `PolicyImpactTelemetryRepo`

### UX Contracts
- Slack packet actions: `approve`, `reject`, `request_changes`, `view_diff`, `view_replay`, `rollback`.
- CLI controls: `quotey policy-opt propose|replay|packet|approve|apply|monitor|rollback`.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Candidate suggests profitable but non-compliant policy | High | Medium | hard compliance guardrails + auto-block status | Safety owner |
| Replay cohort bias leads to misleading projections | High | Medium | cohort stratification checks + bias diagnostics in packet | Data owner |
| Unauthorized apply/rollback | High | Low | signed approvals + role-gated commands + immutable audit | Security owner |
| Forecast drift after apply | Medium | Medium | monitoring alerts + rollback thresholds + cooldown windows | Runtime owner |
| Operator overload from noisy candidates | Medium | Medium | candidate scoring thresholds + batched review cadence | Product owner |
| Determinism regression from engine changes | High | Low | replay checksum pinning + invariant tests in CI | Core owner |

## Execution Backlog (Canonical TODO List)
These items are tracked as beads and are the authoritative implementation checklist.

### Epic
- [ ] `bd-lmuc` W2 [CLO] Closed-Loop Policy Optimizer

### Primary Tasks
- [ ] `bd-lmuc.1` Spec KPI Guardrails
- [ ] `bd-lmuc.2` Data Model Persistence
- [ ] `bd-lmuc.3` Deterministic Replay Impact Engine
- [ ] `bd-lmuc.4` Candidate Generation + Explainability
- [ ] `bd-lmuc.5` Approval Packet UX (Slack + CLI)
- [ ] `bd-lmuc.6` Signed Apply + Rollback Pipeline
- [ ] `bd-lmuc.7` Safety Evaluation + Red-Team Harness
- [ ] `bd-lmuc.8` Telemetry + Outcome Measurement
- [ ] `bd-lmuc.9` Operator Controls + Runbook
- [ ] `bd-lmuc.10` End-to-End Demo + Rollout Gate

### Granular Subtasks
- [ ] `bd-lmuc.2.1` Migration Fixtures
- [ ] `bd-lmuc.3.1` Determinism Invariant Tests
- [ ] `bd-lmuc.4.1` Candidate Diff Schema
- [ ] `bd-lmuc.5.1` Approval Packet Contract
- [ ] `bd-lmuc.6.1` Rollback Drill Automation
- [ ] `bd-lmuc.7.1` Adversarial Scenario Corpus
- [ ] `bd-lmuc.8.1` KPI Query Pack
- [ ] `bd-lmuc.10.1` Demo Checklist Artifact

### Dependency Order (Execution Plan)
1. `bd-lmuc.1`
2. `bd-lmuc.2` + `bd-lmuc.2.1`
3. `bd-lmuc.3` + `bd-lmuc.3.1`
4. `bd-lmuc.4` + `bd-lmuc.4.1`
5. `bd-lmuc.5` + `bd-lmuc.5.1`
6. `bd-lmuc.6` + `bd-lmuc.6.1`
7. `bd-lmuc.7` + `bd-lmuc.7.1`
8. `bd-lmuc.8` + `bd-lmuc.8.1`
9. `bd-lmuc.9`
10. `bd-lmuc.10` + `bd-lmuc.10.1`

## Quality Gates
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets --all-features`
- `ubs --diff` scoped to changed files
- Determinism replay invariants and rollback drills must pass before rollout gate

## Guardrail Exit Checklist (Before Task 2 Coding)
- [ ] Scope/non-goals approved.
- [ ] KPI formulas and owners locked.
- [ ] Deterministic safety constraints mapped to tests.
- [ ] Candidate lifecycle contracts reviewed across core/db/slack/cli boundaries.
- [ ] Risk mitigations assigned with owning beads.

## Notes
- This spec is planning source-of-truth for the CLO track.
- Beads are the execution system-of-record; update statuses there as work progresses.
