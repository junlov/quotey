# W1 REN Renewal Delta Intelligence Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-271.9`
(`Renewal Delta Intelligence`) so renewal reps gain speed and context for better outcomes through
automated diff analysis and recommendation scoring.

## Scope
### In Scope
- Renewal context diff (current contract vs proposed changes).
- Expansion opportunity identification (seat increases, addon upsells).
- Risk mitigation option generation (churn indicators, competitive threats).
- Deterministic recommendation scoring based on historical patterns.
- Slack renewal delta card with actionable option sets.
- Integration with existing quote workflow for seamless renewal creation.

### Out of Scope (for Wave 1)
- Predictive churn modeling using external data sources.
- Automatic renewal quote generation without rep review.
- Competitive pricing intelligence from external APIs.
- Multi-year renewal trajectory optimization.
- Integration with customer health scoring platforms.

## Rollout Slices
- `Slice A` (contracts): renewal diff model, recommendation schema, scoring weights.
- `Slice B` (engine): contract retrieval, delta computation, recommendation generation.
- `Slice C` (UX): Slack renewal delta card, option set actions, thread integration.
- `Slice D` (ops): renewal outcome tracking, recommendation quality metrics, runbook.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Renewal quote creation time | 45 min | <= 15 min | Sales Ops owner | median time from renewal trigger to quote ready |
| Expansion opportunity detection rate | 60% | >= 85% | Product owner | `% renewals with identified expansion identified` |
| Recommendation acceptance rate | N/A | >= 50% | Product owner | `% AI-suggested options selected by rep` |
| Delta card engagement rate | N/A | >= 70% | UX owner | `% renewal threads with delta card interaction` |
| Churn risk flag accuracy | N/A | >= 75% | Data owner | `% flagged at-risk renewals that actually churn` |
| P95 delta computation latency | N/A | <= 500ms | Platform owner | contract retrieval to Slack response posted |

## Deterministic Safety Constraints
- All recommendations must be derived from deterministic analysis of contract data and historical patterns.
- Renewal delta calculations must use exact contract terms, never LLM-generated estimates.
- Expansion calculations must delegate to pricing engine for accuracy.
- Risk flags must be based on explicit business rules, not opaque ML models.
- Rep maintains full control; system suggests but never auto-commits renewal changes.
- All delta analysis must be persisted with source references for audit.

## Interface Boundaries (Draft)
### Domain Contracts
- `RenewalContext`: `account_id`, `current_contract`, `renewal_date`, `usage_data`.
- `ContractDelta`: `current_terms`, `proposed_changes`, `financial_impact`, `risk_indicators`.
- `RenewalRecommendation`: `recommendation_type`, `rationale`, `expected_value`, `confidence`.
- `ExpansionOpportunity`: `product_id`, `suggested_quantity`, `upsell_value`, `compatibility_check`.

### Service Contracts
- `RenewalIntelligence::analyze_contract(account_id) -> ContractDelta`
- `RenewalIntelligence::generate_recommendations(delta) -> Vec<RenewalRecommendation>`
- `RenewalIntelligence::identify_expansion_opportunities(contract) -> Vec<ExpansionOpportunity>`
- `RenewalIntelligence::assess_churn_risk(account_id) -> RiskAssessment`
- `RenewalIntelligence::create_renewal_quote(recommendations) -> QuoteDraft`

### Persistence Contracts
- `ContractRepo`: current contract terms, renewal history, usage telemetry.
- `RenewalAnalysisRepo`: delta computations, recommendation outputs, decision audit.
- `HistoricalRenewalRepo`: past renewal outcomes for pattern analysis.
- `RecommendationAuditRepo`: append-only log of suggestions and outcomes.

### Slack Contract
- Automatic delta card when renewal intent detected in thread.
- Card sections: current contract summary, proposed changes, expansion options, risk flags.
- Action buttons: `Create Renewal Quote`, `View Details`, `Adjust Terms`, `Dismiss`.
- Thread context maintained across renewal workflow.

### Crate Boundaries
- `quotey-core`: delta computation, recommendation engine, risk assessment.
- `quotey-db`: contract data, renewal history, analysis persistence.
- `quotey-slack`: delta card rendering, action handling (no business logic).
- `quotey-agent`: renewal workflow orchestration, rep interaction flow.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Inaccurate contract data leads to wrong deltas | High | Medium | data validation + source system sync checks | Data owner |
| Over-aggressive upsell suggestions annoy customers | Medium | Medium | conservative scoring + rep override controls | Product owner |
| False churn risk flags create unnecessary urgency | Medium | Medium | calibrated thresholds + explicit uncertainty flags | Data owner |
| Renewal recommendations become stale | Medium | Medium | real-time contract sync + freshness indicators | Runtime owner |
| Performance degradation with complex contract history | Low | Medium | incremental analysis + caching | Platform owner |
| Privacy concerns with usage data analysis | Low | Low | data minimization + explicit consent checks | Compliance owner |

## Guardrail Checklist (Pre-implementation Exit)
- [ ] Scope and non-goals approved for Wave 1 execution.
- [ ] KPI baseline, target, owner, and query definitions captured.
- [ ] Deterministic constraints mapped to runtime acceptance tests.
- [ ] Service and repository contracts aligned with crate ownership boundaries.
- [ ] Risk mitigations reviewed and assigned before Task 2 implementation.
- [ ] Renewal card UX reviewed for clarity and actionability.
