# Foundation Scaffold Implementation Plan

**Epic:** bd-3d8 - Foundation Scaffold and Core Bootstrapping  
**Approach:** Conservative, Sequential, Rust-Idiomatic  
**Target:** Production-ready foundation for all AI-Native CPQ features

---

## Executive Summary

This plan details the step-by-step implementation of Quotey's Rust foundation. Each task builds upon the previous, with explicit success criteria and verification steps. No task begins until its dependencies are complete and verified.

**Core Principles:**
1. **Compiler-First Development** - Let the type system catch errors
2. **Explicit Over Implicit** - Clear module boundaries, no magic
3. **Test-First for Contracts** - Traits have mock implementations before real ones
4. **Fail Fast at Compile Time** - `unwrap()` only in tests, proper error handling in prod
5. **Observability Built-In** - Tracing from day one, not retrofitted

---

## Phase 0: Environment & Tooling Validation

### Task 0.1: Validate Development Environment
**Purpose:** Ensure consistent, reproducible builds

**Checklist:**
- [ ] Rust version >= 1.75 (for async fn in traits)
- [ ] `cargo fmt` configured with `rustfmt.toml`
- [ ] `cargo clippy` with strict lints enabled
- [ ] `cargo sqlx-cli` installed for migrations
- [ ] `cargo nextest` for faster test execution
- [ ] `cargo deny` for dependency auditing

**Verification:**
```bash
rustc --version  # Should be 1.75+
cargo fmt -- --check  # Should pass with no changes
cargo clippy -- -D warnings  # Should pass with no warnings
```

---

## Phase 1: Crate Skeleton (Task bd-3d8.1)

### 1.1 Workspace Structure
**File:** `Cargo.toml` (workspace root)

**Rationale:** Single workspace with multiple crates for clean boundaries:
- `quotey-core`: Domain logic, pure business rules (no async)
- `quotey-db`: Database layer, SQLx queries, migrations
- `quotey-slack`: Slack Socket Mode integration
- `quotey-agent`: Agent runtime, LLM orchestration
- `quotey-cli`: Command-line interface
- `quotey-server`: Binary crate that wires everything together

**Structure:**
```
quotey/
├── Cargo.toml                    # Workspace manifest
├── Cargo.lock                    # Locked dependencies
├── rustfmt.toml                  # Code formatting rules
├── deny.toml                     # Dependency auditing
├── crates/
│   ├── core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── domain/
│   │       ├── pricing/
│   │       ├── constraints/
│   │       └── policy/
│   ├── db/
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── connection.rs
│   │   │   ├── migrations/
│   │   │   └── repositories/
│   │   └── migrations/           # SQLx migration files
│   ├── slack/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── socket/
│   │       ├── events/
│   │       └── blocks/
│   ├── agent/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── runtime/
│   │       ├── tools/
│   │       └── guardrails/
│   ├── cli/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       └── commands/
│   └── server/
│       ├── Cargo.toml
│       └── src/
│           └── main.rs           # Production entry point
├── templates/                    # PDF/HTML templates
├── config/
│   └── default.toml             # Default configuration
└── tests/                       # Integration tests
    └── integration/
```

**Dependencies Strategy:**
- **Core:** No async, no I/O - pure logic (serde, thiserror, chrono, uuid)
- **DB:** sqlx with sqlite feature, async-trait
- **Slack:** slack-morphism, tokio, tokio-tungstenite
- **Agent:** async-openai (behind trait), serde_json
- **Server:** axum (for health checks), tokio, tracing
- **CLI:** clap, tracing-subscriber

### 1.2 Core Crate Foundation
**Files:**
- `crates/core/Cargo.toml`
- `crates/core/src/lib.rs`

**Key Design Decisions:**
- All domain types implement `Clone`, `Debug`, `PartialEq` for testability
- Error types use `thiserror` with `#[source]` chains
- No `std::sync` or `tokio::sync` - this crate is thread-agnostic
- All monetary values as `rust_decimal::Decimal` (never float)

