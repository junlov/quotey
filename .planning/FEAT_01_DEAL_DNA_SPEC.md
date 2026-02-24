# FEAT-01 Deal DNA Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-70d.1`
(`Deal DNA - Configuration Fingerprint Matching`) so similarity-based quote recommendations remain auditable and deterministic.

## Scope
### In Scope
- Configuration fingerprint generation using MinHash/SimHash LSH techniques.
- Fingerprint persistence and indexing for sub-linear similarity search.
- Similar deal retrieval API with configurable similarity thresholds.
- Quote-to-deal matching for Ghost Quotes and Precedent Intelligence integration.
- Telemetry for match quality, index performance, and recommendation acceptance.

### Out of Scope (for Wave 1)
- Real-time fingerprint updates on every quote mutation (batch/recalc model only).
- Cross-tenant similarity search (single-tenant scope only).
- ML-based similarity beyond LSH (neural embeddings, etc.).
- Automatic quote modification based on similar deals.

## Rollout Slices
- `Slice A` (contracts): fingerprint schema, hash functions, similarity scoring model.
- `Slice B` (index): SQLite persistence, MinHash signature storage, inverted index.
- `Slice C` (runtime): SimilarityEngine with threshold-based candidate retrieval.
- `Slice D` (integration): Ghost Quotes + Precedent Intelligence consumption, telemetry.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Similarity search P95 latency | N/A | <= 50ms | Index owner | query time for top-k similar quotes |
| Fingerprint generation time | N/A | <= 10ms | Runtime owner | time to hash full quote configuration |
| Match relevance score | N/A | >= 0.75 | Product owner | user acceptance rate of similar deal suggestions |
| Index storage overhead | N/A | <= 5% of quote data | Data owner | fingerprint storage / quote table storage |
| False positive match rate | N/A | <= 10% | Determinism owner | low-similarity quotes returned as candidates |

## Deterministic Safety Constraints
- Fingerprint generation must be deterministic: same quote configuration always produces same signature.
- Hash functions and seeds must be versioned and persisted for reproducibility.
- Similarity scores are computed deterministically from fingerprints, never guessed or LLM-derived.
- Candidate retrieval uses exact LSH bands; no probabilistic approximate search without explicit bounds.
- Similar deals are advisory only; final quote decisions remain with deterministic CPQ engines.

## Interface Boundaries (Draft)
### Domain Contracts
- `ConfigurationFingerprint`: MinHash signature (128-bit vector), quote_id, version, created_at.
- `SimilarityCandidate`: quote_id, similarity_score (0.0-1.0), match_details.
- `FingerprintVersion`: hash_algorithm, num_hashes, seed_version for reproducibility.

### Service Contracts
- `FingerprintService::generate(quote) -> ConfigurationFingerprint`
- `FingerprintService::regenerate(quote_id) -> ConfigurationFingerprint`
- `SimilarityEngine::find_similar(query_quote, threshold, limit) -> Vec<SimilarityCandidate>`
- `SimilarityEngine::compute_similarity(quote_a, quote_b) -> f64`

### Persistence Contracts
- `FingerprintRepo`: store/retrieve/update fingerprints by quote_id.
- `FingerprintIndexRepo`: LSH band index for sub-linear candidate retrieval.
- `SimilarityQueryAuditRepo`: append-only log of similarity queries and results.

### Integration Contracts
- GhostQuotes: uses similarity search to find comparable historical deals.
- PrecedentIntelligence: consumes similarity candidates for pricing baseline analysis.

### Crate Boundaries
- `quotey-core`: fingerprint generation algorithms, SimilarityEngine trait.
- `quotey-db`: FingerprintRepo, index storage, query audit persistence.
- `quotey-agent`: orchestrates fingerprint generation on quote changes.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Hash collisions produce false matches | High | Low | 128-bit signatures + band verification | Determinism owner |
| Index query performance degrades with scale | Medium | Medium | LSH bands + candidate threshold capping | Index owner |
| Fingerprint version incompatibility | Medium | Low | explicit version field + migration path | Data owner |
| Similar deals recommend inappropriate pricing | High | Low | clear advisory labeling + no auto-apply | Product owner |
| Cross-customer data leakage via similarity | High | Low | tenant-scoped index queries | Security owner |

## Guardrail Checklist (Pre-implementation Exit)
- Scope and non-goals agreed.
- KPI owner and metric formula explicitly documented.
- Deterministic constraints copied into implementation task templates.
- Interface contracts reviewed against existing crate boundaries.
- Risks and mitigations acknowledged by feature owner.

## Migration Contract
### Schema Additions (`0016_deal_dna`)
- `quote_fingerprints`: MinHash signatures keyed by quote_id and version.
- `fingerprint_index_lsh`: LSH band index for efficient candidate retrieval.
- `similarity_query_audit`: append-only query log for telemetry and debugging.
- `fingerprint_versions`: version registry for hash algorithm parameters.

### Version and Audit Semantics
- Each fingerprint is bound to a specific quote version (immutable after generation).
- Fingerprint regeneration creates new version entry, old version retained for audit.
- LSH index updates are atomic with fingerprint persistence.

### Migration Behavior and Rollback
- Migration is additive; existing quotes have null fingerprints initially.
- Backfill job generates fingerprints for historical quotes post-migration.
- Rollback removes index tables only; fingerprint data preserved for re-indexing.
