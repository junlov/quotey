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

## Task 2 Migration Behavior and Rollback Expectations
- Migration unit: `migrations/0015_clo_policy_optimizer.{up,down}.sql`.
- Up migration creates six lifecycle tables plus indexes:
  `policy_candidate`, `policy_replay_evaluation`, `policy_approval_decision`,
  `policy_apply_record`, `policy_rollback_record`, `policy_lifecycle_audit`.
- Up migration is idempotent (`IF NOT EXISTS`) and must apply on both empty databases and
  already-migrated environments without destructive rewrites.
- Down migration drops CLO indexes/tables in dependency-safe reverse order to preserve full
  reversibility for `MIGRATOR.undo(...)` and up/down/up schema signature checks.
- Persistence fixtures for edge conditions are required in repository tests:
  conflicting candidates (same base version), stale approvals (`is_stale=1` + expired),
  and rollback chains (`parent_rollback_id`, increasing `rollback_depth`).
- Downstream engine tasks (`bd-lmuc.3+`) must treat idempotency keys and replay checksums as
  authoritative dedupe markers for apply/rollback/audit operations.

## Task 3 Deterministic Replay Engine Contract
- Engine module: `crates/core/src/policy/optimizer.rs`.
- Replay request contract includes:
  `candidate_id`, policy versions, canonicalizable `policy_diff_json`,
  canonicalizable `cohort_scope_json`, pinned `engine_version`, optional expected checksum,
  and a non-empty historical snapshot cohort.
- Snapshot contract per historical quote includes baseline/candidate values for:
  margin bps, win-rate proxy bps, approval-required flag, hard-violation counts,
  plus cohort/segment/rule annotations for blast-radius analysis.
- Deterministic checksum behavior:
  - request is normalized (sorted snapshots, deduped/sorted rule IDs, canonical JSON serialization),
  - checksum is `sha256:<hex>` over canonical payload bytes,
  - if `expected_input_checksum` is provided and mismatches, replay is blocked.
- Impact metrics produced per replay:
  - `projected_margin_delta_bps` (cohort average),
  - `projected_win_rate_proxy_delta_bps` (cohort average),
  - `projected_approval_load_delta_bps` (delta in approval-required rate),
  - `projected_hard_violation_delta` (cohort aggregate delta).
- Blast-radius summary is order-independent and stable:
  - `impacted_quote_count`,
  - `impacted_quote_ratio_bps`,
  - sorted unique impacted segment keys,
  - sorted unique impacted rule IDs,
  - sorted unique impacted cohort IDs.
- Hard guardrail gates block candidate promotion when thresholds are violated:
  - margin delta below minimum,
  - win-rate proxy delta below minimum,
  - approval-load delta above maximum,
  - hard-violation delta above maximum.
- Determinism invariant tests required in core unit tests:
  - identical input -> identical output,
  - checksum mismatch -> explicit blocked error,
  - reversed snapshot ordering (and rule list permutations) -> identical blast-radius output/checksum.

## Task 4 Candidate Diff Schema + Generation Contracts
- Candidate schema implementation lives in `crates/core/src/policy/optimizer.rs` as
  `PolicyCandidateDiffV1` with explicit schema version `clo_candidate_diff.v1`.
- Canonical schema fields include:
  - rule-level diffs (`rule_id`, operation, field, before/after JSON values, rationale),
  - cohort scope (`segment_keys`, `region_keys`, `quote_ids`, `time_window_days`),
  - projected impact (replay checksum + deterministic pass bit + key deltas),
  - confidence bounds (`lower_bps`, `point_estimate_bps`, `upper_bps`),
  - provenance (`source_replay_evaluation_ids`, outcome window, generator identity),
  - deterministic rationale summary.
- Deterministic serialization contract:
  - normalization lowercases/sorts canonical identity fields where appropriate,
  - list fields are deduped and sorted,
  - value payloads are canonicalized JSON,
  - output is a stable canonical JSON string suitable for replay/audit artifacts.
- Validation contract (`PolicyCandidateDiffV1::validate`):
  - rejects empty candidate IDs, missing rule diffs, invalid per-operation payloads,
    duplicate rule-diff keys, invalid/empty cohort scope, invalid checksum,
    non-deterministic replay evidence, malformed confidence bounds, missing provenance,
    and empty rationale summary.
  - rejects unsafe candidate payloads when projected hard-violation delta is positive.
- Candidate generation contract (`PolicyCandidateGenerator`):
  - requires replay evidence + rule signals + scope/provenance/confidence inputs,
  - links generated candidate payload directly to replay checksum/impact deltas,
  - emits deterministic machine-readable candidate diff JSON + provenance/cohort artifacts,
  - auto-rejects high-risk candidates when replay guardrails fail (with explicit reasons).
