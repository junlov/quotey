# W1 EXP Explain Any Number Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-271.2`
(`Explain Any Number`) so every quote explanation is auditable, deterministic, and thread-usable.

## Scope
### In Scope
- Quote-thread explanation workflow for totals and line-item amounts.
- Evidence assembly from deterministic pricing trace and policy artifacts.
- Deterministic explanation payload model with source references.
- Slack response format with expandable breakdown sections.
- Telemetry for usage, latency, and explanation quality outcomes.

### Out of Scope (for Wave 1)
- Narrative-only explanations not anchored to persisted trace data.
- Free-form "what-if" simulation and hypothetical repricing.
- Cross-quote portfolio analytics and dashboards.
- LLM-generated arithmetic or policy decisions.

## Rollout Slices
- `Slice A` (contracts): request/response schema, evidence model, and deterministic guardrails.
- `Slice B` (data): persisted explanation-ready trace/policy retrieval and repository pathways.
- `Slice C` (runtime): deterministic explanation service and fallback/error pathways.
- `Slice D` (UX + ops): Slack command/card UX, telemetry, and runbook.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Median pricing dispute resolution time | 45 min | <= 10 min | Product + Sales Ops owner | median time from first explain request to quote unblock |
| Explain command success rate | 92% | >= 99% | Runtime owner | `explain_success / explain_requests` |
| Evidence coverage completeness | 80% | >= 99% | Determinism owner | `% explanation responses with trace_id + policy_evidence refs` |
| P95 explanation latency | 4.0s | <= 1.2s | Platform owner | request received to Slack response posted |
| "Not enough evidence" error rate | 6.0% | <= 1.0% | Data owner | `explain_errors_missing_evidence / explain_requests` |

## Deterministic Safety Constraints
- Every numeric explanation must be computed from persisted deterministic artifacts only.
- Pricing trace and policy outputs are the only allowed sources for arithmetic and rule evidence.
- LLMs may rewrite wording for readability but cannot introduce or modify numbers, thresholds, or outcomes.
- Explanations must include stable source references (`quote_id`, trace snapshot id/version, rule/policy id).
- Missing evidence must fail closed with explicit user-visible next actions, never guessed values.

## Interface Boundaries (Draft)
### Domain Contracts
- `ExplainRequest`: `quote_id`, optional `line_id`, `thread_id`, `actor_id`, `correlation_id`.
- `ExplanationEvidence`: pricing trace steps, policy decision details, and applied rule identifiers.
- `ExplanationPayload`: deterministic arithmetic chain, policy rationale, and user-facing summary sections.

### Service Contracts
- `ExplainAnyNumberService::explain_total(request) -> ExplanationPayload`
- `ExplainAnyNumberService::explain_line(request) -> ExplanationPayload`
- `ExplainAnyNumberService::explain_policy(request) -> ExplanationPayload`
- `ExplainAnyNumberService::validate_evidence(request) -> EvidenceValidationResult`

### Persistence Contracts
- `QuotePricingSnapshotRepo`: fetch immutable pricing trace snapshot by `quote_id` and version.
- `PolicyEvaluationRepo`: fetch policy decision payload and violation metadata for the quote snapshot.
- `ExplanationAuditRepo`: append explanation request/response metadata and failure modes.

### Slack Contract
- `/quote explain` command is valid only inside a mapped quote thread context.
- Slack blocks expose: amount requested, arithmetic breakdown, policy evidence, and source references.
- Error card always includes deterministic next actions (`retry`, `refresh quote`, `request support context`).

### Crate Boundaries
- `quotey-core`: deterministic explanation assembly logic and evidence typing.
- `quotey-db`: data fetch pathways for pricing snapshots, policy evaluations, and audit append.
- `quotey-slack`: command parsing and response card rendering only (no business logic).
- `quotey-agent`: orchestration and guardrail enforcement, delegating numeric truth to core/db.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Explanation mismatches latest quote version | High | Medium | strict snapshot version binding + version mismatch errors | Data owner |
| Slack command used outside quote thread | Medium | High | thread mapping check + actionable error guidance | Slack owner |
| Non-deterministic narrative drift from LLM wording | High | Low | numeric fields locked before optional phrasing step | Determinism owner |
| Missing policy evidence for legacy records | Medium | Medium | compatibility reader + migration/backfill checklist | Runtime owner |
| Latency regressions under large trace payloads | Medium | Medium | bounded sections + pre-aggregated evidence view | Platform owner |

## Task 2 Persistence and Migration Contract
### Schema Additions (`0013_explain_any_number`)
- `explanation_requests`: request-level persisted context (`quote_id`, optional `line_id`, thread,
  actor, correlation id, version, lifecycle status, latency/error fields).
- `explanation_evidence`: deterministic evidence references and payload records keyed by request.
- `explanation_audit`: append-only event trail for request lifecycle and failure paths.
- `explanation_response_cache`: cached deterministic summaries for repeated explain prompts.
- `explanation_request_stats`: singleton operational aggregate table updated via request triggers.

### Version and Audit Semantics
- `quote_version` is required on all explanation requests to bind each explain result to a specific
  quote snapshot state.
- Request status transitions (`pending` -> terminal state) are persisted with `completed_at` and
  `latency_ms` to support deterministic SLA and error-rate measurement.
- Audit rows are append-only and keyed by correlation id + request id to preserve replayable trace.

### Migration Behavior and Rollback
- Migration is additive and does not mutate pre-existing CPQ tables.
- EXP table names are collision-safe relative to pre-existing policy explanation tables.
- Rollback (`0013_explain_any_number.down.sql`) removes only EXP explanation artifacts in reverse
  dependency order (triggers -> stats/cache -> audit/evidence -> requests).
- Up/down/up behavior is validated by db migration signature tests.

## Guardrail Checklist (Pre-implementation Exit)
- Scope and non-goals approved for Wave 1 execution.
- KPI baseline, target, owner, and query definitions captured.
- Deterministic constraints mapped to runtime acceptance tests.
- Service and repository contracts aligned with crate ownership boundaries.
- Risk mitigations reviewed and assigned before Task 2 implementation.
