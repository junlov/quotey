# W1 HLT Quote Health Score + Next-Best Fixes Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-271.5`
(`Quote Health Score + Next-Best Fixes`) so reps know exactly what to fix next before requesting
approval through scored risk/quality assessment and concrete remediation actions.

## Scope
### In Scope
- Real-time quote health scoring based on deterministic signals.
- Risk category decomposition (completeness, policy, pricing, configuration).
- Concrete next-best-fix recommendations with priority ordering.
- Slack health panel with one-click fix actions.
- Progress indicator as fixes are applied.
- Health score history tracking over quote lifecycle.

### Out of Scope (for Wave 1)
- Predictive health scoring based on external market data.
- Automatic quote fixing without rep confirmation.
- Competitive benchmarking against peer quotes.
- ML-based health prediction from historical patterns.
- Complex multi-quote portfolio health analysis.

## Rollout Slices
- `Slice A` (contracts): health score model, risk categories, fix action schema.
- `Slice B` (engine): scoring service, signal detection, recommendation ranking.
- `Slice C` (UX): Slack health panel, fix action buttons, progress indicator.
- `Slice D` (ops): health metrics dashboard, fix outcome tracking, runbook.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Quote health score accuracy | N/A | >= 90% | Data owner | `% health scores matching actual approval outcomes` |
| Time to quote ready | 6.5 hours | <= 2.5 hours | Sales Ops owner | median time from draft to approval-ready |
| Fix action success rate | N/A | >= 85% | UX owner | `% fix actions resolving targeted issues` |
| Rep engagement with health panel | N/A | >= 80% | UX owner | `% quotes with health panel interactions` |
| Issues fixed per health check | N/A | >= 2.0 | Product owner | average issues resolved per health-guided session |
| P95 health score latency | N/A | <= 200ms | Platform owner | quote change to updated health score |

## Deterministic Safety Constraints
- Health scores must be computed from deterministic signals only (constraint violations, policy flags, missing fields).
- Score calculation must be reproducible: same quote state → same health score.
- Fix recommendations must delegate to deterministic engines for execution.
- LLMs may format recommendation text but cannot invent fixes or modify scores.
- Rep maintains full control; system suggests but never auto-applies fixes.
- All scoring rationale must be explainable with source references.

## Interface Boundaries (Draft)
### Domain Contracts
- `QuoteHealth`: `overall_score`, `category_scores`, `issues`, `fix_recommendations`.
- `HealthCategory`: `completeness`, `policy_compliance`, `pricing_validity`, `configuration`.
- `HealthIssue`: `issue_type`, `severity`, `description`, `affected_fields`, `fix_action`.
- `FixRecommendation`: `priority`, `action_type`, `description`, `expected_impact`, `automation_level`.

### Service Contracts
- `QuoteHealthService::compute_health(quote_id) -> QuoteHealth`
- `QuoteHealthService::get_category_breakdown(quote_id, category) -> CategoryDetail`
- `QuoteHealthService::rank_fixes(issues) -> Vec<FixRecommendation>`
- `QuoteHealthService::apply_fix(quote_id, fix_id) -> FixResult`
- `QuoteHealthService::track_progress(quote_id) -> HealthHistory`
- `QuoteHealthService::explain_score(quote_id) -> ScoreExplanation`

### Persistence Contracts
- `QuoteHealthRepo`: health score snapshots with quote version linkage.
- `HealthIssueRepo`: detected issues with resolution status.
- `FixActionRepo`: available fix actions and their outcomes.
- `HealthHistoryRepo`: score evolution over quote lifecycle.

### Slack Contract
- Health panel appears in quote thread with overall score (0-100).
- Category breakdown with visual indicators (green/yellow/red).
- Prioritized fix list with `Fix Now` buttons for each issue.
- Progress indicator updates as fixes are applied.
- Health score refresh after each quote modification.

### Crate Boundaries
- `quotey-core`: health scoring logic, signal detection, recommendation ranking.
- `quotey-db`: health storage, issue tracking, history logging.
- `quotey-slack`: health panel rendering, fix action handling (no business logic).
- `quotey-agent`: health workflow orchestration, rep interaction flow.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Health score misleading (false positive/negative) | High | Medium | multi-signal validation + confidence thresholds | Data owner |
| Fix actions fail or cause unexpected changes | Medium | Medium | deterministic delegation + preview step + undo | Runtime owner |
| Rep overwhelmed by too many fix suggestions | Medium | Medium | prioritization + batching + progressive disclosure | UX owner |
| Score latency impacts real-time collaboration | Medium | Medium | caching + incremental updates + async scoring | Platform owner |
| Gaming of health score without real improvement | Low | Medium | outcome validation + audit logging | Product owner |
| Health panel adds friction to simple quotes | Low | Medium | collapsible panel + auto-hide for healthy quotes | UX owner |

## Guardrail Checklist (Pre-implementation Exit)
- [ ] Scope and non-goals approved for Wave 1 execution.
- [ ] KPI baseline, target, owner, and query definitions captured.
- [ ] Deterministic constraints mapped to runtime acceptance tests.
- [ ] Service and repository contracts aligned with crate ownership boundaries.
- [ ] Risk mitigations reviewed and assigned before Task 2 implementation.
- [ ] Health panel UX reviewed for clarity and actionable guidance.
