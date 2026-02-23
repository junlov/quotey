# Quotey Project - Comprehensive Research Document

**Document Type:** Technical Architecture & Project Research  
**Author:** ResearchAgent (kimi-k2)  
**Date:** 2026-02-23  
**Project:** Quotey - Rust-based, Local-first CPQ Agent for Slack  
**Version:** 0.1.1

---

## Executive Summary

Quotey is a Rust-based, local-first Configure-Price-Quote (CPQ) agent that operates within Slack. It addresses the market gap created by Salesforce CPQ's End-of-Sale (March 2025) by providing an agent-first, deterministic pricing solution that eliminates the traditional CPQ implementation cycle.

**Key Differentiators:**
- Agent-first natural language interaction (not form-based)
- Deterministic pricing engine with full audit trails
- Local-first deployment (SQLite + single binary)
- Catalog bootstrap from unstructured data (PDFs, CSVs, spreadsheets)
- Constraint-based configuration (not rule-based)

---

## 1. Project Architecture Overview

### 1.1 Six-Layer Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    SLACK BOT INTERFACE                          │
│  Socket Mode · Slash Commands · Thread Events · Block Kit UI    │
├─────────────────────────────────────────────────────────────────┤
│                    AGENT RUNTIME                                │
│  Intent Extraction · Slot Filling · Guardrails · Tool Registry  │
├──────────────────┬──────────────────┬───────────────────────────┤
│  DETERMINISTIC   │   CPQ CORE       │   TOOL ADAPTERS           │
│  FLOW ENGINE     │                  │                           │
│                  │  Product Catalog  │  slack.* (post/update)    │
│  State Machine   │  Constraint       │  crm.* (sync/read/write) │
│  Required Fields │    Engine         │  doc.* (render/attach)   │
│  Allowed         │  Pricing Engine   │  composio.* (REST)       │
│    Transitions   │  Policy Engine    │  catalog.* (bootstrap)   │
│                  │                  │  intelligence.*          │
├──────────────────┴──────────────────┴───────────────────────────┤
│                    SQLITE DATA STORE                            │
│  Products · Price Books · Quotes · Approvals · Audit Log        │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 Workspace Crate Structure

| Crate | Responsibility | Key Dependencies |
|-------|---------------|------------------|
| `core` | Domain models, CPQ engines, flows | None (pure logic) |
| `db` | SQLite connection, migrations, repositories | `core`, `sqlx` |
| `agent` | Runtime orchestration, LLM integration | `core`, `db` |
| `slack` | Socket mode, events, Block Kit | `core`, `slack-morphism` |
| `cli` | Operator commands (migrate, doctor, smoke) | `core`, `db`, `clap` |
| `server` | Bootstrap, health checks, process entry | All above |

**Dependency Flow:** `core` ← (`db`, `agent`, `slack`) ← `server`

---

## 2. Core Domain Architecture

### 2.1 Domain Models (`crates/core/src/domain/`)

#### Quote Domain
```rust
pub struct Quote {
    pub id: QuoteId,                    // Newtype: String wrapper
    pub status: QuoteStatus,            // 11-state lifecycle enum
    pub lines: Vec<QuoteLine>,          // Individual items
    pub created_at: DateTime<Utc>,
}

pub enum QuoteStatus {
    Draft, Validated, Priced, Approval, Approved, 
    Finalized, Sent, Expired, Cancelled, Rejected, Revised
}
```

**State Machine Transitions:**
```
Draft → Validated → Priced → (Approval) → Approved → Finalized → Sent
                  ↘ Policy Clear ───────↗
```

#### Product Domain
- `ProductId`: Newtype String wrapper
- `Product`: Simple entity with SKU, name, active flag
- Supports configurable products, bundles, and attributes

#### Customer Domain
- `CustomerId`: UUID-based identifier
- `Customer`: Segment-aware (SMB, Mid-Market, Enterprise)

