# Pricing Snapshot Persistence Strategy (EXP-RESEARCH)

**Task:** bd-beo8  
**Researcher:** ResearchAgent (kimi-k2)  
**Date:** 2026-02-23  
**Status:** Completed (implemented in `bd-3v0z`, 2026-02-24)

---

## 1. Problem Statement

The Explain Any Number feature requires pricing snapshots for deterministic explanations, but pricing is currently computed on-demand with no persistence layer. This creates challenges:

- **Non-deterministic explanations**: Re-computing pricing may yield different results if rules/prices changed
- **Audit gaps**: No historical record of what pricing was at explanation time
- **Performance**: Re-computing pricing for every explanation is expensive
- **Consistency**: Quote state may drift from explained state

---

## 2. Analysis of Options

### Option A: Pricing Snapshot Table (Explicit)

**Design:**
```rust
pub struct PricingSnapshot {
    pub id: String,
    pub quote_id: QuoteId,
    pub quote_version: u32,
    pub snapshot_data: PricingSnapshotData,
    pub created_at: DateTime<Utc>,
    pub pricing_engine_version: String,
}

pub struct PricingSnapshotData {
    pub line_items: Vec<PricedLineItem>,
    pub subtotal: Decimal,
    pub discount_total: Decimal,
    pub tax_total: Decimal,
    pub total: Decimal,
    pub currency: String,
    pub price_book_id: String,
    pub volume_tiers_applied: Vec<VolumeTierRecord>,
    pub formulas_applied: Vec<FormulaRecord>,
    pub policy_violations: Vec<PolicyViolationRecord>,
}
```

**Pros:**
- Complete historical record
- Fast explanation lookups
- Immutable audit trail
- Easy to validate against

**Cons:**
- Additional storage overhead
- Must snapshot at key moments
- Schema evolution complexity

---

### Option B: Ledger Content Hash (Implicit)

**Design:**
- Use quote ledger entry hash as version identifier
- Re-compute pricing from quote lines when explanation requested
- Verify quote state matches ledger hash before explaining

**Pros:**
- No additional storage
- Leverages existing ledger
- Always current

**Cons:**
- Re-computation is expensive
- May get different results if rules changed
- Complex validation logic
- Race conditions possible

---

### Option C: Hybrid (Recommended)

**Design:**
1. Add `pricing_snapshot` to ledger entries (optional)
2. Store lightweight snapshot only when explanation requested
3. Fall back to on-demand computation if snapshot missing

**Schema:**
```sql
CREATE TABLE pricing_snapshots (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    quote_version INTEGER NOT NULL,
    ledger_entry_id TEXT REFERENCES quote_ledger(id),
    snapshot_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    created_by TEXT NOT NULL,  -- 'system' or user_id
    
    UNIQUE(quote_id, quote_version)
);

CREATE INDEX idx_pricing_snapshots_quote ON pricing_snapshots(quote_id, quote_version);
```

**Pros:**
- Lazy snapshot creation (only when needed)
- Complete audit trail
- Backwards compatible (fallback to compute)
- Efficient storage

**Cons:**
- First explanation is slower
- Slightly more complex logic

---

## 3. Recommended Approach: Option C (Hybrid)

### 3.1 Workflow

```rust
pub struct PricingSnapshotService {
    snapshot_repo: Arc<dyn PricingSnapshotRepository>,
    pricing_engine: Arc<dyn PricingEngine>,
    ledger_service: Arc<LedgerService>,
}

impl PricingSnapshotService {
    /// Get or create pricing snapshot for explanation
    pub async fn get_snapshot_for_explanation(
        &self,
        quote_id: &QuoteId,
        version: u32,
    ) -> Result<PricingSnapshot, SnapshotError> {
        // 1. Try to get existing snapshot
        if let Some(snapshot) = self.snapshot_repo
            .get_by_quote_version(quote_id, version)
            .await? {
            return Ok(snapshot);
        }
        
        // 2. Verify quote state matches ledger
        let ledger_entry = self.ledger_service
            .get_entry(quote_id, version)
            .await?
            .ok_or(SnapshotError::QuoteVersionNotFound)?;
        
        // 3. Re-compute pricing
        let quote = self.reconstruct_quote_from_ledger(&ledger_entry).await?;
        let pricing_result = self.pricing_engine.price(&quote).await?;
        
        // 4. Create and store snapshot
        let snapshot = PricingSnapshot {
            id: generate_id(),
            quote_id: quote_id.clone(),
            quote_version: version,
            ledger_entry_id: Some(ledger_entry.id),
            snapshot_data: pricing_result.into(),
            created_at: Utc::now(),
            created_by: "system".to_string(),
        };
        
        self.snapshot_repo.save(&snapshot).await?;
        
        Ok(snapshot)
    }
}
```

