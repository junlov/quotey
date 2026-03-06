# quotey-core

The core business logic crate containing domain models, deterministic engines, and flow state machines.

## Overview

`quotey-core` is the heart of Quotey. It contains:

- **Domain models** — Quote, Product, Customer, Approval entities
- **CPQ engines** — Pricing, constraints, policy evaluation
- **Flow engine** — State machine for quote lifecycle
- **Audit system** — Immutable event logging
- **Advanced features** — Deal DNA, autopsy, optimizer

## Design Principles

1. **No external dependencies** — Pure business logic, no I/O
2. **Deterministic** — Same inputs always produce same outputs
3. **Testable** — Everything is unit testable
4. **Type-safe** — Extensive use of newtypes and enums

## Module Structure

```
crates/core/src/
├── lib.rs                 # Public exports
├── errors.rs              # Error taxonomy
├── config.rs              # Configuration types
├── audit.rs               # Audit event system
├── execution_engine.rs    # Deterministic execution
├── domain/                # Domain entities
│   ├── mod.rs
│   ├── quote.rs           # Quote, QuoteLine, QuoteStatus
│   ├── product.rs         # Product, ProductId
│   ├── customer.rs        # Customer, Account
│   ├── approval.rs        # ApprovalRequest, ApprovalStatus
│   ├── execution.rs       # ExecutionTask
│   ├── simulation.rs      # Simulation models
│   ├── precedent.rs       # Precedent intelligence
│   ├── optimizer.rs       # Policy optimization
│   └── autopsy.rs         # Deal autopsy
├── cpq/                   # CPQ engines
│   ├── mod.rs             # CpqRuntime trait
│   ├── pricing.rs         # PricingEngine
│   ├── constraints.rs     # ConstraintEngine
│   ├── policy.rs          # PolicyEngine
│   ├── catalog.rs         # Product catalog
│   ├── simulator.rs       # Deal simulator
│   └── precedent.rs       # Precedent matching
├── flows/                 # Flow engine
│   ├── mod.rs             # Flow trait
│   ├── engine.rs          # FlowEngine, transitions
│   └── states.rs          # FlowState, FlowEvent
├── explanation/           # Explanation engine
│   └── mod.rs
├── dna/                   # Deal DNA
│   └── mod.rs
├── ledger/                # Immutable quote ledger
│   └── mod.rs
├── ghost/                 # Ghost quotes
│   └── mod.rs
├── ml/                    # ML features
│   └── mod.rs
├── collab/                # Collaboration
│   └── mod.rs
├── archaeology/           # Configuration archaeology
│   └── mod.rs
├── autopsy/               # Autopsy engine
│   └── mod.rs
├── policy/                # Policy management
│   ├── mod.rs
│   └── optimizer.rs
└── suggestions/           # Smart suggestions
    ├── mod.rs
    ├── engine.rs
    ├── scoring.rs
    └── types.rs
```

## Domain Types

### Quote

```rust
pub struct Quote {
    pub id: QuoteId,
    pub version: i32,
    pub status: QuoteStatus,
    pub account_id: Option<AccountId>,
    pub deal_id: Option<DealId>,
    pub currency: String,
    pub term_months: Option<i32>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub valid_until: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_by: String,
    pub lines: Vec<QuoteLine>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct QuoteLine {
    pub product_id: ProductId,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub discount_pct: f64,
    pub discount_amount: Decimal,
    pub subtotal: Decimal,
    pub attributes: Option<JsonValue>,
    pub notes: Option<String>,
}
```

### Product

```rust
pub struct Product {
    pub id: ProductId,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub product_type: ProductType,
    pub category: Option<String>,
    pub attributes: Vec<Attribute>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub enum ProductType {
    Simple,
    Configurable,
    Bundle,
}
```

## CPQ Runtime

The `CpqRuntime` trait combines all CPQ engines:

```rust
#[async_trait]
pub trait CpqRuntime: Send + Sync {
    async fn evaluate_quote(&self, input: CpqEvaluationInput) -> CpqEvaluation;
}

pub struct CpqEvaluationInput<'a> {
    pub quote: &'a Quote,
    pub currency: &'a str,
    pub policy_input: PolicyInput,
}

pub struct CpqEvaluation {
    pub constraints: ConstraintResult,
    pub pricing: PricingResult,
    pub policy: PolicyEvaluation,
}
```

### DeterministicCpqRuntime

The default implementation:

```rust
pub struct DeterministicCpqRuntime<C, P, O> 
where
    C: ConstraintEngine,
    P: PricingEngine, 
    O: PolicyEngine,
{
    constraint_engine: C,
    pricing_engine: P,
    policy_engine: O,
}

impl<C, P, O> CpqRuntime for DeterministicCpqRuntime<C, P, O> {
    async fn evaluate_quote(&self, input: CpqEvaluationInput) -> CpqEvaluation {
        let constraints = self.constraint_engine.validate(&input.quote.lines);
        let pricing = self.pricing_engine.price(input.quote).await;
        let policy = self.policy_engine.evaluate(input.quote, &pricing);
        
        CpqEvaluation { constraints, pricing, policy }
    }
}
```

