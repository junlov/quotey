# FEAT-09 Configuration Archaeology Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-70d.9`
(`Configuration Archaeology`) so users can understand why products cannot be added and receive actionable resolution paths.

## Scope
### In Scope
- Dependency graph construction from product catalog and constraints.
- Blockage detection: identify why a product cannot be added.
- Root cause analysis with full dependency walkback.
- Resolution pathfinding: alternative ways to enable the product.
- Natural language explanation generation from deterministic analysis.
- Slack integration with interactive resolution options.

### Out of Scope (for Wave 1)
- Automatic constraint relaxation without user approval.
- Cross-catalog dependency analysis (single catalog scope).
- Historical constraint evolution (current state only).
- ML-based resolution prediction.

## Rollout Slices
- `Slice A` (contracts): dependency graph schema, blockage report model, resolution format.
- `Slice B` (engine): graph builder, blockage analyzer, resolution pathfinder.
- `Slice C` (runtime): archaeology service, NL explanation generator, action executor.
- `Slice D` (integration): Slack thread cards, tree visualization, interactive fixes.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Blockage explanation time | 20 min (manual) | <= 2 min | Product owner | query to explanation delivered |
| Resolution success rate | N/A | >= 80% | Runtime owner | resolved blockages / attempted resolutions |
| User comprehension score | N/A | >= 4.0/5 | UX owner | clarity rating of explanations |
| False resolution rate | N/A | <= 5% | Determinism owner | resolutions creating new violations |
| Explanation completeness | N/A | >= 95% | Determinism owner | blockages with full dependency trace |

## Deterministic Safety Constraints
- Dependency graph built deterministically from catalog and constraints.
- Resolution paths computed via graph traversal; no LLM-generated fixes.
- User must confirm any configuration change; no automatic application.
- Each resolution step validated against constraints before proceeding.
- Explanations reflect actual graph state; no hallucinated dependencies.

## Interface Boundaries (Draft)
### Domain Contracts
- `DependencyNode`: node_id, node_type, name, status, metadata.
- `DependencyEdge`: from, to, edge_type, condition.
- `BlockageReport`: target_product, blockages (type, description, resolution_hint).
- `ResolutionPath`: steps, total_effort, outcome_preview.

### Service Contracts
- `ArchaeologyService::build_graph(quote) -> DependencyGraph`
- `ArchaeologyService::analyze_blockage(graph, product_id) -> Option<BlockageReport>`
- `ArchaeologyService::find_resolution_paths(blockage) -> Vec<ResolutionPath>`
- `ArchaeologyService::apply_resolution(quote_id, path, actor) -> ResolutionResult`
- `ArchaeologyService::explain_in_nl(report, resolutions) -> String`

### Persistence Contracts
- `ConstraintGraphRepo`: cached dependency graphs per quote version.
- `BlockageAuditRepo`: blockage queries and resolution attempts.
- `ResolutionPathRepo`: computed paths and selection history.

### Slack Contract
- "Why can't I add X?" natural language query triggers analysis.
- Response includes: dependency tree visualization, root cause, fix options.
- Tree shown as ASCII or nested list with status emojis.
- Resolution options ranked by effort (easy/medium/hard).
- Action buttons apply fixes with confirmation steps.

### Crate Boundaries
- `quotey-core`: dependency graph, blockage analysis, resolution pathfinding.
- `quotey-db`: graph caching, blockage audit, path persistence.
- `quotey-slack`: NL query parsing, tree visualization, interactive cards.
- `quotey-agent`: explanation generation, resolution orchestration.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Resolution creates cascading violations | High | Medium | step validation + rollback capability | Determinism owner |
| Graph construction performance at scale | Medium | Medium | incremental updates + caching | Platform owner |
| NL explanation misrepresents constraints | High | Low | deterministic graph source + validation | Runtime owner |
| Circular dependency detection failure | High | Low | cycle detection in graph builder | Data owner |
| User confusion from complex dependencies | Medium | Medium | progressive disclosure + simplification | UX owner |

## Guardrail Checklist (Pre-implementation Exit)
- [ ] Scope and non-goals agreed.
- [ ] KPI owner and metric formula explicitly documented.
- [ ] Deterministic constraints copied into implementation task templates.
- [ ] Interface contracts reviewed against existing crate boundaries.
- [ ] Risks and mitigations acknowledged by feature owner.

## Migration Contract
### Schema Additions (`0025_configuration_archaeology`)
- `constraint_graphs`: cached dependency graphs per quote.
- `blockage_queries`: audit log of blockage analysis requests.
- `resolution_paths`: computed resolution options.
- `archaeology_settings`: feature enablement and caching config.

### Version and Audit Semantics
- Graphs cached per quote version; regenerated on quote change.
- Blockage queries logged for pattern analysis.
- Resolutions validated against current quote state at apply time.

### Migration Behavior and Rollback
- Migration adds archaeology tables; no changes to catalog schema.
- Graph caching disabled by default; enable via configuration.
- Rollback removes archaeology tables; core CPQ unaffected.
