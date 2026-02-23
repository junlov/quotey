# W1 SIM Deal Flight Simulator Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-271.7`
(`Deal Flight Simulator`) so reps can generate and compare negotiation scenarios deterministically
before committing quotes.

## Scope
### In Scope
- Quote-thread scenario generation for alternative pricing configurations.
- Side-by-side comparison of multiple quote scenarios (base case vs alternatives).
- Deterministic scenario pricing using existing CPQ engines.
- Scenario persistence with linkage to parent quote for audit trails.
- Slack Block Kit rendering for comparison cards.
- Telemetry for scenario generation usage and conversion outcomes.

### Out of Scope (for Wave 1)
- Real-time market data integration (competitor pricing, market indices).
- ML-based outcome prediction or win probability modeling.
- Complex multi-variable optimization (linear programming solver).
- Historical deal replay with time-travel debugging.
- Scenario sharing and collaborative editing between reps.

## Rollout Slices
- `Slice A` (contracts): scenario request/response schema, comparison model, deterministic guardrails.
- `Slice B` (engine): scenario generation service, CPQ delegation, and persistence layer.
- `Slice C` (UX): Slack `/quote simulate` command and comparison card rendering.
- `Slice D` (ops): telemetry dashboards, runbook, and scenario quality metrics.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Scenario generation success rate | N/A (new) | >= 98% | Runtime owner | `sim_success / sim_requests` |
| Median scenarios generated per quote | 0 | >= 2.5 | Product owner | average scenarios created before quote finalization |
| Quote iteration reduction | 4.2 iterations | <= 2.5 iterations | Sales Ops owner | median quote versions before approval |
| P95 scenario latency | N/A | <= 800ms | Platform owner | request received to Slack response posted |
| Scenario-to-close conversion | N/A | >= 40% | Product owner | `% scenarios where selected variant wins` |
| Deterministic consistency rate | N/A | 100% | Determinism owner | `identical_inputs_same_output / total_regenerations` |

## Deterministic Safety Constraints
- All scenario pricing must delegate to existing deterministic CPQ engines (pricing, policy, constraint).
- Scenario generation must never alter the parent quote state; scenarios are read-only forks.
- Identical scenario inputs must produce identical outputs 100% of the time (pure function guarantee).
- Scenario comparison must clearly distinguish between "scenario" (hypothetical) and "quote" (committed).
- LLMs may format comparison text but cannot modify scenario calculations or rankings.
- All scenario data must be persisted with parent quote linkage for audit purposes.

## Interface Boundaries (Draft)
### Domain Contracts
- `ScenarioRequest`: `quote_id`, `thread_id`, `actor_id`, `variation_params` (discount_pct, term_months, addons).
- `ScenarioVariant`: `variant_id`, `params`, `pricing_result`, `policy_decision`, `comparison_rank`.
- `ScenarioComparison`: `base_quote`, `variants: Vec<ScenarioVariant>`, `tradeoff_summary`.
- `ScenarioFork`: parent quote snapshot + variant parameters → isolated scenario quote.

### Service Contracts
- `DealFlightSimulator::generate_scenarios(request, count) -> Vec<ScenarioVariant>`
- `DealFlightSimulator::compare_variants(variants) -> ScenarioComparison`
- `DealFlightSimulator::apply_variant(variant_id) -> QuoteUpdateResult` (optional promotion to quote)
- `DealFlightSimulator::persist_scenario(scenario) -> ScenarioId`
- `DealFlightSimulator::list_scenarios(quote_id) -> Vec<ScenarioSummary>`

### Persistence Contracts
- `ScenarioRepo`: save/fork scenario quotes with parent linkage.
- `ScenarioComparisonRepo`: store comparison results and selected variants.
- `ScenarioAuditRepo`: append-only log of scenario generation and selection events.

### Slack Contract
- `/quote simulate [discount=X%] [term=Y months]` command in quote thread context.
- Comparison card shows: base quote, 2-3 variants, side-by-side pricing breakdown.
- Variant selection button maps deterministically to scenario promotion workflow.
- Error card includes: param validation failures, constraint violations, next actions.

### Crate Boundaries
- `quotey-core`: scenario generation logic, CPQ delegation, comparison engine.
- `quotey-db`: scenario persistence, parent-child quote relationships, audit logging.
- `quotey-slack`: command parsing, comparison card rendering (no business logic).
- `quotey-agent`: orchestration, guardrail enforcement, user interaction flow.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Scenario pricing diverges from quote pricing over time | High | Medium | immutable snapshots + versioned CPQ rules | Data owner |
| User confusion between scenario and committed quote | High | Medium | clear visual distinction + confirmation flows | UX owner |
| Scenario explosion (too many variants generated) | Medium | Medium | bounded generation (max 3 variants) + param validation | Runtime owner |
| Non-deterministic results from floating-point drift | Medium | Low | Decimal arithmetic only + deterministic ordering | Determinism owner |
| Performance degradation with complex quote configs | Medium | Medium | scenario caching + async generation for large configs | Platform owner |
| Scenario persistence bloat | Low | Medium | TTL cleanup policy + archival for old scenarios | Data owner |

## Guardrail Checklist (Pre-implementation Exit)
- [ ] Scope and non-goals approved for Wave 1 execution.
- [ ] KPI baseline, target, owner, and query definitions captured.
- [ ] Deterministic constraints mapped to runtime acceptance tests.
- [ ] Service and repository contracts aligned with crate ownership boundaries.
- [ ] Risk mitigations reviewed and assigned before Task 2 implementation.
- [ ] Scenario vs Quote visual distinction approved by UX review.