**Module Structure:**
```rust
// lib.rs - explicit exports only
pub mod domain {
    pub mod quote;
    pub mod product;
    pub mod customer;
    pub mod pricing;
}
pub mod pricing {
    pub mod engine;
    pub mod trace;
}
pub mod constraints {
    pub mod engine;
    pub mod types;
}
pub mod policy {
    pub mod engine;
    pub mod rules;
}
pub mod flow {
    pub mod engine;
    pub mod states;
}

// Re-export commonly used types
pub use domain::quote::{Quote, QuoteId, QuoteLine};
pub use domain::product::{Product, ProductId};
```

### 1.3 Database Crate Foundation
**Files:**
- `crates/db/Cargo.toml`
- `crates/db/src/lib.rs`

**Key Design Decisions:**
- Single `DbPool` type alias for connection pool
- Repository pattern: one struct per aggregate root
- All queries compile-time checked with `sqlx::query!`/`sqlx::query_as!`
- Migrations embedded in binary using `sqlx::migrate!`

**Module Structure:**
```rust
// lib.rs
pub mod connection;
pub mod repositories {
    pub mod quote;
    pub mod product;
    pub mod customer;
}
pub mod migrations;

pub use connection::{DbPool, connect};
pub use repositories::quote::QuoteRepository;
```

### 1.4 Slack Crate Foundation
**Files:**
- `crates/slack/Cargo.toml`
- `crates/slack/src/lib.rs`

**Key Design Decisions:**
- Socket Mode only (no HTTP server required)
- Event handlers return `Result<(), SlackError>` - failures logged, not crashed
- Block Kit builders are type-safe (no raw JSON construction)
- Rate limiting handled internally

### 1.5 Agent Crate Foundation
**Files:**
- `crates/agent/Cargo.toml`
- `crates/agent/src/lib.rs`

**Key Design Decisions:**
- LLM behind trait: `trait LlmClient { async fn complete(&self, prompt) -> Result<String, LlmError>; }`
- Tools behind trait: `trait Tool { fn name() -> &'static str; async fn execute(&self, input) -> Result<ToolOutput, ToolError>; }`
- Agent runtime is deterministic - LLM only provides suggestions, runtime decides
- Guardrails are compile-time enforced (limited tool set, typed inputs)

### 1.6 CLI Crate Foundation
**Files:**
- `crates/cli/Cargo.toml`
- `crates/cli/src/main.rs`

**Commands to Support:**
- `quotey start` - Start the Slack bot
- `quotey migrate` - Run database migrations
- `quotey seed` - Load demo data
- `quotey config` - Show effective configuration
- `quotey doctor` - Health check

### 1.7 Server Crate Foundation
**Files:**
- `crates/server/Cargo.toml`
- `crates/server/src/main.rs`

**Responsibilities:**
- Configuration loading
- Component wiring (dependency injection)
- Graceful shutdown handling
- Health check endpoint (for monitoring)

**Verification:**
```bash
cargo build --workspace  # Should compile with no errors
cargo test --workspace   # Should pass (only stub tests exist)
cargo fmt -- --check     # Should pass
cargo clippy --workspace -- -D warnings  # Should pass
```

---

## Phase 2: Configuration & Bootstrap (Task bd-3d8.2)

### 2.1 Configuration Schema
**File:** `crates/core/src/config.rs`

**Configuration Sources (in priority order):**
1. Defaults compiled into binary
2. Config file (`config.toml` or `quotey.toml`)
3. Environment variables (`QUOTEY_*`)
4. CLI arguments

**Configuration Sections:**
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub slack: SlackConfig,
    pub llm: LlmConfig,
    pub server: ServerConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,  // sqlite://quotey.db or :memory: for tests
    pub max_connections: u32,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SlackConfig {
    pub app_token: SecretString,  // xapp-* for Socket Mode
    pub bot_token: SecretString,  // xoxb-* for API calls
    pub signing_secret: SecretString,  // For request verification
}

