# Architecture

**Analysis Date:** 2026-02-26

## Pattern Overview

**Overall:** Rust-based local-first CPQ (Configure, Price, Quote) agent running in Slack with AI agent extensibility via MCP

**Key Characteristics:**
- **Multi-channel**: Slack-first (Socket Mode), but extensible to MCP for AI agents
- **Deterministic core**: LLMs translate natural language, but pricing/constraints/policies are deterministic Rust code
- **Local-first**: SQLite database, no external cloud dependencies
- **Event-driven**: State machine for quote lifecycle with explicit transitions
- **Audit-first**: Full audit trail for compliance (ledger, autopsy, revenue genome)

## Layers

### 1. Interface Layer (`crates/slack`, `crates/mcp`)

**Slack Bot Interface:**
- Location: `crates/slack/src/`
- Contains: Socket Mode WebSocket handling, slash commands, Block Kit UI builders
- Key modules: `socket.rs`, `commands.rs`, `events.rs`, `blocks.rs`
- Depends on: `agent`, `core`, `db`

**MCP Server (AI Agents):**
- Location: `crates/mcp/src/`
- Contains: MCP protocol implementation using `rmcp` crate
- Key modules: `server.rs`, `tools.rs`, `auth.rs`
- Depends on: `core`, `db`
- Current tools: catalog_search, catalog_get, quote_create, quote_get, quote_price, quote_list, approval_request, approval_status, approval_pending, quote_pdf

### 2. Agent Runtime (`crates/agent`)

**Purpose:** LLM-powered intent extraction and orchestration
- Location: `crates/agent/src/`
- Contains: Agent runtime, guardrails, conversation handling, tool orchestration
- Key modules: `runtime.rs`, `guardrails.rs`, `conversation.rs`, `llm.rs`, `tools.rs`
- Depends on: `core`
- Safety principle: LLM translates NL → structured intent, NEVER decides prices/policies

### 3. Flows Engine (`crates/core/src/flows`)

**Purpose:** State machine for quote lifecycle
- Location: `crates/core/src/flows/`
- Contains: Flow definitions, state transitions, transition outcomes
- Files: `engine.rs`, `states.rs`
- Quote statuses: Draft → Validated → Priced → Approval → Approved → Finalized → Sent

### 4. CPQ Core (`crates/core/src/cpq`)

**Purpose:** Deterministic pricing, constraints, and policy evaluation
- Location: `crates/core/src/cpq/`
- Contains: Pricing engine, constraint engine, policy engine, simulator
- Files: `pricing.rs`, `constraints.rs`, `policy.rs`, `simulator.rs`, `catalog.rs`, `precedent.rs`
- All deterministic - no LLM involvement

### 5. Domain Layer (`crates/core/src/domain`)

**Purpose:** Core domain models and business logic
- Location: `crates/core/src/domain/`
- Key entities:
  - `quote.rs`: Quote, QuoteLine, QuoteStatus with transition validation
  - `product.rs`: Product, ProductId
  - `customer.rs`: Customer entity
  - `approval.rs`: ApprovalRequest, ApprovalStatus
  - `execution.rs`: ExecutionTask, ExecutionTaskState
  - `simulation.rs`: Simulation models
  - `precedent.rs`: Precedent intelligence
  - `optimizer.rs`: Policy optimization models
  - `autopsy.rs`: Deal autopsy models

### 6. Data Layer (`crates/db`)

**Purpose:** SQLite persistence and repositories
- Location: `crates/db/src/`
- Contains: Database connection, migrations, repositories, fixtures
- Key modules: `connection.rs`, `migrations.rs`, `repositories/`
- Repositories: quote, product, customer, approval, precedent, simulation, execution_queue, pricing_snapshot, optimizer, explanation

### 7. Execution Engine (`crates/core/src/execution_engine.rs`)

**Purpose:** Deterministic task execution with retry policies
- Location: `crates/core/src/execution_engine.rs`
- Contains: TransitionResult, RetryPolicy, DeterministicExecutionEngine

## Data Flow

**Slack → Agent Flow:**
```
Slack Message → SocketModeRunner → EventDispatcher → AgentRuntime
  → GuardrailPolicy (validate intent)
  → Tool execution (CPQ operations)
  → Response to Slack
```

**MCP → CPQ Flow:**
```
MCP Request → QuoteyMcpServer → Tool Router
  → Database queries / Quote operations
  → Response to MCP client
```

**Quote Lifecycle:**
```
Draft → Validated (constraint check) → Priced (pricing engine) → Approval (if needed)
  → Approved → Finalized → Sent
```

## Key Abstractions

**CpqRuntime Trait:**
- Purpose: Unified interface for quote evaluation
- Implementation: `DeterministicCpqRuntime<C, P, O>` with configurable engines
- Combines: ConstraintEngine, PricingEngine, PolicyEngine

**AgentRuntime:**
- Purpose: Orchestrates LLM + guardrails + tools
- Pattern: Intent classification → Guardrail evaluation → Tool execution

**FlowEngine:**
- Purpose: Manages quote state transitions
- Pattern: Explicit state machine with allowed transitions

## Entry Points

**Server (Slack):**
- Location: `crates/server/src/main.rs`
- Triggers: Slack WebSocket events, slash commands
- Responsibilities: Bootstrap app, start Slack runner, handle health checks

**MCP Server:**
- Location: `crates/mcp/src/main.rs`
- Triggers: MCP client requests (stdio transport)
- Responsibilities: Tool routing, authentication, audit logging

**CLI:**
- Location: `crates/cli/src/main.rs`
- Commands: start, migrate, seed, smoke, config, doctor, policy-packet, genome
- Responsibilities: Operational tasks, diagnostics, policy management

## Cross-Cutting Concerns

**Audit:**
- Module: `crates/core/src/audit.rs`
- Full audit trail for all quote operations

**Ledger:**
- Module: `crates/core/src/ledger/`
- Immutable quote ledger for compliance

**Explanations:**
- Module: `crates/core/src/explanation/`
- Policy violation explanations, pricing breakdowns

**Autopsy & Revenue Genome:**
- Module: `crates/core/src/autopsy/`, `crates/core/src/domain/autopsy.rs`
- Deal analysis, counterfactual simulation, attribution graphs

---

*Architecture analysis: 2026-02-26*
