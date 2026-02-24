# FEAT-08 Immutable Quote Ledger Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-70d.8`
(`Immutable Quote Ledger`) so all quote mutations are append-only, auditable, and cryptographically verifiable.

## Scope
### In Scope
- Append-only quote version chain with cryptographic hashing.
- Content-addressed storage: each version has unique hash.
- Chain verification: detect tampering via hash mismatch.
- Efficient retrieval: latest version, specific version, version range.
- Audit API: replay quote evolution, verify chain integrity.
- Integration with Explainable Policy for version-bound explanations.

### Out of Scope (for Wave 1)
- Blockchain or distributed ledger (single-tenant SQLite only).
- Automatic conflict resolution for concurrent edits.
- External notarization or third-party attestation.
- Content deduplication across quotes (per-quote chains only).

## Rollout Slices
- `Slice A` (contracts): ledger entry schema, hash algorithm, chain structure.
- `Slice B` (storage): SQLite schema, hash computation, append-only enforcement.
- `Slice C` (runtime): ledger service, verification API, retrieval operations.
- `Slice D` (integration): policy/explanation version binding, audit UI, telemetry.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Ledger append latency | N/A | <= 5ms | Platform owner | mutation to ledger entry persisted |
| Chain verification time | N/A | <= 100ms for 1000 versions | Data owner | full chain hash verification |
| Storage overhead | N/A | <= 15% delta-only | Data owner | ledger storage / quote table storage |
| Audit query latency | N/A | <= 200ms | Platform owner | version range retrieval |
| Tamper detection coverage | N/A | 100% | Security owner | all mutations covered by ledger |

## Deterministic Safety Constraints
- Ledger entries are immutable after creation; no updates or deletes permitted.
- Hash computation includes: content hash, prev_hash, timestamp, actor, action.
- Content hash is deterministic: same quote state always produces same hash.
- Chain breaks (hash mismatch) trigger immediate alert and investigation.
- All quote mutations must create ledger entry; no silent changes permitted.

## Interface Boundaries (Draft)
### Domain Contracts
- `LedgerEntry`: version, content_hash, prev_hash, timestamp, actor_id, action, metadata.
- `QuoteVersion`: quote_id, version_number, ledger_entry_ref, content_snapshot.
- `ChainVerification`: valid boolean, first_break_at (if invalid), integrity_score.

### Service Contracts
- `LedgerService::append(quote_id, action, actor, content) -> LedgerEntry`
- `LedgerService::get_version(quote_id, version) -> QuoteVersion`
- `LedgerService::get_latest(quote_id) -> QuoteVersion`
- `LedgerService::get_range(quote_id, start, end) -> Vec<QuoteVersion>`
- `LedgerService::verify_chain(quote_id) -> ChainVerification`
- `LedgerService::replay(quote_id, from_version) -> Vec<QuoteVersion>`

### Persistence Contracts
- `QuoteLedgerRepo`: append-only ledger entry storage.
- `QuoteVersionRepo`: version-to-content mapping.
- `LedgerAuditRepo`: integrity verification log.

### Integration Contracts
- Policy evaluations bind to specific quote_version.
- Explanations reference quote_version for reproducibility.
- Similarity search uses versioned fingerprints.

### Crate Boundaries
- `quotey-core`: ledger logic, hash computation, chain verification.
- `quotey-db`: ledger entry persistence, version retrieval.
- `quotey-agent`: mutation interception, ledger append orchestration.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Hash collision (theoretical) | High | Very Low | 256-bit SHA-256 | Security owner |
| Chain corruption from bugs | High | Low | verification alerts + backup recovery | Data owner |
| Performance degradation at scale | Medium | Medium | delta storage + indexing | Platform owner |
| Immutable data privacy concerns | Medium | Low | GDPR-compliant deletion via tombstones | Compliance owner |
| Concurrent append conflicts | Medium | Low | optimistic locking + retry | Data owner |

## Guardrail Checklist (Pre-implementation Exit)
- Scope and non-goals agreed.
- KPI owner and metric formula explicitly documented.
- Deterministic constraints copied into implementation task templates.
- Interface contracts reviewed against existing crate boundaries.
- Risks and mitigations acknowledged by feature owner.

## Migration Contract
### Schema Additions (`0020_quote_ledger`)
- `quote_ledger`: append-only ledger entries with hashes.
- `quote_versions`: version-to-content mapping.
- `ledger_integrity_checks`: periodic verification results.

### Version and Audit Semantics
- Existing quotes start at version 1 with initial snapshot.
- All historical mutations backfilled as ledger entries.
- Integrity verification runs on migration completion.

### Migration Behavior and Rollback
- Migration creates ledger tables; existing quote data preserved.
- Backfill job generates initial ledger entries for all quotes.
- Rollback not recommended (data loss); archival recovery only.
