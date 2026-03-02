# No-Fake Critical-Path Matrix

**Bead:** quotey-115.2.3 (A3)
**Date:** 2026-03-02
**Policy:** Every critical path MUST have at least one real-component test exercising the full SQLite stack.

## Critical Path Definitions

A "critical path" is any code path where incorrect behavior causes:
- Financial data corruption (pricing, discounts, tax calculations)
- Data loss (quote state transitions, approval status)
- Security bypass (portal token auth, MCP API auth)
- Audit trail gaps (approval decisions, execution events)

## Matrix

### Tier 1: Financial Integrity (MUST have real-DB tests)

| Path | Module(s) | Current Coverage | Status |
|------|-----------|-----------------|--------|
| Quote pricing | `core/cpq/pricing.rs`, `core/cpq/policy.rs` | 1 pure unit test each | **GAP** — needs real-DB pricing round-trip |
| Discount calculation | `core/cpq/pricing.rs` | Tested via MCP `quote_price` (real DB) | **OK** |
| Tax computation | `server/portal.rs` | Portal tests with real DB | **OK** |
| Quote line subtotals | `server/portal.rs` | Portal tests with real DB | **OK** |
| Constraint enforcement | `core/cpq/constraints.rs` | 1 pure unit test | **GAP** — needs real-DB constraint violation test |

### Tier 2: State Integrity (MUST have real-DB tests)

| Path | Module(s) | Current Coverage | Status |
|------|-----------|-----------------|--------|
| Quote lifecycle (create→draft→sent) | `core/domain/quote.rs`, `db/repositories/quote.rs` | Real DB round-trip tests | **OK** |
| Approval lifecycle (request→approve/reject) | `db/repositories/approval.rs` | Real DB round-trip tests | **OK** |
| Execution queue state machine | `core/execution_engine.rs`, `db/repositories/execution_queue.rs` | Both real DB and pure unit | **OK** |
| Dialogue session transitions | `core/domain/dialogue.rs`, `db/repositories/dialogue.rs` | Both real DB and pure unit | **OK** |

### Tier 3: Security Boundaries (MUST have real-component tests)

| Path | Module(s) | Current Coverage | Status |
|------|-----------|-----------------|--------|
| Portal token auth | `server/portal.rs` | Real DB tests (resolve_quote_by_token, etc.) | **OK** |
| Portal link creation/revocation | `server/portal.rs` | Real DB tests | **OK** |
| MCP API key auth | `mcp/auth.rs` | Integration tests (no DB needed) | **OK** |
| MCP rate limiting | `mcp/auth.rs` | Integration tests | **OK** |

### Tier 4: Audit Trail (MUST have real-component tests)

| Path | Module(s) | Current Coverage | Status |
|------|-----------|-----------------|--------|
| Approval audit events | `server/portal.rs` | Real DB tests | **OK** |
| Execution transition log | `db/repositories/execution_queue.rs` | Real DB tests | **OK** |
| Quote change audit | `core/audit.rs` | InMemoryAuditSink only | **GAP** — needs real sink test |
| Pricing snapshot persistence | `db/repositories/pricing_snapshot.rs` | Real DB tests | **OK** |

### Tier 5: Data Persistence (MUST have real-DB tests)

| Path | Module(s) | Current Coverage | Status |
|------|-----------|-----------------|--------|
| Product catalog CRUD | `db/repositories/product.rs` | Via MCP tool tests (real DB) | **OK** |
| Product FTS search | `db/repositories/product.rs` | **Known broken** (FTS5 column name mismatch) | **GAP** — FTS5 schema fix needed |
| Quote persistence | `db/repositories/quote.rs` | Real DB round-trip tests | **OK** |
| Customer data | `db/repositories/customer.rs` | No tests found | **GAP** |
| Suggestion feedback | `db/repositories/suggestion_feedback.rs` | Real DB tests | **OK** |

### Tier 6: External Integration Boundaries (real-component where feasible)

| Path | Module(s) | Current Coverage | Status |
|------|-----------|-----------------|--------|
| CRM sync | `server/crm.rs` | **No tests** (empty test module) | **GAP** |
| Slack Socket Mode | `slack/socket.rs` | Pure unit tests (reconnect logic) | **ACCEPTABLE** — external API |
| Slack events | `slack/events.rs` | Pure unit tests (dispatch routing) | **ACCEPTABLE** — external API |
| wkhtmltopdf integration | `server/pdf.rs` | Tested with `None` path | **ACCEPTABLE** — external binary |

## Gap Summary

| Gap ID | Path | Priority | Effort | Blocked By |
|--------|------|----------|--------|------------|
| G-001 | Pricing engine real-DB round-trip | P0 | Medium | None |
| G-002 | Constraint engine real-DB test | P1 | Low | None |
| G-003 | Audit sink real persistence test | P1 | Low | None |
| G-004 | FTS5 schema fix + search test | P1 | Medium | Schema migration |
| G-005 | Customer repository tests | P2 | Low | None |
| G-006 | CRM sync tests | P2 | Medium | External API contract definition |

## Required Actions (quotey-115.3)

1. **G-001**: Add integration test that creates quote → prices it → verifies pricing snapshot persisted correctly
2. **G-002**: Add integration test that creates products with constraints → attempts invalid configuration → verifies rejection
3. **G-003**: Add SQL-backed audit sink test that writes events and reads them back
4. **G-004**: Fix FTS5 `product_id` vs `id` column mismatch in migration 0019, add product search test
5. **G-005**: Add basic customer repository CRUD tests
6. **G-006**: Add CRM sync unit tests with mock HTTP client
