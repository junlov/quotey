# Quotey Technical Specification

**Document Type:** Technical Specification & API Reference  
**Author:** ResearchAgent (kimi-k2)  
**Date:** 2026-02-23  
**Status:** Research Document

---

## 1. API Specifications

### 1.1 Core Domain APIs

#### Quote Lifecycle API

```rust
// crates/core/src/domain/quote.rs

pub struct QuoteId(String);
pub struct QuoteLine {
    pub product_id: ProductId,
    pub quantity: u32,
    pub unit_price: Option<Decimal>,
    pub discount_pct: Decimal,
    pub attributes_json: Option<String>,
}

pub struct Quote {
    pub id: QuoteId,
    pub status: QuoteStatus,
    pub lines: Vec<QuoteLine>,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Quote {
    /// Validates if a state transition is legal
    pub fn can_transition_to(&self, target: QuoteStatus) -> bool;
    
    /// Performs state transition with validation
    pub fn transition_to(&mut self, target: QuoteStatus) -> Result<(), DomainError>;
    
    /// Calculates subtotal from line items
    pub fn calculate_subtotal(&self) -> Decimal;
}
```

#### Product Catalog API

```rust
// crates/core/src/domain/product.rs

pub struct ProductId(String);

pub struct Product {
    pub id: ProductId,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub product_type: ProductType,  // Simple, Configurable, Bundle
    pub attributes_json: Option<String>,
    pub active: bool,
}

pub enum ProductType {
    Simple,
    Configurable,
    Bundle,
}
```

### 1.2 CPQ Engine APIs

#### Constraint Engine

```rust
// crates/core/src/cpq/constraints.rs

pub trait ConstraintEngine: Send + Sync {
    fn validate(&self, input: &ConstraintInput) -> ConstraintResult;
}

pub struct ConstraintInput<'a> {
    pub quote: &'a Quote,
    pub catalog: &'a dyn Catalog,
}

pub struct ConstraintResult {
    pub valid: bool,
    pub violations: Vec<ConstraintViolation>,
}

pub struct ConstraintViolation {
    pub code: String,
    pub message: String,
    pub suggestion: Option<String>,
}

// Implementation
pub struct DeterministicConstraintEngine;
impl ConstraintEngine for DeterministicConstraintEngine {
    fn validate(&self, input: &ConstraintInput) -> ConstraintResult {
        // Validates: minimum one line item
        // Future: dependency rules, exclusion rules, attribute validation
    }
}
```

#### Pricing Engine

```rust
// crates/core/src/cpq/pricing.rs

pub trait PricingEngine: Send + Sync {
    fn price(&self, quote: &Quote, currency: &str) -> PricingResult;
}

pub struct PricingResult {
    pub subtotal: Decimal,
    pub discount_total: Decimal,
    pub tax_total: Decimal,
    pub total: Decimal,
    pub approval_required: bool,
    pub trace: PricingTrace,
}

pub struct PricingTrace {
    pub steps: Vec<PricingTraceStep>,
    pub priced_at: DateTime<Utc>,
}

pub struct PricingTraceStep {
    pub stage: String,
    pub detail: String,
    pub amount: Option<Decimal>,
}

// Implementation
pub struct DeterministicPricingEngine<C: Catalog> {
    catalog: C,
}

impl<C: Catalog> PricingEngine for DeterministicPricingEngine<C> {
    fn price(&self, quote: &Quote, currency: &str) -> PricingResult {
        // Pipeline:
        // 1. Look up base prices from catalog
        // 2. Apply volume tiers
        // 3. Apply bundle discounts
        // 4. Apply formulas
        // 5. Apply requested discounts (capped by policy)
        // 6. Compute totals
        // 7. Generate trace
    }
}
```

#### Policy Engine

