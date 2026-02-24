# Policy Evaluation Persistence Model - Research Findings

**Research Task:** bd-imtv - Design policy evaluation persistence model  
**Research Agent:** ResearchAgent  
**Date:** 2026-02-24  
**Status:** Complete

---

## Executive Summary

This research analyzes approaches for persisting policy evaluations in the quotey CPQ system. The Explain Any Number feature requires policy evidence for deterministic explanations, but policy evaluations are currently computed on-demand with no persistence layer.

### Key Recommendation

**Adopt Approach C: Hybrid with Policy Versioning** - Store policy evaluations in `quote_pricing_snapshot.policy_evaluation_json` (lightweight), add `policy_version` to track rule changes, and compute-on-demand when snapshot missing.

---

## Current State Analysis

### Policy Engine Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     POLICY EVALUATION FLOW                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────────────────┐                                                │
│  │  PolicyInput         │  requested_discount_pct                        │
│  │  (from Quote)        │  deal_value                                    │
│  │                      │  minimum_margin_pct                            │
│  └──────────┬───────────┘                                                │
│             │                                                            │
│             ▼                                                            │
│  ┌──────────────────────┐                                                │
│  │  DeterministicPolicy │  evaluate_policy_input()                       │
│  │  Engine              │  • Check discount > 20%                        │
│  │  (on-demand)         │  • Return PolicyDecision                       │
│  └──────────┬───────────┘                                                │
│             │                                                            │
│             ▼                                                            │
│  ┌──────────────────────┐                                                │
│  │  PolicyDecision      │  approval_required: bool                       │
│  │                      │  approval_status: Pending/Approved             │
│  │  (ephemeral)         │  reasons: Vec<String>                          │
│  │                      │  violations: Vec<PolicyViolation>              │
│  └──────────────────────┘                                                │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Current Data Model

**Policy Input (from Quote):**
```rust
pub struct PolicyInput {
    pub requested_discount_pct: Decimal,  // e.g., 25.00 = 25%
    pub deal_value: Decimal,              // Total deal amount
    pub minimum_margin_pct: Decimal,      // Required margin floor
}
```

**Policy Decision (computed, not stored):**
```rust
pub struct PolicyDecision {
    pub approval_required: bool,
    pub approval_status: ApprovalStatus,  // Pending, Approved, Rejected
    pub reasons: Vec<String>,             // Human-readable explanations
    pub violations: Vec<PolicyViolation>, // Specific rule violations
}

pub struct PolicyViolation {
    pub policy_id: String,                // e.g., "discount-cap"
    pub reason: String,                   // e.g., "Discount above 20%"
    pub required_approval: Option<String>, // e.g., "sales_manager"
}
```

**Policy Rules (static, in database):**
```sql
CREATE TABLE policy_rules (
    id TEXT PRIMARY KEY,
    rule_key TEXT UNIQUE,           -- e.g., "max_discount_pct"
    condition_expression TEXT,       -- e.g., "discount > 20"
    action_expression TEXT,          -- e.g., "require_approval('manager')"
    explanation_template TEXT,       -- Human-readable template
    rule_category TEXT               -- 'pricing', 'approval', 'config'
);
```

### Database Schema (Existing)

**quote_pricing_snapshot (migration 0017):**
```sql
CREATE TABLE quote_pricing_snapshot (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    ledger_entry_id TEXT,
    ledger_content_hash TEXT,
    subtotal REAL NOT NULL,
    discount_total REAL NOT NULL,
    tax_total REAL NOT NULL,
    total REAL NOT NULL,
    currency TEXT NOT NULL,
    pricing_trace_json TEXT NOT NULL,
    policy_evaluation_json TEXT,    -- <-- CAN STORE POLICY EVALUATION
    priced_at TEXT NOT NULL,
    priced_by TEXT NOT NULL
);
```

### Explanation Module Requirements