- Task 4 invariant tests in core cover:
  - candidate diff canonicalization stability under ordering/casing variations,
  - validation rejection for incomplete/unsafe payloads,
  - deterministic generator output for identical inputs,
  - explicit guardrail-based rejection for unsafe replay evidence.

## Task 5 Approval Packet Contract + Reviewer UX
- Approval packet contract is implemented in `crates/core/src/policy/optimizer.rs`:
  - `ApprovalPacket` (schema version `clo_approval_packet.v1`)
  - `ApprovalPacketActionPayload` (action version `clo_approval_packet_action.v1`)
  - deterministic packet/action id generation and strict section/payload validation.
- Required packet sections are enforced by contract validation:
  - candidate diff section (`PolicyCandidateDiffV1`),
  - replay evidence section (`ReplayImpactReport`),
  - risk score section (`risk_score_bps`),
  - blast-radius section (`BlastRadiusSummary`),
  - fallback plan section (`fallback_plan`).
- Packet/action idempotency + version-awareness:
  - packet IDs are deterministic hashes over candidate/version/checksum identity,
  - action payloads include explicit action-version metadata and deterministic idempotency keys,
  - reject/request-changes actions require explicit reviewer reason text.
- Deterministic transition mapping:
  - `approve -> approved`
  - `reject -> rejected`
  - `request_changes -> changes_requested`
  - implemented via `ApprovalPacketActionPayload::target_status`.
- Append-only audit event contract:
  - reviewer action payload can deterministically emit `PolicyLifecycleAuditEvent`
    with stable idempotency key linkage.
- Slack reviewer surface (`crates/slack/src/blocks.rs`):
  - `policy_approval_packet_message` renders all required packet sections,
  - action buttons include version-aware idempotent payload values,
  - reject/request-changes actions are explicitly labeled as reason-required.
- CLI reviewer surface (`crates/cli/src/commands/policy_packet.rs` + `crates/cli/src/lib.rs`):
  - `quotey policy-packet build` builds deterministic approval packets from JSON contracts,
  - `quotey policy-packet action` emits deterministic reviewer action payload + target status.
- Task 5 tests validate:
  - packet ID determinism/versioning,
  - required-section schema enforcement,
  - deterministic action idempotency and status mapping,
  - action payload version gating and audit-event generation,
  - Slack packet-card required-section/action rendering,
  - CLI build/action command behavior and reject-without-reason guardrail.

## Task 6 Signed Apply/Rollback Pipeline + Task 6.1 Drill Automation
- Lifecycle engine implementation is in `crates/core/src/policy/optimizer.rs`:
  - `InMemoryPolicyLifecycleEngine`
  - `PolicyApplyRequest` / `PolicyApplyOutcome`
  - `PolicyRollbackRequest` / `PolicyRollbackOutcome`
  - `RollbackDrillReport`
  - `PolicyLifecycleError` with `user_safe_message()` remediation text.
- Apply contract (`InMemoryPolicyLifecycleEngine::apply`):
  - requires valid `ApprovalPacket` and `ApprovalPacketActionPayload`,
  - requires `approve` decision (reject/request-changes are blocked),
  - requires packet/action candidate + version identity match,
  - requires replay checksum parity between candidate diff and replay report,
  - requires current active policy version to match packet base version,
  - requires signing metadata (`signature_key_id`, `signing_secret`) and emits deterministic apply signature/checksum artifacts.
- Apply idempotency + auditability:
  - idempotency key is action-derived by default (or explicit override),
  - repeated apply requests with same idempotency key return the existing `PolicyApplyRecord`,
  - apply events are append-only `PolicyLifecycleAuditEvent` records with deterministic correlation/idempotency linkage.
- Rollback contract (`InMemoryPolicyLifecycleEngine::rollback`):
  - requires non-empty rollback reason,
  - requires apply record existence and candidate identity match,
  - requires active policy version to match the applied version being rolled back,
  - restores `prior_policy_version` deterministically and emits signed rollback artifacts,
  - supports idempotent rollback retries via rollback idempotency key.
- Queryability contract:
  - apply records are queryable by applied policy version,
  - rollback records are queryable by rollback target policy version,
  - lifecycle event stream remains immutable and ordered by emitted operations.
- Drill automation contract (`InMemoryPolicyLifecycleEngine::run_rollback_drill`):
  - executes deterministic forward apply -> rollback -> re-apply sequence,
  - outputs `RollbackDrillReport` containing timing metrics (`*_duration_ms`),
    verification checksums (first apply / rollback / reapply), safety pass bit, and final policy version.