```rust
// crates/core/src/cpq/policy.rs

pub trait PolicyEngine: Send + Sync {
    fn evaluate(&self, input: &PolicyInput) -> PolicyDecision;
}

pub struct PolicyInput {
    pub requested_discount_pct: Option<Decimal>,
    pub deal_value: Decimal,
    pub minimum_margin_pct: Option<Decimal>,
    pub customer_segment: Option<String>,
}

pub struct PolicyDecision {
    pub approval_required: bool,
    pub approval_status: ApprovalStatus,
    pub reasons: Vec<String>,
    pub violations: Vec<PolicyViolation>,
}

pub enum ApprovalStatus {
    AutoApproved,
    ApprovalRequired,
    Denied,
}

// Implementation
pub struct DeterministicPolicyEngine;
impl PolicyEngine for DeterministicPolicyEngine {
    fn evaluate(&self, input: &PolicyInput) -> PolicyDecision {
        // Rules:
        // - Discounts ≤ 20%: Auto-approved
        // - Discounts > 20%: Requires sales_manager approval
    }
}
```

### 1.3 Flow Engine APIs

```rust
// crates/core/src/flows/engine.rs

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

impl<F: FlowDefinition> FlowEngine<F> {
    /// Pure function for state transition
    pub fn apply(&self, current: FlowState, event: FlowEvent, ctx: &FlowContext) 
        -> Result<TransitionOutcome, FlowTransitionError>;
    
    /// Transition with audit logging
    pub async fn apply_with_audit<S: AuditSink>(
        &self,
        current: FlowState,
        event: FlowEvent,
        ctx: &FlowContext,
        sink: &S,
    ) -> Result<TransitionOutcome, FlowTransitionError>;
    
    /// Run full CPQ evaluation
    pub fn evaluate_cpq(
        &self,
        quote: &Quote,
        cpq: &dyn CpqRuntime,
    ) -> Result<CpqEvaluation, CpqError>;
}
```

### 1.4 Agent Runtime APIs

```rust
// crates/agent/src/runtime.rs

pub struct AgentRuntime {
    guardrails: GuardrailPolicy,
}

impl AgentRuntime {
    pub fn new(guardrails: GuardrailPolicy) -> Self;
    
    /// Main entry point for thread message handling
    pub async fn handle_thread_message(&self, text: &str) -> Result<String>;
}

// crates/agent/src/conversation.rs

pub struct IntentExtractor;
impl IntentExtractor {
    pub fn extract(text: &str) -> ExtractedIntent;
}

pub struct ConstraintMapper<E: ConstraintEngine> {
    engine: E,
    catalog: Vec<CatalogItem>,
}

impl<E: ConstraintEngine> ConstraintMapper<E> {
    pub fn map(&self, intent: &ExtractedIntent) -> ConstraintSet;
}
```

### 1.5 Slack Interface APIs

```rust
// crates/slack/src/socket.rs

#[async_trait]
pub trait SocketTransport: Send + Sync {
    async fn connect(&self) -> Result<(), TransportError>;
    async fn next_envelope(&self) -> Result<Option<SlackEnvelope>, TransportError>;
    async fn acknowledge(&self, envelope_id: &str) -> Result<(), TransportError>;
    async fn disconnect(&self) -> Result<(), TransportError>;
}

pub struct SocketModeRunner {
    transport: Arc<dyn SocketTransport>,
    dispatcher: EventDispatcher,
    reconnect_policy: ReconnectPolicy,
}

impl SocketModeRunner {
    pub fn new(
        transport: Arc<dyn SocketTransport>,
        dispatcher: EventDispatcher,
        reconnect_policy: ReconnectPolicy,
    ) -> Self;
    
    pub async fn start(&self) -> Result<(), TransportError>;
}

// crates/slack/src/commands.rs

#[async_trait]
pub trait QuoteCommandService: Send + Sync {
    async fn new_quote(
        &self,
        customer_hint: Option<String>,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;
    
    async fn status_quote(
        &self,
        quote_id: Option<String>,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;
    
    async fn list_quotes(
        &self,
        filter: Option<String>,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;
}
```

### 1.6 Database Repository APIs

```rust
// crates/db/src/repositories/mod.rs

#[async_trait]
pub trait QuoteRepository: Send + Sync {
    async fn find_by_id(&self, id: &QuoteId) -> Result<Option<Quote>, RepositoryError>;
    async fn save(&self, quote: Quote) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait ProductRepository: Send + Sync {
    async fn find_by_id(&self, id: &ProductId) -> Result<Option<Product>, RepositoryError>;
    async fn find_by_sku(&self, sku: &str) -> Result<Option<Product>, RepositoryError>;
    async fn search(&self, query: &str) -> Result<Vec<Product>, RepositoryError>;
}

#[async_trait]
pub trait ExecutionQueueRepository: Send + Sync {
    async fn find_task_by_id(&self, id: &ExecutionTaskId) -> Result<Option<ExecutionTask>, RepositoryError>;
    async fn list_tasks_for_quote(
        &self,
        quote_id: &QuoteId,
        state: Option<ExecutionTaskState>,
    ) -> Result<Vec<ExecutionTask>, RepositoryError>;
    async fn save_task(&self, task: ExecutionTask) -> Result<(), RepositoryError>;
    async fn append_transition(&self, transition: ExecutionTransitionEvent) -> Result<(), RepositoryError>;
}
```

