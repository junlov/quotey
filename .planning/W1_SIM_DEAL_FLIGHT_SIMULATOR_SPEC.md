# W1 SIM Deal Flight Simulator Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-271.7`
(`Deal Flight Simulator`) so reps can run counterfactual "what-if" scenarios and compare
price/policy/approval outcomes deterministically before committing quote changes.

## Product Shape
`Deal Flight Simulator` is the Slack-facing "What-if Lab" for CPQ:
- Input: one baseline quote plus a set of structured hypothetical changes.
- Processing: deterministic re-validation, re-pricing, and policy/approval evaluation.
- Output: side-by-side deltas for totals, margins, policy outcomes, and approval routes.

No hypothetical result is committed unless the rep explicitly promotes a chosen variant.

## Scope
### In Scope
- Quote-thread scenario generation for alternative pricing configurations.
- Side-by-side comparison of multiple quote scenarios (base case vs alternatives).
- Deterministic scenario pricing using existing CPQ engines.
- Deterministic policy and approval-route delta analysis per scenario.
- Scenario persistence with linkage to parent quote for audit trails.
- Slack Block Kit rendering for comparison cards.
- Telemetry for scenario generation usage and conversion outcomes.

### Out of Scope (for Wave 1)
- Real-time market data integration (competitor pricing, market indices).
- ML-based outcome prediction or win probability modeling.
- Complex multi-variable optimization (linear programming solver).
- Historical deal replay with time-travel debugging.
- Scenario sharing and collaborative editing between reps.

## Counterfactual Delta Contract
Every scenario response must include these deterministic deltas from baseline:
- `price_delta`: subtotal, discount_total, tax_total, total, margin (absolute and percentage deltas).
- `policy_delta`: newly-failed rules, newly-cleared rules, severity transitions.
- `approval_delta`: required approver role changes, chain length changes, escalation risk.
- `configuration_delta`: changed line items, quantities, attributes, and constraint outcomes.

The UI may summarize these deltas in natural language, but source values come only from
deterministic engines and persisted artifacts.

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
| Validation parity with standard pricing path | N/A (new) | 100% | CPQ owner | same input through SIM and standard quote path must match exactly |
| Median scenarios generated per quote | 0 | >= 2.5 | Product owner | average scenarios created before quote finalization |
| Quote iteration reduction | 4.2 iterations | <= 2.5 iterations | Sales Ops owner | median quote versions before approval |
| P95 scenario latency | N/A | <= 800ms | Platform owner | request received to Slack response posted |
| Scenario-to-close conversion | N/A | >= 40% | Product owner | percent of scenarios where selected variant wins |
| Deterministic consistency rate | N/A | 100% | Determinism owner | `identical_inputs_same_output / total_regenerations` |
| Approval surprise reduction | N/A (new) | >= 30% | Sales Ops owner | decrease in late-stage approval reroutes after scenario pre-check |

## Deterministic Safety Constraints
- All scenario pricing must delegate to existing deterministic CPQ engines (pricing, policy, constraint).
- Scenario generation must never alter the parent quote state; scenarios are read-only forks.
- Identical scenario inputs must produce identical outputs 100% of the time (pure function guarantee).
- Scenario comparison must clearly distinguish between "scenario" (hypothetical) and "quote" (committed).
- LLMs may format comparison text but cannot modify scenario calculations or rankings.
- All scenario data must be persisted with parent quote linkage for audit purposes.
- Scenario ranking must be deterministic (stable tie-breaking by `variant_id`).
- Promotion from scenario to quote must be explicit, idempotent, and auditable.
- Unsupported parameters must fail fast with user-safe errors (no silent parameter drop).

## Interface Boundaries (Draft)
### Domain Contracts
- `ScenarioRequest`: `quote_id`, `thread_id`, `actor_id`, `base_version`, `variation_params`.
- `ScenarioVariant`: `variant_id`, `params`, `pricing_result`, `policy_decision`, `approval_route`, `comparison_rank`.
- `ScenarioDelta`: `price_delta`, `policy_delta`, `approval_delta`, `configuration_delta`.
- `ScenarioComparison`: `base_quote`, `variants: Vec<ScenarioVariant>`, `deltas: Vec<ScenarioDelta>`, `tradeoff_summary`.
- `ScenarioFork`: parent quote snapshot + variant parameters -> isolated scenario quote.

