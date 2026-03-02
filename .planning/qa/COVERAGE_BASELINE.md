# Test Coverage Baseline Report

**Generated:** 2026-03-02
**Bead:** quotey-115.2 (Track A)
**Scanner:** `scripts/test_inventory.sh`
**Reproducible command:** `bash scripts/test_inventory.sh --markdown`

## Executive Summary

The quotey workspace contains **~492 test functions** across **70 files** in 7 crates. Tests fall into three categories:

| Category | Count | % | Description |
|----------|-------|---|-------------|
| Real-DB (SQLite + migrations) | ~115 | 23% | Exercise full SQL stack via `sqlite::memory:` with migration runner |
| InMemory/Fake implementations | ~50 | 10% | Use hand-rolled `InMemory*` structs as repository stand-ins |
| Pure unit tests | ~327 | 67% | Test domain logic, parsing, scoring with no DB dependency |

## Coverage by Crate

| Crate | Total Tests | Real DB | InMemory | Pure Unit | Key Patterns |
|-------|-------------|---------|----------|-----------|--------------|
| **core** | ~228 | 0 | ~50 | ~178 | Pure domain logic; uses `InMemory*` stores for integration-style tests |
| **db** | ~62 | ~48 | 6 | 8 | SQL repo round-trips; InMemory self-tests; seed contract |
| **mcp** | ~51 | ~35 | 0 | ~16 | Tool contract tests (server.rs:35); auth tests; no InMemory usage |
| **slack** | ~79 | 0 | ~10 | ~69 | Block builders, command parsing, socket reconnect logic |
| **agent** | 14 | 0 | 0 | 14 | Intent extraction, guardrails, audit emission |
| **cli** | ~18 | ~8 | 0 | ~10 | Command runtime with env-injected DB; config tests |
| **server** | ~39 | ~37 | 0 | 2 | Portal endpoints, health, PDF, bootstrap (all real DB) |

## InMemory/Fake Type Inventory

### Production-code InMemory types (crates/db/src/repositories/memory.rs)

| Type | Trait | Backing Store | Used In |
|------|-------|---------------|---------|
| `InMemoryQuoteRepository` | `QuoteRepository` | `RwLock<HashMap>` | memory.rs self-tests |
| `InMemoryProductRepository` | `ProductRepository` | `RwLock<HashMap>` | memory.rs self-tests |
| `InMemoryApprovalRepository` | `ApprovalRepository` | `RwLock<HashMap>` | memory.rs self-tests |
| `InMemoryExecutionQueueRepository` | `ExecutionQueueRepository` | `RwLock<HashMap>` | memory.rs self-tests |
| `InMemoryIdempotencyRepository` | `IdempotencyRepository` | `RwLock<HashMap>` | memory.rs self-tests |
| `InMemoryPolicyOptimizerRepository` | `PolicyOptimizerRepository` | Multiple `RwLock<HashMap>` | memory.rs self-tests |
| `InMemorySuggestionFeedbackRepository` | `SuggestionFeedbackRepository` | `RwLock<HashMap>` | memory.rs self-tests |

### Core-crate InMemory types (test-only/internal)

| Type | Module | Purpose |
|------|--------|---------|
| `InMemoryAuditSink` | `core/src/audit.rs` | Captures audit events for flow tests |
| `InMemoryGhostQuoteStore` | `core/src/ghost/mod.rs` | Ghost quote persistence for unit tests |
| `InMemoryCustomerHistoryProvider` | `core/src/ghost/mod.rs` | Customer history for ghost quote tests |
| `InMemoryLifecycleStore` | `core/src/dna/mod.rs` | Lifecycle data for DNA analysis tests |
| `InMemoryCalendarAvailabilityClient` | `core/src/approvals/mod.rs` | Calendar availability for approval routing |
| `InMemoryPrecedentAuditSink` | `core/src/cpq/precedent.rs` | Precedent audit for CPQ tests |

### Notable: No Mock Frameworks

The codebase uses **zero external mocking libraries** (no `mockall`, `mock-it`, etc.). All test doubles are hand-rolled `InMemory*` structs with `async_trait` implementations.

## Test Infrastructure Patterns

### `test_db()` Pattern (Two Variants)

1. **Full schema** (`server.rs`, `portal.rs`, `db/repositories/*.rs`):
   ```rust
   let pool = connect_with_settings("sqlite::memory:", 1, 30).await;
   migrations::run_pending(&pool).await;
   ```

2. **Bare connection** (`mcp/tests/integration_tests.rs`):
   ```rust
   let pool = connect("sqlite::memory:").await;
   // No migrations — only tests auth layer, not DB queries
   ```

### Integration Test Files

| File | Crate | Tests | Real DB? |
|------|-------|-------|----------|
| `crates/mcp/tests/integration_tests.rs` | mcp | 9 | Bare (no migrations) |
| `crates/cli/tests/commands_runtime.rs` | cli | 8 | Full (via `migrate::run()`) |
| `crates/db/tests/seed_contract.rs` | db | 8 | 2 with DB, 6 pure JSON |

## Gaps and Risks

### Critical Paths Without Real-DB Coverage

1. **CRM sync** (`server/src/crm.rs`): Empty test module — no tests at all
2. **Slack event handlers** (`slack/src/events.rs`, `slack/src/socket.rs`): Pure unit tests only; no DB interaction tested
3. **Agent conversation flow** (`agent/src/conversation.rs`): No DB tests, only parsing/extraction

### InMemory Types in Critical Paths

The following InMemory types are used in critical-path test code and should be evaluated for migration to real-DB tests:

| Type | Used By | Critical Path? | Migration Priority |
|------|---------|----------------|-------------------|
| `InMemoryAuditSink` | Flow engine tests | Yes — audit integrity | Medium |
| `InMemoryCalendarAvailabilityClient` | Approval routing | Yes — approval routing | Low (external API mock) |
| `InMemoryPolicyOptimizerRepository` | Policy optimizer | Yes — pricing decisions | High |

### Coverage Blind Spots

- **No E2E scenario tests** that exercise a full quote lifecycle (create → price → approve → send)
- **No cross-crate integration tests** that wire MCP → core → db → server
- **PDF generation** tested with forced `None` wkhtmltopdf path; no real PDF output validation