---

## 2. Data Models

### 2.1 Database Schema Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     REFERENCE DATA                              │
├─────────────────────────────────────────────────────────────────┤
│ product           - Product catalog                             │
│ product_relationship - Constraints (requires/excludes)          │
│ price_book        - Named price collections                     │
│ price_book_entry  - Individual product prices                   │
│ volume_tier       - Quantity-based price breaks                 │
│ pricing_formula   - Custom calculation expressions              │
│ bundle            - Product groupings                           │
│ constraint_rule   - Configuration validation rules              │
│ discount_policy   - Business rules for discounts                │
│ approval_threshold - Multi-level approval routing               │
├─────────────────────────────────────────────────────────────────┤
│                   DEALS & CUSTOMERS                             │
├─────────────────────────────────────────────────────────────────┤
│ account           - Customer organizations                      │
│ contact           - People at customer orgs                     │
│ deal              - Sales opportunities                         │
├─────────────────────────────────────────────────────────────────┤
│                      QUOTES                                     │
├─────────────────────────────────────────────────────────────────┤
│ quote             - Primary work product                        │
│ quote_line        - Individual line items                       │
│ quote_pricing_snapshot - Immutable pricing records              │
├─────────────────────────────────────────────────────────────────┤
│                   WORKFLOW & APPROVALS                          │
├─────────────────────────────────────────────────────────────────┤
│ flow_state        - Current position in workflow                │
│ approval_request  - Multi-level approval tracking               │
│ approval_chain    - Sequential/parallel approval chains         │
├─────────────────────────────────────────────────────────────────┤
│                AUDIT & OBSERVABILITY                            │
├─────────────────────────────────────────────────────────────────┤
│ audit_event       - Complete action history                     │
│ slack_thread_map  - Quote-to-Slack thread linkage               │
│ crm_sync_state    - Incremental sync tracking                   │
│ llm_interaction_log - All LLM calls                             │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 Key Entity Relationships

```
quote (1) ────(*) quote_line
   │
   ├────(1) flow_state
   ├────(*) approval_request
   ├────(*) quote_pricing_snapshot
   │
   └────(*) deal (via deal_id)

deal (1) ─────(1) account
account (1) ───(*) contact

product (1) ───(*) price_book_entry ───(1) price_book
product (1) ───(*) product_relationship ───(1) product
```

### 2.3 JSON Schema Examples

#### Pricing Trace JSON

```json
{
  "quote_id": "Q-2026-0042",
  "priced_at": "2026-02-23T14:30:00Z",
  "currency": "USD",
  "price_book_id": "pb_enterprise_us",
  "lines": [
    {
      "line_id": "ql_001",
      "product_id": "plan_pro_v2",
      "quantity": 150,
      "base_unit_price": 10.00,
      "volume_tier_applied": {
        "tier": "100+",
        "tier_unit_price": 8.00
      },
      "formula_applied": {
        "formula_id": "f_annual",
        "expression": "unit_price * quantity * (term_months / 12)",
        "inputs": {"unit_price": 8.00, "quantity": 150, "term_months": 12},
        "result": 14400.00
      },
      "discount_applied": {
        "type": "percentage",
        "requested": 10.0,
        "authorized": 10.0,
        "amount": 1440.00,
        "policy_check": "PASS (10% <= 15% segment cap)"
      },
      "line_total": 12960.00
    }
  ],
  "subtotal": 12960.00,
  "discount_total": 1440.00,
  "tax": {"rate": 0.0, "amount": 0.00},
  "total": 12960.00,
  "approval_required": false
}
```

#### Policy Evaluation JSON

