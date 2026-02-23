# W1 MEM Adaptive Extraction Memory Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-271.8`
(`Adaptive Extraction Memory`) so the system learns accepted terminology corrections by account
and team to improve intent extraction precision over time.

## Scope
### In Scope
- Per-account terminology learning and storage.
- Per-team extraction pattern adaptation.
- Explicit correction capture from rep interactions.
- Versioned memory updates with rollback capability.
- Policy-bounded learning (approved corrections only).
- Correction chips in Slack for teaching Quotey.
- Memory source labels for transparency.

### Out of Scope (for Wave 1)
- Unsupervised learning from all conversations without explicit correction.
- Cross-account memory sharing or federation.
- Automatic terminology extraction without rep confirmation.
- Complex NLP model fine-tuning or retraining.
- Real-time learning during active quote workflows.

## Rollout Slices
- `Slice A` (contracts): memory schema, correction model, versioned update protocol.
- `Slice B` (engine): memory store, correction processing, policy enforcement.
- `Slice C` (UX): correction chips in Slack, memory source labels, teaching actions.
- `Slice D` (ops): memory quality metrics, rollback procedures, runbook.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| First-try extraction accuracy | 65% | >= 85% | ML/Extraction owner | `correct_first_extraction / total_extractions` |
| Correction acceptance rate | N/A | >= 70% | UX owner | `% corrections accepted vs dismissed` |
| Repetitive correction reduction | 5.2 per quote | <= 2.0 per quote | Product owner | average identical corrections per account per month |
| Memory lookup latency | N/A | <= 50ms | Platform owner | memory query to extraction result |
| Memory rollback success rate | N/A | 100% | Data owner | `successful_rollbacks / rollback_requests` |
| Cross-rep memory consistency | N/A | >= 95% | Determinism owner | `consistent_extractions_across_reps / total_extractions` |

## Deterministic Safety Constraints
- Memory updates require explicit rep confirmation; no automatic learning from implicit signals.
- All memory changes are versioned and reversible with full audit trail.
- Policy-bounded learning: only approved terminology types can be learned (no pricing, no policy).
- Memory must never override deterministic engine results; it only aids intent extraction.
- Identical inputs must produce identical extractions given the same memory state.
- Memory is scoped (account/team) to prevent inappropriate cross-contamination.

## Interface Boundaries (Draft)
### Domain Contracts
- `ExtractionMemory`: `account_id`, `learned_terms`, `extraction_patterns`, `version`.
- `CorrectionEvent`: `original_extraction`, `corrected_value`, `correction_type`, `actor_id`.
- `MemoryUpdate`: `update_type`, `previous_value`, `new_value`, `justification`, `approved_by`.
- `TermEntry`: `term_alias`, `canonical_form`, `confidence`, `usage_count`, `last_used`.

### Service Contracts
- `AdaptiveMemory::record_correction(event) -> MemoryUpdate`
- `AdaptiveMemory::apply_update(update) -> UpdateResult`
- `AdaptiveMemory::lookup_term(account_id, alias) -> Option<TermEntry>`
- `AdaptiveMemory::get_extraction_hints(account_id) -> ExtractionHints`
- `AdaptiveMemory::rollback_to_version(account_id, version) -> RollbackResult`
- `AdaptiveMemory::export_memory(account_id) -> MemorySnapshot`

### Persistence Contracts
- `ExtractionMemoryRepo`: per-account memory storage with versioning.
- `CorrectionEventRepo`: append-only log of all corrections and outcomes.
- `MemoryVersionRepo`: version history with rollback capability.
- `MemoryAuditRepo`: complete audit trail of memory changes.

### Slack Contract
- Correction chips appear when extraction confidence is low or rep overrides.
- `Teach Quotey` action captures correction with confirmation.
- Memory source label shows: "Learned from your team" or "Account-specific term".
- Visibility into what Quotey knows: `/quote memory show` command.

### Crate Boundaries
- `quotey-core`: memory logic, correction processing, policy enforcement.
- `quotey-db`: memory storage, versioning, audit logging.
- `quotey-agent`: extraction orchestration, correction flow, memory integration.
- `quotey-slack`: correction UI, memory visualization (no business logic).

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Incorrect term learning propagates errors | High | Medium | explicit confirmation + confidence thresholds + review queue | ML owner |
| Memory bloat from too many learned terms | Medium | Medium | TTL expiration + usage-based pruning + max term limits | Data owner |
| Cross-account term leakage | High | Low | strict scoping + access controls + audit logging | Security owner |
| Inability to rollback bad updates | Medium | Low | versioned storage + rollback tests + runbook | Data owner |
| Rep confusion about what Quotey learned | Low | Medium | transparent source labels + memory inspection commands | UX owner |
| Performance degradation with large memory | Low | Medium | indexing + caching + lazy loading | Platform owner |

## Guardrail Checklist (Pre-implementation Exit)
- [ ] Scope and non-goals approved for Wave 1 execution.
- [ ] KPI baseline, target, owner, and query definitions captured.
- [ ] Deterministic constraints mapped to runtime acceptance tests.
- [ ] Service and repository contracts aligned with crate ownership boundaries.
- [ ] Risk mitigations reviewed and assigned before Task 2 implementation.
- [ ] Correction UX reviewed for clarity and minimal friction.
