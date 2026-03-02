# No-Fake QA Policy

**Bead:** quotey-115.2.4 (A4) + quotey-115.2.5 (A5)
**Date:** 2026-03-02
**Status:** Active
**Owner:** quotey engineering team

## Policy Statement

All critical-path code in the quotey workspace MUST be covered by real-component tests that exercise the actual SQLite persistence layer. InMemory/fake implementations are permitted only for non-critical seams or external API boundaries.

## Definitions

- **Real-component test**: A test that uses `connect_with_settings("sqlite::memory:", ...)` + `migrations::run_pending()` to exercise the full SQL stack.
- **Fake-only test**: A test that uses `InMemory*`, `Mock*`, or `Fake*` implementations as the sole coverage for a code path.
- **Critical path**: As defined in `CRITICAL_PATH_MATRIX.md` — pricing, state transitions, security boundaries, audit trails, data persistence.
- **Non-critical seam**: Pure domain logic, serialization, parsing, scoring algorithms, UI block construction.

## Rules

### R-001: Critical paths require real-DB tests

Every critical path listed in the matrix at Tier 1-5 MUST have at least one `#[tokio::test]` that exercises the real SQLite stack end-to-end (write → read → verify).

### R-002: InMemory implementations are supplementary only

`InMemory*` repository implementations MAY be used:
- As supplementary tests alongside real-DB tests
- For testing pure domain logic that doesn't need DB interaction
- For external API boundaries where real integration is impractical (e.g., Slack API, calendar services)

`InMemory*` implementations MUST NOT be:
- The sole test coverage for any critical path
- Used to avoid writing SQL migration tests
- Used to hide schema mismatch bugs

### R-003: New repository methods require real-DB tests

Any new method added to a `*Repository` trait MUST have a corresponding test in the SQL implementation's `#[cfg(test)]` block that exercises real SQLite.

### R-004: No permanent fake-only exceptions

All fake-only exceptions MUST have:
- An owner (agent or human)
- A written rationale
- An expiry date (max 30 days)
- A compensating control description
- A follow-up bead for migration to real tests

## Exception Rubric

### Current Exceptions

| ID | Path | Rationale | Owner | Expiry | Compensating Control | Follow-up Bead |
|----|------|-----------|-------|--------|---------------------|----------------|
| ~~EX-001~~ | ~~`core/audit.rs` InMemoryAuditSink~~ | **RESOLVED** — `SqlAuditEventRepository` + 4 real-DB tests added in quotey-115.3 G-003 | GentleSpring | — | — | — |
| EX-002 | `core/ghost/mod.rs` InMemoryGhostQuoteStore | Ghost quotes are speculative feature; no SQL schema yet | — | 2026-04-01 | Pure logic tests cover scoring | quotey-115.3 (if promoted to production) |
| EX-003 | `core/dna/mod.rs` InMemoryLifecycleStore | DNA analysis is speculative feature; no SQL schema yet | — | 2026-04-01 | Pure logic tests cover analysis | quotey-115.3 (if promoted to production) |
| EX-004 | `core/approvals/mod.rs` InMemoryCalendarAvailabilityClient | External API boundary; no real calendar service | — | Permanent (external) | Calendar logic tested via pure unit tests | N/A |
| EX-005 | Slack API integration | External API; cannot run real Slack in tests | — | Permanent (external) | Socket/event handlers tested as pure unit tests | N/A |
| EX-006 | CRM sync (`server/crm.rs`) | No tests at all; CRM is external API | — | 2026-04-01 | None — **unmitigated risk** | quotey-115.3 G-006 |

### Requesting a New Exception

1. Create a bead with prefix `qa-exception-` describing the path
2. Fill in all fields in the rubric above
3. Get approval from a human reviewer
4. Set expiry to max 30 days
5. Create follow-up bead for migration

## CI Enforcement Plan

### Phase 1: Advisory (immediate)

- `scripts/test_inventory.sh` runs on demand to produce coverage reports
- Coverage baseline committed to `.planning/qa/COVERAGE_BASELINE.md`
- Manual review of critical-path matrix before each release

### Phase 2: Gating (after quotey-115.3 gaps closed)

- Pre-commit hook extension: `cargo test -p quotey-db --lib` must pass
- CI job: Run `scripts/test_inventory.sh --json` and fail if real-DB coverage drops below current baseline
- New repository methods flagged if no corresponding test

### Phase 3: Full enforcement (after quotey-115.4 E2E harness)

- E2E scenario tests run as part of CI
- Coverage regression detection: real-DB test count must not decrease
- Exception registry validated: expired exceptions block merge
