# QA-PROGRAM Testing Excellence Spec

## Purpose
Define scope, KPI contract, testing philosophy, and implementation boundaries for the QA-PROGRAM so Quotey achieves production-grade testing with maximal real-component coverage, minimal fake-only critical-path tests, and comprehensive E2E scenario validation.

## Scope
### In Scope
- Unit coverage baseline with explicit per-crate thresholds
- Critical-path migration from in-memory fakes to real-component tests
- Complete E2E quote lifecycle scenarios (net-new, renewal, discount, recovery)
- Structured logging standard with machine-validated output
- CI-enforced quality gates (coverage, replay, E2E log quality)
- Deterministic replay harness for pricing/policy/flow decisions
- SQLite-backed integration tests for repository/provider seams
- Test fixture orchestration with canonical seed datasets

### Out of Scope (for Wave 1)
- Fuzzing or property-based testing
- Chaos engineering / random failure injection
- Performance/stress testing (load, soak, spike)
- Security penetration testing
- Multi-node distributed testing

## Rollout Slices
- `Slice A` (audit): File-by-file gap matrix, fake-only test inventory, coverage baseline
- `Slice B` (contracts): Critical-path no-fake policy, exception rubric, canonical event schema
- `Slice C` (implementation): Coverage instrumentation, SQL-backed tests, E2E harness
- `Slice D` (integration): CI gates, log validator, runbook, program closeout

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Line coverage | Unknown | >= 80% | QA | `cargo-llvm-cov` line percentage |
| Critical-path real-component test ratio | ~30% | >= 90% | QA | `% of critical-path tests using real components` |
| E2E scenario coverage | 0 complete flows | 4 complete flows | QA | `net-new, renewal, discount, recovery` |
| E2E log validation pass rate | N/A | 100% | QA | `% of E2E runs passing log schema validation` |
| CI quality gate enforcement | Partial | 100% | Platform | `% of PRs passing all gates` |
| Mean test suite duration | ~60s | <= 120s | Platform | `cargo test --workspace wall time` |

## Deterministic Safety Constraints
- Fake-only tests must not assert on deterministic outcomes (pricing, policy, flow transitions)
- Real-component tests must use deterministic fixtures (same seed = same results)
- E2E tests must produce replayable transcripts with correlation IDs
- Coverage gates must never be bypassed or disabled
- Test failures must include full context (input, expected, actual, diff)

## Interface Boundaries (Draft)
### Domain Contracts
- `TestFixture`: seed_data, expected_outcomes, cleanup_strategy
- `CoverageReport`: line_pct, branch_pct, function_pct, per_crate_breakdown
- `E2ELogSchema`: correlation_id, stage_checkpoints, outcome_fields
- `ReplayEvidence`: input_snapshot, output_snapshot, diff, verdict

### Service Contracts
- `TestHarness::run_e2e(scenario, fixtures) -> E2EResult`
- `CoverageService::generate_report() -> CoverageReport`
- `LogValidator::validate(e2e_output) -> ValidationResult`
- `ReplayService::replay(transcript) -> ReplayResult`

### Persistence Contracts
- `TestFixtureRepo`: store/retrieve canonical seed datasets
- `CoverageHistoryRepo`: track coverage trends over time
- `E2EArtifactRepo`: store transcripts, traces, snapshots

### CI/Gate Contracts
- `QualityGate::run() -> GateResult` (pass/fail with reasons)
- Gates: build, fmt, clippy, test, coverage, deny, doc, e2e

### Crate Boundaries
- `quotey-core`: Unit tests with in-memory + real-component variants
- `quotey-db`: SQL-backed integration tests (sqlite::memory:)
- `quotey-slack`: E2E event/command fixtures
- `quotey-agent`: Runtime deterministic integration tests
- `quotey-cli`: Command integration tests
- `quotey-server`: Bootstrap/health failure-injection tests

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Real-component tests are flaky | High | Medium | Deterministic fixtures, isolated databases, retry with backoff | QA |
| Coverage gates slow development | Medium | High | Parallel test execution, incremental coverage checks, sensible thresholds | Platform |
| E2E tests too slow for CI | High | Medium | Selective E2E on PRs, full suite on main, parallel execution | Platform |
| Fake-to-real migration breaks existing tests | Medium | Medium | Gradual migration, keep fakes for non-critical paths, dual-run period | QA |
| Log schema changes invalidate E2E | Medium | Low | Versioned schemas, backward compatibility period, migration scripts | QA |

## Guardrail Checklist (Pre-implementation Exit)
- [ ] Scope and non-goals agreed.
- [ ] KPI owner and metric formula explicitly documented.
- [ ] Deterministic constraints copied into implementation task templates.
- [ ] Interface contracts reviewed against existing crate boundaries.
- [ ] Risks and mitigations acknowledged by feature owner.

## Migration Contract
### No Schema Changes
This is a testing infrastructure program - no database schema changes required.

### Configuration Additions
```toml
[testing]
coverage_threshold_line = 80
coverage_threshold_branch = 70
coverage_threshold_function = 75
e2e_enabled = true
e2e_scenarios = ["net-new", "renewal", "discount", "recovery"]
```

### Environment Variables
- `QUOTEY_TEST_COVERAGE_THRESHOLD` - override coverage gate
- `QUOTEY_TEST_E2E_ENABLED` - enable/disable E2E in CI
- `QUOTEY_TEST_FIXTURE_PATH` - custom fixture directory

### Rollback Behavior
- Coverage gates can be temporarily disabled via env var (emergency only)
- E2E tests can be skipped via env var
- Fake-only tests remain available as fallback

## Sub-Tasks Overview
| ID | Title | Slice | Status |
|---|---|---|---|
| bd-3vp2.1 | Baseline audit + quality target definition | A | in_progress |
| bd-3vp2.2 | Coverage instrumentation + threshold enforcement | C | open |
| bd-3vp2.3 | Critical-path migration away from fake-only testing | C | open |
| bd-3vp2.4 | Core deterministic engine unit/integration completeness | C | open |
| bd-3vp2.5 | Data-layer SQLite integration test expansion | C | open |
| bd-3vp2.6 | Adapter-layer integration hardening | C | open |
| bd-3vp2.7 | Deterministic E2E harness + fixture orchestration | C | open |
| bd-3vp2.8 | Full quote lifecycle E2E scenario suite | C | open |
| bd-3vp2.9 | Structured logging standard + automated log validation | D | open |
| bd-3vp2.10 | CI/local gate integration | D | open |
| bd-3vp2.11 | Docs/runbook/handoff | D | open |
| bd-3vp2.12 | Program closeout verification | D | open |

## Research Beads
- bd-1vyz: Current test coverage by crate and module
- bd-wijg: Fake-only vs real-component test gaps
- bd-2q8k: E2E testing infrastructure and patterns
- bd-3kha: Coverage tooling options (tarpaulin vs grcov)
- bd-2u7n: Async test patterns with sqlx and tokio

## Reference Specs
- W1_REL_EXECUTION_QUEUE_SPEC.md: Clean structure, good baseline
- W1_EXP_EXPLAIN_ANY_NUMBER_SPEC.md: Full example with migration contract

## Dependencies
- cargo-llvm-cov (for coverage)
- cargo-tarpaulin (alternative coverage)
- Installed in CI via cargo install