#[derive(Debug, Clone, Deserialize)]
pub struct LlmConfig {
    pub provider: LlmProvider,  // OpenAi, Anthropic, Ollama
    pub api_key: Option<SecretString>,
    pub base_url: Option<String>,  // For Ollama or proxies
    pub model: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub bind_address: String,  // For health checks
    pub health_check_port: u16,
    pub graceful_shutdown_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub level: String,  // trace, debug, info, warn, error
    pub format: LogFormat,  // Json, Pretty
    pub filter: Option<String>,  // tracing directive
}
```

**Secrets Handling:**
- Use `secrecy::SecretString` for all sensitive values
- Secrets are zeroed on drop (best effort)
- Secrets never appear in `Debug` output
- Secrets loaded from environment or files, never committed

### 2.2 Configuration Loading
**File:** `crates/core/src/config/loader.rs`

**Algorithm:**
1. Start with built-in defaults
2. Load config file if exists (search: `./config.toml`, `~/.quotey/config.toml`, `/etc/quotey/config.toml`)
3. Override with environment variables using ` envy ` crate
4. Override with CLI arguments
5. Validate (ensure required fields present)
6. Return frozen `Config` struct

**Validation Rules:**
- Database URL must parse as valid SQLite connection string
- Slack tokens must have correct prefixes (xapp-, xoxb-)
- LLM timeout must be > 0 and < 300 seconds
- At least one of OpenAI key or Ollama URL must be provided

**Verification:**
```rust
#[test]
fn test_config_loading() {
    let config = Config::load_from_str(r#"
        [database]
        url = "sqlite::memory:"
        
        [slack]
        app_token = "xapp-test"
        bot_token = "xoxb-test"
        signing_secret = "secret"
        
        [llm]
        provider = "ollama"
        base_url = "http://localhost:11434"
        model = "llama3.2"
    "#).unwrap();
    
    assert_eq!(config.database.url, "sqlite::memory:");
}
```

### 2.3 Bootstrap Wiring
**File:** `crates/server/src/bootstrap.rs`

**Responsibilities:**
- Initialize tracing subscriber
- Load and validate configuration
- Create database connection pool
- Run pending migrations
- Initialize LLM client based on config
- Build dependency injection container
- Return `Application` struct with all components

**Dependency Injection Pattern:**
```rust
pub struct Application {
    pub config: Arc<Config>,
    pub db_pool: DbPool,
    pub quote_repo: Arc<dyn QuoteRepository>,
    pub product_repo: Arc<dyn ProductRepository>,
    pub llm_client: Arc<dyn LlmClient>,
    pub agent_runtime: AgentRuntime,
    pub slack_client: SlackClient,
}

impl Application {
    pub async fn bootstrap(config: Config) -> Result<Self, BootstrapError> {
        // Implementation...
    }
}
```

---

## Phase 3: Database Layer (Task bd-3d8.3)

### 3.1 Base Schema Migration
**File:** `crates/db/migrations/000001_initial_schema.up.sql`

**Tables (in dependency order):**

1. **`_sqlx_migrations`** - Automatically managed by sqlx

2. **`products`** - Product catalog
3. **`product_relationships`** - Product constraints
4. **`price_books`** - Pricing segments
5. **`price_book_entries`** - Product prices
6. **`volume_tiers`** - Quantity discounts
7. **`pricing_formulas`** - Custom calculations
8. **`bundles`** - Product bundles
9. **`constraint_rules`** - Configuration validation
10. **`discount_policies`** - Approval thresholds
11. **`customers`** - Account information
12. **`deals`** - Opportunities
13. **`quotes`** - Quote headers
14. **`quote_lines`** - Line items
15. **`quote_pricing_snapshots`** - Pricing history
16. **`approvals`** - Approval requests
17. **`approval_decisions`** - Approval audit trail
18. **`audit_events`** - System audit log
19. **`slack_thread_mappings`** - Thread state
20. **`flow_states`** - Quote workflow state

**Key Schema Decisions:**
- All IDs are TEXT (UUID v4 as string) for consistency
- All timestamps are TEXT in ISO 8601 format (SQLite has no native datetime)
- All JSON columns use TEXT with CHECK(json_valid(column))
- Foreign keys enabled with PRAGMA (enforced in connection setup)
- Indexes on all foreign keys and commonly queried columns

**Migration Principles:**
- Migrations are idempotent where possible
- Down migrations provided for all changes
- Migrations run in transactions
- Schema version tracked in `_sqlx_migrations`

### 3.2 Connection Management
**File:** `crates/db/src/connection.rs`

**Implementation:**
```rust
pub type DbPool = sqlx::SqlitePool;

pub async fn connect(config: &DatabaseConfig) -> Result<DbPool, sqlx::Error> {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(config.max_connections)
        .acquire_timeout(Duration::from_secs(config.timeout_secs))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                // Enable foreign keys
                sqlx::query("PRAGMA foreign_keys = ON").execute(conn).await?;
                // Use WAL mode for better concurrency
                sqlx::query("PRAGMA journal_mode = WAL").execute(conn).await?;
                Ok(())
            })
        })
        .connect(&config.url)
        .await?;
    
    Ok(pool)
}
```

### 3.3 Repository Traits
**File:** `crates/db/src/repositories/mod.rs`

**Pattern:**
```rust
#[async_trait::async_trait]
pub trait QuoteRepository: Send + Sync {
    async fn find_by_id(&self, id: &QuoteId) -> Result<Option<Quote>, DbError>;
    async fn create(&self, quote: &Quote) -> Result<Quote, DbError>;
    async fn update(&self, quote: &Quote) -> Result<Quote, DbError>;
    async fn list_by_deal(&self, deal_id: &DealId) -> Result<Vec<Quote>, DbError>;
}

