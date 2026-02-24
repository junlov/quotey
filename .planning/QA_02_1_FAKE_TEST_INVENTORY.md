# QA-02.1: Fake/In-Memory Test Inventory

**Bead:** bd-3vp2.3.1  
**Generated:** 2026-02-24  
**Status:** Complete

## Executive Summary

All 89 tests in the codebase use fake/in-memory implementations. **Zero tests use `#[sqlx::test]` or real SQLite-backed integration tests.**

---

## Inventory by Crate

### crates/core (Domain Logic)

| File | Fake Types | Test Count | Risk Level |
|------|-----------|------------|------------|
| `explanation/mod.rs` | `InMemoryPricingProvider`, `InMemoryPolicyProvider` | 9 | HIGH |
| `policy/optimizer.rs` | `InMemoryPolicyLifecycleEngine` | 27 | HIGH |
| `autopsy/mod.rs` | (fixture-based) | 24 | MEDIUM |
| `cpq/simulator.rs` | (unit tests with fixtures) | 12 | MEDIUM |
| `cpq/precedent.rs` | `InMemoryPrecedentAuditSink` | 6 | HIGH |
| `approvals/mod.rs` | `InMemoryCalendarAvailabilityClient` | 6 | HIGH |
| `flows/engine.rs` | `InMemoryAuditSink`, `DeterministicCpqRuntime` | 3 | MEDIUM |
| `cpq/mod.rs` | `TestConstraintEngine`, `TestPricingEngine`, `TestPolicyEngine` | 2 | MEDIUM |
| `audit.rs` | `InMemoryAuditSink` | 1 | LOW |
| `policy/mod.rs` | (template-based) | 5 | LOW |
| `config.rs` | (pure functions) | 5 | LOW |
| `domain/precedent.rs` | (pure functions) | 4 | LOW |
| `domain/autopsy.rs` | (pure functions) | 6 | LOW |

**Core Total:** 114 tests, all fake-only

### crates/db (Data Layer)

| File | Fake Types | Test Count | Risk Level |
|------|-----------|------------|------------|
| `repositories/memory.rs` | `InMemoryQuoteRepository`, `InMemoryProductRepository`, `InMemoryApprovalRepository`, `InMemoryExecutionQueueRepository`, `InMemoryIdempotencyRepository`, `InMemoryPolicyOptimizerRepository` | 6 | CRITICAL |
| `repositories/simulation.rs` | (record round-trip) | 4 | HIGH |
| `repositories/precedent.rs` | SQL-backed | 4 | LOW |
| `repositories/pricing_snapshot.rs` | SQL-backed | 4 | LOW |
| `repositories/optimizer.rs` | SQL-backed | 4 | LOW |
| `repositories/explanation.rs` | SQL-backed | 4 | LOW |
| `repositories/execution_queue.rs` | SQL-backed | 4 | LOW |
| `fixtures.rs` | E2E seed dataset | 1 | LOW |

**DB Total:** 31 tests
- 10 tests use in-memory fakes (CRITICAL/HIGH risk)
- 21 tests are SQL-backed but use record-roundtrip pattern (not `#[sqlx::test]`)

### crates/agent (Agent Runtime)

| File | Fake Types | Test Count | Risk Level |
|------|-----------|------------|------------|
| `conversation.rs` | `IntentExtractor`, `ConstraintMapper` | 9 | MEDIUM |
| `runtime.rs` | `AgentRuntime` with `GuardrailPolicy` | 3 | MEDIUM |
| `guardrails.rs` | `GuardrailPolicy` | 3 | LOW |

**Agent Total:** 15 tests, all fake-only

### crates/slack (Slack Integration)

| File | Fake Types | Test Count | Risk Level |
|------|-----------|------------|------------|
| `blocks.rs` | (pure message builders) | 28 | LOW |
| `commands.rs` | `NoopQuoteCommandService`, `RecordingService` | 11 | MEDIUM |
| `events.rs` | `default_dispatcher()` | 4 | MEDIUM |
| `socket.rs` | `ScriptedTransport` | 4 | HIGH |

**Slack Total:** 47 tests
- 15 use fake services (MEDIUM/HIGH risk)
- 32 are pure function tests (LOW risk)

---

## Fake Implementation Patterns

### 1. InMemory* Repository Pattern
All repository traits have corresponding `InMemory*` implementations:
- `InMemoryQuoteRepository`
- `InMemoryProductRepository`
- `InMemoryApprovalRepository`
- `InMemoryExecutionQueueRepository`
- `InMemoryIdempotencyRepository`
- `InMemoryPolicyOptimizerRepository`

