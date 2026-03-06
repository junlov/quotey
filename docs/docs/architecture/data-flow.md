# Data Flow

This document describes how data flows through the Quotey system in various scenarios.

## Request Flow Overview

```mermaid
flowchart TB
    subgraph User["User Layer"]
        SLACK["Slack"]
        CLI["CLI"]
        MCP["MCP Client"]
    end
    
    subgraph Interface["Interface Layer"]
        SLACK_ADAPTER["Slack Adapter"]
        CLI_ADAPTER["CLI Adapter"]
        MCP_ADAPTER["MCP Adapter"]
    end
    
    subgraph Runtime["Runtime Layer"]
        AGENT["Agent Runtime"]
        GUARDRAILS["Guardrails"]
    end
    
    subgraph Core["Core Layer"]
        FLOW["Flow Engine"]
        CPQ["CPQ Runtime"]
    end
    
    subgraph Data["Data Layer"]
        DB[("SQLite")]
    end
    
    SLACK --> SLACK_ADAPTER
    CLI --> CLI_ADAPTER
    MCP --> MCP_ADAPTER
    
    SLACK_ADAPTER --> AGENT
    CLI_ADAPTER --> DB
    MCP_ADAPTER --> DB
    
    AGENT --> GUARDRAILS
    GUARDRAILS --> FLOW
    GUARDRAILS --> CPQ
    
    FLOW --> CPQ
    FLOW --> DB
    CPQ --> DB
```

## Scenario 1: Creating a New Quote (Slack)

### Step-by-Step Flow

```mermaid
sequenceDiagram
    actor User
    participant Slack
    participant Socket as Socket Mode
    participant Agent as Agent Runtime
    participant Guard as Guardrails
    participant Flow as Flow Engine
    participant CPQ as CPQ Runtime
    participant Repo as Repository
    participant DB as SQLite
    
    User->>Slack: /quote new for Acme, Pro Plan
    Slack->>Socket: Slash command event
    Socket->>Socket: Parse command
    Socket->>Agent: Intent::CreateQuote
    
    Agent->>Agent: Extract entities
    Agent->>Guard: Check guardrails
    Guard-->>Agent: Allow
    
    Agent->>Repo: Create quote draft
    Repo->>DB: INSERT quote
    DB-->>Repo: Quote created
    Repo-->>Agent: Quote
    
    Agent->>Flow: Apply(Draft, RequiredFieldsCollected)
    Flow->>Flow: Check missing fields
    Flow-->>Agent: Need: start_date, billing_country
    
    Agent->>Repo: Update flow state
    Repo->>DB: INSERT flow_state
    
    Agent->>Slack: Message: "Quote created. Missing: start_date, billing_country"
    Slack-->>User: Interactive message with buttons
```

### Data Transformations

1. **Slack Event → Domain Intent**
   ```
   {
     "command": "/quote",
     "text": "new for Acme, Pro Plan",
     "user_id": "U123",
     "channel_id": "C456"
   }
   ↓
   Intent::CreateQuote {
     account_hint: "Acme",
     product_hints: ["Pro Plan"],
     context: Context { user: U123, channel: C456 }
   }
   ```

2. **Intent → Quote Entity**
   ```
   Intent::CreateQuote { ... }
   ↓ (with account lookup, product matching)
   Quote {
     id: "Q-2026-0042",
     account_id: "acct_123",
     status: Draft,
     lines: [QuoteLine { product_id: "plan_pro", ... }],
     ...
   }
   ```

3. **Quote → Database Row**
   ```rust
   sqlx::query(
       "INSERT INTO quote (id, account_id, status, ...)
        VALUES (?, ?, ?, ...)"
   )
   .bind(&quote.id.0)
   .bind(quote.account_id.as_ref().map(|a| &a.0))
   .bind(&quote.status.to_string())
   // ...
   ```

## Scenario 2: Pricing a Quote

```mermaid
sequenceDiagram
    participant Agent as Agent Runtime
    participant CPQ as CPQ Runtime
    participant Pricing as Pricing Engine
    participant Policy as Policy Engine
    participant Constraints as Constraint Engine
    participant Repo as Repository
    participant DB as SQLite
    
    Agent->>CPQ: evaluate_quote(input)
    
    CPQ->>Constraints: validate(lines)
    Constraints->>Constraints: Check all constraints
    Constraints-->>CPQ: ConstraintResult
    
    CPQ->>Repo: Get price book
    Repo->>DB: SELECT * FROM price_book WHERE ...
    DB-->>Repo: PriceBook
    Repo-->>Pricing: PriceBook
    
    Pricing->>Pricing: Calculate line prices
    Pricing->>Pricing: Apply volume tiers
    Pricing->>Pricing: Apply formulas
    Pricing->>Pricing: Generate trace
    Pricing-->>CPQ: PricingResult
    
    CPQ->>Repo: Get policies
    Repo->>DB: SELECT * FROM discount_policy WHERE active = true
    DB-->>Repo: Policies
    Repo-->>Policy: Policies
    
    Policy->>Policy: Check discount caps
    Policy->>Policy: Check margin floors
    Policy->>Policy: Check deal thresholds
    Policy-->>CPQ: PolicyEvaluation
    
    CPQ->>Repo: Save pricing snapshot
    Repo->>DB: INSERT INTO quote_pricing_snapshot
    
    CPQ-->>Agent: CpqEvaluation
```

## Scenario 3: MCP Tool Call

