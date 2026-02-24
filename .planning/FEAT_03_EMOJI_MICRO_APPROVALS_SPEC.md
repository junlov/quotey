# FEAT-03 Emoji-Based Micro-Approvals Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-70d.3`
(`Emoji-Based Micro-Approvals`) so low-value policy exceptions can be approved via Slack emoji reactions while maintaining audit compliance.

## Scope
### In Scope
- Emoji reaction capture (✅ 👍 🚀) as approval signals in Slack threads.
- Policy-aware routing: which reactions count as approvals for which policy types.
- Authority verification: emoji reactor must have approval permission.
- Approval audit trail with emoji context (who, when, which emoji).
- Fallback to formal approval for high-value exceptions.
- Telemetry for emoji approval usage and escalation rates.

### Out of Scope (for Wave 1)
- Custom emoji creation or management.
- Emoji approvals outside Slack (email, mobile apps).
- Complex multi-emoji sequences or voting thresholds.
- Automatic approval based on emoji sentiment analysis.

## Rollout Slices
- `Slice A` (contracts): emoji approval policy schema, authority mapping, audit format.
- `Slice B` (routing): Slack event ingestion, reactor authority check, policy matching.
- `Slice C` (runtime): emoji approval service, formal approval fallback, audit logging.
- `Slice D` (UX): reaction instructions in thread, approval confirmation, metrics.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Emoji approval adoption rate | 0% | >= 30% | Product owner | emoji approvals / total micro-approvals |
| Approval latency (micro) | 4 hours (formal) | <= 5 min | UX owner | request posted to emoji approval recorded |
| Escalation to formal approval | N/A | <= 20% | Runtime owner | escalations / emoji approval attempts |
| Audit completeness | N/A | 100% | Compliance owner | emoji approvals with full audit trail |
| User confusion rate | N/A | <= 5% | UX owner | support tickets related to emoji approvals |

## Deterministic Safety Constraints
- Only pre-configured emojis (✅ 👍 🚀) trigger approval processing.
- Reactor authority is verified against policy before approval recorded.
- Emoji approval only valid for micro-approval policy tier (<=$X value).
- High-value exceptions always require formal approval workflow.
- Each emoji approval creates immutable audit entry with full context.
- LLMs never interpret emoji sentiment; only exact emoji matches count.

## Interface Boundaries (Draft)
### Domain Contracts
- `EmojiApprovalPolicy`: allowed_emojis, value_threshold, required_authority.
- `EmojiApprovalEvent`: emoji, reactor_id, thread_id, timestamp, quote_id.
- `EmojiApprovalResult`: status (approved/escalated/rejected), audit_id, reason.

### Service Contracts
- `EmojiApprovalService::process_reaction(event) -> EmojiApprovalResult`
- `EmojiApprovalService::check_authority(reactor_id, policy) -> AuthorityCheck`
- `EmojiApprovalService::escalate_to_formal(quote_id, reason) -> EscalationResult`
- `EmojiApprovalService::get_policy_for_quote(quote_id) -> EmojiApprovalPolicy`

### Persistence Contracts
- `EmojiApprovalPolicyRepo`: per-tenant emoji approval configuration.
- `EmojiApprovalAuditRepo`: append-only emoji approval event log.
- `EmojiEscalationRepo`: formal approval escalation records.

### Slack Contract
- Bot listens to `reaction_added` events in quote threads.
- Emoji reactions trigger immediate authority check and response.
- Approved: thread reply with confirmation and audit reference.
- Escalated: thread reply with formal approval link and reason.
- Instructions posted when micro-approval threshold applies.

### Crate Boundaries
- `quotey-slack`: emoji event handling, thread response formatting.
- `quotey-core`: emoji approval logic, authority verification.
- `quotey-db`: policy storage, audit persistence.
- `quotey-agent`: routing decisions, escalation handling.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Unauthorized emoji approval | High | Low | strict authority check + value threshold gating | Security owner |
| Emoji approval ambiguity | Medium | Medium | clear instructions + limited emoji set | UX owner |
| Audit gap from missed events | High | Low | idempotent event processing + reconciliation | Runtime owner |
| Social pressure to approve | Medium | Medium | private escalation option + approval justification | Compliance owner |
| Race condition on concurrent reactions | Medium | Low | first-valid-wins + duplicate detection | Data owner |

## Guardrail Checklist (Pre-implementation Exit)
- Scope and non-goals agreed.
- KPI owner and metric formula explicitly documented.
- Deterministic constraints copied into implementation task templates.
- Interface contracts reviewed against existing crate boundaries.
- Risks and mitigations acknowledged by feature owner.

## Migration Contract
### Schema Additions (`0018_emoji_approvals`)
- `emoji_approval_policies`: per-tenant emoji approval configuration.
- `emoji_approval_audit`: emoji approval event log.
- `emoji_escalations`: formal approval escalation records.

### Version and Audit Semantics
- Each emoji approval creates immutable audit entry.
- Escalations preserve original emoji context for audit.
- Policy changes versioned; old approvals remain valid under prior policy.

### Migration Behavior and Rollback
- Migration adds emoji policy tables with sensible defaults (disabled).
- Emoji approvals disabled by default; enable per-tenant via policy.
- Rollback removes emoji-specific tables; core approval system unaffected.
