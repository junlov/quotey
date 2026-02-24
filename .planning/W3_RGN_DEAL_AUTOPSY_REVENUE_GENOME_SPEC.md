# W3 RGN Deal Autopsy & Revenue Genome Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, interfaces, and execution backlog for `bd-rgn`
(`W3 [RGN] Deal Autopsy & Revenue Genome`) so Quotey can perform automated causal attribution on
every deal outcome using deterministic audit trails, building a queryable Revenue Genome that
identifies which decisions at which stages produce which outcomes.

## Problem Statement
Quotey collects the richest audit trail in CPQ — immutable ledger entries, pricing traces, policy
evaluations, negotiation transcripts, approval chains — but nothing reasons over outcomes using
that data. CLO learns rule improvements but doesn't understand WHY deals succeed or fail.
Precedent Intelligence finds similar deals but doesn't explain causation. Win Probability predicts
but doesn't prescribe strategy.

RGN closes the loop by performing deterministic causal attribution from actual audit trails.
Every terminal deal is autopsied into a structured set of decision forks with outcome-attributed
scores, building a persistent attribution graph (the Revenue Genome) that answers causal
strategy questions with deterministic, evidence-backed results.

## Product Goal
Build a compound learning system that gets measurably smarter with every deal. RGN must be
deterministic (same inputs → same outputs), evidence-backed (every attribution traceable to
audit trail entries), and accretive (feeds higher-quality inputs to CLO candidate generation,
NXT strategy ranking, Precedent Intelligence scoring, and Ghost Quote simulation).

## Scope
### In Scope
- Deal autopsy lifecycle (terminal quote → structured autopsy report with attributed decision forks).
- Decision fork extraction from audit trails (pricing path, constraint resolution, discount level,
  approval exceptions, negotiation concessions).
- Deterministic causal attribution scoring (outcome attribution to specific decision points).
- Attribution graph persistence in SQLite (decision forks as nodes, outcome-weighted edges).
- Revenue Genome queries (strategy questions → deterministic evidence-backed answers).
- Counterfactual simulation via deterministic replay ("what if we had offered X instead of Y?").
- Temporal pattern detection (how deal patterns shift across time periods).
- Integration hooks for CLO candidate generation, NXT strategy ranking, Precedent Intelligence
  scoring, and Ghost Quote simulation enrichment.
- Telemetry for attribution quality, query accuracy, and genome coverage metrics.

### Out of Scope (Wave 3)
- ML/neural network-based attribution (deterministic rule-based only).
- Real-time attribution during active deals (batch/post-outcome only).
- Cross-tenant genome sharing or federated attribution graphs.
- Autonomous strategy execution without human review.
- Direct LLM authority over attribution scores, genome mutations, or causal conclusions.
- Non-deterministic attribution evaluation paths.

## Rollout Slices
- `Slice A` (contracts/spec): domain contracts, KPI contract, deterministic guardrails, attribution model.
- `Slice B` (data): persistence schema, repositories for autopsy lifecycle + attribution graph.
- `Slice C` (engine): deal autopsy engine, decision fork extractor, attribution scorer.
- `Slice D` (genome): attribution graph builder, revenue genome query engine, counterfactual simulator.
- `Slice E` (integration): CLO/NXT/Precedent/Ghost integration hooks, telemetry.
- `Slice F` (operations): CLI commands, demo script, rollout gate.

`Slice A` must complete before `Slice B/C` execution.
`Slice C` must pass determinism/safety gates before `Slice D` genome capability is enabled.
`Slice D/E` must complete before `Slice F` rollout decision.

## KPI Contract
| KPI | Baseline | Wave-3 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Attribution coverage | N/A | >= 90% of terminal deals | Data owner | `autopsied_terminal_deals / total_terminal_deals` |
| Decision fork extraction accuracy | N/A | >= 95% | Core owner | `correctly_extracted_forks / total_extractable_forks` (validated against audit trail sample) |
| Query determinism rate | N/A | 100% | Core owner | `identical_query_inputs_with_identical_outputs / total_query_repeats` |
| Counterfactual replay determinism | N/A | 100% | Core owner | `identical_counterfactual_inputs_with_identical_outputs / total_counterfactual_repeats` |
| Genome query P95 latency | N/A | <= 200ms | Platform owner | request received to result returned |
| Attribution-enhanced CLO acceptance rate improvement | N/A | >= +5% absolute | Revenue ops owner | `clo_acceptance_rate_with_rgn - clo_acceptance_rate_without_rgn` |
| Attribution-enhanced NXT suggestion acceptance improvement | N/A | >= +10% absolute | Product owner | `nxt_acceptance_rate_with_rgn - nxt_acceptance_rate_without_rgn` |

## Deterministic Safety Constraints
- LLMs may summarize attribution findings and narrate autopsy reports; they cannot set attribution
  scores, modify the attribution graph, or author causal conclusions.