```mermaid
sequenceDiagram
    actor MCP as MCP Client
    participant Server as MCP Server
    participant Auth as Auth
    participant Router as Tool Router
    participant Repo as Repository
    participant DB as SQLite
    
    MCP->>Server: tools/call
    Server->>Auth: Validate API key
    Auth-->>Server: OK
    
    Server->>Router: Route tool call
    
    alt catalog_search
        Router->>DB: SELECT * FROM product WHERE name LIKE ?
        DB-->>Router: Products
    else quote_get
        Router->>DB: SELECT * FROM quote WHERE id = ?
        DB-->>Router: Quote
    else quote_price
        Router->>CPQ: evaluate_quote()
        CPQ-->>Router: PricingResult
    end
    
    Router-->>Server: ToolResult
    Server->>Server: Log to audit
    Server-->>MCP: Response
```

## Scenario 4: Approval Workflow

```mermaid
sequenceDiagram
    actor Rep as Sales Rep
    actor Approver as Approver
    participant Slack as Slack Bot
    participant Agent as Agent Runtime
    participant Flow as Flow Engine
    participant Repo as Repository
    participant DB as SQLite
    
    Rep->>Slack: Request 15% discount
    Slack->>Agent: Process request
    Agent->>CPQ: Evaluate policy
    CPQ-->>Agent: PolicyViolationDetected
    
    Agent->>Flow: Apply(Priced, PolicyViolationDetected)
    Flow-->>Agent: Transition to Approval
    
    Agent->>Repo: Create approval request
    Repo->>DB: INSERT INTO approval_request
    
    Agent->>Slack: Post to #approvals
    Slack-->>Approver: Approval request with context
    
    Approver->>Slack: Click "Approve"
    Slack->>Agent: Interaction event
    Agent->>Flow: Apply(Approval, ApprovalGranted)
    Flow-->>Agent: Transition to Approved
    
    Agent->>Repo: Update approval status
    Repo->>DB: UPDATE approval_request SET status = 'approved'
    
    Agent->>Slack: Notify rep
    Slack-->>Rep: "Approved by @manager"
```

## Data Persistence Patterns

### Write-Through Cache Pattern

```rust
// Repository implements caching with write-through
pub struct CachingQuoteRepository {
    inner: Box<dyn QuoteRepository>,
    cache: Arc<RwLock<HashMap<QuoteId, Quote>>>,
}

#[async_trait]
impl QuoteRepository for CachingQuoteRepository {
    async fn get(&self, id: &QuoteId) -> Result<Option<Quote>> {
        // Check cache first
        if let Some(quote) = self.cache.read().await.get(id) {
            return Ok(Some(quote.clone()));
        }
        
        // Load from database
        let quote = self.inner.get(id).await?;
        
        // Update cache
        if let Some(ref q) = quote {
            self.cache.write().await.insert(id.clone(), q.clone());
        }
        
        Ok(quote)
    }
    
    async fn update(&self, quote: &Quote) -> Result<Quote> {
        // Write to database first
        let updated = self.inner.update(quote).await?;
        
        // Invalidate cache
        self.cache.write().await.remove(&quote.id);
        
        Ok(updated)
    }
}
```

### Event Sourcing for Audit

```rust
// All changes emit audit events
pub struct AuditingRepository<R> {
    inner: R,
    audit_sink: Arc<dyn AuditSink>,
}

#[async_trait]
impl<R: QuoteRepository> QuoteRepository for AuditingRepository<R> {
    async fn update(&self, quote: &Quote) -> Result<Quote> {
        let before = self.inner.get(&quote.id).await?;
        
        let updated = self.inner.update(quote).await?;
        
        // Emit audit event
        self.audit_sink.emit(AuditEvent::new(
            Some(quote.id.clone()),
            /* ... */
            "quote.updated",
            AuditCategory::Quote,
        ).with_metadata("before", json!(before))
         .with_metadata("after", json!(updated)));
        
        Ok(updated)
    }
}
```

## Error Flow

```mermaid
flowchart TD
    A[Operation] --> B{Success?}
    B -->|Yes| C[Return Result]
    B -->|No| D[Create Error]
    D --> E{Error Type?}
    E -->|Domain| F[InterfaceError::BadRequest]
    E -->|Persistence| G[InterfaceError::ServiceUnavailable]
    E -->|Config| H[InterfaceError::Internal]
    F --> I[Return Error with correlation_id]
    G --> I
    H --> I
    I --> J[Log Error]
    J --> K[Slack Error Message]
```

## Performance Considerations

### Database Queries

- Use indexed lookups by primary key
- Batch related queries where possible
- Connection pool size: 5 (SQLite handles concurrency via WAL)

### Caching Strategy

| Data | Cache Duration | Invalidation |
|------|---------------|--------------|
| Products | 5 minutes | Manual/config change |
| Price Books | 5 minutes | Manual/config change |
| Quotes | No cache | Always fresh |
| Flow States | 1 minute | On transition |

### Async Boundaries

- Database operations: async
- CPQ engine: sync (deterministic, fast)
- LLM calls: async (network I/O)
- Slack API: async (network I/O)

## Monitoring Points

Key metrics to track:

| Metric | Type | Description |
|--------|------|-------------|
| `quote_creation_duration` | Histogram | Time to create a quote |
| `pricing_calculation_duration` | Histogram | Time to calculate pricing |
| `policy_evaluation_duration` | Histogram | Time to evaluate policies |
| `slack_api_latency` | Histogram | Slack API response times |
| `llm_request_duration` | Histogram | LLM API response times |
| `db_query_duration` | Histogram | Database query times |
| `quote_count_by_status` | Gauge | Current quotes by status |
| `approval_pending_count` | Gauge | Pending approvals |

## See Also

- [Architecture Overview](./overview) — High-level system design
- [Six-Box Model](./six-box-model) — Detailed component descriptions
- [Safety Principle](./safety-principle) — LLM/determinism boundary
