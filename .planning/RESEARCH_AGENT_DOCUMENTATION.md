# Quotey Research Agent Documentation

**Agent:** Research Agent  
**Session Started:** 2026-02-23  
**Project:** quotey - Rust CPQ Agent for Slack

---

## Executive Summary

Quotey is a local-first, Rust-based CPQ (Configure, Price, Quote) agent that operates within Slack. It replaces traditional rigid CPQ interfaces with natural language interaction while maintaining deterministic, auditable pricing and configuration logic.

### Key Value Proposition

Sales reps can create accurate, policy-compliant, fully-audited quotes through natural conversation in Slack — without touching a CPQ UI, without waiting days for approvals, and without the 6-18 month implementation cycle of traditional CPQ.

---

## Architecture Overview

### 6-Component Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    SLACK BOT INTERFACE                          │
│  Socket Mode listener · Slash commands · Message events         │
│  Interactive components (buttons/modals) · File uploads         │
├─────────────────────────────────────────────────────────────────┤
│                    AGENT RUNTIME                                │
│  Intent extraction · Slot filling · Action selection            │
│  Guardrails · Tool permissions · Conversation management        │
├──────────────────┬──────────────────┬───────────────────────────┤
│  DETERMINISTIC   │   CPQ CORE       │   TOOL ADAPTERS           │
│  FLOW ENGINE     │                  │                           │
│                  │  Product catalog  │  slack.* (post/update/    │
│  State machine   │  Constraint       │          upload)          │
│  Required fields │    engine         │  crm.* (sync/read/write) │
│  Allowed         │  Pricing engine   │  doc.* (render/attach)   │
│    transitions   │  Discount         │  composio.* (REST)       │
│  "What happens   │    policies       │  catalog.* (bootstrap/   │
│   next"          │  Approval         │           ingest)         │
│                  │    thresholds     │  intelligence.*           │
│                  │                  │    (parse/extract)        │
├──────────────────┴──────────────────┴───────────────────────────┤
│                    SQLITE DATA STORE                            │
│  Products · Price books · Deals · Quotes · Approvals            │
│  Configuration rules · Pricing policies · Audit log             │
│  Slack thread mapping · CRM sync state                          │
└─────────────────────────────────────────────────────────────────┘
```

### Workspace Crate Structure

| Crate | Purpose | Dependencies |
|-------|---------|--------------|
| `quotey-core` | Domain primitives, CPQ engines, flows, audit | None (pure domain) |
| `quotey-db` | SQLite connection, migrations, repositories | `core` |
| `quotey-slack` | Slack Socket Mode, Block Kit, commands | `core` |
| `quotey-agent` | Runtime orchestration, LLM integration | `core` |
| `quotey-cli` | Operator commands (start, migrate, seed, doctor) | `core`, `db` |
| `quotey-server` | Bootstrap/wiring, process startup | All above |

---

## Current Implementation Status

### ✅ Completed Components

1. **Core Domain Models** (`crates/core/src/domain/`)
   - `quote.rs`: Quote, QuoteLine, QuoteId, QuoteStatus
   - `product.rs`: Product, ProductId
   - `approval.rs`: ApprovalRequest, ApprovalId, ApprovalStatus
   - `execution.rs`: ExecutionTask, IdempotencyRecord, ExecutionTransitionEvent
   - `customer.rs`: Account, Contact, Deal

2. **CPQ Core Engines** (`crates/core/src/cpq/`)
   - `DeterministicCpqRuntime`: Orchestrates constraint, pricing, policy engines
   - `ConstraintEngine`: Configuration validation trait + implementation
   - `PricingEngine`: Pricing calculation with trace
   - `PolicyEngine`: Policy evaluation for approvals

3. **Flow Engine** (`crates/core/src/flows/`)
   - `FlowEngine`: State machine for quote lifecycle
   - `NetNewFlow`: Draft → Validated → Priced → (Approved) → Sent
   - `FlowState`, `FlowEvent`, `FlowAction`: Type-safe state transitions
   - Typed rejection errors with replay-stability tests

4. **Deal DNA** (`crates/core/src/dna/`)
   - `FingerprintGenerator`: 128-bit SimHash signatures
   - `SimilarityEngine`: Find similar past deals
   - Deterministic, order-independent fingerprinting

5. **Immutable Ledger** (`crates/core/src/ledger/`)
   - `LedgerService`: Cryptographic quote integrity
   - SHA-256 content hashing, HMAC-SHA256 signatures
   - Chain linking between versions, tamper detection

6. **Audit System** (`crates/core/src/audit.rs`)
   - Structured audit events with quote_id, thread_id, correlation_id
   - `AuditSink` trait for emission
   - Complete event trail for compliance

7. **Error Taxonomy** (`crates/core/src/errors.rs`)
   - `DomainError` → `ApplicationError` → `InterfaceError`
   - Explicit conversion boundaries
   - User-safe interface messages

8. **Configuration** (`crates/core/src/config.rs`)
   - Typed startup config with load precedence
   - Environment variable interpolation
   - Fail-fast validation

9. **Database Layer** (`crates/db/`)
   - Connection pooling with sqlx
   - Migration runner
   - Repository pattern for all entities

10. **Slack Integration** (`crates/slack/`)
    - Socket Mode WebSocket connection
    - Block Kit builders
    - Command and event handlers

11. **CLI Tools** (`crates/cli/`)
    - `start`: Run the server
    - `migrate`: Database migrations
    - `seed`: Load demo fixtures
    - `smoke`: Health checks
    - `doctor`: Diagnostics

### 🔄 In Progress

1. **Execution Queue** (`crates/core/src/domain/execution.rs`, `crates/db/src/repositories/execution_queue.rs`)
   - Durable idempotent execution for Slack/CRM/PDF actions
   - State machine: Queued → Running → Completed/Failed
   - Idempotency ledger for duplicate prevention
   - Migration: `migrations/0012_execution_queue_rel.up.sql`

### 📋 Pending Features (Beads)

| Bead ID | Title | Priority | Status |
|---------|-------|----------|--------|
| bd-271.1 | W1 [REL] Resilient Execution Queue | P0 | In Progress |
| bd-70d.1 | FEAT-01: Deal DNA | P1 | Open |
| bd-70d.2 | FEAT-02: Conversational Constraint Solver | P1 | Open |
| bd-70d.3 | FEAT-03: Emoji-Based Micro-Approvals | P1 | Open |
| bd-70d.5 | FEAT-05: Explainable Policy Engine | P1 | Open |
| bd-70d.8 | FEAT-08: Immutable Quote Ledger | P1 | Open |
| bd-271.10 | W1 [SAN] Rule Sandbox | P1 | Open |

---

## Database Schema

### 12 Migrations Complete

1. `0001_initial` - Core tables (products, price books, quotes, etc.)
2. `0002_emoji_approvals` - Emoji reaction-based approvals
3. `0003_configuration_fingerprints` - Deal DNA storage
4. `0004_dialogue_state` - Conversational constraint state
5. `0005_policy_explanations` - Explainable policy engine
6. `0006_quote_sessions` - Quote collaboration sessions
7. `0007_quote_ledger` - Immutable quote ledger
8. `0008_dependency_graph` - Constraint dependency graphs
9. `0009_ghost_signals` - Lost deal signal detection
10. `0010_approval_routing` - Smart approval routing
11. `0011_win_probability` - Win probability ML features
12. `0012_execution_queue_rel` - Resilient execution queue

---

## Key Design Principles

### 1. Safety Principle (Non-Negotiable)

**LLMs are translators, NEVER decision makers.**

| What LLMs DO | What LLMs DON'T DO |
|--------------|-------------------|
| Natural language → structured intent | Decide prices |
| Fuzzy product names → product IDs | Validate configurations |
| Structured data → summaries | Approve discounts |
| Draft approval justification | Choose workflow steps |

### 2. Deterministic Core

All financial/policy decisions are deterministic:
- Constraint engine validates configurations
- Pricing engine computes prices with full trace
- Policy engine determines approval requirements
- Flow engine controls state transitions

### 3. Local-First Architecture

- SQLite database (zero ops)
- Slack Socket Mode (no public URL needed)
- Single binary deployment
- Works fully offline

### 4. Auditability

Every action produces an audit event:
- Quote lifecycle events
- Pricing calculations with trace
- Policy evaluations
- Approval decisions
- LLM interactions

---

## Current Build Status

**Status:** ⚠️ Build Failure  
**Issue:** `chrono` import error in `crates/db/src/repositories/execution_queue.rs`  
**Root Cause:** Missing `chrono` re-export in `quotey-core`

**Fix Required:**
Add to `crates/core/src/lib.rs`:
```rust
pub use chrono;
```

---

## Research Findings

### Market Opportunity

Salesforce CPQ entered End-of-Sale (March 2025) with projected EOL 2029-2030. This creates a 3-year window for alternatives that:
1. Don't require big-bang migrations
2. Can be adopted incrementally
3. Provide value from Day 1
4. Don't lock into cloud platforms

### Competitive Differentiation

| Competitor | Weakness | Quotey Advantage |
|------------|----------|------------------|
| Salesforce CPQ | Performance ceilings, EOL | No lock-in, local-first |
| DealHub | Less mature for manufacturing | Constraint-based config |
| Conga | Implementation complexity | Agent-first bootstrap |
| PROS | High cost, needs data science | Local simplicity |
| Tacton | Narrow manufacturing focus | Full CPQ + agent layer |

### Why CPQ Implementations Fail

1. **Data readiness** (43% still use spreadsheets)
2. **Scope creep** from pricing complexity
3. **Integration stalls** with CRM/ERP
4. **User adoption** (reps revert to spreadsheets)
5. **Organizational misalignment**

Quotey addresses #1 with catalog bootstrap agent, #4 with Slack-native UX.

---

## Agent Mail Registration

**Agent Name:** ResearchAgent  
**Role:** Research, documentation, code investigation  
**Project:** /data/projects/quotey  

### Capabilities
- Codebase analysis and architecture documentation
- Beads status tracking
- Build issue investigation
- Cross-agent coordination

### Current Priority
1. ✅ Fix build failure (execution_queue chrono import) - COMPLETED
2. Complete research documentation
3. Coordinate with other agents on bead priorities

### Work In Progress
- **bd-271.1.3**: [REL] Task 3 Deterministic Engine Logic (Claimed by ResearchAgent)
  - Building deterministic execution engine for resilient queue processing
  - Unblocks: bd-271.1.4

---

## Next Steps

1. **Immediate:** Fix chrono import in execution_queue repository
2. **Short-term:** Verify build passes all quality gates
3. **Medium-term:** Research agent should track bead progress
4. **Long-term:** Maintain architecture documentation as system evolves

---

## Quality Gates

Per `AGENTS.md` and `FOUNDATION_QUALITY_GATES.md`:

```bash
# Required before any commit
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
ubs <changed-files>
```

---

## Resources

- **Planning:** `.planning/PROJECT.md`
- **Agent Instructions:** `AGENTS.md`
- **Claude Guide:** `CLAUDE.md`
- **Beads:** `.beads/issues.jsonl`
- **Migrations:** `migrations/`
- **Specs:** `.planning/W1_*_SPEC.md`

---

*Document Version: 1.0*  
*Last Updated: 2026-02-23*