#### Execution Domain
- `ExecutionTask`: Durable task with idempotency keys
- `ExecutionTaskState`: Queued → Running → Completed/Failed
- `IdempotencyRecord`: Exactly-once execution guarantee

### 2.2 CPQ Core Engines (`crates/core/src/cpq/`)

**Critical Safety Principle:** LLMs never decide prices, configuration validity, or policy compliance. Deterministic engines are always the source of truth.

#### Constraint Engine
```rust
pub trait ConstraintEngine: Send + Sync {
    fn validate(&self, input: &ConstraintInput) -> ConstraintResult;
}

pub struct ConstraintResult {
    pub valid: bool,
    pub violations: Vec<ConstraintViolation>,
}
```

**Constraint Types:**
1. **Requires** - Product A requires Product B
2. **Excludes** - Product A incompatible with Product B
3. **Attribute Constraints** - Conditional field validation
4. **Quantity Constraints** - Min/max quantity rules
5. **Bundle Constraints** - Composition rules
6. **Cross-product Constraints** - Multi-line-item validation

#### Pricing Engine
```rust
pub trait PricingEngine: Send + Sync {
    fn price(&self, quote: &Quote, currency: &str) -> PricingResult;
}

pub struct PricingResult {
    pub subtotal: Decimal,
    pub discount_total: Decimal,
    pub tax_total: Decimal,
    pub total: Decimal,
    pub approval_required: bool,
    pub trace: PricingTrace,        // Immutable audit trail
}
```

**Pricing Pipeline:**
1. Select price book(s) by segment/region/currency
2. Look up base prices
3. Apply volume tiers
4. Apply bundle discounts
5. Apply formulas
6. Apply requested discounts (capped by policy)
7. Compute subtotals
8. Apply tax (stub for v1)
9. Generate pricing trace

#### Policy Engine
```rust
pub trait PolicyEngine: Send + Sync {
    fn evaluate(&self, input: &PolicyInput) -> PolicyDecision;
}

pub struct PolicyDecision {
    pub approval_required: bool,
    pub approval_status: ApprovalStatus,
    pub reasons: Vec<String>,
    pub violations: Vec<PolicyViolation>,
}
```

**Policy Types:**
- Discount caps by segment
- Margin floors by product type
- Deal size thresholds
- Product-specific rules
- Temporal policies (EOQ rules)

### 2.3 Flow Engine (`crates/core/src/flows/`)

```rust
pub trait FlowDefinition: Send + Sync {
    fn flow_type(&self) -> FlowType;
    fn initial_state(&self) -> FlowState;
    fn transition(
        &self,
        current: FlowState,
        event: FlowEvent,
        ctx: &FlowContext,
    ) -> Result<TransitionOutcome, FlowTransitionError>;
}

pub struct FlowEngine<F: FlowDefinition> {
    definition: F,
}
```

**Net-New Flow Transitions:**
| Current State | Event | Next State | Actions |
|--------------|-------|------------|---------|
| Draft | RequiredFieldsCollected | Validated | EvaluatePricing |
| Validated | PricingCalculated | Priced | EvaluatePolicy |
| Priced | PolicyClear | Finalized | FinalizeQuote |
| Priced | PolicyViolationDetected | Approval | RouteApproval |
| Approval | ApprovalGranted | Approved | FinalizeQuote |
| Approved | QuoteDelivered | Sent | MarkQuoteSent |

---

## 3. Advanced Capabilities

### 3.1 Deal DNA / Fingerprinting (`crates/core/src/dna/`)

**Purpose:** Intelligent similarity matching for quotes using SimHash fingerprints.

```rust
pub struct ConfigurationFingerprint {
    pub simhash: u128,              // 128-bit SimHash signature
}

pub struct SimilarityEngine {
    candidates: Vec<SimilarityCandidate>,
    threshold: f64,                 // Default 0.8 (80% similar)
}

pub struct SimilarDeal {
    pub outcome: DealOutcomeMetadata,
    pub similarity_score: f64,
    pub hamming_distance: u32,
}
```

