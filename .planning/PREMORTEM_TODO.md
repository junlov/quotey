# Premortem Remediation — Master TODO

## Status Key
- [ ] Not started
- [~] In progress
- [x] Complete

---

## P0-1: Cross-Crate Integration Tests (bd-pm01)

- [ ] 1.1 Survey existing test infrastructure (test helpers, fixtures, test DB setup)
- [ ] 1.2 Create integration test crate or `tests/` directory with shared harness
- [ ] 1.3 Write test: quote creation end-to-end (command parse -> domain -> DB persist -> verify)
- [ ] 1.4 Write test: pricing with constraints (product config -> constraint eval -> price calc -> trace)
- [ ] 1.5 Write test: approval routing (threshold check -> routing decision -> Slack packet render)
- [ ] 1.6 Verify all 3 tests pass in CI (`cargo test --all-targets`)

## P0-2: Integer BPS Arithmetic Audit (bd-pm02)

- [ ] 2.1 Grep all f64/f32 usage in pricing paths (cpq/pricing.rs, policy/optimizer.rs, etc.)
- [ ] 2.2 Catalog every place a floating-point value represents money/margin/discount
- [ ] 2.3 Identify which fields already use _bps integer convention
- [ ] 2.4 For fields not yet in _bps: add i64 _bps alternatives or convert
- [ ] 2.5 Separate replay checksum into structural checksum + numeric tolerance check
- [ ] 2.6 Add invariant tests proving bps round-trip stability
- [ ] 2.7 Update CLO replay engine to use tolerance-based comparison where appropriate

## P0-3: NXT Engine-First (bd-pm03) — DESIGN ONLY (epic, not implementable in single session)

- [ ] 3.1 Review NXT spec Slice A/B/C contracts
- [ ] 3.2 Identify minimum viable engine: ConcessionEnvelope + BoundaryEvaluation + CounterofferPlan
- [ ] 3.3 Draft struct definitions in quotey-core
- [ ] 3.4 Draft CLI exerciser command in quotey-cli

## P1-4: CLO Minimum Cohort Gate (bd-pm04)

- [ ] 4.1 Read current candidate generation code in policy/optimizer.rs
- [ ] 4.2 Add `min_cohort_size` config field (default 200)
- [ ] 4.3 Add cohort-size check before candidate generation
- [ ] 4.4 Return descriptive analytics mode result when below threshold
- [ ] 4.5 Add unit test for cohort gate behavior

## P1-5: NXT Deal Context (bd-pm05) — DESIGN ONLY

- [ ] 5.1 Document deal_context field requirements for NXT state model
- [ ] 5.2 Map precedent graph fields that feed into concession envelopes

## P1-6: Migration Up-Chain Tests (bd-pm06)

- [ ] 6.1 Create migration test that runs all ups from empty DB
- [ ] 6.2 Create test that runs up/down/up for last 5 migrations (14-18)
- [ ] 6.3 Add to CI quality gate

## P1-7: Human-Readable Approval Summaries (bd-pm07)

- [ ] 7.1 Read current ApprovalPacket and Slack rendering code
- [ ] 7.2 Add `executive_summary: String` field to ApprovalPacket
- [ ] 7.3 Generate 2-sentence summary from candidate diff + replay impact data
- [ ] 7.4 Render summary prominently in Slack Block Kit card (before evidence sections)
- [ ] 7.5 Add summary to CLI packet display
- [ ] 7.6 Add test for summary generation

## P1-8: Replay Provenance Verification (bd-pm08)

- [ ] 8.1 Read current replay snapshot input path
- [ ] 8.2 Add provenance check: verify snapshot quote_ids exist in quote ledger
- [ ] 8.3 Add provenance check: verify timestamps fall within claimed window
- [ ] 8.4 Block replay when provenance verification fails
- [ ] 8.5 Add test for provenance rejection

## P2-9: Deal Cockpit Web View (bd-pm09) — FUTURE

- [ ] 9.1 Design read-only endpoint in server crate
- [ ] 9.2 Implement minimal HTML template for negotiation state

## P2-10: SQLite Concurrency Benchmark (bd-pm10) — FUTURE

- [ ] 10.1 Write benchmark harness with 15 concurrent writers
- [ ] 10.2 Measure P95 latency and document ceiling