**PolicyEvaluationProvider trait:**
```rust
#[async_trait]
pub trait PolicyEvaluationProvider: Send + Sync {
    async fn get_evaluation(
        &self,
        quote_id: &QuoteId,
        version: i32,
    ) -> Result<PolicyEvaluation, ExplanationError>;
}

pub struct PolicyEvaluation {
    pub quote_id: QuoteId,
    pub version: i32,
    pub overall_status: String,        // "approved", "violation", "waived"
    pub violations: Vec<PolicyViolation>,
    pub applied_rules: Vec<AppliedRule>,
    pub evaluated_at: String,
}
```

---

## Research Questions & Answers

### Q1: Should policy evaluations be snapshotted or computed fresh?

**Analysis:**

| Factor | Snapshot | Compute Fresh |
|--------|----------|---------------|
| **Determinism** | ✅ Same result every time | ❌ May differ if rules changed |
| **Auditability** | ✅ Historical record | ⚠️ Must trust current rules |
| **Storage** | ⚠️ Additional JSON column | ✅ No storage needed |
| **Complexity** | ⚠️ Must handle versioning | ✅ Simple |
| **Rule Changes** | ⚠️ May show outdated violations | ✅ Always current rules |

**Answer:** Snapshot for auditability, but support compute-on-demand fallback. Store in `quote_pricing_snapshot.policy_evaluation_json`.

---

### Q2: How to handle policy changes after quote was evaluated?

**Analysis:**

**Scenario:** Quote was evaluated on Monday with discount limit 20%. On Tuesday, policy changes to 15% limit.

**Options:**

1. **Immutable Snapshot (Historical Truth)**
   - Store policy evaluation at time of quote
   - Explanation shows: "Violation: discount 18% exceeded 20% limit (policy v1.2)"
   - Pros: Auditable, deterministic
   - Cons: May show outdated rules

2. **Re-evaluate on Request (Current Truth)**
   - Always use current policy rules
   - Explanation shows: "Violation: discount 18% exceeded 15% limit (current policy)"
   - Pros: Always current
   - Cons: Explanation changes over time, not auditable

3. **Versioned Policies (Hybrid)**
   - Store policy_version with evaluation
   - Compare: evaluated_with_policy_v1.2 vs current_policy_v1.3
   - Explanation shows: "At evaluation time: violation. Current rules: no violation."
   - Pros: Both historical and current context
   - Cons: More complex

**Answer:** Implement Versioned Policies (Option 3) for production use. For MVP, use Immutable Snapshot (Option 1).

---

### Q3: Do we need a separate `policy_evaluation` table?

**Analysis:**

| Approach | Schema | Pros | Cons |
|----------|--------|------|------|
| **A: JSON in snapshot** | `policy_evaluation_json` in `quote_pricing_snapshot` | Simple, atomic with pricing | Less queryable |
| **B: Separate table** | `policy_evaluation` with FK to snapshot | Queryable by violation type | More joins |
| **C: Both** | JSON for storage, view for querying | Best of both | More complex |

**Answer:** Start with Approach A (JSON column). Add materialized view if query needs emerge.

---

## Recommended Approaches

### Approach A: Compute on Demand (Current)

**How it works:**
1. When explanation requested, run `PolicyEngine::evaluate()`
2. Use current policy rules
3. Return fresh evaluation

**Pros:**
- Simple implementation
- Always current rules
- No storage overhead