**Algorithm:**
- SimHash with FNV-1a hashing for local-sensitive hashing
- Key-order independent canonical JSON
- Sub-millisecond similarity search over 10k candidates

**Usage:** Find similar past deals for pricing intelligence and precedent analysis.

### 3.2 Immutable Quote Ledger (`crates/core/src/ledger/`)

**Purpose:** Cryptographic chain integrity for quote versions.

```rust
pub struct LedgerEntry {
    pub version: u32,
    pub quote_id: QuoteId,
    pub action: LedgerAction,
    pub content_hash: String,       // SHA-256 of quote state
    pub prev_hash: String,          // Hash of previous entry
    pub entry_hash: String,         // Hash of this entry
    pub signature: String,          // HMAC-SHA256
}

pub enum LedgerAction {
    Create,
    Update,
    Approve,
    Reject,
    Custom(String),
}
```

**Security Features:**
- SHA-256 content hashing
- Chain linking (each entry includes previous hash)
- HMAC-SHA256 signatures
- Tamper detection via chain verification

### 3.3 Ghost Quote Generator (`crates/core/src/ghost/`)

**Purpose:** Proactive draft quotes from buying signals in Slack messages.

```rust
pub struct SignalDetector {
    config: SignalDetectorConfig,
}

pub struct Signal {
    pub confidence: f64,
    pub detected_company: Option<String>,
    pub intent_keywords: Vec<String>,
    pub timeline_hint: Option<String>,
    pub competitor_mentions: Vec<String>,
}

pub struct GhostQuote {
    pub confidence: f64,
    pub draft_quote: Quote,
    pub suggested_discount_pct: Option<u8>,
}
```

**Signal Detection:**
- Keywords: budget, expand, evaluating, pricing, renewal
- Company extraction: Inc, Corp, LLC, Ltd, GmbH suffixes
- Timeline detection: Q1-Q4, this/next quarter
- Competitor mentions: Salesforce, HubSpot, Oracle, SAP

### 3.4 Archaeology / Dependency Graph (`crates/core/src/archaeology/`)

**Purpose:** Graph analysis for configuration blockages and resolution paths.

```rust
pub struct DependencyGraph {
    pub nodes: Vec<DependencyNode>,
    pub edges: Vec<DependencyEdge>,
}

pub enum ConstraintEdgeType {
    Requires,
    Excludes,
    Alternative,
}

pub struct DependencyGraphEngine;
```

**Capabilities:**
- BFS/DFS chain detection
- Shortest enablement path finding
- Root cause analysis for blockages
- Alternative product suggestions

### 3.5 Operational Transform (`crates/core/src/collab/`)

**Purpose:** Conflict resolution for concurrent quote edits.

```rust
pub struct OperationalTransform;

pub struct QuoteOperation {
    pub op_type: OperationType,
    pub target_product_id: ProductId,
    pub authority: OperationAuthority,
}

pub struct TransformResult {
    pub applied: Vec<QuoteOperation>,
    pub overridden: Vec<QuoteOperation>,
    pub rejected: Vec<QuoteOperation>,
}
```

**Conflict Resolution:** Authority-based precedence (rank → timestamp → id).

### 3.6 Policy Explanation Generator (`crates/core/src/policy/`)

**Purpose:** Human-readable explanations for policy violations.

```rust
pub struct ExplanationTemplate {
    pub rule_id: String,
    pub default_template: String,
    pub role_templates: HashMap<String, String>,
    pub resolution_paths: Vec<String>,
}

pub struct GeneratedExplanation {
    pub citation: String,
    pub summary: String,
    pub resolution_paths: Vec<String>,
}
```

---

## 4. Database Layer (`crates/db/`)

### 4.1 Connection Management

```rust
pub type DbPool = sqlx::SqlitePool;

pub async fn connect(database_url: &str) -> Result<DbPool, sqlx::Error>
pub async fn connect_with_settings(
    database_url: &str,
    max_connections: u32,
    timeout_secs: u64,
) -> Result<DbPool, sqlx::Error>
```

