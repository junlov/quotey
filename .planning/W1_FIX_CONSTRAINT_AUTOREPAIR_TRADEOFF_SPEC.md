# W1 FIX Constraint Auto-Repair and Tradeoff Explorer Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-271.3`
(`Constraint Auto-Repair and Tradeoff Explorer`) so invalid configurations can be repaired with
auditable, deterministic alternatives in quote threads.

## Scope
### In Scope
- Quote-thread "fix this config" workflow for invalid configurations.
- Deterministic nearest-valid alternative generation from constraint outputs.
- Tradeoff ranking model (cost, policy friction, fit) implemented as explicit rule weights.
- Explanation payload that shows why each option is valid and what changed.
- Telemetry for invalid-config churn, acceptance rate, and repair latency.

### Out of Scope (for Wave 1)
- LLM-generated configuration repair decisions.
- Non-deterministic multi-objective optimization.
- Cross-quote recommendation learning.
- Automatic quote mutation without explicit user action.

## Rollout Slices
- `Slice A` (contracts): repair request/response schema, tradeoff criteria, deterministic rules.
- `Slice B` (data): persistent snapshots for invalid input, candidate repairs, and decision audit.
- `Slice C` (runtime): deterministic repair engine and ranked tradeoff output generation.
- `Slice D` (UX + ops): Slack option cards, user actions, telemetry, and runbook.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D`
production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Invalid configuration dead-end rate | 22% | <= 5% | Product + Sales Ops owner | `% invalid quote sessions with no successful next action within 10m` |
| Auto-repair option acceptance rate | 0% | >= 60% | Runtime owner | `repair_option_applied / repair_options_presented` |
| Median time from invalid config to valid config | 18 min | <= 4 min | Workflow owner | median `invalid_detected_at -> valid_transition_at` |
| Repair correctness error rate | 7% | <= 0.5% | Determinism owner | `repair_candidates_rejected_as_invalid / repair_candidates_generated` |
| P95 repair suggestion latency | 5.0s | <= 1.5s | Platform owner | request received to Slack repair card posted |

## Deterministic Safety Constraints
- Repair candidates must be derived only from deterministic constraint and policy engines.
- Every candidate must pass deterministic validation before presentation.
- Tradeoff ranking must use explicit, versioned rule weights (no hidden heuristics).
- LLMs may rephrase rationale text only; they cannot generate or rank candidates.
- Candidate application must be explicit and reversible via deterministic state transitions.

## Interface Boundaries (Draft)
### Domain Contracts
- `RepairRequest`: `quote_id`, `thread_id`, `actor_id`, `correlation_id`, invalid snapshot id.
- `RepairCandidate`: modified config delta, validation proof, policy impact summary.
- `TradeoffProfile`: score components (`cost_delta`, `policy_friction`, `fit_penalty`) and total score.
- `RepairDecisionRecord`: selected option id, actor, applied timestamp, resulting quote version.

### Service Contracts
- `ConstraintRepairService::generate_candidates(request) -> Vec<RepairCandidate>`
- `ConstraintRepairService::rank_candidates(candidates, ruleset_version) -> Vec<RankedCandidate>`
- `ConstraintRepairService::validate_candidate(candidate) -> ValidationResult`
- `ConstraintRepairService::apply_candidate(request, candidate_id) -> ApplyResult`

### Persistence Contracts
- `InvalidConfigSnapshotRepo`: append/read invalid configuration snapshots by quote/version.
- `RepairCandidateRepo`: persist deterministic candidates and score breakdowns.
- `RepairDecisionAuditRepo`: append request, ranking, selection, and result transitions.

### Slack Contract
- Repair card valid only in mapped quote thread context.
- Option cards include: what changed, cost/policy tradeoff, and deterministic validation status.
- Action buttons map deterministically to candidate application (`apply`, `show_details`, `dismiss`).
- Error cards include actionable fallback (`retry`, `open manual config`, `request approver context`).

### Crate Boundaries
- `quotey-core`: deterministic candidate generation, ranking, and typed tradeoff model.
- `quotey-db`: snapshot/candidate/audit persistence and retrieval pathways.
- `quotey-slack`: repair interaction rendering and action payload parsing only.
- `quotey-agent`: orchestration and guardrail enforcement; no numeric/policy authority.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Candidate appears valid but violates hidden policy coupling | High | Medium | dual-pass validation (`constraints` then `policy`) before display | Determinism owner |
| Overly aggressive ranking prefers low-fit options | Medium | Medium | versioned score weights + canary review on top-1 acceptance outcomes | Product owner |
| Slack users apply wrong option due unclear deltas | Medium | Medium | mandatory "delta preview" section and confirmation action | Slack owner |
| Persistence drift between invalid snapshot and apply path | High | Low | quote version binding + fail-closed on mismatch | Data owner |
| Latency spikes when candidate set is large | Medium | Medium | bounded candidate count + deterministic pruning before rank | Platform owner |

## Guardrail Checklist (Pre-implementation Exit)
- Scope and non-goals approved for Wave 1.
- KPI baseline, target, owner, and query definitions captured.
- Deterministic constraints mapped to runtime acceptance tests.
- Service and repository contracts aligned with crate ownership boundaries.
- Risk mitigations reviewed and assigned before Task 2 implementation.