**Location:** `crates/db/src/repositories/memory.rs`

### 2. Fake Provider Pattern
Domain services use fake providers for testing:
- `InMemoryPricingProvider` (explanation)
- `InMemoryPolicyProvider` (explanation)
- `InMemoryCalendarAvailabilityClient` (approvals)
- `InMemoryPrecedentAuditSink` (precedent)
- `InMemoryPolicyLifecycleEngine` (optimizer)
- `InMemoryAuditSink` (audit)

### 3. Deterministic* Engine Pattern
CPQ module uses deterministic engines:
- `DeterministicCpqRuntime`
- `DeterministicConstraintEngine`
- `DeterministicPricingEngine`
- `DeterministicPolicyEngine`

### 4. Scripted/Mock Transport Pattern
Slack socket tests use `ScriptedTransport` to mock WebSocket behavior.

### 5. Test* Engine Pattern
Unit test engines:
- `TestConstraintEngine`
- `TestPricingEngine`
- `TestPolicyEngine`

### 6. Noop/Recording Service Pattern
Slack commands use:
- `NoopQuoteCommandService`
- `RecordingService`

---

## Risk Assessment

### CRITICAL Risk (No SQL Coverage)
- **Data layer repositories** (`crates/db/src/repositories/memory.rs`)
- These are the seams between fake and real - most critical to verify

### HIGH Risk (Financial/Policy Decisions)
- Policy optimizer (27 tests)
- Explanation engine (9 tests)
- Precedent engine (6 tests)
- Calendar availability (6 tests)
- Simulation repository (4 tests)
- Slack socket transport (4 tests)

### MEDIUM Risk (Business Logic)
- Autopsy engine (24 tests)
- Simulator (12 tests)
- Flow engine (3 tests)
- Agent runtime (6 tests)
- Slack commands (11 tests)
- Slack events (4 tests)

### LOW Risk (Pure Functions/Builders)
- Config (5 tests)
- Domain primitives (10 tests)
- Policy templates (5 tests)
- Audit (1 test)
- Slack blocks (28 tests)
- SQL-backed repositories (21 tests)

---

## Gaps Identified

1. **No `#[sqlx::test]` usage** - Zero integration tests with real SQLite
2. **No contract tests** - InMemory vs SQL behavior not verified
3. **Repository seam untested** - All repository tests use fakes
4. **Policy optimizer** - 27 tests but no real persistence validation
5. **Explanation engine** - 9 tests with fake providers only

---

## Recommendations

### Immediate (P1)
1. Add `#[sqlx::test]` integration tests for SQL repository implementations
2. Create contract tests verifying InMemory and SQL implementations behave identically
3. Add at least one E2E test using real database for each critical flow

### Short-term (P2)
1. Migrate high-risk fake tests to SQL-backed tests
2. Add failure-path integration tests for DB errors
3. Add concurrency/idempotency tests with real SQLite

### Long-term (P3)
1. Establish policy: new repository tests must include SQL variant
2. CI gate: fail if critical path lacks SQL-backed test
3. Gradual migration of medium-risk tests

---

## Files Requiring SQL-Backed Tests

### Critical (Must Have)
- [ ] `crates/db/src/repositories/memory.rs` - Add SQL parity tests
- [ ] `crates/db/src/repositories/simulation.rs` - Add `#[sqlx::test]` variants

### High Priority
- [ ] `crates/core/src/policy/optimizer.rs` - Add integration tests
- [ ] `crates/core/src/explanation/mod.rs` - Add SQL-backed provider tests
- [ ] `crates/core/src/cpq/precedent.rs` - Add SQL audit sink tests
- [ ] `crates/core/src/approvals/mod.rs` - Add calendar integration tests
- [ ] `crates/slack/src/socket.rs` - Add integration tests with real transport

### Medium Priority
- [ ] `crates/core/src/autopsy/mod.rs` - Add SQL-backed autopsy tests
- [ ] `crates/core/src/cpq/simulator.rs` - Add SQL simulation tests
- [ ] `crates/core/src/flows/engine.rs` - Add SQL flow state tests
- [ ] `crates/agent/src/conversation.rs` - Add integration tests
- [ ] `crates/agent/src/runtime.rs` - Add integration tests
- [ ] `crates/slack/src/commands.rs` - Add command integration tests
- [ ] `crates/slack/src/events.rs` - Add event integration tests