**SQLite Configuration:**
- `PRAGMA foreign_keys = ON`
- `PRAGMA journal_mode = WAL`
- `PRAGMA busy_timeout = 5000`

### 4.2 Migration System

- 6 migration files covering full schema
- 40+ tables including: quote, quote_line, flow_state, audit_event, emoji_approvals, execution_queue_task
- Comprehensive test coverage for reversibility

### 4.3 Repository Pattern

| Repository | Purpose | Implementation |
|------------|---------|----------------|
| `QuoteRepository` | Quote CRUD | SQL stub (placeholder) |
| `ProductRepository` | Product lookup | SQL stub (placeholder) |
| `ApprovalRepository` | Approval requests | SQL stub (placeholder) |
| `ExecutionQueueRepository` | Task queue | **Full SQL implementation** |
| `IdempotencyRepository` | Deduplication | **Full SQL implementation** |
| `CustomerRepository` | Customer data | SQL stub (placeholder) |

**In-Memory Implementations:** Available for testing via `InMemory*Repository` types.

---

## 5. Agent Runtime (`crates/agent/`)

### 5.1 Runtime Orchestration

```rust
pub struct AgentRuntime {
    guardrails: GuardrailPolicy,
}

impl AgentRuntime {
    pub async fn handle_thread_message(&self, text: &str) -> Result<String>
}
```

### 5.2 Intent Extraction (`src/conversation.rs`)

```rust
pub struct ExtractedIntent {
    pub product_mentions: Vec<String>,
    pub quantity_mentions: Vec<u32>,
    pub budget_cents: Option<i64>,
    pub timeline_hint: Option<String>,
    pub requested_discount_pct: Option<u8>,
    pub confidence_score: u8,
}

pub struct IntentExtractor;
pub struct ConstraintMapper<E: ConstraintEngine>;
```

**Extraction Capabilities:**
- Product name matching from aliases
- Budget parsing ("$50k" → 5,000,000 cents)
- Quantity detection
- Timeline hints
- Discount requests

### 5.3 Guardrails (`src/guardrails.rs`)

```rust
pub struct GuardrailPolicy {
    pub llm_can_set_prices: bool,       // Default: false
    pub llm_can_approve_discounts: bool, // Default: false
}
```

**Safety Enforcement:** Prevents LLM from making business decisions.

### 5.4 Tools (`src/tools.rs`)

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    async fn execute(&self, input: Value) -> Result<Value>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}
```

---

## 6. Slack Integration (`crates/slack/`)

### 6.1 Socket Mode (`src/socket.rs`)

```rust
#[async_trait]
pub trait SocketTransport: Send + Sync {
    async fn connect(&self) -> Result<(), TransportError>;
    async fn next_envelope(&self) -> Result<Option<SlackEnvelope>, TransportError>;
    async fn acknowledge(&self, envelope_id: &str) -> Result<(), TransportError>;
}

pub struct SocketModeRunner {
    transport: Arc<dyn SocketTransport>,
    dispatcher: EventDispatcher,
    reconnect_policy: ReconnectPolicy,
}
```

**Features:**
- Exponential backoff reconnect
- Correlation ID extraction
- Graceful degradation

### 6.2 Commands (`src/commands.rs`)

```rust
pub enum QuoteCommand {
    New { customer_hint: Option<String>, freeform_args: String },
    Status { quote_id: Option<String>, freeform_args: String },
    List { filter: Option<String> },
    Help,
}