**Cons:**
- Not auditable (can't prove what rules were checked)
- Explanation may change over time
- Can't explain historical decisions

**When to use:** Development, simple deployments without audit requirements.

---

### Approach B: Snapshot at Quote Version (Recommended for MVP)

**How it works:**
1. When quote is priced, also evaluate policies
2. Store `PolicyEvaluation` as JSON in `quote_pricing_snapshot.policy_evaluation_json`
3. Explanation uses stored snapshot

**Schema:**
```sql
-- Already exists in migration 0017
policy_evaluation_json TEXT,  -- JSON-encoded PolicyEvaluation
```

**JSON Structure:**
```json
{
  "quote_id": "Q-123",
  "version": 3,
  "overall_status": "violation",
  "violations": [
    {
      "policy_id": "discount-cap",
      "policy_name": "Maximum Discount",
      "severity": "blocking",
      "threshold_value": "20.00",
      "actual_value": "25.00",
      "message": "Discount exceeds 20% maximum",
      "suggested_resolution": "Request VP approval for discounts >20%"
    }
  ],
  "applied_rules": [
    {
      "rule_id": "rule-001",
      "rule_name": "Discount Cap",
      "rule_section": "Pricing Policy 4.2",
      "rule_description": "No discount may exceed 20% without VP approval"
    }
  ],
  "evaluated_at": "2026-02-24T10:30:00Z",
  "policy_version": "v1.2.0"  -- Added for versioning
}
```

**Pros:**
- Auditable (snapshot at time of pricing)
- Deterministic explanations
- Simple (one JSON column)
- Works with existing schema

**Cons:**
- Doesn't detect policy changes
- JSON less queryable than normalized tables

**Implementation:**
```rust
// In PricingService or LedgerService
async fn price_quote(&self, quote: &Quote) -> Result<PricingSnapshot, Error> {
    let pricing = self.calculate_pricing(quote).await?;
    let policy = self.policy_engine.evaluate(&quote.into());
    
    let snapshot = PricingSnapshot {
        // ... pricing fields ...
        policy_evaluation_json: Some(serde_json::to_string(&policy)?),
        // ...
    };
    
    self.repo.save_snapshot(snapshot).await
}
```

---

### Approach C: Hybrid with Policy Versioning (Recommended for Production)

**How it works:**
1. Store policy evaluation in snapshot (like Approach B)
2. Add `policy_version` to track which rule set was used
3. On explanation, compare `evaluated_policy_version` vs `current_policy_version`
4. If different, optionally re-evaluate or show warning

**Schema Additions:**
```sql
-- Add to quote_pricing_snapshot
ALTER TABLE quote_pricing_snapshot ADD COLUMN policy_version TEXT;

-- Add to policy_rules for versioning
CREATE TABLE policy_rule_versions (
    version_id TEXT PRIMARY KEY,      -- e.g., "v1.2.0"
    version_name TEXT,                 -- e.g., "Q1 2026 Policy"
    effective_date TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Link rules to versions
ALTER TABLE policy_rules ADD COLUMN version_id TEXT 
    REFERENCES policy_rule_versions(version_id);
```

**Enhanced JSON:**
```json
{
  "quote_id": "Q-123",
  "version": 3,
  "overall_status": "violation",
  "violations": [...],
  "applied_rules": [...],
  "evaluated_at": "2026-02-24T10:30:00Z",
  "policy_version": "v1.2.0",
  "policy_version_name": "Q1 2026 Pricing Policy"
}
```

**Explanation Engine Enhancement:**
```rust
impl ExplanationEngine {
    async fn explain_policy(&self, quote_id: &QuoteId, version: i32) -> Result<ExplanationResponse, Error> {
        let snapshot = self.pricing_provider.get_snapshot(quote_id, version).await?;
        let current_policy_version = self.policy_repo.get_current_version().await?;
        
        let evaluation = if let Some(json) = &snapshot.policy_evaluation_json {
            let eval: PolicyEvaluation = serde_json::from_str(json)?;
            
            // Check if policy changed since evaluation
            if eval.policy_version != current_policy_version {
                // Option 1: Show warning
                return Ok(ExplanationResponse {
                    user_summary: format!(
                        "Policy evaluation from {} (current policy: {}). \
                         Rules may have changed since evaluation.",
                        eval.policy_version, current_policy_version
                    ),
                    // ...
                });
                
                // Option 2: Re-evaluate with current rules
                // let current_eval = self.policy_engine.evaluate(&quote.into());
                // return Ok(ExplanationResponse { ... });
            }
            
            eval
        } else {
            // Fallback: compute on demand
            self.policy_engine.evaluate(&quote.into())
        };
        
        // Build explanation from evaluation...
    }
}
```

**Pros:**
- Auditable (historical snapshot)
- Detects policy drift
- Can show both historical and current context
- Extensible for policy change workflows

**Cons:**
- More complex schema
- Requires policy versioning discipline

---

## Implementation Recommendations

### Phase 1: MVP (Immediate)

**Use Approach B (Snapshot with JSON)**

1. **Update Pricing/Ledger Service:**
```rust
// In LedgerService::append_entry or PricingService
pub async fn snapshot_with_policy(
    &self,
    quote: &Quote,
    pricing: PricingSnapshot,
) -> Result<QuotePricingSnapshot, Error> {
    let policy_input: PolicyInput = quote.into();
    let policy_decision = self.policy_engine.evaluate(&policy_input);
    
    let policy_evaluation = PolicyEvaluation {
        quote_id: quote.id.clone(),
        version: quote.version,
        overall_status: policy_decision.approval_status.as_str().to_string(),
        violations: policy_decision.violations.into_iter().map(|v| {
            PolicyViolation {
                policy_id: v.policy_id,
                policy_name: self.get_rule_name(&v.policy_id),
                severity: if v.required_approval.is_some() { "blocking" } else { "warning" },
                threshold_value: None, // Extract from rule
                actual_value: quote.total_discount(), // Calculate
                message: v.reason,
                suggested_resolution: v.required_approval.map(|r| format!("Request {} approval", r)),
            }
        }).collect(),
        applied_rules: self.get_applied_rules(&policy_input),
        evaluated_at: Utc::now().to_rfc3339(),
    };
    
    QuotePricingSnapshot {
        // ... pricing fields ...
        policy_evaluation_json: Some(serde_json::to_string(&policy_evaluation)?),
        // ...
    }
}
```

2. **Implement PolicyEvaluationProvider:**
```rust
pub struct SnapshotPolicyEvaluationProvider {
    pool: DbPool,
}

#[async_trait]
impl PolicyEvaluationProvider for SnapshotPolicyEvaluationProvider {
    async fn get_evaluation(
        &self,
        quote_id: &QuoteId,
        version: i32,
    ) -> Result<PolicyEvaluation, ExplanationError> {
        let row = sqlx::query(
            "SELECT policy_evaluation_json FROM quote_pricing_snapshot 
             WHERE quote_id = ? AND version = ?"
        )
        .bind(&quote_id.0)
        .bind(version)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ExplanationError::EvidenceGatheringFailed { reason: e.to_string() })?;
        
        if let Some(row) = row {
            let json: String = row.try_get("policy_evaluation_json")
                .map_err(|_| ExplanationError::MissingPolicyEvaluation { quote_id: quote_id.clone() })?;
            
            serde_json::from_str(&json)
                .map_err(|e| ExplanationError::EvidenceGatheringFailed { reason: e.to_string() })
        } else {
            Err(ExplanationError::MissingPolicyEvaluation { quote_id: quote_id.clone() })
        }
    }
}
```

3. **Wire into ExplanationEngine:**
```rust
let explanation_engine = ExplanationEngine::new(
    SqlPricingSnapshotProvider::new(pool.clone()),
    SnapshotPolicyEvaluationProvider::new(pool.clone()),
);
```

### Phase 2: Production (Future)

**Enhance to Approach C (Hybrid with Versioning)**

1. Add `policy_version` column to `quote_pricing_snapshot`
2. Create `policy_rule_versions` table
3. Update policy rule management UI
4. Enhance explanation to show version comparison

---

## Testing Strategy

### Unit Tests

```rust
#[tokio::test]
async fn policy_evaluation_provider_returns_snapshot_when_present() {
    let pool = setup_test_db().await;
    let provider = SnapshotPolicyEvaluationProvider::new(pool.clone());
    
    // Insert test snapshot with policy evaluation
    insert_test_snapshot(&pool, "Q-TEST", 1, r#"{
        "quote_id": "Q-TEST",
        "version": 1,
        "overall_status": "violation",
        "violations": [{"policy_id": "test-rule", "message": "Test violation"}],
        "evaluated_at": "2026-02-24T10:00:00Z"
    }"#).await;
    
    let eval = provider.get_evaluation(&QuoteId("Q-TEST".into()), 1).await.unwrap();
    assert_eq!(eval.overall_status, "violation");
    assert_eq!(eval.violations.len(), 1);
}

#[tokio::test]
async fn policy_evaluation_provider_returns_error_when_missing() {
    let pool = setup_test_db().await;
    let provider = SnapshotPolicyEvaluationProvider::new(pool.clone());
    
    let result = provider.get_evaluation(&QuoteId("Q-NOEXIST".into()), 1).await;
    assert!(matches!(result, Err(ExplanationError::MissingPolicyEvaluation { .. })));
}
```

### Integration Tests

```rust
#[tokio::test]
async fn end_to_end_explanation_with_policy_evidence() {
    // Create quote with policy violation
    let quote = create_test_quote_with_discount(25.0).await; // 25% discount
    
    // Price quote (creates snapshot with policy evaluation)
    let pricing_service = PricingService::new(...);
    pricing_service.price_quote(&quote).await.unwrap();
    
    // Request explanation
    let engine = ExplanationEngine::new(...);
    let explanation = engine.explain_total(&quote.id, 1).await.unwrap();
    
    // Verify policy evidence in explanation
    assert!(explanation.policy_evidence.iter().any(|e| 
        e.decision == "violated" && e.policy_id == "discount-cap"
    ));
}
```

---

## Decision Record

### Decision: Store Policy Evaluations as JSON in quote_pricing_snapshot

**Status:** Proposed  
**Date:** 2026-02-24  
**Decision Owner:** ResearchAgent

**Context:**
- Explain Any Number requires policy evidence for deterministic explanations
- Policy evaluations are currently computed on-demand (ephemeral)
- Need auditable record of what rules were checked at pricing time

**Decision:**
Use Approach B (Snapshot at Quote Version) for MVP, with path to Approach C (Hybrid with Versioning) for production.

**Consequences:**

**Positive:**
- Deterministic, auditable explanations
- Simple implementation (one JSON column)
- Works with existing schema
- No breaking changes

**Negative:**
- JSON less queryable than normalized tables
- Doesn't automatically detect policy changes (mitigated by policy_version in Phase 2)

**Alternatives Considered:**
- Approach A (Compute on Demand): Rejected due to lack of auditability
- Separate policy_evaluation table: Rejected as overkill for MVP

**Migration Path:**
1. Implement Approach B now
2. Add policy_version column in next iteration
3. Add policy_rule_versions table for full versioning

---

## Related Code References

| File | Purpose |
|------|---------|
| `crates/core/src/cpq/policy.rs` | PolicyEngine trait and DeterministicPolicyEngine |
| `crates/core/src/explanation/mod.rs` | ExplanationEngine and PolicyEvaluationProvider trait |
| `crates/core/src/domain/explanation.rs` | PolicyEvaluation, PolicyViolation domain types |
| `crates/core/src/ledger/mod.rs` | LedgerService for version tracking |
| `crates/db/src/repositories/explanation.rs` | SqlExplanationRepository |
| `migrations/0017_quote_pricing_snapshot.up.sql` | quote_pricing_snapshot table schema |
| `migrations/0005_policy_explanations.up.sql` | policy_rules table schema |

---

## Open Questions

1. **Policy Rule Versioning:** How do we version policy rules? Git-like SHA? Semantic versioning? Timestamp?
2. **Policy Change Notifications:** Should we notify users when a quote's policy evaluation is "stale" (rules changed)?
3. **Re-evaluation Workflow:** Should reps be able to request re-evaluation with current rules?
4. **Performance:** Is JSON parsing overhead acceptable, or should we use protobuf/msgpack?

---

## Next Steps

1. ✅ **Complete** - Research policy evaluation persistence approaches
2. 🔶 **Next** - Create bead for implementing SnapshotPolicyEvaluationProvider
3. 🔶 **Next** - Update Pricing/Ledger service to store policy evaluations
4. 🔶 **Next** - Add integration tests for policy evidence in explanations

---

*Document Version: 1.0*  
*Research Task: bd-imtv*  
*Status: Complete*