## Error Handling

Hierarchical error types:

```rust
// Domain errors - business logic violations
pub enum DomainError {
    InvalidQuoteTransition { from: QuoteStatus, to: QuoteStatus },
    InvariantViolation(String),
    // ...
}

// Application errors - infrastructure failures
pub enum ApplicationError {
    Domain(DomainError),
    Persistence(String),
    Integration(String),
    Configuration(String),
}

// Interface errors - user-facing
pub enum InterfaceError {
    BadRequest { message: String, correlation_id: String },
    ServiceUnavailable { message: String, correlation_id: String },
    Internal { message: String, correlation_id: String },
}
```

## Usage Examples

### Creating a Quote

```rust
use quotey_core::domain::quote::{Quote, QuoteId, QuoteStatus};
use quotey_core::domain::product::ProductId;
use chrono::Utc;
use rust_decimal::Decimal;

let quote = Quote {
    id: QuoteId("Q-2026-0042".to_string()),
    version: 1,
    status: QuoteStatus::Draft,
    account_id: Some(AccountId("acct_123".to_string())),
    deal_id: None,
    currency: "USD".to_string(),
    term_months: Some(12),
    start_date: None,
    end_date: None,
    valid_until: None,
    notes: None,
    created_by: "user_456".to_string(),
    lines: vec![
        QuoteLine {
            product_id: ProductId("plan-pro".to_string()),
            quantity: 100,
            unit_price: Decimal::new(1000, 2), // $10.00
            discount_pct: 0.0,
            discount_amount: Decimal::ZERO,
            subtotal: Decimal::new(100000, 2), // $1,000.00
            attributes: None,
            notes: None,
        }
    ],
    created_at: Utc::now(),
    updated_at: Utc::now(),
};
```

### Evaluating Constraints

```rust
use quotey_core::cpq::constraints::{ConstraintEngine, Constraint};

let engine = ConstraintEngine::new();
let constraints = vec![
    Constraint::requires("sso_addon", "enterprise_tier"),
];

let result = engine.validate(&quote.lines, &constraints);

if !result.valid {
    for violation in &result.violations {
        println!("Violation: {}", violation.message);
    }
}
```

### Running Pricing

```rust
use quotey_core::cpq::{CpqRuntime, DeterministicCpqRuntime};
use quotey_core::cpq::policy::PolicyInput;

let runtime = DeterministicCpqRuntime::default();

let evaluation = runtime.evaluate_quote(CpqEvaluationInput {
    quote: &quote,
    currency: "USD",
    policy_input: PolicyInput {
        requested_discount_pct: Decimal::from(10),
        deal_value: Decimal::new(100000, 2),
        minimum_margin_pct: Decimal::new(6000, 2), // 60%
    },
}).await;

println!("Total: {}", evaluation.pricing.total);
println!("Approval required: {}", evaluation.policy.approval_required);
```

### State Machine Transitions

```rust
use quotey_core::flows::{FlowEngine, NetNewFlow, FlowState, FlowEvent};

let engine = FlowEngine::new(NetNewFlow);

let outcome = engine.apply(
    &FlowState::Draft,
    &FlowEvent::RequiredFieldsCollected,
    &FlowContext::default(),
)?;

println!("Transitioned from {:?} to {:?}", outcome.from, outcome.to);
println!("Actions: {:?}", outcome.actions);
```

## Testing

The core crate is extensively tested:

```rust
#[test]
fn quote_transition_valid() {
    let quote = test_quote();
    let new_status = quote.status.transition_to(QuoteStatus::Validated).unwrap();
    assert_eq!(new_status, QuoteStatus::Validated);
}

#[test]
fn constraint_violation_detected() {
    let engine = ConstraintEngine::new();
    let lines = vec![
        line_with_product("sso_addon"),
        // Missing required enterprise_tier
    ];
    
    let result = engine.validate(&lines, &all_constraints());
    
    assert!(!result.valid);
    assert!(result.violations.iter().any(|v| 
        v.message.contains("requires Enterprise Tier")
    ));
}

#[test]
fn pricing_is_deterministic() {
    let engine = PricingEngine::new(test_price_book());
    let quote = test_quote();
    
    let result1 = engine.price(&quote);
    let result2 = engine.price(&quote);
    
    assert_eq!(result1.total, result2.total);
    assert_eq!(result1.trace, result2.trace);
}
```

## See Also

- [CPQ Engine](../core-concepts/cpq-engine) — How pricing and constraints work
- [Flow Engine](../core-concepts/flow-engine) — State machine documentation
- [Database Crate](./db) — Persistence layer