#[async_trait]
pub trait QuoteCommandService: Send + Sync {
    async fn new_quote(&self, ...) -> Result<MessageTemplate, CommandRouteError>;
    async fn status_quote(&self, ...) -> Result<MessageTemplate, CommandRouteError>;
    async fn list_quotes(&self, ...) -> Result<MessageTemplate, CommandRouteError>;
}
```

### 6.3 Events (`src/events.rs`)

| Handler | Event Type | Purpose |
|---------|-----------|---------|
| `SlashCommandHandler` | SlashCommand | Routes `/quote` commands |
| `ThreadMessageHandler` | ThreadMessage | Processes thread replies |
| `ReactionAddedHandler` | ReactionAdded | Emoji approvals (👍/👎/💬) |

**Emoji Approval Support:**
- 👍 / +1 / thumbsup → Approve
- 👎 / -1 / thumbsdown → Reject
- 💬 / speech_balloon → Discuss

### 6.4 Block Kit Builders (`src/blocks.rs`)

```rust
pub struct MessageBuilder { ... }
pub struct SectionBuilder { ... }
pub struct ActionsBuilder { ... }

pub fn quote_status_message(quote_id: &str, status: QuoteStatus) -> MessageTemplate
pub fn approval_request_message(quote_id: &str, approver_role: &str) -> MessageTemplate
pub fn error_message(summary: &str, correlation_id: &str) -> MessageTemplate
```

---

## 7. CLI Interface (`crates/cli/`)

### 7.1 Commands

| Command | Purpose | Key Checks |
|---------|---------|------------|
| `start` | Preflight before server | Config, DB, migrations |
| `migrate` | Apply migrations | Config, DB, SQLx migrate |
| `seed` | Load demo fixtures | Config, DB (no-op currently) |
| `smoke` | E2E validation | Config, Slack tokens, DB |
| `config` | Inspect config | Shows value + source |
| `doctor` | Readiness checks | Full system health |

### 7.2 Output Format

```json
{
  "command": "doctor",
  "status": "ok|error",
  "error_class": "ConfigError|DatabaseError|...",
  "message": "..."
}
```

---

## 8. Server Bootstrap (`crates/server/`)

### 8.1 Bootstrap Sequence

```rust
pub async fn bootstrap(options: LoadOptions) -> Result<Application, BootstrapError> {
    // 1. Load configuration
    let config = AppConfig::load(options)?;
    
    // 2. Connect to database
    let db_pool = connect_with_settings(...).await?;
    
    // 3. Run migrations
    migrations::run_pending(&db_pool).await?;
    
    // 4. Create agent runtime
    let agent_runtime = AgentRuntime::new(GuardrailPolicy::default());
    
    // 5. Create Slack runner
    let slack_runner = SocketModeRunner::default();
    
    Ok(Application { config, db_pool, agent_runtime, slack_runner })
}
```

### 8.2 Health Checks (`src/health.rs`)

```rust
pub async fn spawn(bind_address: &str, port: u16, db_pool: DbPool) -> std::io::Result<()>

// GET /health
// Returns 200 if DB reachable, 503 if not
```

### 8.3 Main Entry (`src/main.rs`)

1. Initialize tracing
2. Bootstrap application
3. Spawn health endpoint
4. Start Slack runner
5. Wait for shutdown signal

---

## 9. Configuration System (`crates/core/src/config.rs`)

### 9.1 Load Precedence

1. Built-in defaults
2. Optional TOML config file
3. `QUOTEY_*` environment variables
4. CLI/runtime overrides

### 9.2 Configuration Sections

```rust
pub struct AppConfig {
    pub database: DatabaseConfig,   // SQLite URL, connections, timeout
    pub slack: SlackConfig,         // App token (xapp-), Bot token (xoxb-)
    pub llm: LlmConfig,             // Provider, API key, model, timeout
    pub server: ServerConfig,       // Bind address, health port
    pub logging: LoggingConfig,     // Level, format
}
```

### 9.3 Environment Interpolation

Config files support `${ENV_VAR}` syntax:
```toml
[slack]
app_token = "${SLACK_APP_TOKEN}"
bot_token = "${SLACK_BOT_TOKEN}"
```

### 9.4 Validation

- Database URL must be SQLite
- Slack tokens must have correct prefixes
- LLM provider requires appropriate credentials
- Ports must be non-zero

---

## 10. Error Handling (`crates/core/src/errors.rs`)

### 10.1 Layered Error Taxonomy

| Layer | Error Type | Purpose |
|-------|-----------|---------|
| Domain | `DomainError` | InvalidQuoteTransition, FlowTransition, InvariantViolation |
| Application | `ApplicationError` | Domain, Persistence, Integration, Configuration |
| Interface | `InterfaceError` | BadRequest (400), ServiceUnavailable (503), Internal (500) |

### 10.2 Conversion Flow

```
DomainError → ApplicationError → InterfaceError
                   ↓
            into_interface(correlation_id)
