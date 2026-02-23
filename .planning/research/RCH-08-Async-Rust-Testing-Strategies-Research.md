# RCH-08: Async Rust Testing Strategies Research

**Bead:** `bd-256v.8`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** IvoryBear (Codex)

## 1. Executive Summary

Quotey should adopt a layered async-testing strategy built around:

1. `#[test]` for pure deterministic domain logic in `crates/core`,
2. `#[tokio::test]` for async service behavior and runtime semantics,
3. `#[sqlx::test]` for isolated DB integration and migration-backed tests,
4. `mockall` for trait-level async mocks,
5. `wiremock` for HTTP adapter contract tests,
6. `cargo-nextest` profiles for fast local feedback and reliable CI,
7. selective `testcontainers` usage for nightly/extended integration confidence.

This strategy balances execution speed, deterministic reproducibility, and production realism.

## 2. Current-State Findings (Repo-Specific)

From current workspace inspection:

1. Workspace already uses Tokio and SQLx (`tokio = 1.43`, `sqlx = 0.8.6`).
2. Existing tests are present in:
   - `crates/core/src/domain/quote.rs` (`#[test]`),
   - `crates/core/src/config.rs` (`#[test]`),
   - `crates/db/src/migrations.rs` (`#[tokio::test]`),
   - `crates/db/src/repositories/memory.rs` (`#[tokio::test]`),
   - `crates/server/src/bootstrap.rs` (`#[tokio::test]`).
3. `cargo-nextest` aliases already exist in `.cargo/config.toml` (`test-all`).
4. No unified test architecture document yet.
5. No explicit strategy yet for:
   - deterministic async-time tests,
   - mock layering,
   - fixture governance,
   - CI flake controls,
   - property-based tests for domain invariants.

## 3. Testing Architecture Recommendation

### 3.1 Layer 0: Pure Domain Unit Tests (`crates/core`)

Use `#[test]` for:

1. invariant checks,
2. parser/normalizer behavior,
3. state-machine transition legality,
4. deterministic pricing and policy calculations where async is unnecessary.

Goal:

- keep these fast and side-effect free so they can run on every edit cycle.

### 3.2 Layer 1: Async Component Tests (`#[tokio::test]`)

Use `#[tokio::test]` for service/repository abstractions requiring async behavior.

Recommended modes:

1. default current-thread runtime for deterministic behavior,
2. `flavor = "multi_thread"` only where true concurrency is part of behavior under test,
3. `start_paused = true` + `tokio::time::advance` for backoff/timer logic.

Why:

- Tokio docs explicitly support per-test runtimes and paused time control, which is ideal for retry/backoff logic without slow wall-clock sleeps.

### 3.3 Layer 2: Database Integration (`#[sqlx::test]`)

Adopt `#[sqlx::test]` for DB tests that need isolation + migrations.

Recommended use:

1. migration tests,
2. repository SQL behavior tests,
3. transaction rollback/consistency tests,
4. fixture-driven read/write behavior tests.

Key benefits from SQLx docs:

1. isolated test DB per test function,
2. automatic migration application,
3. fixture scripts support in declared order.

### 3.4 Layer 3: Adapter Contract Tests

#### HTTP/CRM adapters

Use `wiremock` to validate:

1. request shape and headers,
2. retry behavior for timeout/5xx/rate limit,
3. mapping error classification.

#### Trait boundary mocks

Use `mockall` for:

1. trait-driven mocks,
2. expectation ordering (`Sequence`) where orchestration order matters,
3. async-trait mock compatibility.

### 3.5 Layer 4: Environment Integration (Selective)

Use `testcontainers` for nightly or pre-release test suites requiring real service behavior (e.g., real Postgres/MySQL parity checks, external sidecar integration contracts).

Rule:

- avoid running these in the default local fast loop unless explicitly requested.

### 3.6 Layer 5: Property-Based Invariant Testing

Use `proptest` for:

1. state transition invariants,
2. pricing monotonicity and boundary rules,
3. policy threshold edge conditions.

Start with targeted, high-value invariants rather than broad randomization everywhere.

## 4. Mocking Strategy (Concrete Guidance)

### 4.1 Preferred Mock Types

1. **Trait mocks** (`mockall`): for service boundaries (`QuoteRepository`, `CrmAdapter`, policy evaluators).
2. **In-memory doubles**: for repository behavior where stateful semantics are needed.
3. **HTTP mocks** (`wiremock`): for adapter transport contracts and retry logic.

### 4.2 Mock Design Rules

1. Mocks should validate behavior contract, not implementation details.
2. Use explicit expectation counts and argument matchers for critical paths.
3. Use `Sequence` only where ordering is part of correctness.
4. Keep one mock responsibility per test to prevent brittle mega-tests.