- Attribution scoring is deterministic: same deal outcome + same audit trail → same attribution report.
- Counterfactual simulations use the existing deterministic replay engine; no new non-deterministic
  evaluation paths are introduced.
- Attribution graph mutations are append-only; existing nodes and edges are never modified or deleted.
- All genome queries are computed from persisted attribution data, never from LLM inference.
- Decision fork extraction operates exclusively on structured audit trail fields; free-text/NLP
  analysis may enrich summaries but cannot alter fork identity, type, or attribution weight.
- Attribution confidence bounds must be explicit and deterministic; unbounded confidence claims
  are rejected at validation.

## Deal Autopsy Lifecycle Contract
1. `pending`: terminal deal outcome detected, autopsy queued.
2. `extracting`: decision fork extraction in progress against audit trail.
3. `scoring`: attribution scoring in progress against extracted forks.
4. `complete`: autopsy finalized with attribution report and checksum.
5. `integrated`: attribution data merged into Revenue Genome graph.
6. `failed`: extraction or scoring failed with explicit error; retryable.

## Interface Boundaries (Draft)
### Domain Contracts
- `DealAutopsyId(String)`: unique autopsy identifier.
- `AttributionNodeId(String)`: unique node in attribution graph.
- `AttributionEdgeId(String)`: unique edge in attribution graph.
- `GenomeQueryId(String)`: unique query identifier for audit.
- `DealAutopsy`: id, quote_id, outcome_status, decision_forks, attribution_scores,
  audit_trail_refs, checksum, lifecycle_state, created_at, completed_at.
- `DecisionFork`: fork_id, fork_type, stage, options_considered, option_chosen,
  audit_ref, timestamp.
- `AttributionScore`: fork_id, outcome_contribution_bps, confidence_bps,
  confidence_lower_bps, confidence_upper_bps, evidence_refs.
- `AttributionNode`: node_id, fork_type, stage, segment_key, option_value,
  sample_count, created_at.
- `AttributionEdge`: edge_id, source_node_id, target_node_id, outcome_weight_bps,
  sample_count, win_rate_bps, margin_delta_bps, updated_at.
- `GenomeQuery`: query_id, query_type, parameters, result_checksum, created_at.
- `GenomeQueryResult`: segments_analyzed, evidence_count, findings, recommendations,
  temporal_window, confidence_bps.
- `CounterfactualRequest`: original_quote_id, alternative_decisions, replay_scope,
  idempotency_key.
- `CounterfactualResult`: projected_outcome, delta_vs_actual, evidence_chain,
  replay_checksum, confidence_bps.

### Service Contracts
- `AutopsyEngine::perform(quote_id, outcome) -> DealAutopsy`
- `AutopsyEngine::extract_forks(quote_id) -> Vec<DecisionFork>`
- `AttributionEngine::score(autopsy) -> Vec<AttributionScore>`
- `AttributionEngine::build_graph(autopsies) -> AttributionGraph`
- `GenomeQueryEngine::query(query) -> GenomeQueryResult`
- `CounterfactualEngine::simulate(request) -> CounterfactualResult`

### Persistence Contracts
- `DealAutopsyRepo`: CRUD + lifecycle transitions for autopsy records.
- `DecisionForkRepo`: append-only fork storage with audit trail linkage.
- `AttributionScoreRepo`: append-only score storage with confidence bounds.
- `AttributionGraphRepo`: append-only node/edge storage with aggregation queries.
- `GenomeQueryAuditRepo`: query audit trail for determinism verification.

### Crate Boundaries
- `quotey-core`: autopsy engine, attribution scoring, genome queries, counterfactual simulation,
  fork extraction rules, attribution formulas, temporal pattern detection.
- `quotey-db`: autopsy schema, migration, repositories, attribution graph persistence.
- `quotey-cli`: `quotey genome query`, `quotey genome autopsy`, `quotey genome counterfactual` commands.
- `quotey-agent`: orchestration hooks for autopsy triggering on deal outcomes,
  integration with CLO/NXT/Precedent/Ghost pipelines.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Attribution bias from skewed outcome data (e.g. only easy wins in early cohort) | Medium | Medium | cohort stratification + confidence bounds + minimum sample thresholds before graph promotion | Data owner |
| Counterfactual simulation divergence from reality | High | Medium | bounded confidence intervals + explicit disclaimer in results + validation against held-out outcomes | Core owner |
| Genome query performance degradation at scale | Medium | Low | indexed attribution graph + query result caching + configurable graph depth limits | Platform owner |
| Over-reliance on historical patterns for future strategy | Medium | Medium | temporal decay weighting + recency bias controls + staleness alerts on aging evidence | Product owner |
| Attribution graph size growing unbounded | Medium | Low | configurable retention policy + node/edge aggregation thresholds + archival pipeline | Data owner |
| Fork extraction missing non-obvious decision points | Medium | Medium | fork type taxonomy versioning + extraction rule coverage metrics + manual audit sampling | Core owner |

