# Quotey Project - Comprehensive Research Document

**Research Agent:** ResearchAgent  
**Date:** 2026-02-23  
**Project:** quotey - Rust-based, local-first CPQ (Configure, Price, Quote) agent for Slack  

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Project Overview](#project-overview)
3. [Architecture Overview](#architecture-overview)
4. [Crate-by-Crate Analysis](#crate-by-crate-analysis)
5. [Key Design Patterns](#key-design-patterns)
6. [Current Implementation Status](#current-implementation-status)
7. [Active Work (Beads)](#active-work-beads)
8. [Research Findings](#research-findings)
9. [Recommendations](#recommendations)

---

## Executive Summary

Quotey is a sophisticated Rust-based CPQ (Configure, Price, Quote) system designed to operate as a Slack-native agent. It combines natural language interaction with deterministic business logic, ensuring that while LLMs assist with intent extraction and user experience, all pricing, policy, and approval decisions are made by auditable, deterministic engines.

**Key Differentiators:**
- Local-first architecture (SQLite, single binary deployment)
- Constraint-based configuration (not rule-based)
- Deal DNA fingerprinting for similarity matching
- Cryptographic quote ledger for tamper-evident audit trails
- Configuration archaeology for forensics
- Ghost quotes for predictive opportunity detection

---

## Project Overview

### What is Quotey?

Quotey replaces traditional rigid CPQ screens with natural language interaction in Slack. Sales reps express intent conversationally, and the agent:
1. Gathers context through dialogue
2. Configures products using constraint-based validation
3. Runs deterministic pricing with full traceability
4. Manages approval workflows with smart routing
5. Generates PDF quotes from HTML templates

### Target Users

- Sales reps and deal desk analysts
- Mid-to-enterprise organizations
- Teams currently using spreadsheets or legacy CPQ tools

### Core Value Proposition

Create accurate, policy-compliant, fully-audited quotes through natural conversation in Slack—without touching a CPQ UI, without waiting days for approvals.

---

## Architecture Overview

### High-Level Architecture (6 Boxes)

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

### The Safety Principle (Non-Negotiable)

**LLMs may:**
- Translate natural language → structured intent
- Map fuzzy product names → product IDs
- Generate human-friendly summaries
- Draft approval justification text

**LLMs NEVER:**
- Decide prices (pricing engine decides)
- Validate configurations (constraint engine decides)
- Approve discounts (approval workflow decides)
- Choose workflow steps (flow engine decides)

---

## Crate-by-Crate Analysis

### 1. `crates/core` - Domain and Business Logic

**Responsibility:** Deterministic domain primitives and engine seams

#### Domain Models (`src/domain/`)

| Module | Key Types | Purpose |
|--------|-----------|---------|
| `quote.rs` | `Quote`, `QuoteLine`, `QuoteId`, `QuoteStatus` | Central quote aggregate with 11-state lifecycle |
| `product.rs` | `Product`, `ProductId` | Product catalog entity |
| `customer.rs` | `Customer`, `CustomerId` | Customer identity and segmentation |
| `approval.rs` | `ApprovalRequest`, `ApprovalId`, `ApprovalStatus` | Approval workflow tracking |
| `execution.rs` | `ExecutionTask`, `IdempotencyRecord`, `ExecutionTransitionEvent` | Async task execution with idempotency |

**Quote Status Lifecycle:**
```
Draft → Validated → Priced → (Approval → Approved) → Finalized → Sent
  ↑      │           │            │        │           │         │
  └─── Revised      │            │        │           │         │
       (from       │            │        │           │         │
        Rejected)  │            │        │           │         │
                    │            │        │           │         │
Terminal: Rejected, Expired, Cancelled
```

#### Flow Engine (`src/flows/`)

- **FlowEngine**: Generic state machine with audit integration
- **NetNewFlow**: Implementation for new quote flows
- **FlowState/FlowEvent/FlowAction**: Type-safe state machine components

**Key Transitions:**
| From | Event | To | Action |
|------|-------|-----|--------|
| Draft | RequiredFieldsCollected | Validated | EvaluatePricing |
| Validated | PricingCalculated | Priced | EvaluatePolicy |
| Priced | PolicyClear | Finalized | FinalizeQuote |
| Priced | PolicyViolationDetected | Approval | RouteApproval |
| Approval | ApprovalGranted | Approved | FinalizeQuote |

#### CPQ Core (`src/cpq/`)

| Module | Trait | Implementation | Purpose |
|--------|-------|----------------|---------|
| `constraints.rs` | `ConstraintEngine` | `DeterministicConstraintEngine` | Configuration validation |
| `pricing.rs` | `PricingEngine` | `DeterministicPricingEngine` | Price calculation with trace |
| `policy.rs` | `PolicyEngine` | `DeterministicPolicyEngine` | Business rule evaluation |
| `catalog.rs` | `Catalog` | In-memory registry | Product lookup |

**CpqRuntime** orchestrates all three engines:
```rust
pub trait CpqRuntime: Send + Sync {
    fn evaluate_quote(&self, input: CpqEvaluationInput<'_>) -> CpqEvaluation;
}
```

#### Advanced Features (`src/`)

| Module | Purpose | Key Algorithm |
|--------|---------|---------------|
| `audit.rs` | Structured audit events | Sink pattern with correlation tracking |
| `dna/mod.rs` | Configuration fingerprinting | SimHash (128-bit LSH) |
| `archaeology/mod.rs` | Configuration forensics | Dependency graph traversal |
| `ledger/mod.rs` | Cryptographic audit trail | Hash chain with HMAC |
| `collab/mod.rs` | Multi-user quote sessions | Operational Transform |
| `ghost/mod.rs` | Predictive opportunity creation | Signal detection + draft generation |
| `policy/mod.rs` | Policy explanations | Template-based explanation generation |

**Deal DNA (SimHash):**
- 128-bit fingerprints for configuration similarity
- Order-independent hashing
- Sub-100ms similarity queries on 10,000 candidates
- Default threshold: 80% similarity

**Ledger (Cryptographic):**
- SHA-256 content hashing
- HMAC-SHA256 signatures
- Chain linking: each entry includes previous hash
- Tamper-evident verification

#### Error Handling (`src/errors.rs`)

Three-layer error taxonomy:
```
DomainError → ApplicationError → InterfaceError
   (business)    (infrastructure)   (user-facing)
```

---

### 2. `crates/db` - Data Persistence

**Responsibility:** SQLite connection, migrations, and repository layer

#### Connection Management (`src/connection.rs`)

- `DbPool = sqlx::SqlitePool`
- WAL mode for concurrency
- Foreign key enforcement
- Busy timeout: 5 seconds

#### Migrations (`src/migrations.rs`)

- SQLx built-in migrator
- 46 tables covering all domains
- 52 indexes for query optimization
- Reversible migrations

#### Repository Pattern (`src/repositories/`)

| Repository | Status | Notes |
|------------|--------|-------|
| `QuoteRepository` | Scaffolded | Stub implementation |
| `ProductRepository` | Scaffolded | Stub implementation |
| `ApprovalRepository` | Scaffolded | Stub implementation |
| `ExecutionQueueRepository` | **Full** | Production-ready with idempotency |
| `IdempotencyRepository` | **Full** | Bundled with Execution Queue |

**Execution Queue Schema:**
- `execution_queue_task`: Task lifecycle management
- `execution_queue_transition_audit`: Complete state history
- `execution_idempotency_ledger`: Duplicate prevention

**In-Memory Implementations:** Available for all repositories (testing)

---

### 3. `crates/agent` - Agent Runtime

**Responsibility:** Runtime orchestration, tool registry, and guardrails

#### Runtime (`src/runtime.rs`)

- `AgentRuntime`: Minimal orchestrator (19 lines)
- `handle_thread_message()`: Main entry point
- Guardrails injected at construction

#### Tools (`src/tools.rs`)

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    async fn execute(&self, input: Value) -> Result<Value>;
}
```

**Current State:** Empty registry skeleton - tools to be registered

#### Conversation (`src/conversation.rs`)

**Intent Extraction:**
- Product mention detection (keyword matching)
- Quantity parsing ("200 seats")
- Budget extraction ("$50k")
- Timeline hints ("this quarter", "Q2")
- Discount parsing ("15%")
- Constraint detection (budget caps, exclusions)

**Confidence Scoring (0-100):**
- Base: 10
- Product mention: +30
- Budget: +20
- Quantity: +15
- Discount: +15
- Constraints: +10
- Timeline: +10

**Clarification Prompts:** Generated when confidence < 40

#### Guardrails (`src/guardrails.rs`)

```rust
pub struct GuardrailPolicy {
    pub llm_can_set_prices: bool,      // Default: false
    pub llm_can_approve_discounts: bool, // Default: false
}
```

Explicit safety policy preventing LLM overreach.

#### LLM Integration (`src/llm.rs`)

Trait-based abstraction for provider flexibility:
```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, prompt: &str) -> Result<String>;
}
```

---

### 4. `crates/slack` - Slack Integration

**Responsibility:** Slack Socket Mode, commands, events, Block Kit UI

#### Socket Mode (`src/socket.rs`)

**SocketModeRunner Features:**
- Exponential backoff: `delay = min(base * 2^attempt, max_delay)`
- Graceful degradation after max retries
- Correlation tracking: `Q-YYYY-XXXX` quote ID extraction
- Structured logging with `event_name`, `correlation_id`, `quote_id`, `thread_id`

**Default ReconnectPolicy:**
- Max retries: 5
- Base delay: 250ms
- Max delay: 5s

#### Commands (`src/commands.rs`)

| Command | Syntax | Purpose |
|---------|--------|---------|
| `/quote new` | `new [for <customer>]` | Create quote |
| `/quote status` | `status <quote_id>` | Check status |
| `/quote list` | `list [filter]` | List quotes |
| `/quote help` | `help` | Show help |

**Quote ID Format:** `Q-YYYY-XXXX` (validated with byte-level checks)

#### Events (`src/events.rs`)

| Handler | Event Type | Service Trait |
|---------|------------|---------------|
| `SlashCommandHandler` | SlashCommand | `QuoteCommandService` |
| `ThreadMessageHandler` | ThreadMessage | `ThreadMessageService` |
| `ReactionAddedHandler` | ReactionAdded | `ReactionApprovalService` |

**Emoji Approvals:**
- 👍 / :+1: → Approve
- 👎 / :-1: → Reject
- 💬 → Request changes

#### Blocks (`src/blocks.rs`)

Block Kit UI generation:
- `MessageBuilder`: Fluent builder pattern
- Pre-built templates: `quote_status_message`, `approval_request_message`, `error_message`, `help_message`

---

### 5. `crates/server` - Server Bootstrap

**Responsibility:** Executable bootstrap/wiring entrypoint

#### Main (`src/main.rs`)

Initialization flow:
1. Initialize tracing subscriber
2. Bootstrap application (DB, migrations, agent runtime)
3. Spawn health check endpoint
4. Start Slack socket mode runner
5. Wait for shutdown signal

#### Bootstrap (`src/bootstrap.rs`)

```rust
pub struct Application {
    pub config: AppConfig,
    pub db_pool: DbPool,
    pub agent_runtime: AgentRuntime,
    pub slack_runner: SocketModeRunner,
}
```

**Integration Tests:**
- Database table verification
- Flow engine state transitions
- CPQ evaluation (constraints, pricing, policy)

#### Health (`src/health.rs`)

- Axum-based HTTP endpoint
- `GET /health` → JSON response
- Database health check via `SELECT 1`
- HTTP 200 (ready) or 503 (degraded)

---

### 6. `crates/cli` - Command Line Interface

**Responsibility:** Operator command surface

#### Commands

| Command | Purpose | Exit Codes |
|---------|---------|------------|
| `start` | Startup preflight | 0, 2-5 |
| `migrate` | Apply migrations | 0, 2-5 |
| `seed` | Load demo fixtures | 0 (no-op) |
| `smoke` | E2E readiness checks | 0, 6 |
| `config` | Inspect configuration | 0, 2 |
| `doctor` | Validate setup | 0, 1 |

**Exit Code Reference:**
- 0: Success
- 2: Config validation failure
- 3: Runtime initialization failure
- 4: Database connectivity failure
- 5: Migration failure
- 6: Smoke test failure

---

## Key Design Patterns

### 1. Newtype Pattern
All IDs are newtype wrappers for type safety:
```rust
pub struct QuoteId(String);
pub struct ProductId(String);
pub struct CustomerId(Uuid);
```

### 2. Trait-Based Abstraction
Engines and repositories use traits for testability:
```rust
pub trait PricingEngine: Send + Sync { ... }
pub trait QuoteRepository { ... }
```

### 3. State Machine Pattern
Explicit states, events, and valid transitions:
```rust
fn transition(&self, current: &FlowState, event: &FlowEvent) 
    -> Result<TransitionOutcome, FlowTransitionError>;
```

### 4. Idempotency Pattern
Execution framework prevents duplicates:
```rust
pub struct IdempotencyRecord {
    pub operation_key: OperationKey,
    pub payload_hash: String,
    pub state: IdempotencyRecordState,
    ...
}
```

### 5. Audit Trail Pattern
Every action emits immutable audit events:
```rust
AuditEvent::new(quote_id, thread_id, correlation_id, 
    event_type, category, actor, outcome)
    .with_metadata("from", "Draft")
    .with_metadata("to", "Validated")
```

### 6. Layered Error Handling
Clear separation between domain, application, and interface concerns.

### 7. Optimistic Concurrency
`state_version` field for conflict detection in `ExecutionTask`.

---

## Current Implementation Status

### Production-Ready Components ✅

1. **Execution Queue Repository** - Full SQL implementation with idempotency
2. **CPQ Runtime** - Deterministic engines for constraints, pricing, policy
3. **Flow Engine** - State machine with audit integration
4. **Deal DNA** - SimHash fingerprinting with similarity search
5. **Ledger** - Cryptographic hash chain
6. **Archaeology** - Dependency graph forensics
7. **Collab** - Operational transform for multi-user sessions
8. **Slack Socket Mode** - WebSocket connection with reconnection
9. **CLI Commands** - start, migrate, smoke, config, doctor
10. **Health Check** - Axum endpoint with DB verification

### Scaffolded / Partial Components 🚧

1. **Quote Repository** - Stub implementation
2. **Product Repository** - Stub implementation
3. **Approval Repository** - Stub implementation
4. **Agent Tool Registry** - Empty skeleton
5. **LLM Implementations** - Trait only, no providers
6. **Seed Command** - Deterministic no-op

### Not Yet Started ❌

1. **CRM Integration** - Composio REST client scaffold needed
2. **PDF Generation** - HTML→PDF pipeline
3. **Catalog Bootstrap** - CSV/PDF ingestion
4. **Quote Intelligence** - RFP/email parsing

---

## Active Work (Beads)

### Current In-Progress Beads

| ID | Title | Priority | Status |
|----|-------|----------|--------|
| bd-271.1 | W1 [REL] Resilient Execution Queue | P0 | In Progress |
| bd-271.3 | W1 [FIX] Constraint Auto-Repair and Tradeoff Explorer | P1 | In Progress |

### Top Ready Beads (Per bv triage)

| ID | Title | Type | Action |
|----|-------|------|--------|
| bd-271.1.3 | [REL] Task 3 Deterministic Engine Logic | task | Work on bd-271.1.2 first |
| bd-271.2.3 | [EXP] Task 3 Deterministic Engine Logic | task | Work on bd-271.2.2 first |
| bd-271.3.2 | [FIX] Task 2 API and Slack Surfaces | task | Available |

### Major Epics

| ID | Title | Description |
|----|-------|-------------|
| bd-70d | EPIC: Quotey AI-Native CPQ Enhancement Initiative | 10 major feature areas including Deal DNA, Conversational Solver, Emoji Approvals |
| bd-271 | EPIC: Power Capabilities Wave 1 (Top-10 Differentiators) | Deal Flight Simulator, Constraint Auto-Repair, Approval Packet Autopilot, etc. |

---

## Research Findings

### Strengths

1. **Solid Architectural Foundation**: Clean separation of concerns between crates
2. **Deterministic Safety**: Core business logic is deterministic and auditable
3. **Comprehensive Test Coverage**: Unit tests in modules + integration tests
4. **Modern Rust Practices**: Async/await, structured logging, error handling
5. **Extensible Design**: Trait-based abstractions allow swapping implementations
6. **Documentation**: Well-documented planning in `.planning/PROJECT.md`

### Areas for Attention

1. **Repository Implementation Gap**: Core repositories (quote, product, approval) are stubs
2. **LLM Provider Gap**: No concrete LLM implementations (OpenAI, Anthropic, Ollama)
3. **Tool Registry Empty**: Agent tools need to be implemented and registered
4. **CRM Integration Missing**: Composio REST client not yet built
5. **PDF Generation Missing**: No HTML→PDF pipeline

### Technical Debt Observations

1. **Placeholder Returns**: Many repository methods return `Ok(())` or `Ok(None)`
2. **No-op Seed Command**: Intentionally empty, needs fixtures
3. **Missing Error Variants**: Some error cases not fully enumerated

---

## Recommendations

### Immediate (Next 2 Weeks)

1. **Complete REL Execution Queue** (bd-271.1) - P0, already in progress
2. **Implement Quote Repository** - Unblock quote persistence
3. **Add LLM Provider** - At least one concrete implementation (Ollama for local-first)

### Short Term (Next Month)

1. **Complete Power Capabilities Wave 1** (bd-271)
2. **Implement Product Repository** - Enable catalog operations
3. **Build CRM Composio Client** - Enable external integrations
4. **Add PDF Generation** - Complete quote delivery pipeline

### Medium Term (Next Quarter)

1. **Complete AI-Native CPQ Epic** (bd-70d)
2. **Performance Optimization** - Query tuning, connection pooling
3. **Observability** - Metrics, alerting, tracing
4. **Security Audit** - Cryptographic verification, input validation

---

## Appendix A: Workspace Dependencies

```toml
[workspace.dependencies]
anyhow = "1.0"
async-trait = "0.1"
chrono = { version = "0.4", features = ["serde"] }
clap = { version = "4.5", features = ["derive"] }
rust_decimal = { version = "1.36", features = ["serde"] }
secrecy = "0.10"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sqlx = { version = "0.8.6", features = [...] }
thiserror = "2.0"
tokio = { version = "1.43", features = [...] }
toml = "0.8"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = [...] }
uuid = { version = "1.11", features = ["v4", "serde"] }
```

## Appendix B: Database Schema Overview

### 46 Tables Across 6 Domains

1. **Quotes**: quote, quote_line, quote_pricing_snapshot
2. **Products**: product, product_relationship, price_book, price_book_entry, volume_tier, pricing_formula, bundle, constraint_rule, discount_policy, approval_threshold
3. **Customers**: account, contact, deal
4. **Workflow**: flow_state, approval_request, approval_chain
5. **Audit**: audit_event, slack_thread_map, crm_sync_state, llm_interaction_log
6. **Advanced**: configuration_fingerprints, similarity_cache, quote_ledger, execution_queue_task, etc.

---

*Document Version: 1.0*  
*Research Agent: ResearchAgent*  
*Status: Complete*