```json
{
  "quote_id": "Q-2026-0042",
  "evaluated_at": "2026-02-23T14:30:01Z",
  "status": "APPROVAL_REQUIRED",
  "policies_evaluated": 12,
  "policies_passed": 11,
  "violations": [
    {
      "policy_id": "pol_discount_cap_smb",
      "policy_name": "SMB Discount Cap",
      "severity": "approval_required",
      "description": "15% discount exceeds 10% cap for SMB segment",
      "threshold": 10.0,
      "actual": 15.0,
      "required_approver_role": "sales_manager",
      "suggested_action": "Request sales manager approval or reduce discount"
    }
  ]
}
```

---

## 3. Configuration Specification

### 3.1 Configuration File Format

```toml
# quotey.toml

[general]
database_path = "quotey.db"
log_level = "info"  # trace, debug, info, warn, error

[database]
url = "sqlite://quotey.db"
max_connections = 5
timeout_secs = 30

[slack]
app_token = "${SLACK_APP_TOKEN}"  # Must start with xapp-
bot_token = "${SLACK_BOT_TOKEN}"  # Must start with xoxb-

[slack.channels]
deal_desk = "#deal-desk"
notifications = "#sales-ops"

[llm]
provider = "anthropic"  # openai, anthropic, ollama, mock

[llm.openai]
api_key = "${OPENAI_API_KEY}"
model = "gpt-4o"

[llm.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
model = "claude-sonnet-4-20250514"

[llm.ollama]
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
format = "pretty"  # pretty, json

[crm]
provider = "stub"  # stub, composio
sync_interval_seconds = 300

[crm.stub]
fixtures_path = "config/demo_fixtures"

[crm.composio]
api_key = "${COMPOSIO_API_KEY}"
default_integration = "hubspot"

[quotes]
default_currency = "USD"
default_valid_days = 30
default_payment_terms = "net_30"
id_prefix = "Q"

[approvals]
auto_escalation_hours = 4
max_approval_chain_length = 5
reminder_interval_hours = 2

[pdf]
converter = "wkhtmltopdf"
converter_path = "/usr/local/bin/wkhtmltopdf"
default_template = "standard"
output_dir = "output/quotes"

[catalog_bootstrap]
require_human_review = true
max_batch_size = 100
```

### 3.2 Environment Variable Overrides

| Variable | Overrides | Example |
|----------|-----------|---------|
| `QUOTEY_DATABASE_URL` | `database.url` | `sqlite://quotey.db` |
| `QUOTEY_DATABASE_MAX_CONNECTIONS` | `database.max_connections` | `5` |
| `QUOTEY_DATABASE_TIMEOUT_SECS` | `database.timeout_secs` | `30` |
| `QUOTEY_SLACK_APP_TOKEN` | `slack.app_token` | `xapp-...` |
| `QUOTEY_SLACK_BOT_TOKEN` | `slack.bot_token` | `xoxb-...` |
| `QUOTEY_LLM_PROVIDER` | `llm.provider` | `ollama` |
| `QUOTEY_LLM_API_KEY` | `llm.api_key` | `sk-...` |
| `QUOTEY_LLM_BASE_URL` | `llm.base_url` | `http://localhost:11434` |
| `QUOTEY_LLM_MODEL` | `llm.model` | `llama3.1` |
| `QUOTEY_LLM_TIMEOUT_SECS` | `llm.timeout_secs` | `30` |
| `QUOTEY_LLM_MAX_RETRIES` | `llm.max_retries` | `2` |
| `QUOTEY_SERVER_BIND_ADDRESS` | `server.bind_address` | `127.0.0.1` |
| `QUOTEY_SERVER_HEALTH_CHECK_PORT` | `server.health_check_port` | `8080` |
| `QUOTEY_LOGGING_LEVEL` | `logging.level` | `info` |
| `QUOTEY_LOGGING_FORMAT` | `logging.format` | `pretty` |

### 3.3 Validation Rules

```rust
pub fn validate_database(database: &DatabaseConfig) -> Result<(), ConfigError> {
    // URL must start with sqlite://, sqlite::, or be :memory:
    // max_connections must be > 0
    // timeout_secs must be in range 1..=300
}

pub fn validate_slack(slack: &SlackConfig) -> Result<(), ConfigError> {
    // app_token must start with xapp-
    // bot_token must start with xoxb-
}

pub fn validate_llm(llm: &LlmConfig) -> Result<(), ConfigError> {
    // timeout_secs must be in range 1..=300
    // openai/anthropic require api_key
    // ollama requires base_url
}
```

