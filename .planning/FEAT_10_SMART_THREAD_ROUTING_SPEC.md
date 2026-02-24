# FEAT-10 Smart Thread Routing Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-70d.10`
(`Smart Thread Routing`) so Slack conversations are intelligently routed to appropriate agents, workflows, or human experts.

## Scope
### In Scope
- Intent classification for incoming Slack messages.
- Quote-to-thread mapping and lifecycle tracking.
- Routing rules: which agent handles which intent type.
- Handoff protocols between agents and to human experts.
- Thread context preservation across routing decisions.
- Telemetry for routing accuracy and resolution times.

### Out of Scope (for Wave 1)
- Voice/video routing or real-time presence.
- Cross-workspace routing (single workspace scope).
- Predictive routing based on user behavior patterns.
- Automatic escalation without user request.

## Rollout Slices
- `Slice A` (contracts): intent taxonomy, routing rules schema, handoff protocol.
- `Slice B` (classifier): intent detection, confidence scoring, fallback handling.
- `Slice C` (runtime): routing engine, agent dispatch, context handoff.
- `Slice D` (UX): thread lifecycle UI, routing transparency, metrics.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Routing accuracy | N/A | >= 90% | ML owner | correct intent classification |
| Routing latency | N/A | <= 200ms | Platform owner | message received to agent dispatch |
| Human escalation rate | N/A | <= 15% | Product owner | escalations / total routed |
| Thread context preservation | N/A | 100% | Runtime owner | successful handoffs with context |
| User satisfaction with routing | N/A | >= 4.0/5 | UX owner | post-interaction CSAT |

## Deterministic Safety Constraints
- Routing rules are explicit and auditable; no opaque ML-only routing.
- Confidence threshold gates: low confidence always escalates to human.
- Financial decisions never made by routing layer; only dispatched to appropriate agent.
- Thread context includes: quote_id, conversation history, pending actions.
- Human escalation is always available via explicit user request.

## Interface Boundaries (Draft)
### Domain Contracts
- `RoutingIntent`: intent_type, confidence, entities, quote_context.
- `RoutingDecision`: target_agent, confidence, fallback_agent, handoff_context.
- `ThreadContext`: thread_id, quote_id, conversation_history, pending_actions.
- `HandoffRecord`: from_agent, to_agent, context_snapshot, timestamp.

### Service Contracts
- `RoutingService::classify_intent(message, context) -> RoutingIntent`
- `RoutingService::route(intent, thread_context) -> RoutingDecision`
- `RoutingService::handoff(from_agent, to_agent, context) -> HandoffResult`
- `RoutingService::escalate_to_human(thread_id, reason) -> EscalationResult`

### Persistence Contracts
- `RoutingRuleRepo`: intent-to-agent routing configuration.
- `ThreadContextRepo`: thread state and conversation history.
- `RoutingAuditRepo`: routing decisions and handoff log.

### Slack Contract
- Bot listens to all thread messages in configured channels.
- Messages classified and routed within 200ms.
- Thread reply indicates routing target (transparency).
- Human escalation available via "@expert" or "help" command.
- Context preserved when switching between agents.

### Crate Boundaries
- `quotey-slack`: message ingestion, thread management, response formatting.
- `quotey-agent`: intent classification, routing engine, handoff orchestration.
- `quotey-core`: routing rules, context model, deterministic guards.
- `quotey-db`: thread context and routing audit persistence.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Misrouting to wrong agent | High | Medium | confidence threshold + explicit confirmation | ML owner |
| Context loss during handoff | High | Low | structured context snapshot + validation | Runtime owner |
| Routing loop between agents | Medium | Low | max handoff counter + cycle detection | Platform owner |
| Over-escalation wasting expert time | Medium | Medium | tiered escalation + routing analytics | Product owner |
| User confusion about which agent responds | Medium | Medium | routing transparency + agent identity cards | UX owner |

## Guardrail Checklist (Pre-implementation Exit)
- Scope and non-goals agreed.
- KPI owner and metric formula explicitly documented.
- Deterministic constraints copied into implementation task templates.
- Interface contracts reviewed against existing crate boundaries.
- Risks and mitigations acknowledged by feature owner.

## Migration Contract
### Schema Additions (`0021_smart_thread_routing`)
- `routing_rules`: intent-to-agent mapping configuration.
- `thread_contexts`: conversation state and history.
- `routing_decisions`: audit log of routing choices.
- `handoff_records`: agent-to-agent context transfer log.

### Version and Audit Semantics
- Routing rules versioned; changes apply to new threads only.
- Each routing decision logged with full context for replay.
- Thread context immutable snapshots at each handoff.

### Migration Behavior and Rollback
- Migration adds routing tables; no changes to existing threads.
- Existing threads continue with current routing (if any).
- Rollback removes routing tables; threads revert to default bot behavior.