```

### 10.3 User-Safe Messages

Interface errors contain messages safe to display to users.

---

## 11. Audit System (`crates/core/src/audit.rs`)

### 11.1 Event Categories

```rust
pub enum AuditCategory {
    Ingress,    // Slack events, API calls
    Flow,       // State transitions
    Pricing,    // Price calculations
    Policy,     // Policy evaluations
    Persistence, // DB operations
    System,     // Startup, errors
}
```

### 11.2 Event Structure

```rust
pub struct AuditEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub actor: String,
    pub actor_type: ActorType,
    pub quote_id: Option<QuoteId>,
    pub event_type: String,
    pub category: AuditCategory,
    pub payload: BTreeMap<String, String>,
    pub metadata: BTreeMap<String, String>,
}
```

### 11.3 Correlation Tracking

All events include:
- `quote_id` - Links to specific quote
- `thread_id` - Links to Slack thread
- `correlation_id` - Request-scoped trace ID

---

## 12. Current Work Status

### 12.1 Active Development

**Epic bd-271: Power Capabilities Wave 1**
- 10 feature tracks for pragmatic CPQ differentiators
- Currently in_progress: bd-271.1 (Resilient Execution Queue)

**Active Agents:**
1. ChartreusePond (codex-cli, gpt-5-codex)
2. IvoryBear (codex-cli, gpt-5)
3. IvoryLake (codex-cli, gpt-5)
4. JadeDesert (codex-cli, gpt-5)
5. LavenderIsland (codex-cli, gpt-5)
6. LilacMountain (codex-cli, gpt-5-codex)
7. ResearchAgent (kimi-cli, kimi-k2) ← **This agent**
8. SageDeer (codex-cli, gpt-5)

### 12.2 Recent Commits

```
86f7f2c docs(planning): add REL execution queue spec and guardrails
9a3a75c feat(core): add ghost quote draft generator
2fc7dae feat(core): add ghost quote buying-signal detector
bc9a87e feat(core): add policy explanation generator service
a10a821 feat(core): add cryptographic ledger hash chain service
9790eac feat(core): add operational transform engine for quote edits
976b681 feat(core): add dependency graph traversal engine
```

### 12.3 In-Progress Beads

| ID | Title | Priority | Status |
|----|-------|----------|--------|
| bd-271.1 | W1 [REL] Resilient Execution Queue | 0 | in_progress |
| bd-70d.10.2 | ROUTE-002: Build smart routing engine | 1 | in_progress |
| bd-271.3 | W1 [FIX] Constraint Auto-Repair | 1 | in_progress |

---

## 13. Technical Debt & Gaps

### 13.1 Placeholder Implementations

The following repositories have stub implementations and need full SQL:
- `SqlQuoteRepository`
- `SqlProductRepository`
- `SqlApprovalRepository`
- `SqlCustomerRepository`

### 13.2 LLM Provider

The `LlmClient` trait exists but no concrete implementations (OpenAI, Anthropic, Ollama) are present.

### 13.3 Slack Transport

Currently uses `NoopSocketTransport`. Real `slack-morphism` integration needed.

### 13.4 CRM Integration

Stub CRM adapter exists but Composio REST client not implemented.

---

## 14. Testing Strategy

### 14.1 Current Coverage

| Area | Test Type | Coverage |
|------|-----------|----------|
| Config | Unit | Comprehensive (precedence, validation, interpolation) |
| Database | Integration | Migration reversibility, schema verification |
| Execution Queue | Integration | Round-trip, state transitions |
| Flows | Unit | State machine transitions |
| CPQ Engines | Unit | Constraint, pricing, policy validation |

### 14.2 Quality Gates

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
ubs --diff
```