#[async_trait::async_trait]
pub trait ProductRepository: Send + Sync {
    async fn find_by_id(&self, id: &ProductId) -> Result<Option<Product>, DbError>;
    async fn search(&self, query: &str) -> Result<Vec<Product>, DbError>;
    async fn create(&self, product: &Product) -> Result<Product, DbError>;
}
```

**Mock Implementations:**
- `InMemoryQuoteRepository` for testing
- `StubProductRepository` with hardcoded products for demos

**Verification:**
```bash
cargo sqlx migrate run  # Apply migrations
cargo sqlx migrate info  # Show status
cargo test --package quotey-db  # Run DB tests
```

---

## Phase 4: Repository & Domain Traits (Task bd-3d8.4)

### 4.1 Domain Entity Definitions
**File:** `crates/core/src/domain/quote.rs`

**Quote Aggregate:**
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub id: QuoteId,
    pub deal_id: DealId,
    pub customer_id: CustomerId,
    pub status: QuoteStatus,
    pub currency: Currency,
    pub lines: Vec<QuoteLine>,
    pub valid_until: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: UserId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuoteLine {
    pub id: QuoteLineId,
    pub product_id: ProductId,
    pub quantity: u32,
    pub attributes: HashMap<String, String>,
    pub unit_price: Option<Decimal>,  // Set after pricing
    pub line_total: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuoteStatus {
    Draft,
    Validated,
    Priced,
    PendingApproval,
    Approved,
    Rejected,
    Finalized,
    Sent,
    Expired,
    Cancelled,
}
```

### 4.2 Pricing Domain
**File:** `crates/core/src/domain/pricing.rs`