---

## 4. Error Handling Specification

### 4.1 Error Taxonomy

```
DomainError
├── InvalidQuoteTransition { from: QuoteStatus, to: QuoteStatus }
├── FlowTransition { flow_type: FlowType, reason: String }
└── InvariantViolation { description: String }

ApplicationError
├── Domain(DomainError)
├── Persistence { operation: String, source: String }
├── Integration { service: String, source: String }
└── Configuration(ConfigError)

InterfaceError
├── BadRequest { correlation_id: String, message: String }
├── ServiceUnavailable { correlation_id: String, message: String }
└── Internal { correlation_id: String, message: String }
```

### 4.2 Error Conversion Mapping

| Source Error | Interface Error | HTTP Status |
|-------------|-----------------|-------------|
| `DomainError::InvalidQuoteTransition` | `BadRequest` | 400 |
| `DomainError::FlowTransition` | `BadRequest` | 400 |
| `ApplicationError::Domain` | `BadRequest` | 400 |
| `ApplicationError::Persistence` | `ServiceUnavailable` | 503 |
| `ApplicationError::Integration` | `ServiceUnavailable` | 503 |
| `ApplicationError::Configuration` | `Internal` | 500 |
| `ConfigError` | `Internal` | 500 |

### 4.3 Error Response Format

```json
{
  "correlation_id": "550e8400-e29b-41d4-a716-446655440000",
  "error": {
    "type": "BadRequest",
    "message": "Cannot transition quote from Draft to Approved"
  }
}
```

---

## 5. Testing Specifications

### 5.1 Unit Test Patterns

```rust
// Config tests
#[test]
fn file_load_supports_env_interpolation() {
    // Verify ${ENV_VAR} syntax works
}

#[test]
fn precedence_defaults_file_env_overrides() {
    // Verify: defaults < file < env < overrides
}

// Flow engine tests
#[test]
fn net_new_flow_draft_to_validated_requires_fields() {
    // Verify RequiredFieldsCollected event triggers transition
}

#[test]
fn net_new_flow_priced_to_approval_on_policy_violation() {
    // Verify PolicyViolationDetected routes to Approval state
}

// CPQ engine tests
#[test]
fn pricing_engine_calculates_correct_totals() {
    // Verify: total = sum(line_totals) - discounts + tax
}

#[test]
fn policy_engine_requires_approval_for_high_discounts() {
    // Verify: >20% discount requires approval
}
```

### 5.2 Integration Test Patterns

```rust
#[sqlx::test]
async fn execution_queue_round_trip(pool: SqlitePool) {
    // 1. Create task
    // 2. Save to DB
    // 3. Load from DB
    // 4. Verify all fields match
}

#[tokio::test]
async fn full_quote_lifecycle() {
    // 1. Create quote (Draft)
    // 2. Add line items
    // 3. Transition to Validated
    // 4. Run pricing
    // 5. Transition to Priced
    // 6. Generate PDF
    // 7. Transition to Sent
}
```

### 5.3 Test Data Fixtures

```rust
// Test helpers
pub fn test_quote() -> Quote {
    Quote {
        id: QuoteId::new("Q-TEST-001"),
        status: QuoteStatus::Draft,
        lines: vec![test_line()],
        currency: "USD".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

pub fn test_line() -> QuoteLine {
    QuoteLine {
        product_id: ProductId::new("prod_test"),
        quantity: 10,
        unit_price: Some(dec!(100.00)),
        discount_pct: dec!(0),
        attributes_json: None,
    }
}
```

---

## 6. Security Specifications

### 6.1 Credential Handling

```rust
// Secrets use secrecy::SecretString
pub struct SlackConfig {
    pub app_token: SecretString,
    pub bot_token: SecretString,
}

// Debug output redacts secrets
impl fmt::Debug for SlackConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SlackConfig")
            .field("app_token", &"[REDACTED]")
            .field("bot_token", &"[REDACTED]")
            .finish()
    }
}
```

### 6.2 SQL Injection Prevention

- All queries use sqlx parameterized statements
- Compile-time checked queries with `query_as!` macro
- No raw SQL string concatenation