## 5. Integration Test Setup Guide

### 5.1 Database

1. Prefer `#[sqlx::test]` over manually constructed in-memory DB tests when migration fidelity matters.
2. Use fixture scripts for stable seed data (`fixtures("users", "quotes", ...)`).
3. Keep fixture sets composable and small.

### 5.2 Runtime/Timing

1. For retry/backoff tests:
   - `#[tokio::test(start_paused = true)]`,
   - use `tokio::time::advance`.
2. Never use real `sleep` for deterministic unit/component tests.

### 5.3 Adapter Integration

1. Use `wiremock::MockServer::start().await` on random local ports.
2. Validate request body fields and headers explicitly.
3. Simulate sequential failures/success for retry verification.

## 6. Fixture Patterns

Recommended fixture layout:

1. SQL fixtures: `crates/db/tests/fixtures/*.sql`
2. JSON API fixtures: `crates/slack/tests/fixtures/*.json`, `crates/agent/tests/fixtures/*.json`
3. golden-output fixtures: `tests/golden/*.json` for normalized command/result snapshots.

Fixture governance:

1. Keep fixtures minimal and scenario-focused.
2. One fixture per business scenario, not per test method.
3. Include schema version comments for migration safety.

## 7. Nextest and CI Configuration

### 7.1 Local Developer Loop

1. `cargo test -p core` for fast invariants.
2. `cargo nextest run --workspace --all-targets` for comprehensive local checks.

### 7.2 CI Profiles (Recommended)

1. **PR profile (fast)**:
   - workspace tests except heavy integration,
   - fail fast enabled,
   - no retries by default.
2. **Main profile (balanced)**:
   - includes DB integration and adapter contract tests,
   - retries for explicitly marked flaky tests only.
3. **Nightly profile (deep)**:
   - includes testcontainers-based suites,
   - broader property-based test counts.

### 7.3 Nextest Controls to Use

1. `test-threads` for concurrency cap.
2. `threads-required` for heavy tests.
3. retry backoff + jitter in config for known flaky external tests.
4. `fail-fast` policy tuned per profile.

## 8. Coverage Approach

Coverage recommendation:

1. Use `cargo-llvm-cov` in CI nightly and optional main-branch gate.
2. Track coverage by crate:
   - `core` target high coverage (invariants),
   - adapters target risk-based scenario coverage.
3. Avoid raw percentage-only gating; pair with:
   - mutation-sensitive critical-path tests,
   - invariant coverage checklists.

Minimal target guidance:

1. `core`: high line + branch coverage (risk-critical deterministic logic).
2. `db/slack/agent`: scenario coverage for error/retry paths.

## 9. Suggested Initial Rollout (Incremental)

1. Introduce nextest profile file (`.config/nextest.toml`) with PR/main defaults.
2. Convert migration/repository DB tests to `#[sqlx::test]` where beneficial.
3. Add `wiremock` contract tests for CRM adapter once implemented.
4. Add first `proptest` suite for quote state transitions and policy thresholds.
5. Add nightly testcontainers job after adapter boundaries stabilize.

## 10. Deliverable Coverage vs Bead Requirements

`bd-256v.8` requested:

1. **Testing strategy document**: Sections 3, 7, 9.
2. **Mock implementation examples**: Sections 4, 5.
3. **Integration test setup guide**: Section 5.
4. **Test fixture patterns**: Section 6.
5. **CI/CD test configuration**: Section 7.
6. **Code coverage approach**: Section 8.
7. **ADR**: see `.planning/research/ADR-008-Async-Rust-Testing-Architecture.md`.

## 11. References (Primary Sources)

1. Tokio `#[tokio::test]`: https://docs.rs/tokio/latest/tokio/attr.test.html
2. Tokio testing topic: https://tokio.rs/tokio/topics/testing
3. Tokio paused time: https://docs.rs/tokio/latest/tokio/time/fn.pause.html
4. SQLx `#[sqlx::test]`: https://docs.rs/sqlx/latest/sqlx/attr.test.html
5. cargo-nextest retries: https://nexte.st/docs/features/retries/
6. cargo-nextest configuration reference: https://nexte.st/docs/configuration/reference/
7. cargo-nextest heavy tests: https://nexte.st/docs/configuration/threads-required/
8. mockall docs: https://docs.rs/mockall/latest/mockall/
9. wiremock docs: https://docs.rs/wiremock/latest/wiremock/
10. testcontainers docs: https://docs.rs/testcontainers/latest/testcontainers/
11. proptest docs: https://docs.rs/proptest/latest/proptest/
