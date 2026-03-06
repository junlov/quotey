# Audit Trail

Quotey maintains a comprehensive audit trail of all actions and decisions. This is essential for compliance, debugging, and trust.

## Overview

Every significant action in Quotey generates an audit event:

- Quote created, updated, versioned
- Pricing calculated
- Approval requested, granted, denied
- Configuration validated
- Policy evaluated
- Agent actions
- System events

## Audit Event Structure

```rust
pub struct AuditEvent {
    pub id: String,                    // Unique event ID
    pub timestamp: DateTime<Utc>,      // When it happened
    pub actor: String,                 // Who did it
    pub actor_type: ActorType,         // human, system, agent, llm
    pub quote_id: Option<QuoteId>,     // Related quote (if any)
    pub thread_id: Option<String>,     // Slack thread correlation
    pub correlation_id: String,        // Request tracing
    pub event_type: String,            // Event categorization
    pub category: AuditCategory,       // Quote, Pricing, Approval, etc.
    pub outcome: AuditOutcome,         // Success, Failure, Rejected
    pub payload: JsonValue,            // Event-specific data
    pub metadata: HashMap<String, String>, // Additional context
}

pub enum ActorType {
    Human,      // User action
    System,     // Automated process
    Agent,      // Agent runtime decision
    Llm,        // LLM operation
}

pub enum AuditCategory {
    Quote,
    Pricing,
    Approval,
    Configuration,
    Catalog,
    Crm,
    Flow,
    Agent,
    System,
}

pub enum AuditOutcome {
    Success,
    Failure,
    Rejected,
}
```

## Event Types

### Quote Events

| Event | When | Payload |
|-------|------|---------|
| `quote.created` | New quote created | Quote details |
| `quote.updated` | Quote modified | Before/after |
| `quote.versioned` | New version created | Parent quote ID |
| `quote.finalized` | Quote marked final | Final state |
| `quote.sent` | PDF delivered | Delivery method |
| `quote.expired` | Validity ended | Expiry reason |
| `quote.cancelled` | Manually cancelled | Cancelled by |

### Pricing Events

| Event | When | Payload |
|-------|------|---------|
| `pricing.calculated` | New pricing computed | Pricing trace |
| `pricing.recalculated` | Pricing updated | Previous/new values |
| `price_book.selected` | Price book chosen | Selection criteria |

### Approval Events

| Event | When | Payload |
|-------|------|---------|
| `approval.requested` | Approval sent | Approver, reason |
| `approval.approved` | Approval granted | Approver, comment |
| `approval.rejected` | Approval denied | Approver, reason |
| `approval.escalated` | Escalated up | New approver |
| `approval.delegated` | Delegated | From/to |

### Configuration Events

| Event | When | Payload |
|-------|------|---------|
| `config.validated` | Constraints passed | Validation result |
| `config.constraint_violation` | Constraint failed | Violation details |
| `config.constraint_resolved` | Fix applied | Resolution |

### Agent Events

| Event | When | Payload |
|-------|------|---------|
| `agent.intent_extracted` | NL parsed | Raw and structured |
| `agent.slot_filled` | Field populated | Field, value |
| `agent.action_selected` | Action chosen | Available/selected |
| `agent.llm_called` | LLM invoked | Model, tokens |

## Example Audit Trail

For a complete quote flow:

```json
[
  {
    "timestamp": "2026-02-23T14:30:00Z",
    "actor": "U123456",
    "actor_type": "human",
    "event_type": "quote.created",
    "category": "quote",
    "outcome": "success",
    "payload": {
      "quote_id": "Q-2026-0042",
      "account_id": "acct_789",
      "initial_status": "draft"
    }
  },
  {
    "timestamp": "2026-02-23T14:30:05Z",
    "actor": "agent",
    "actor_type": "agent",
    "event_type": "agent.intent_extracted",
    "category": "agent",
    "outcome": "success",
    "payload": {
      "raw": "Pro Plan 100 seats",
      "intent": {
        "type": "add_line_item",
        "product": "plan_pro",
        "quantity": 100
      }
    }
  },
  {
    "timestamp": "2026-02-23T14:30:10Z",
    "actor": "system",
    "actor_type": "system",
    "event_type": "pricing.calculated",
    "category": "pricing",
    "outcome": "success",
    "payload": {
      "quote_id": "Q-2026-0042",
      "total": "10000.00",
      "currency": "USD",
      "trace_id": "trace_456"
    }
  },
  {
    "timestamp": "2026-02-23T14:30:15Z",
    "actor": "system",
    "actor_type": "system",
    "event_type": "policy.evaluated",
    "category": "approval",
    "outcome": "rejected",
    "payload": {
      "quote_id": "Q-2026-0042",
      "violations": [{
        "policy": "discount_cap",
        "requested": 20,
        "max_allowed": 15
      }]
    }
  },
  {
    "timestamp": "2026-02-23T14:35:00Z",
    "actor": "U789012",
    "actor_type": "human",
    "event_type": "approval.approved",
    "category": "approval",
    "outcome": "success",
    "payload": {
      "quote_id": "Q-2026-0042",
      "approver": "U789012",
      "comment": "Approved for strategic account"
    }
  }
]
```

## Querying the Audit Trail

### By Quote

```rust
let events = audit_repo
    .query()
    .by_quote(&quote_id)
    .order_by_timestamp()
    .fetch()
    .await?;
```

### By Time Range

```rust
let events = audit_repo
    .query()
    .since(start_date)
    .until(end_date)
    .by_category(AuditCategory::Approval)
    .fetch()
    .await?;
```

### By Actor

```rust
let events = audit_repo
    .query()
    .by_actor("U123456")
    .by_category(AuditCategory::Quote)
    .fetch()
    .await?;
```

## Correlation IDs

Every request gets a correlation ID for tracing:

```
User Request → correlation_id: "req-abc123"
    ↓
Slack Handler → same correlation_id
    ↓
Agent Runtime → same correlation_id
    ↓
CPQ Engine → same correlation_id
    ↓
Database → same correlation_id
```

This allows tracing a complete request across all components:

```rust
let events = audit_repo
    .query()
    .by_correlation_id("req-abc123")
    .fetch()
    .await?;
```

## Immutability

Audit events are immutable:
- Never updated
- Never deleted
- Appended-only log

This provides strong compliance guarantees:
- Tamper-evident history
- Complete record of all changes
- Regulatory audit support

## Retention

Configure retention policies:

```toml
[audit]
retention_days = 2555  # 7 years
archive_after_days = 365
archive_location = "s3://quotey-audit-archive/"
```

Old events can be:
- Archived to cold storage
- Summarized for analytics
- Deleted per policy (with warning)

## CLI Access

View audit trail via CLI:

```bash
# View events for a quote
quotey audit quote Q-2026-0042

# Export to file
quotey audit export --since=2026-01-01 --format=json > audit.json

# Search events
quotey audit search "approval.rejected"
```

## See Also

- [Safety Principle](../architecture/safety-principle) — Audit as safety mechanism
- [Determinism](./determinism) — Reproducible outcomes enable audit
- [CLI Reference](../api/cli-commands) — Audit commands
