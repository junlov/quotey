# Determinism

Determinism is a core architectural principle in Quotey. This document explains why it matters and how it's implemented.

## What is Determinism?

A system is **deterministic** if the same inputs always produce the same outputs, with no randomness or variability.

```
Input A → System → Output X
Input A → System → Output X  (always the same)
Input B → System → Output Y
```

## Why Determinism Matters

### 1. Auditability

Enterprise sales requires proving how decisions were made:

```
Auditor: "How was this $50,000 price calculated?"
Quotey:  "Here's the pricing trace with every step..."

Auditor: "Was this discount within policy?"
Quotey:  "Here's the policy evaluation at the time..."
```

Non-deterministic systems cannot provide this level of evidence.

### 2. Reproducibility

When bugs occur, you need to reproduce them:

```
Bug Report: "Quote was priced incorrectly"
Action:     Re-run pricing with same inputs
Result:     Same bug reproduced → Can fix
```

If the system were non-deterministic, the bug might not reproduce.

### 3. Trust

Users must trust that the system won't arbitrarily change behavior:

```
Sales Rep A quotes: Pro Plan, 100 seats → $10,000
Sales Rep B quotes: Pro Plan, 100 seats → $10,000
```

Same input, same output. Consistency builds trust.

### 4. Testing

Deterministic systems are testable:

```rust
#[test]
fn pricing_is_deterministic() {
    let quote = test_quote();
    
    let result1 = engine.price(&quote);
    let result2 = engine.price(&quote);
    let result3 = engine.price(&quote);
    
    assert_eq!(result1.total, result2.total);
    assert_eq!(result2.total, result3.total);
    // All three are identical
}
```

## Deterministic Components

These parts of Quotey are strictly deterministic:

| Component | Deterministic? | Why |
|-----------|---------------|-----|
| Constraint Engine | ✅ Yes | Rules-based logic |
| Pricing Engine | ✅ Yes | Formula calculations |
| Policy Engine | ✅ Yes | Threshold comparisons |
| Flow Engine | ✅ Yes | State machine |
| Quote calculations | ✅ Yes | Math operations |
| Database queries | ✅ Yes | Same query → same data |

## Non-Deterministic Components

These parts are intentionally non-deterministic:

| Component | Deterministic? | Mitigation |
|-----------|---------------|------------|
| LLM extraction | ❌ No | Validation, fallbacks |
| LLM summarization | ❌ No | Only used for display |
| Timestamp generation | ❌ No | Recorded, not used for logic |
| ID generation | ❌ No | UUIDs, doesn't affect logic |

## Handling Time

Time can break determinism. Quotey handles it carefully:

### Recording Time (Non-Deterministic, OK)

```rust
// Recording when something happened
pub struct Quote {
    pub created_at: DateTime<Utc>,  // Non-deterministic
    // ...
}
```

This is fine because `created_at` is metadata, not used in calculations.

### Time-Based Logic (Careful)

```rust
// Time-based policies use explicit reference time
pub fn evaluate_temporal_policy(
    quote: &Quote,
    now: DateTime<Utc>,  // Explicit parameter
) -> PolicyResult {
    // Policy: "End of quarter deals need extra scrutiny"
    if is_end_of_quarter(now) && quote.discount_pct > 15.0 {
        PolicyResult::ApprovalRequired
    } else {
        PolicyResult::Pass
    }
}
```

By making `now` a parameter, the function becomes deterministic:
- Same quote + same `now` → same result
- Can test with any timestamp

## Snapshot Testing

Deterministic outputs enable snapshot testing:

```rust
#[test]
fn pricing_trace_matches_snapshot() {
    let quote = test_quote();
    let result = engine.price(&quote);
    
    // First run: Creates snapshot
    // Subsequent runs: Compares to snapshot
    insta::assert_json_snapshot!(result.trace);
}
```

Snapshot file (`pricing_trace.snap`):
```json
{
  "lines": [
    {
      "product_id": "plan_pro",
      "base_price": 10.00,
      "tier_price": 8.00,
      "quantity": 100,
      "line_total": 800.00
    }
  ],
  "total": 800.00
}
```

If pricing logic changes, the snapshot test fails, alerting you to verify the change.

## Replay Capability

Determinism enables replay of historical quotes:

```rust
pub fn replay_quote(
    &self,
    quote_id: &QuoteId,
    at_time: DateTime<Utc>,
) -> Result<CpqEvaluation> {
    // 1. Load quote as it was at `at_time`
    let quote = self.load_quote_version(quote_id, at_time)?;
    
    // 2. Load price book as it was at `at_time`
    let price_book = self.load_price_book_version(at_time)?;
    
    // 3. Re-run pricing
    let engine = PricingEngine::with_price_book(price_book);
    let result = engine.price(&quote);
    
    Ok(result)
}
```

This is used for:
- **Debugging**: "What was the price on Tuesday?"
- **Policy optimization**: "What if we had used new prices?"
- **Compliance**: "Prove the price was correct at the time"

## The LLM Boundary

The boundary between deterministic and non-deterministic is explicit:

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   User Input    │────→│  LLM Extraction  │────→│  Deterministic  │
│  (Natural Lang) │     │  (May vary)      │     │  Processing     │
└─────────────────┘     └──────────────────┘     │  (Always same)  │
                                                  └─────────────────┘
                                                           ↓
                                                  ┌──────────────────┐
                                                  │  Deterministic   │
                                                  │  Results         │
                                                  └──────────────────┘
                                                           ↓
                                                  ┌──────────────────┐
                                                  │  LLM Formatting  │
                                                  │  (Display only)  │
                                                  └──────────────────┘
```

The LLM is in the "translation" layers only, never in the "decision" layer.

## Testing Determinism

```rust
#[test]
fn replay_is_deterministic() {
    let engine = FlowEngine::default();
    let events = [
        FlowEvent::RequiredFieldsCollected,
        FlowEvent::PricingCalculated,
        FlowEvent::PolicyClear,
    ];
    
    // Run twice with identical inputs
    let result1 = run_events(&engine, &events);
    let result2 = run_events(&engine, &events);
    
    // Must be identical
    assert_eq!(result1, result2);
}

fn run_events(engine: &FlowEngine, events: &[FlowEvent]) -> FlowState {
    let mut state = FlowState::Draft;
    for event in events {
        let outcome = engine.apply(&state, event, &FlowContext::default())
            .expect("valid transition");
        state = outcome.to;
    }
    state
}
```

## Trade-offs

Determinism has trade-offs:

| Benefit | Cost |
|---------|------|
| Auditability | Cannot use "AI magic" for decisions |
| Reproducibility | Must explicitly handle time |
| Testability | More verbose code |
| Trust | Less "flexibility" in responses |

For enterprise CPQ, the benefits outweigh the costs.

## See Also

- [Safety Principle](../architecture/safety-principle) — LLM boundaries
- [CPQ Engine](./cpq-engine) — Deterministic pricing
- [Flow Engine](./flow-engine) — Deterministic state machine