**Key Types:**
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct PricingResult {
    pub subtotal: Decimal,
    pub discount_total: Decimal,
    pub tax: Decimal,
    pub total: Decimal,
    pub lines: Vec<PricedLine>,
    pub trace: PricingTrace,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PricingTrace {
    pub quote_id: QuoteId,
    pub priced_at: DateTime<Utc>,
    pub steps: Vec<PricingStep>,
}

pub struct PricingStep {
    pub step_name: &'static str,
    pub inputs: serde_json::Value,
    pub outputs: serde_json::Value,
}
```

### 4.3 Constraint Domain
**File:** `crates/core/src/domain/constraints.rs`

**Constraint Types:**
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    Requires { source: ProductId, target: ProductId },
    Excludes { source: ProductId, target: ProductId },
    Attribute { product_id: ProductId, condition: AttributeCondition },
    Quantity { product_id: ProductId, min: Option<u32>, max: Option<u32> },
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub violations: Vec<ConstraintViolation>,
}

pub struct ConstraintViolation {
    pub constraint_id: String,
    pub constraint_type: String,
    pub message: String,
    pub suggestion: Option<String>,
}
```

### 4.4 Policy Domain
**File:** `crates/core/src/domain/policy.rs`

**Policy Types:**
```rust
#[derive(Debug, Clone)]
pub enum Policy {
    DiscountCap { segment: Option<String>, max_auto: Decimal, max_with_approval: Option<Decimal> },
    MarginFloor { product_category: Option<String>, min_margin_pct: Decimal },
    DealSizeThreshold { min: Option<Decimal>, max: Option<Decimal>, approver_role: String },
}

pub struct PolicyEvaluation {
    pub status: PolicyStatus,
    pub violations: Vec<PolicyViolation>,
}

pub enum PolicyStatus {
    Pass,
    ApprovalRequired { approver_role: String },
    Blocked { reason: String },
}
```

---

## Phase 5: Slack Socket Mode (Task bd-3d8.5)

### 5.1 Socket Mode Client
**File:** `crates/slack/src/socket/client.rs`

**Implementation Approach:**
- Use `slack-morphism` crate for Socket Mode support
- Single WebSocket connection with automatic reconnection
- Event dispatch to handlers based on event type
- Rate limiting via tokio::time::Interval

**Client Structure:**
```rust
pub struct SlackSocketClient {
    config: SlackConfig,
    event_tx: mpsc::Sender<SlackEvent>,
    shutdown: tokio::sync::watch::Sender<bool>,
}

impl SlackSocketClient {
    pub async fn connect(&self) -> Result<(), SlackError> {
        // Establish WebSocket connection
        // Spawn event loop task
        // Return immediately (connection runs in background)
    }
    
    pub async fn disconnect(&self) -> Result<(), SlackError> {
        // Signal shutdown
        // Wait for clean close
    }
}
```

### 5.2 Event Handlers
**File:** `crates/slack/src/events/handlers.rs`

**Handler Pattern:**
```rust
#[async_trait]
pub trait EventHandler: Send + Sync {
    fn event_type(&self) -> SlackEventType;
    async fn handle(&self, event: &SlackEvent, ctx: &EventContext) -> Result<(), HandlerError>;
}

pub struct SlashCommandHandler {
    command_router: Arc<CommandRouter>,
}

#[async_trait]
impl EventHandler for SlashCommandHandler {
    fn event_type() -> SlackEventType { SlackEventType::SlashCommand }
    
    async fn handle(&self, event: &SlackEvent, ctx: &EventContext) -> Result<(), HandlerError> {
        let command = parse_command(event)?;
        self.command_router.route(command, ctx).await
    }
}
```

### 5.3 Command Router
**File:** `crates/slack/src/commands/router.rs`

**Commands:**
```rust
pub enum SlashCommand {
    New { customer_hint: Option<String> },
    Status { quote_id: Option<String> },
    List { filter: Option<String> },
    Help,
}

pub struct CommandRouter {
    handlers: HashMap<String, Box<dyn CommandHandler>>,
}

impl CommandRouter {
    pub fn register<H: CommandHandler>(&mut self, command: &str, handler: H) {
        self.handlers.insert(command.to_string(), Box::new(handler));
    }
    
    pub async fn route(&self, cmd: SlashCommand, ctx: &EventContext) -> Result<(), RouterError> {
        // Route to appropriate handler
    }
}
```

### 5.4 Block Kit Builders
**File:** `crates/slack/src/blocks/builders.rs`

**Type-Safe Block Kit:**
```rust
pub struct MessageBuilder {
    blocks: Vec<SlackBlock>,
    text: Option<String>,
}

impl MessageBuilder {
    pub fn new() -> Self { /* ... */ }
    
    pub fn section<F>(mut self, f: F) -> Self 
    where F: FnOnce(&mut SectionBuilder) { /* ... */ }
    
    pub fn actions<F>(mut self, f: F) -> Self
    where F: FnOnce(&mut ActionsBuilder) { /* ... */ }
    
    pub fn build(self) -> SlackMessageContent { /* ... */ }
}

// Usage:
let message = MessageBuilder::new()
    .section(|s| s.text("*Quote Ready*").mrkdwn(true))
    .actions(|a| {
        a.button("confirm", "Confirm", SlackButtonStyle::Primary);
        a.button("edit", "Edit", SlackButtonStyle::Default);
    })
    .build();
```

---

## Phase 6: Integration & Verification (Task bd-3d8.10)

### 6.1 Integration Test Suite
**File:** `tests/integration/smoke_test.rs`

**Smoke Tests:**
```rust
#[tokio::test]
async fn test_full_quote_flow() {
    // 1. Start application with in-memory database
    let app = TestApp::new().await;
    
    // 2. Create a quote via slash command
    let response = app.send_command("/quote new Acme Corp").await;
    assert!(response.contains("Quote created"));
    
    // 3. Add a line item
    let response = app.send_message("Add Pro Plan x 100").await;
    assert!(response.contains("Line added"));
    
    // 4. Run pricing
    let response = app.send_message("Price this").await;
    assert!(response.contains("Total:"));
    
    // 5. Verify database state
    let quotes = app.db().list_quotes().await.unwrap();
    assert_eq!(quotes.len(), 1);
}
```

### 6.2 Health Check Endpoint
**File:** `crates/server/src/health.rs`

**Endpoint:** `GET /health`

**Response:**
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "checks": {
    "database": "connected",
    "slack": "connected",
    "llm": "available"
  },
  "timestamp": "2026-02-23T14:30:00Z"
}
```

### 6.3 CLI Commands
**Commands to Verify:**

```bash
# Build verification
cargo build --release --workspace