### 6.3 Audit Trail Integrity

```rust
pub struct LedgerEntry {
    // Cryptographic chain
    pub content_hash: String,   // SHA-256(quote state)
    pub prev_hash: String,      // Hash of previous entry
    pub entry_hash: String,     // Hash of this entry
    pub signature: String,      // HMAC-SHA256(key, entry)
}
```

### 6.4 Input Validation

| Input Source | Validation |
|-------------|------------|
| Slack messages | Sanitized before DB operations |
| CSV imports | Schema validation before ingestion |
| PDF parsing | Text extraction only (no execution) |
| User uploads | File type validation, size limits |

---

## 7. Performance Specifications

### 7.1 Target Metrics

| Operation | Target Latency | Notes |
|-----------|---------------|-------|
| Config validation | <10ms | File + env parsing |
| Quote creation | <50ms | DB insert + initial state |
| Pricing calculation | <100ms | Full CPQ pipeline |
| Constraint validation | <50ms | Dependency checking |
| Similarity search (DNA) | <10ms | SimHash over 10k candidates |
| Slack message response | <500ms | Including LLM extraction |

### 7.2 Database Optimization

```sql
-- Indexes for common queries
CREATE INDEX idx_quote_status ON quote(status);
CREATE INDEX idx_quote_account ON quote(account_id);
CREATE INDEX idx_quote_line_quote ON quote_line(quote_id);
CREATE INDEX idx_approval_quote ON approval_request(quote_id);
CREATE INDEX idx_audit_event_quote ON audit_event(quote_id);
CREATE INDEX idx_flow_state_quote ON flow_state(quote_id);
```

### 7.3 Connection Pool Settings

```rust
// Default pool configuration
SqlitePoolOptions::new()
    .max_connections(5)
    .acquire_timeout(Duration::from_secs(30))
    .connect(database_url)
```

---

## 8. Deployment Specifications

### 8.1 Binary Targets

| Target | Platform | Notes |
|--------|----------|-------|
| `x86_64-unknown-linux-gnu` | Linux x64 | Primary deployment target |
| `aarch64-unknown-linux-gnu` | Linux ARM64 | For ARM servers |
| `x86_64-apple-darwin` | macOS x64 | Development |
| `aarch64-apple-darwin` | macOS ARM64 | Apple Silicon |

### 8.2 Release Build Configuration

```toml
# Cargo.toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

### 8.3 Docker Configuration

```dockerfile
FROM rust:1.75-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y wkhtmltopdf
COPY --from=builder /app/target/release/quotey /usr/local/bin/
COPY templates/ /app/templates/
EXPOSE 8080
CMD ["quotey", "start"]
```

---

## 9. CLI Command Reference

### 9.1 Command Summary

| Command | Description | Example |
|---------|-------------|---------|
| `quotey start` | Start the bot | `quotey start` |
| `quotey migrate` | Run DB migrations | `quotey migrate` |
| `quotey seed-demo` | Load demo data | `quotey seed-demo` |
| `quotey smoke` | Run health checks | `quotey smoke --json` |
| `quotey config` | Show effective config | `quotey config` |
| `quotey doctor` | System readiness | `quotey doctor --json` |

### 9.2 Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Configuration error |
| 3 | Database error |
| 4 | Slack API error |

---

## 10. References

### 10.1 External Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `tokio` | 1.43 | Async runtime |
| `sqlx` | 0.8.6 | SQLite driver |
| `slack-morphism` | latest | Slack SDK |
| `serde` | 1.0 | Serialization |
| `chrono` | 0.4 | Date/time |
| `rust_decimal` | 1.36 | Decimal math |
| `thiserror` | 2.0 | Error types |
| `anyhow` | 1.0 | Error handling |
| `tracing` | 0.1 | Logging |
| `clap` | 4.5 | CLI parsing |

### 10.2 Documentation Links

- Project planning: `.planning/PROJECT.md`
- Agent instructions: `AGENTS.md`
- Quick start: `README.md`
- Research document: `.planning/RESEARCH_DOCUMENT.md`
- This specification: `.planning/TECHNICAL_SPEC.md`

---

*Document compiled by ResearchAgent (kimi-k2) for the quotey project.*
*Last updated: 2026-02-23*
