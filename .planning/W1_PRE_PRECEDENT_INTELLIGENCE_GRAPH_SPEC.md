# W1 PRE Precedent Intelligence Graph Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-271.4`
(`Precedent Intelligence Graph`) so similar-deal guidance is auditable, deterministic, and actionable in-thread.

## Scope
### In Scope
- Quote-scoped precedent lookup using deterministic similarity scoring.
- Outcome-context payloads for similar prior deals (pricing, discounting, approval path, result).
- Filterable rationale in Slack thread context tied to `quote_id`.
- Deterministic evidence lineage for each recommended precedent.
- Telemetry for recommendation usage and consistency impact.

### Out of Scope (for Wave 1)
- Autonomous deal decisions (no auto-approval or auto-pricing from precedent output).
- Probabilistic black-box ranking without deterministic fallback.
- Cross-tenant/global benchmarks outside local data boundary.
- BI dashboarding beyond thread and operator CLI surfaces.

## Rollout Slices
- `Slice A` (contracts): precedent query model, similarity evidence schema, and deterministic constraints.
- `Slice B` (data): persistence primitives for fingerprints, outcomes, and approval-path evidence.
- `Slice C` (runtime): deterministic graph/ranking service with versioned similarity strategy.
- `Slice D` (UX + ops): Slack panel/filter UX, telemetry, and operator runbook.

`Slice A/B` are required before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Discount decision variance on similar deals | 22 pts | <= 8 pts | Revenue Ops owner | median absolute delta of chosen discount vs precedent band |
| Approval-path consistency for comparable deals | 63% | >= 90% | Policy owner | `% comparable deals routed to same approval tier` |
| Time to provide precedent context in thread | 75 s | <= 10 s | Slack owner | request timestamp to precedent panel render |
| Precedent panel usage rate (eligible quotes) | 0% | >= 70% | Product owner | `% eligible quote threads invoking precedent panel` |
| Precedent evidence completeness | 78% | >= 99% | Determinism owner | `% results including similarity inputs + outcome refs + approval refs` |

## Deterministic Safety Constraints
- Similarity scoring must be deterministic, versioned, and replayable for identical inputs.
- Only persisted deterministic artifacts (fingerprints, outcomes, approvals) may influence ranking.
- LLMs may summarize context text but cannot alter similarity scores or reorder deterministic rankings.
- Every surfaced precedent must carry source identifiers (`quote_id`, fingerprint id/version, outcome id).
- Missing evidence must fail closed with explicit user-visible recovery actions.

## Interface Boundaries (Draft)
### Domain Contracts
- `PrecedentQuery`: `quote_id`, optional scope filters (segment, region, product family), `limit`.
- `PrecedentResult`: candidate quote reference, similarity score, outcome summary, approval summary.
- `PrecedentEvidence`: normalized feature vector refs, score components, and source identifiers.

### Service Contracts
- `PrecedentGraphService::rank_similar(query) -> Vec<PrecedentResult>`
- `PrecedentGraphService::explain_match(query, candidate_quote_id) -> PrecedentEvidence`
- `PrecedentGraphService::validate_replay(query, expected_version) -> ReplayValidationResult`

### Persistence Contracts
- `FingerprintRepository`: write/read deterministic fingerprints with version metadata.
- `DealOutcomeRepository`: write/read final outcome and commercial context per quote.
- `ApprovalPathRepository`: write/read routed and final approval path evidence.

### Slack Contract
- Precedent panel is available only in mapped quote threads.
- Panel supports deterministic filters (segment, region, term) with explicit score/rationale display.
- Empty/insufficient evidence states include next-action guidance (`refresh context`, `adjust filters`).

### Crate Boundaries
- `quotey-core`: deterministic similarity/ranking primitives and replay contract.
- `quotey-db`: persistence of fingerprints/outcomes/approval evidence and query pathways.
- `quotey-slack`: panel rendering and user interactions only.
- `quotey-agent`: orchestration and guardrail enforcement; no financial truth ownership.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Stale precedents bias recommendations | High | Medium | freshness windows + stale-result indicators | Data owner |
| Incorrect comparability due to weak feature normalization | High | Medium | deterministic feature schema + invariant tests | Determinism owner |
| Missing approval history for legacy quotes | Medium | Medium | compatibility fallback + backfill playbook | Runtime owner |
| Latency spikes on large candidate sets | Medium | Medium | bounded candidate windows + indexed query pathways | Platform owner |
| Over-trust by users treating precedents as mandates | Medium | Medium | explicit advisory copy + policy guardrails in UI | Product owner |

## Guardrail Checklist (Pre-implementation Exit)
- Scope and non-goals approved for Wave 1.
- KPI baseline, target, owner, and measurement formulas documented.
- Deterministic constraints mapped to runtime acceptance tests.
- Service and persistence boundaries aligned with crate ownership.
- Risk mitigations assigned before Task 2 data-model implementation.
