# FEAT-02 Conversational Constraint Solver Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-70d.2`
(`Conversational Constraint Solver`) so users can resolve configuration conflicts through natural language dialogue while maintaining deterministic CPQ safety.

## Scope
### In Scope
- Natural language constraint violation explanation (why X cannot be added).
- Interactive resolution path presentation (options to resolve conflict).
- Dependency graph traversal for blockage root cause analysis.
- Resolution action application with user confirmation.
- Conversation state persistence across Slack thread messages.
- Telemetry for resolution success rate and path selection patterns.

### Out of Scope (for Wave 1)
- Automatic constraint resolution without user confirmation.
- Cross-quote constraint solving (single quote scope only).
- Learning/ML for resolution prediction.
- Voice or non-Slack conversational interfaces.

## Rollout Slices
- `Slice A` (contracts): resolution path model, conversation state schema, NL explanation format.
- `Slice B` (engine): dependency graph builder, blockage analyzer, resolution pathfinder.
- `Slice C` (runtime): constraint solver service, conversation state machine, action executor.
- `Slice D` (UX): Slack thread integration, interactive resolution cards, telemetry.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Constraint resolution time | 15 min (manual) | <= 3 min | Product owner | time from violation detected to resolved |
| Resolution success rate | N/A | >= 85% | Runtime owner | successful resolutions / attempted resolutions |
| User satisfaction score | N/A | >= 4.0/5 | Product owner | post-resolution CSAT survey |
| False resolution rate | N/A | <= 5% | Determinism owner | resolutions that create new violations |
| NL explanation clarity score | N/A | >= 4.0/5 | UX owner | user rating of explanation helpfulness |

## Deterministic Safety Constraints
- Resolution paths are computed deterministically from constraint graph; no LLM-generated fixes.
- User must explicitly confirm any configuration change; no auto-application.
- Each resolution step validates constraint satisfaction before proceeding.
- Conversation state is append-only; actions are recorded for audit.
- LLMs may rephrase explanations but cannot modify valid resolution options.

## Interface Boundaries (Draft)
### Domain Contracts
- `ConstraintViolation`: product_id, constraint_type, blocking_products, description.
- `ResolutionPath`: ordered steps, estimated difficulty, outcome preview.
- `ResolutionStep`: action_type, target_product, preconditions, postconditions.
- `ConversationState`: quote_id, thread_id, current_violation, offered_paths, selected_path.

### Service Contracts
- `ConstraintSolverService::explain_violation(quote_id, product_id) -> ViolationExplanation`
- `ConstraintSolverService::find_resolution_paths(violation) -> Vec<ResolutionPath>`
- `ConstraintSolverService::apply_resolution(quote_id, path_id, actor) -> ResolutionResult`
- `ConstraintSolverService::validate_resolution(quote_id, path) -> ValidationResult`

### Persistence Contracts
- `ConstraintViolationRepo`: store/retrieve violations with resolution attempts.
- `ResolutionPathRepo`: persist computed paths and selection history.
- `SolverConversationRepo`: conversation state and user interaction log.

### Slack Contract
- Constraint violations trigger thread notification with explanation card.
- Resolution paths presented as numbered options with difficulty indicators.
- User selects path via button; each step requires confirmation.
- Progress updates posted as reply messages in thread.

### Crate Boundaries
- `quotey-core`: dependency graph, constraint analysis, resolution pathfinding.
- `quotey-db`: violation and conversation persistence.
- `quotey-slack`: thread integration, card rendering, button handling.
- `quotey-agent`: NL explanation generation (post-deterministic path computation).

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Resolution creates cascading violations | High | Medium | post-action validation + rollback capability | Determinism owner |
| LLM hallucinates invalid resolution paths | High | Low | paths computed deterministically before NL generation | Runtime owner |
| Conversation state lost mid-resolution | Medium | Medium | persistent state machine + recovery prompt | UX owner |
| User confusion from too many options | Medium | Medium | rank by difficulty + limit to top 3 paths | Product owner |
| Concurrent edits during resolution | High | Medium | optimistic locking + conflict detection | Data owner |

## Guardrail Checklist (Pre-implementation Exit)
- Scope and non-goals agreed.
- KPI owner and metric formula explicitly documented.
- Deterministic constraints copied into implementation task templates.
- Interface contracts reviewed against existing crate boundaries.
- Risks and mitigations acknowledged by feature owner.

## Migration Contract
### Schema Additions (`0017_constraint_solver`)
- `constraint_violations`: violation records with resolution status.
- `resolution_paths`: computed paths with step sequences.
- `solver_conversations`: thread-scoped conversation state.
- `resolution_audit`: action execution log for replay and debugging.

### Version and Audit Semantics
- Each resolution attempt is logged with full context (quote version, actor, timestamp).
- Successful resolutions create audit trail entry for compliance.
- Failed resolutions include error details for troubleshooting.

### Migration Behavior and Rollback
- Migration adds tables only; no changes to existing constraint logic.
- Rollback removes solver tables; core CPQ constraint engine unaffected.