### 3.2 Integration with Explanation Engine

```rust
pub struct ExplainAnyNumberService {
    snapshot_service: Arc<PricingSnapshotService>,
    explanation_generator: Arc<ExplanationGenerator>,
}

impl ExplainAnyNumberService {
    pub async fn explain(
        &self,
        request: &ExplainRequest,
    ) -> Result<Explanation, ExplanationError> {
        // Get deterministic snapshot
        let snapshot = self.snapshot_service
            .get_snapshot_for_explanation(&request.quote_id, request.quote_version)
            .await?;
        
        // Generate explanation from snapshot
        let explanation = self.explanation_generator
            .generate(&snapshot, request)
            .await?;
        
        Ok(explanation)
    }
}
```

### 3.3 Migration Schema

```sql
-- Migration: 0015_pricing_snapshots.up.sql

CREATE TABLE pricing_snapshots (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL REFERENCES quote(id),
    quote_version INTEGER NOT NULL,
    ledger_entry_id TEXT REFERENCES quote_ledger(id),
    
    -- Snapshot data (denormalized for fast access)
    subtotal REAL NOT NULL,
    discount_total REAL NOT NULL,
    tax_total REAL NOT NULL,
    total REAL NOT NULL,
    currency TEXT NOT NULL,
    
    -- Full snapshot JSON for complex queries
    snapshot_json TEXT NOT NULL,
    
    -- Provenance
    created_at TEXT NOT NULL,
    created_by TEXT NOT NULL,
    pricing_engine_version TEXT NOT NULL,
    
    UNIQUE(quote_id, quote_version)
);

CREATE INDEX idx_pricing_snapshots_quote ON pricing_snapshots(quote_id, quote_version);
CREATE INDEX idx_pricing_snapshots_ledger ON pricing_snapshots(ledger_entry_id);

-- Migration: 0015_pricing_snapshots.down.sql
DROP TABLE IF EXISTS pricing_snapshots;
```

---

## 4. Acceptance Criteria

- [x] Decision record documenting chosen approach (this document)
- [x] Migration schema for new table (`migrations/0017_quote_pricing_snapshot.*.sql`)
- [x] Update explanation provider contract to support SQL-backed snapshot provider
- [x] Integration tests for snapshot lifecycle (`crates/db/src/repositories/pricing_snapshot.rs`)
- [ ] Performance benchmarks (first vs subsequent explanations)

---

## 5. Related Code

- `crates/core/src/cpq/pricing.rs` - Pricing computation
- `crates/core/src/ledger/mod.rs` - Ledger service with versioning
- `crates/core/src/explanation/mod.rs` - Explanation engine

---

## 6. Decision

**Adopt Option C (Hybrid)** for the following reasons:

1. **Lazy creation** avoids storage bloat for quotes never explained

---

## 7. Implementation Notes (2026-02-24)

- Added immutable `quote_pricing_snapshot` table with ledger linkage columns:
  - `ledger_entry_id` (FK to `quote_ledger.entry_id`)
  - `ledger_content_hash`
- Added `SqlPricingSnapshotRepository` implementing `PricingSnapshotProvider` with:
  - quote existence validation,
  - ledger version validation (`VersionMismatch` on invalid version),
  - cached snapshot retrieval,
  - fallback snapshot build from `quote_line` data,
  - cache write-through on first retrieval.
- Added deterministic mismatch safeguards:
  - cached snapshot vs ledger entry/hash mismatch raises `EvidenceGatheringFailed`.
- Added targeted integration tests for:
  - cached hit,
  - fallback build + cache,
  - invalid version,
  - missing quote,
  - ledger mismatch detection.
2. **Immutable snapshots** ensure deterministic explanations
3. **Ledger linkage** provides audit trail
4. **Fallback mechanism** maintains backwards compatibility
5. **Performance** improves after first explanation

---

*Research completed by ResearchAgent for the quotey project.*