# Database operations
cargo run --bin quotey-cli -- migrate
cargo run --bin quotey-cli -- seed

# Configuration
cargo run --bin quotey-cli -- config

# Health check
cargo run --bin quotey-cli -- doctor

# Start server (requires Slack tokens)
QUOTEY_SLACK_APP_TOKEN=xapp-test QUOTEY_SLACK_BOT_TOKEN=xoxb-test \
  cargo run --bin quotey-server
```

### 6.4 Verification Checklist

Before declaring Phase 6 complete:

- [ ] `cargo build --workspace` compiles with zero warnings
- [ ] `cargo test --workspace` passes all tests
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo fmt -- --check` passes
- [ ] `cargo deny check` passes (no known vulnerabilities)
- [ ] Migrations run successfully on fresh database
- [ ] Migrations are reversible (down + up = same state)
- [ ] CLI `doctor` command reports all green
- [ ] Integration test exercises full quote flow
- [ ] Documentation (`cargo doc`) builds with no warnings

---

## Appendix A: Crate Dependency Graph

```
                    ┌─────────────┐
                    │   server    │
                    └──────┬──────┘
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
        ▼                  ▼                  ▼
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   agent     │◄───│    core     │───►│     db      │
└──────┬──────┘    └─────────────┘    └──────┬──────┘
       │                                      │
       ▼                                      ▼
┌─────────────┐                       ┌─────────────┐
│   slack     │◄──────────────────────┘  sqlite    │
└─────────────┘

Legend:
──► = Depends on (uses)
◄── = Is depended on by
```

## Appendix B: File Naming Conventions

| Type | Pattern | Example |
|------|---------|---------|
| Module | `snake_case.rs` | `quote_repository.rs` |
| Trait | `TraitName` in `trait_name.rs` | `pub trait QuoteRepository` in `quote_repository.rs` |
| Struct | `PascalCase` | `pub struct QuoteService` |
| Error | `XxxError` | `pub enum PricingError` |
| Test | `xxx_test.rs` or inline `#[cfg(test)]` | `quote_service_test.rs` |
| Migration | `NNNNNN_description.up.sql` | `000001_initial_schema.up.sql` |

## Appendix C: Error Handling Strategy

**Layer 1 - Domain Errors:**
```rust
#[derive(thiserror::Error, Debug)]
pub enum PricingError {
    #[error("Product not found: {0}")]
    ProductNotFound(ProductId),
    
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("Policy violation: {violations:?}")]
    PolicyViolation { violations: Vec<PolicyViolation> },
}
```

**Layer 2 - Application Errors:**
```rust
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Domain error: {0}")]
    Domain(#[from] PricingError),
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Slack API error: {0}")]
    Slack(String),
}
```

**Layer 3 - HTTP/CLI Errors:**
```rust
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::Domain(e) => (StatusCode::BAD_REQUEST, e.to_string()),
            AppError::Database(e) => {
                tracing::error!("Database error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error")
            }
            // ...
        }.into_response()
    }
}
```

---

## Success Criteria

This phase is complete when:

1. **Build:** `cargo build --release` produces a working binary
2. **Test:** `cargo test` passes with >80% coverage
3. **Lint:** `cargo clippy` reports zero warnings
4. **Format:** `cargo fmt` makes no changes
5. **Security:** `cargo deny` reports no vulnerabilities
6. **Docs:** `cargo doc` builds with no warnings
7. **Runtime:** `quotey doctor` reports all systems green
8. **Demo:** Full quote flow works end-to-end in test environment

**Next Phase Ready When:**
- All foundation tasks closed
- Deal DNA (bd-70d.1) schema task unblocked
- Development team confident in architecture