- Task 6/6.1 tests in `policy::optimizer::tests` validate:
  - apply gating for approval decision + replay checksum integrity,
  - idempotent apply/rollback behavior and lifecycle event immutability,
  - queryability of apply/rollback records by policy version,
  - rollback drill artifact completeness and safety pass conditions,
  - user-safe remediation messaging for key failure paths.
- Runbook ownership/cadence requirement (for operator documentation in Task 9):
  - rollback drill must run at least weekly,
  - primary owner is Runtime owner (backup owner: Revenue Ops owner),
  - each drill execution must retain report artifacts for audit trail review.

## Task 7 Safety Evaluation + Red-Team Harness
- Red-team evaluation contract is implemented in `crates/core/src/policy/optimizer.rs`:
  - `RedTeamScenarioCode`
  - `RedTeamScenarioOutcome`
  - `RedTeamSafetyEvaluation` (schema `clo_red_team_eval.v1`)
  - deterministic scenario evaluators + artifact checksum generation.
- Required adversarial scenarios covered in deterministic checks:
  - `margin_collapse`: blocks broad-impact margin degradation outside the safe envelope.
  - `policy_bypass`: blocks manipulative control-surface edits (for example removal/disable of approval/guardrail controls).
  - `biased_cohort_regression`: blocks concentrated single-segment win-rate regressions.
- Candidate generation integration (`PolicyCandidateGenerator::generate`):
  - executes red-team evaluation before candidate package emission,
  - blocks unsafe candidates with explicit scenario-specific reasons,
  - emits a canonical `safety_evaluation_json` artifact under candidate provenance when checks pass.
- Candidate validation hardening (`PolicyCandidateDiffV1::validate`):
  - validates and normalizes `safety_evaluation_json` when present,
  - enforces schema version, candidate/replay checksum linkage, and evidence checksum integrity,
  - rejects blocked/unsafe safety artifacts from entering approval/apply workflows.
- Approval packet attachment + persistence contract:
  - safety artifact is embedded in candidate provenance, which is included in approval packets,
  - artifact persists through existing candidate storage paths (`policy_diff_json` / `provenance_json`) without non-deterministic mutation.
- Task 7 tests validate:
  - explicit blocking for all three adversarial scenario classes,
  - deterministic artifact generation in successful candidate builds,
  - compatibility with existing replay/approval/apply regression suites.

## Task 8 Telemetry + Outcome Measurement
- Telemetry/KPI contract is implemented in `crates/core/src/policy/optimizer.rs`:
  - `RealizedOutcomeObservation`
  - `PolicyLifecycleTelemetryThresholds`
  - `PolicyLifecycleKpiSnapshot`
  - `compute_policy_lifecycle_kpis(...)`
- KPI formulas map to deterministic lifecycle/replay/outcome fields:
  - candidate throughput = count of `candidate_created` lifecycle events.
  - review decision count = count of `{approved,rejected,changes_requested}` lifecycle events.
  - adoption rate (bps) = `approved_count / review_decision_count`.
  - rollback rate (bps) = unique rolled-back candidates / unique applied candidates.
  - false-positive rate (bps) = candidates that were both applied and later rolled back / applied candidates.
  - approval latency (seconds) = average `(first decision ts - review packet ts)` per candidate.
  - projected margin delta (bps) = average from replay reports (`projected_margin_delta_bps`).
  - realized margin delta (bps) = average from latest realized observations per candidate.
  - projected-vs-realized gap (bps) = average `(projected_margin_delta_bps - realized_margin_delta_bps)` on overlap set.
- Alert thresholds are deterministic and explicit:
  - rollback spike: `rollback_rate_bps > max_rollback_rate_bps`
  - false-positive spike: `false_positive_rate_bps > max_false_positive_rate_bps`
  - projected/realized drift: `abs(avg_projected_vs_realized_margin_gap_bps) > max_projected_realized_margin_gap_bps`
- Query/dashboards alignment:
  - `PolicyLifecycleKpiSnapshot` provides direct, persisted-field-compatible values for
    projected-vs-realized comparison cards and operator alerting surfaces.
- Task 8 tests validate:
  - deterministic KPI outputs under reordered event/report/outcome inputs,
  - correct formula outputs for throughput/adoption/rollback/latency/margin metrics,
  - alert behavior for rollback spikes, false-positive spikes, and projected-realized drift.

## Guardrail Exit Checklist (Before Task 2 Coding)
- [ ] Scope/non-goals approved.
- [ ] KPI formulas and owners locked.
- [ ] Deterministic safety constraints mapped to tests.
- [ ] Candidate lifecycle contracts reviewed across core/db/slack/cli boundaries.
- [ ] Risk mitigations assigned with owning beads.

## Notes
- This spec is planning source-of-truth for the CLO track.
- Beads are the execution system-of-record; update statuses there as work progresses.
