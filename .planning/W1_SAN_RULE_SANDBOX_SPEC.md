# W1 SAN Rule Sandbox and Blast Radius Replay Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-271.10`
(`Rule Sandbox and Blast Radius Replay`) so deal desk can evolve pricing policy without regressions
by replaying historical quotes under proposed changes.

## Scope
### In Scope
- Historical quote replay against proposed policy/rule changes.
- Before/after diff computation with full trace evidence.
- Impact quantification (quotes affected, revenue impact, violation changes).
- CLI interface for policy testing and Slack summary reports.
- Deterministic replay guarantee (same inputs → same outputs).
- Blast radius visualization (which quotes would be impacted).

### Out of Scope (for Wave 1)
- Automatic policy optimization or ML-based rule suggestion.
- Real-time A/B testing of policies in production.
- Multi-policy simulation with interaction effects analysis.
- Cross-tenant policy comparison or benchmarking.
- Predictive modeling of future quote patterns.

## Rollout Slices
- `Slice A` (contracts): replay request/response schema, diff model, impact metrics.
- `Slice B` (engine): historical quote retrieval, policy override, deterministic replay.
- `Slice C` (CLI + UX): CLI replay commands, Slack summary cards, blast radius reports.
- `Slice D` (ops): impact dashboards, policy change approval workflow, runbook.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Replay determinism rate | N/A | 100% | Determinism owner | `identical_replays_same_result / total_replays` |
| Policy change detection accuracy | N/A | >= 99% | Runtime owner | `correct_impact_predictions / total_changes` |
| Median replay latency (per quote) | N/A | <= 200ms | Platform owner | time to replay single quote under new policy |
| Blast radius coverage | N/A | >= 95% | Data owner | `% affected quotes identified in 12mo window` |
| Policy regression prevention | N/A | >= 90% | Product owner | `% policy changes with zero unintended impacts` |
| CLI replay success rate | N/A | >= 98% | Runtime owner | `successful_cli_replays / total_cli_requests` |

## Deterministic Safety Constraints
- Replay must use identical CPQ engines with only the specified policy/rule overrides.
- Historical quote state must be immutable; replay operates on snapshots, never live data.
- Identical inputs (quote + policy override) must produce identical outputs 100% of time.
- Impact calculations must be derived from deterministic diff, never LLM estimation.
- Policy changes cannot be applied to production without explicit approval workflow.
- All replay results must be persisted with full trace for audit purposes.

## Interface Boundaries (Draft)
### Domain Contracts
- `ReplayRequest`: `policy_override`, `quote_filter` (date range, segments), `baseline_policy_ref`.
- `ReplayResult`: `quote_id`, `baseline_outcome`, `proposed_outcome`, `diff`, `impact_level`.
- `BlastRadiusReport`: `affected_quotes_count`, `revenue_impact`, `violation_changes`, `segment_breakdown`.
- `PolicyOverride`: rule changes, threshold adjustments, formula modifications.

### Service Contracts
- `RuleSandbox::replay_quote(quote_id, policy_override) -> ReplayResult`
- `RuleSandbox::replay_batch(filter, policy_override) -> Vec<ReplayResult>`
- `RuleSandbox::compute_blast_radius(policy_override) -> BlastRadiusReport`
- `RuleSandbox::diff_outcomes(baseline, proposed) -> OutcomeDiff`
- `RuleSandbox::propose_policy_change(diff) -> PolicyChangeProposal`

### Persistence Contracts
- `PolicySnapshotRepo`: versioned policy state for replay baselines.
- `ReplayResultRepo`: store replay outcomes with policy references.
- `BlastRadiusRepo`: cache impact analysis for proposed changes.
- `PolicyChangeAuditRepo`: append-only log of proposed and applied changes.

### CLI Contract
- `quotey sandbox replay --quote-id=Q-2026-001 --policy-file=new-policy.toml`
- `quotey sandbox blast-radius --policy-file=new-policy.toml --since=2025-01-01`
- `quotey sandbox diff --baseline=v1.2 --proposed=v1.3`

### Slack Contract
- Summary report posted to `#deal-desk` with impact overview.
- Drill-down links to detailed quote-level diffs.
- Approval workflow buttons for policy change promotion.

### Crate Boundaries
- `quotey-core`: replay engine, diff computation, impact analysis.
- `quotey-db`: historical quote snapshots, policy versioning, audit logging.
- `quotey-cli`: command-line interface for replay and blast radius.
- `quotey-slack`: report rendering and approval workflow (no business logic).

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Replay uses wrong policy version as baseline | High | Medium | explicit version pinning + validation checks | Data owner |
| Historical quote data incomplete or corrupted | Medium | Medium | data quality checks + missing data alerts | Data owner |
| Non-deterministic results from engine changes | High | Low | engine version pinning + deterministic test suite | Determinism owner |
| False negatives in blast radius (missed impacts) | High | Medium | comprehensive quote sampling + edge case tests | Runtime owner |
| Policy change approved without adequate review | Medium | Medium | mandatory approval workflow + impact thresholds | Product owner |
| Performance issues with large historical datasets | Medium | Medium | incremental replay + caching + async processing | Platform owner |

## Guardrail Checklist (Pre-implementation Exit)
- [ ] Scope and non-goals approved for Wave 1 execution.
- [ ] KPI baseline, target, owner, and query definitions captured.
- [ ] Deterministic constraints mapped to runtime acceptance tests.
- [ ] Service and repository contracts aligned with crate ownership boundaries.
- [ ] Risk mitigations reviewed and assigned before Task 2 implementation.
- [ ] CLI interface reviewed for consistency with existing commands.