## Execution Backlog (Canonical TODO List)
These items are tracked in beads and are the authoritative RGN implementation checklist.

### Epic
- [ ] `bd-rgn` W3 [RGN] Deal Autopsy & Revenue Genome

### Primary Tasks
- [ ] `bd-rgn.1` [RGN] Task 1 Spec KPI Guardrails
- [ ] `bd-rgn.2` [RGN] Task 2 Domain Model + Persistence Schema
- [ ] `bd-rgn.3` [RGN] Task 3 Deal Autopsy Engine
- [ ] `bd-rgn.4` [RGN] Task 4 Decision Fork Extractor
- [ ] `bd-rgn.5` [RGN] Task 5 Attribution Scoring Engine
- [ ] `bd-rgn.6` [RGN] Task 6 Attribution Graph Builder
- [ ] `bd-rgn.7` [RGN] Task 7 Revenue Genome Query Engine
- [ ] `bd-rgn.8` [RGN] Task 8 Counterfactual Simulation Engine
- [ ] `bd-rgn.9` [RGN] Task 9 Integration Hooks (CLO/NXT/Precedent/Ghost)
- [ ] `bd-rgn.10` [RGN] Task 10 CLI Commands + Telemetry
- [ ] `bd-rgn.11` [RGN] Task 11 End-to-End Demo + Rollout Gate

### Granular Subtasks
- [ ] `bd-rgn.2.1` [RGN-DATA] Migration schema + fixtures for autopsy/attribution/genome tables
- [ ] `bd-rgn.2.2` [RGN-DATA] Repository methods for autopsy lifecycle + graph mutations
- [ ] `bd-rgn.3.1` [RGN-ENGINE] Autopsy lifecycle state machine + audit trail walker
- [ ] `bd-rgn.4.1` [RGN-ENGINE] Fork type taxonomy + extraction rules per decision type
- [ ] `bd-rgn.5.1` [RGN-ENGINE] Attribution formula + confidence bound calculator
- [ ] `bd-rgn.6.1` [RGN-GRAPH] Incremental graph update + aggregation logic
- [ ] `bd-rgn.7.1` [RGN-QUERY] Query type registry + deterministic result builder
- [ ] `bd-rgn.7.2` [RGN-QUERY] Temporal pattern detection + trend analysis
- [ ] `bd-rgn.8.1` [RGN-COUNTERFACTUAL] Replay integration + delta calculator
- [ ] `bd-rgn.9.1` [RGN-INTEGRATION] CLO candidate enhancement with causal evidence
- [ ] `bd-rgn.9.2` [RGN-INTEGRATION] NXT strategy ranking with empirical win patterns
- [ ] `bd-rgn.10.1` [RGN-CLI] `quotey genome query` + `quotey genome autopsy` commands
- [ ] `bd-rgn.11.1` [RGN-ROLLOUT] Demo checklist + go/no-go gate

### Dependency Order (Execution Plan)
1. `bd-rgn.1`
2. `bd-rgn.2` -> (`bd-rgn.2.1`, `bd-rgn.2.2`)
3. `bd-rgn.3` -> (`bd-rgn.3.1`) + `bd-rgn.4` -> (`bd-rgn.4.1`) [parallel]
4. `bd-rgn.5` -> (`bd-rgn.5.1`)
5. `bd-rgn.6` -> (`bd-rgn.6.1`)
6. `bd-rgn.7` -> (`bd-rgn.7.1`, `bd-rgn.7.2`) + `bd-rgn.8` -> (`bd-rgn.8.1`) [parallel]
7. `bd-rgn.9` -> (`bd-rgn.9.1`, `bd-rgn.9.2`)
8. `bd-rgn.10` -> (`bd-rgn.10.1`)
9. `bd-rgn.11` -> (`bd-rgn.11.1`)

## Quality Gates
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets --all-features`
- `ubs --diff` scoped to changed files
- Attribution determinism invariants and counterfactual replay tests must pass before rollout gate
- Genome query determinism suite must demonstrate identical outputs for identical inputs across runs

## Guardrail Exit Checklist (Before Task 2 Coding)
- [ ] Scope/non-goals accepted.
- [ ] KPI formulas and owners locked.
- [ ] Deterministic safety constraints mapped to testable invariants.
- [ ] Autopsy lifecycle/attribution contracts reviewed across core/db/cli/agent boundaries.
- [ ] Risk mitigations mapped to owning beads.

## Notes
- This spec is planning source-of-truth for the RGN track.
- Beads remain the execution system-of-record; status updates belong in `br`.