### Service Contracts
- `DealFlightSimulator::generate_scenarios(request, count) -> Vec<ScenarioVariant>`
- `DealFlightSimulator::compare_variants(variants) -> ScenarioComparison`
- `DealFlightSimulator::promote_variant(variant_id, idempotency_key) -> PromotionResult`
- `DealFlightSimulator::persist_scenario(scenario) -> ScenarioId`
- `DealFlightSimulator::list_scenarios(quote_id) -> Vec<ScenarioSummary>`

### Persistence Contracts
- `ScenarioRepo`: save/fork scenario quotes with parent linkage.
- `ScenarioComparisonRepo`: store comparison results and selected variants.
- `ScenarioDeltaRepo`: store normalized deltas for queryable analytics.
- `ScenarioAuditRepo`: append-only log of scenario generation and selection events.

### Slack Contract
- `/quote simulate [discount=X%] [term=Y months]` command in quote thread context.
- Comparison card shows: base quote, 2-3 variants, side-by-side pricing breakdown.
- Variant selection button maps deterministically to scenario promotion workflow.
- Error card includes: parameter validation failures, constraint violations, and next actions.

### Crate Boundaries
- `quotey-core`: scenario generation logic, CPQ delegation, comparison engine.
- `quotey-db`: scenario persistence, parent-child quote relationships, audit logging.
- `quotey-slack`: command parsing, comparison card rendering (no business logic).
- `quotey-agent`: orchestration, guardrail enforcement, user interaction flow.

## API and Persistence Addendum
### API Surface (Wave 1)
- `simulate_quote(ScenarioRequest) -> ScenarioComparison`
- `list_quote_scenarios(quote_id, base_version) -> Vec<ScenarioSummary>`
- `promote_scenario(quote_id, scenario_id, idempotency_key) -> PromotionResult`

### Persistence Surface (Wave 1)
- `scenario_run`: one simulator invocation tied to `quote_id`, `base_version`, `actor_id`.
- `scenario_variant`: immutable variant payload plus deterministic outputs.
- `scenario_delta`: normalized deltas for rendering and analytics.
- `scenario_audit`: append-only event stream (`generated`, `compared`, `promoted`, `rejected`).

## Task 2 Persistence and Migration Contract
### Schema Additions (Migration `0014_deal_flight_simulator`)
- `deal_flight_scenario_run`
- `deal_flight_scenario_variant`
- `deal_flight_scenario_delta`
- `deal_flight_scenario_audit`

### Fixture and Seed Assumptions
- Scenario records always reference an existing baseline quote (`quote.id` FK enforced).
- Variant cardinality remains bounded (`1..=5`) for deterministic runtime costs.
- Delta rows are unique per `(scenario_variant_id, delta_type)` to prevent ambiguity in comparisons.
- Promotion is represented as one selected variant per run (`selected_for_promotion` flag).

### Migration Verification Requirements
- Apply cleanly on empty schema and existing schema with prior migrations.
- Undo removes all simulator-managed objects cleanly.
- Up/down/up preserves managed schema signature exactly.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Scenario pricing diverges from quote pricing over time | High | Medium | immutable snapshots + versioned CPQ rules | Data owner |
| User confusion between scenario and committed quote | High | Medium | clear visual distinction + confirmation flows | UX owner |
| Scenario explosion (too many variants generated) | Medium | Medium | bounded generation (max 3 variants) + parameter validation | Runtime owner |
| Non-deterministic results from floating-point drift | Medium | Low | Decimal arithmetic only + deterministic ordering | Determinism owner |
| Performance degradation with complex quote configs | Medium | Medium | scenario caching + async generation for large configs | Platform owner |
| Scenario persistence bloat | Low | Medium | TTL cleanup policy + archival for old scenarios | Data owner |

## Deterministic Test Matrix Requirements
- Identical `ScenarioRequest` replay yields byte-identical `ScenarioComparison`.
- Policy/approval deltas are exact set differences from baseline evaluation artifacts.
- Promotion path creates one quote revision per idempotency key, even with retries.
- Constraint violations are reflected both in variant result and user-safe Slack error copy.
- Decimal precision is preserved across baseline and scenario runs (no floating-point drift).

## Implementation Exit Criteria for Task 1
| Area | Exit Signal |
|---|---|
| Scope | In-scope/out-of-scope boundaries are explicit and actionable for task slicing. |
| KPI | Baseline/target/owner/measurement is defined for each SIM objective metric. |
| Determinism | Guardrail statements map to testable deterministic invariants. |
| Contracts | Domain/service/persistence/Slack boundaries are mapped to crate ownership. |
| Risk | Mitigation path exists for each high-impact identified risk. |
