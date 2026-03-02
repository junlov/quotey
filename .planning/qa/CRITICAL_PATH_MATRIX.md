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
| Quote pricing | `core/cpq/pricing.rs`, `core/cpq/policy.rs` | 4 real-DB tests (G-001) | **CLOSED** |
| Discount calculation | `core/cpq/pricing.rs` | Tested via MCP `quote_price` (real DB) | **OK** |
| Tax computation | `server/portal.rs` | Portal tests with real DB | **OK** |
| Quote line subtotals | `server/portal.rs` | Portal tests with real DB | **OK** |
| Constraint enforcement | `core/cpq/constraints.rs` | 5 real-DB tests (G-002) | **CLOSED** |

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
| Quote change audit | `core/audit.rs`, `db/repositories/audit.rs` | 4 real-DB tests (G-003) | **CLOSED** |
| Pricing snapshot persistence | `db/repositories/pricing_snapshot.rs` | Real DB tests | **OK** |

### Tier 5: Data Persistence (MUST have real-DB tests)

| Path | Module(s) | Current Coverage | Status |
|------|-----------|-----------------|--------|
| Product catalog CRUD | `db/repositories/product.rs` | Via MCP tool tests (real DB) | **OK** |
| Product FTS search | `db/repositories/product.rs` | 3 real-DB tests + migration 0027 fix (G-004) | **CLOSED** |
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

| Gap ID | Path | Priority | Effort | Status |
|--------|------|----------|--------|--------|
| G-001 | Pricing engine real-DB round-trip | P0 | Medium | **CLOSED** — 4 tests in `critical_path_coverage.rs` |
| G-002 | Constraint engine real-DB test | P1 | Low | **CLOSED** — 5 tests in `critical_path_coverage.rs` |
| G-003 | Audit sink real persistence test | P1 | Low | **CLOSED** — SqlAuditEventRepository + 4 tests |
| G-004 | FTS5 schema fix + search test | P1 | Medium | **CLOSED** — migration 0027 + query quoting fix + 3 tests |
| G-005 | Customer repository tests | P2 | Low | **BLOCKED** — no customer table in schema (see EX-002/EX-003 in QA_POLICY.md) |
| G-006 | CRM sync tests | P2 | Medium | **DEFERRED** — external API boundary, no trait abstraction (see EX-006 in QA_POLICY.md) |

## Completed Actions (quotey-115.3)

All P0/P1 gaps closed. Remaining P2 gaps documented with exceptions.

### Deliverables

- `crates/db/src/repositories/audit.rs` — SqlAuditEventRepository (save, find_by_quote_id, find_by_type, count)
- `crates/db/tests/critical_path_coverage.rs` — 16 integration tests (4× G-001, 5× G-002, 4× G-003, 3× G-004)
- `crates/db/src/repositories/product.rs` — FTS search query quoting fix (prevents FTS5 operator injection)
- `migrations/0027_fix_product_fts.{up,down}.sql` — standalone FTS5 table (fixes content-table column mismatch)

### Remaining Actions (future beads)

1. **G-005**: Create customer table migration + implement SqlCustomerRepository (if customer feature promoted)
2. **G-006**: Extract CRM HTTP client trait + add unit tests with mock client
