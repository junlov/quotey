# Codebase Structure

**Analysis Date:** 2026-02-26

## Directory Layout

```
quotey/
├── .beads/                    # Issue tracking (br/beads)
├── .planning/                 # Planning source of truth
│   ├── config.json
│   ├── PROJECT.md
│   └── *.md                   # Specs and research docs
├── crates/                    # Rust workspace members
│   ├── core/                  # Core domain and CPQ logic
│   ├── db/                    # Database layer
│   ├── slack/                 # Slack bot integration
│   ├── agent/                 # Agent runtime (LLM + guardrails)
│   ├── cli/                   # CLI commands
│   ├── server/                # HTTP server
│   └── mcp/                   # MCP server for AI agents
├── migrations/                # SQLx database migrations
├── templates/                 # HTML/CSS quote templates
├── config/                    # Runtime config + demo fixtures
└── scripts/                   # Utility scripts
```

## Crate Purposes

### `crates/core`
- **Purpose:** Core business logic, CPQ engine, domain models
- **Contains:**
  - Domain entities (quote, product, customer, approval)
  - CPQ engines (pricing, constraints, policy)
  - Flow state machine
  - Execution engine
  - Audit, ledger, explanations
  - Advanced features (DNA, autopsy, optimizer)
- **Key files:**
  - `src/lib.rs`: Public exports
  - `src/domain/quote.rs`: Quote status machine
  - `src/cpq/mod.rs`: CPQ runtime trait
  - `src/flows/engine.rs`: Flow state machine

### `crates/db`
- **Purpose:** SQLite persistence layer
- **Contains:**
  - Database connection management
  - SQLx migrations
  - Repository implementations
  - Test fixtures
- **Key files:**
  - `src/connection.rs`: DbPool creation
  - `src/repositories/`: Quote, product, customer, approval repositories

### `crates/slack`
- **Purpose:** Slack bot interface
- **Contains:**
  - Socket Mode WebSocket handling
  - Slash command handlers
  - Event processing
  - Block Kit UI builders
- **Key files:**
  - `src/socket.rs`: WebSocket event loop
  - `src/commands.rs`: Slash command handlers
  - `src/blocks.rs`: Rich message builders

### `crates/agent`
- **Purpose:** Agent runtime for LLM orchestration
- **Contains:**
  - Intent extraction from natural language
  - Guardrail enforcement
  - Tool orchestration
  - LLM provider abstraction
- **Key files:**
  - `src/runtime.rs`: Main orchestrator
  - `src/guardrails.rs`: Safety policies
  - `src/conversation.rs`: Context handling

### `crates/mcp`
- **Purpose:** MCP server for AI agent integration
- **Contains:**
  - MCP protocol implementation (using `rmcp`)
  - Tool definitions and routing
  - Authentication
- **Key files:**
  - `src/server.rs`: MCP server (843 lines - main implementation)
  - `src/tools.rs`: Tool category organization
  - `src/auth.rs`: API key authentication

### `crates/cli`
- **Purpose:** Operator CLI
- **Contains:**
  - Commands: start, migrate, seed, smoke, config, doctor
  - Policy packet builder
  - Revenue genome commands
- **Key files:**
  - `src/main.rs`: CLI entry point
  - `src/commands/`: Individual command implementations

### `crates/server`
- **Purpose:** Main server application
- **Contains:**
  - Server bootstrap
  - Health check endpoint
  - App initialization
- **Key files:**
  - `src/main.rs`: Server entry point
  - `src/bootstrap.rs`: App initialization

## Key File Locations

**Entry Points:**
- `crates/server/src/main.rs`: Slack server (run via `cargo run -p quotey-server`)
- `crates/mcp/src/main.rs`: MCP server (run via `cargo run -p quotey-mcp`)
- `crates/cli/src/main.rs`: CLI (run via `cargo run -p quotey-cli -- <command>`)

**Configuration:**
- `crates/core/src/config.rs`: App configuration (26,571 lines - extensive)
- `config/`: Runtime config and demo fixtures

**Database:**
- `migrations/`: SQLx migration files
- `crates/db/src/migrations.rs`: Migration runner

## Where to Add New Code

**New Feature:**
- Primary code: `crates/core/src/domain/` (domain logic)
- Or create new module under `crates/core/src/`

**New MCP Tool:**
- Add tool definition in `crates/mcp/src/server.rs`
- Follow `#[tool]` macro pattern with input/output types

**New Slack Command:**
- Add handler in `crates/slack/src/commands.rs`

**New CLI Command:**
- Add subcommand in `crates/cli/src/lib.rs`
- Implement in `crates/cli/src/commands/`

**Database Changes:**
- Add migration in `migrations/`
- Add repository method in `crates/db/src/repositories/`

## Naming Conventions

**Files:**
- Rust conventions: `snake_case.rs`
- Modules match file names

**Crates:**
- `quotey-core`, `quotey-db`, `quotey-slack`, `quotey-agent`, `quotey-cli`, `quotey-server`, `quotey-mcp`

**Domain Types:**
- `QuoteId`, `ProductId`, `ApprovalId` (newtype wrappers)
- `QuoteStatus`, `ApprovalStatus` (enums)

## Special Directories

**`.beads/`:**
- Purpose: Issue tracking
- Contains: `issues.jsonl`, `beads.db`

**`.planning/`:**
- Purpose: Planning source of truth
- Contains: `config.json`, `PROJECT.md`, specs, research

**`migrations/`:**
- Purpose: Database schema migrations
- Generated: Yes (SQLx auto-generate)

**`templates/`:**
- Purpose: HTML/CSS for PDF quote generation
- Generated: No

---

*Structure analysis: 2026-02-26*