---

## 15. Deployment Model

### 15.1 Target Artifacts

1. Single Rust binary (`quotey`)
2. SQLite database file (created on first run)
3. Configuration file (`quotey.toml`)
4. HTML templates for PDF generation
5. Demo fixture CSVs (optional)

### 15.2 Installation Steps

1. Download binary for platform
2. Create Slack app with Socket Mode
3. Configure tokens in `quotey.toml`
4. Run `quotey migrate`
5. Run `quotey seed-demo` (optional)
6. Run `quotey start`

### 15.3 Runtime Requirements

- No public URL needed (Socket Mode)
- No cloud deployment required
- Runs on laptop/desktop
- SQLite (embedded)

---

## 16. Market Context

### 16.1 Salesforce CPQ End-of-Sale

- EoS announced: March 2025
- Projected EOL: 2029-2030
- Migration to Revenue Cloud: 18-24 months
- Market window: 3 years for alternatives

### 16.2 Competitive Position

| Competitor | Strength | Quotey Advantage |
|------------|----------|------------------|
| Salesforce CPQ | CRM integration | No lock-in, local-first, agent-first UX |
| DealHub | Fast implementation | Constraint-based config, open architecture |
| Conga/Apttus | Document generation | Catalog bootstrap, NL interaction |
| PROS | ML pricing | Local simplicity, open LLM layer |
| Tacton | Constraint engine | Full CPQ flow, agent-first UX |

### 16.3 Agent-First Differentiation

1. **Catalog Bootstrap** - Ingest unstructured data (PDFs, CSVs, spreadsheets)
2. **Quote Intelligence** - Parse RFPs/emails to pre-populate quotes
3. **Natural Language** - Express intent, not fill forms
4. **Deterministic Core** - LLMs translate, engines decide

---

## 17. References

### 17.1 Key Documentation

- `/data/projects/quotey/AGENTS.md` - Agent execution instructions
- `/data/projects/quotey/.planning/PROJECT.md` - Detailed architecture spec
- `/data/projects/quotey/.planning/config.json` - Planning configuration
- `/data/projects/quotey/README.md` - Quick start guide

### 17.2 Configuration Example

```toml
[database]
url = "sqlite://quotey.db"
max_connections = 5
timeout_secs = 30

[slack]
app_token = "${SLACK_APP_TOKEN}"
bot_token = "${SLACK_BOT_TOKEN}"

[llm]
provider = "ollama"
base_url = "http://localhost:11434"
model = "llama3.1"
timeout_secs = 30
max_retries = 2

[server]
bind_address = "127.0.0.1"
health_check_port = 8080
graceful_shutdown_secs = 15

[logging]
level = "info"
format = "pretty"
```

---

## 18. Research Notes

### 18.1 Key Design Decisions

1. **Rust for Performance & Safety** - Single binary, memory safety
2. **SQLite for Local-First** - Zero-ops, portable, inspectable
3. **Slack Socket Mode** - No infrastructure, runs on laptop
4. **Constraint vs Rules-Based** - Better scalability (O(N) vs O(N²))
5. **LLM as Translator** - Never source of truth for pricing
6. **Immutable Audit Trail** - Cryptographic verification

### 18.2 Safety Principles

- LLMs extract intent, not decide prices
- Deterministic engines validate all configurations
- Pricing traces prove calculation correctness
- Approval workflows route, LLMs don't approve
- All state changes are append-only

### 18.3 Next Research Areas

1. Composio REST API integration details
2. PDF generation pipeline (wkhtmltopdf integration)
3. Complete SQL repository implementations
4. LLM provider implementations
5. Slack Socket Mode transport implementation
6. End-to-end flow testing scenarios

---

*Document compiled by ResearchAgent (kimi-k2) for the quotey project.*
*Last updated: 2026-02-23*
