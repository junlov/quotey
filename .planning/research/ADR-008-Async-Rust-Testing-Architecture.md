# ADR-008: Async Rust Testing Architecture

**Status:** Accepted  
**Date:** 2026-02-23  
**Related Bead:** `bd-256v.8`

## Context

Quotey is an async Rust workspace with deterministic core logic, SQLite persistence, Slack adapter surfaces, and external integration boundaries.

Current test coverage exists but lacks a unified architecture for:

1. async runtime determinism,
2. DB-isolated integration tests,
3. adapter-contract mocking strategy,
4. CI profile split between fast and deep suites.

## Decision

Adopt a layered async-testing architecture:

1. **Pure unit tests** (`#[test]`) for deterministic domain logic.
2. **Async component tests** (`#[tokio::test]`) for runtime-aware behavior.
3. **DB integration tests** (`#[sqlx::test]`) for migration-backed isolated database tests.
4. **Trait mocks** (`mockall`) and **HTTP contract mocks** (`wiremock`) for adapter/service boundaries.
5. **Nextest profile-driven execution** for PR/main/nightly pipelines.
6. **Selective testcontainers usage** only for deeper environment integration suites.
7. **Property-based tests** (`proptest`) for core invariants and threshold logic.

## Rationale

1. Keeps deterministic logic fast and heavily validated.
2. Enables reproducible async tests without wall-clock sleeps.
3. Improves DB test realism while preserving test isolation.
4. Validates adapter contracts without external dependency flakiness.
5. Supports predictable CI throughput with explicit profile boundaries.

## Consequences

### Positive

1. Better test determinism and lower flake rates.
2. Clear separation of fast local feedback vs deep integration confidence.
3. Stronger confidence in reliability-sensitive paths (retry/idempotency).
4. Easier onboarding due consistent testing conventions across crates.

### Negative

1. More tooling and conventions to maintain.
2. Additional setup work for fixtures/profile configuration.
3. Potential overuse of mocks if test ownership boundaries are not enforced.

## Guardrails

1. No real-time sleeps in deterministic async tests when paused-time testing is viable.
2. No adapter implementation merged without contract tests (mocked or wiremocked).
3. No migration-layer changes merged without DB integration coverage.
4. No flaky tests masked by broad retries; retries must be explicit and limited.

## Verification Plan

1. Introduce nextest profiles and validate CI run times.
2. Convert selected DB tests to `#[sqlx::test]` and confirm isolation.
3. Add one wiremock-based adapter test suite and one mockall-based orchestration suite.
4. Add proptest suite for quote/approval invariant checks.
5. Track test failures by class (deterministic bug vs flake) over first rollout period.

## Revisit Triggers

1. Significant growth in CI time or flaky-test rate.
2. Multi-service integration scope expansion.
3. Shift from SQLite-only test strategy to multi-database support.
4. Major runtime model changes in async orchestration.

## References

1. https://docs.rs/tokio/latest/tokio/attr.test.html
2. https://docs.rs/sqlx/latest/sqlx/attr.test.html
3. https://nexte.st/docs/
4. https://docs.rs/mockall/latest/mockall/
5. https://docs.rs/wiremock/latest/wiremock/
