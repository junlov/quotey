# FEAT-05 Explainable Policy Engine Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-70d.5`
(`Explainable Policy Engine`) so policy decisions are transparent, auditable, and comprehensible to non-technical users.

## Scope
### In Scope
- Policy decision decomposition (why was this discount rejected/approved).
- Human-readable explanation generation from policy rule evaluation.
- Evidence assembly: which rules fired, what values were checked.
- Explanation persistence and retrieval by quote version.
- Slack-friendly explanation formatting with expandable details.
- Telemetry for explanation helpfulness and policy dispute rates.

### Out of Scope (for Wave 1)
- Policy change suggestions or automatic policy optimization.
- Natural language policy authoring.
- Cross-policy impact analysis (what-if scenarios).
- Predictive policy outcomes (will this be approved next quarter).

## Rollout Slices
- `Slice A` (contracts): explanation schema, evidence model, rule annotation format.
- `Slice B` (engine): policy evaluator enhancement for evidence capture.
- `Slice C` (runtime): explanation service, NL generation, persistence.
- `Slice D` (UX): Slack explanation cards, drill-down navigation, metrics.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Explanation request latency | N/A | <= 500ms | Platform owner | request to explanation payload ready |
| Policy dispute resolution time | 2 days | <= 4 hours | Product owner | dispute raised to resolution |
| User comprehension score | N/A | >= 4.0/5 | UX owner | user rating of explanation clarity |
| Explanation completeness | N/A | >= 99% | Determinism owner | explanations with all rule evidence |
| Policy override rate | 15% | <= 10% | Compliance owner | overrides / total policy decisions |

## Deterministic Safety Constraints
- Explanations reflect actual policy evaluation trace; no post-hoc rationalization.
- All numeric values in explanations come from deterministic policy engine outputs.
- LLMs may rephrase for readability but cannot modify rule logic or outcomes.
- Explanations are version-locked to policy version active at decision time.
- Missing evidence scenarios fail closed with explicit "evidence unavailable" message.

## Interface Boundaries (Draft)
### Domain Contracts
- `PolicyExplanation`: decision_summary, rule_evaluations, evidence_refs, policy_version.
- `RuleEvaluation`: rule_id, rule_name, condition, result, input_values, output_values.
- `ExplanationEvidence`: pricing_snapshot_ref, policy_version_ref, evaluation_timestamp.

### Service Contracts
- `ExplainablePolicyService::explain_decision(quote_id, decision_id) -> PolicyExplanation`
- `ExplainablePolicyService::explain_violation(quote_id, violation_id) -> PolicyExplanation`
- `ExplainablePolicyService::get_evidence(explanation_id) -> ExplanationEvidence`
- `ExplainablePolicyService::render_for_slack(explanation) -> SlackBlocks`

### Persistence Contracts
- `PolicyExplanationRepo`: store/retrieve explanations by quote and decision.
- `PolicyEvaluationTraceRepo`: detailed rule evaluation step records.
- `ExplanationAuditRepo`: explanation request and delivery log.

### Slack Contract
- `/quote explain` command triggers explanation for latest quote decision.
- Explanation card shows: decision, summary, expandable rule details.
- Rule breakdown includes: condition, checked value, threshold, result.
- Evidence references link to pricing snapshot and policy version.
- Error state: "explanation unavailable" with support escalation path.

### Crate Boundaries
- `quotey-core`: policy evaluator with evidence capture, explanation assembly.
- `quotey-db`: explanation and evaluation trace persistence.
- `quotey-slack`: explanation card rendering, command handling.
- `quotey-agent`: NL explanation generation (post-deterministic assembly).

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Explanation diverges from actual policy logic | High | Low | trace-based evidence + version locking | Determinism owner |
| LLM hallucinates rule details | High | Low | numeric fields locked before NL generation | Runtime owner |
| Evidence missing for legacy decisions | Medium | Medium | graceful degradation + backfill where possible | Data owner |
| Explanation verbosity overwhelms users | Medium | Medium | progressive disclosure + summary first | UX owner |
| Policy version mismatch | High | Low | explicit version binding in explanation | Compliance owner |

## Guardrail Checklist (Pre-implementation Exit)
- Scope and non-goals agreed.
- KPI owner and metric formula explicitly documented.
- Deterministic constraints copied into implementation task templates.
- Interface contracts reviewed against existing crate boundaries.
- Risks and mitigations acknowledged by feature owner.

## Migration Contract
### Schema Additions (`0019_explainable_policy`)
- `policy_explanations`: explanation records with rule evaluation summaries.
- `policy_evaluation_traces`: detailed rule step-by-step evaluations.
- `explanation_audit`: request and delivery tracking.

### Version and Audit Semantics
- Explanations reference specific policy_version and quote_version.
- Evaluation traces are immutable after creation.
- Audit log enables replay of explanation generation.

### Migration Behavior and Rollback
- Migration adds explanation tables; no changes to policy engine tables.
- Historical decisions have no explanation (null); forward-only from deployment.
- Rollback removes explanation tables; core policy engine unaffected.
