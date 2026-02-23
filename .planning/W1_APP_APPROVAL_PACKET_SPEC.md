# W1 APP Approval Packet Autopilot Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-271.6`
(`Approval Packet Autopilot`) so approvers can decide in one pass with full context through
auto-assembled concise complete packets.

## Scope
### In Scope
- Automatic assembly of approval context from quote, policy, and historical data.
- Deterministic packet generation with required fields completeness check.
- Routing logic delegation to policy engine.
- One-tap decision actions (approve/reject/request changes) in Slack.
- Packet preview for rep review before submission.
- Approval audit trail with full packet contents.

### Out of Scope (for Wave 1)
- ML-based approver sentiment analysis or decision prediction.
- Automatic approval for low-risk requests without human review.
- Complex multi-level sequential approval chains.
- Integration with external approval systems (email, Salesforce).
- Real-time approver availability or workload balancing.

## Rollout Slices
- `Slice A` (contracts): packet schema, routing model, decision action protocol.
- `Slice B` (engine): packet assembly service, completeness validation, routing logic.
- `Slice C` (UX): packet preview cards, one-tap actions, Slack thread integration.
- `Slice D` (ops): approval metrics, packet quality scoring, runbook.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Approval decision time | 4.2 hours | <= 30 min | Sales Ops owner | median time from packet sent to decision |
| Packet completeness rate | 75% | >= 98% | Product owner | `% packets with all required context fields` |
| One-pass approval rate | 45% | >= 75% | Product owner | `% approvals decided without follow-up questions` |
| Routing accuracy | 88% | >= 95% | Determinism owner | `% packets routed to correct approver first time` |
| Packet generation latency | N/A | <= 300ms | Platform owner | quote finalization to packet ready |
| Approver satisfaction score | 3.2/5 | >= 4.2/5 | UX owner | post-approval survey ratings |

## Deterministic Safety Constraints
- Routing decisions must be 100% delegated to deterministic policy engine.
- Packet contents must be assembled from deterministic sources only (quote, policy, history).
- LLMs may format packet text but cannot modify routing decisions or approval thresholds.
- All required fields must be present; incomplete packets cannot be submitted.
- Approver identity and authority must be verified before accepting decision.
- All packet contents and decisions must be immutably logged for audit.

## Interface Boundaries (Draft)
### Domain Contracts
- `ApprovalPacket`: `quote_summary`, `policy_violations`, `financial_context`, `historical_precedents`, `decision_actions`.
- `PacketSection`: `section_type`, `content`, `source_refs`, `priority`.
- `RoutingDecision`: `approver_role`, `approver_id`, `routing_reason`, `escalation_path`.
- `DecisionAction`: `action_type` (approve/reject/changes), `payload`, `auth_requirements`.

### Service Contracts
- `ApprovalPacketService::assemble_packet(quote_id) -> ApprovalPacket`
- `ApprovalPacketService::validate_completeness(packet) -> CompletenessResult`
- `ApprovalPacketService::route_packet(packet) -> RoutingDecision`
- `ApprovalPacketService::preview_packet(packet) -> PacketPreview`
- `ApprovalPacketService::process_decision(packet, action) -> DecisionResult`
- `ApprovalPacketService::get_packet_audit(packet_id) -> PacketAudit`

### Persistence Contracts
- `ApprovalPacketRepo`: packet storage with immutability guarantees.
- `RoutingDecisionRepo`: routing logic outcomes and approver assignments.
- `DecisionAuditRepo`: complete decision history with packet snapshots.
- `ApproverAuthorityRepo`: approver roles, limits, and delegation rules.

### Slack Contract
- Packet preview card in quote thread before submission.
- Approval request card in approver DM or channel with full context.
- One-tap buttons: `Approve`, `Reject`, `Request Changes`.
- Decision confirmation with comment field.
- Thread notification of decision outcome with reasoning.

### Crate Boundaries
- `quotey-core`: packet assembly, routing logic, decision processing.
- `quotey-db`: packet storage, audit logging, authority verification.
- `quotey-slack`: card rendering, action handling (no business logic).
- `quotey-agent`: workflow orchestration, rep/approver interaction flow.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Incomplete packet leads to poor decision | High | Medium | mandatory completeness check + preview step | Product owner |
| Wrong approver routed | High | Medium | policy engine validation + escalation fallback | Determinism owner |
| Packet content too verbose | Medium | High | section prioritization + collapsible sections | UX owner |
| Approver authority verification failure | High | Low | strict auth checks + audit logging | Security owner |
| Decision action fails silently | High | Low | explicit confirmation + retry logic + alerts | Runtime owner |
| Performance issues with complex quotes | Medium | Medium | async assembly + caching + pagination | Platform owner |

## Guardrail Checklist (Pre-implementation Exit)
- [ ] Scope and non-goals approved for Wave 1 execution.
- [ ] KPI baseline, target, owner, and query definitions captured.
- [ ] Deterministic constraints mapped to runtime acceptance tests.
- [ ] Service and repository contracts aligned with crate ownership boundaries.
- [ ] Risk mitigations reviewed and assigned before Task 2 implementation.
- [ ] Approval packet UX reviewed for completeness and clarity.
