# Deal DNA (FEAT-01) - Deep Technical Research

**Feature:** Configuration Fingerprint Matching  
**Researcher:** ResearchAgent (kimi-k2)  
**Date:** 2026-02-23  
**Priority:** P1

---

## 1. Technical Overview

Deal DNA provides intelligent similarity matching for quotes using Locality-Sensitive Hashing (LSH) techniques. The system generates compact fingerprints from quote configurations and enables sub-linear time similarity search.

---

## 2. Algorithm Research

### 2.1 MinHash Algorithm

**Purpose:** Estimate Jaccard similarity between sets in O(1) time.

**Mathematical Foundation:**
```
Jaccard(A, B) = |A ∩ B| / |A ∪ B|

MinHash simulates this by:
1. Apply k hash functions to each element
2. For each hash function, keep minimum value
3. Signature = vector of k minimums
4. Similarity ≈ fraction of matching minima
```

**For Quotey:**
- Each quote configuration is a set of features
- Features: product_ids, attributes, quantities, segments
- k=128 hash functions provides good accuracy

**Rust Implementation Sketch:**
```rust
pub struct MinHash {
    num_hashes: usize,  // k = 128
    hash_seeds: Vec<u64>,
}

impl MinHash {
    pub fn new(num_hashes: usize) -> Self {
        let hash_seeds: Vec<u64> = (0..num_hashes)
            .map(|i| fasthash::murmur3(&(i as u64).to_le_bytes()))
            .collect();
        Self { num_hashes, hash_seeds }
    }
    
    pub fn signature(&self, features: &[Feature]) -> Vec<u64> {
        self.hash_seeds.iter()
            .map(|&seed| {
                features.iter()
                    .map(|f| fasthash::murmur3_with_seed(&f.to_bytes(), seed))
                    .min()
                    .unwrap_or(u64::MAX)
            })
            .collect()
    }
    
    pub fn similarity(sig_a: &[u64], sig_b: &[u64]) -> f64 {
        let matches = sig_a.iter().zip(sig_b.iter())
            .filter(|(a, b)| a == b)
            .count();
        matches as f64 / sig_a.len() as f64
    }
}
```

### 2.2 SimHash (Locality-Sensitive Hashing)

**Purpose:** Map similar items to similar hash values (Hamming distance correlates with similarity).

**Algorithm:**
```
1. Initialize vector v of dimension d with zeros
2. For each feature f in set:
   a. Compute hash h(f) → d-dimensional vector of +1/-1
   b. Add h(f) to v
3. Final hash = sign(v) → bits (1 if v[i] > 0, else 0)
```

**For Quotey:**
- 128-bit SimHash provides good granularity
- Hamming distance threshold of 10-15 bits for "similar"
- Enables fast XOR-based similarity computation

**Implementation:**
```rust
pub struct SimHash {
    dimensions: usize,  // 128 bits
}

impl SimHash {
    pub fn hash(&self, features: &[Feature]) -> u128 {
        let mut vector = vec![0i32; self.dimensions];
        
        for feature in features {
            let hash = self.feature_hash(feature);
            for i in 0..self.dimensions {
                vector[i] += if (hash >> i) & 1 == 1 { 1 } else { -1 };
            }
        }
        
        vector.iter().enumerate()
            .map(|(i, &v)| if v > 0 { 1u128 << i } else { 0 })
            .fold(0u128, |acc, bit| acc | bit)
    }
    
    pub fn hamming_distance(a: u128, b: u128) -> u32 {
        (a ^ b).count_ones()
    }
    
    pub fn similarity(a: u128, b: u128) -> f64 {
        let distance = Self::hamming_distance(a, b) as f64;
        1.0 - (distance / 128.0)
    }
}
```

### 2.3 Algorithm Comparison

| Aspect | MinHash | SimHash |
|--------|---------|---------|
| Similarity Metric | Jaccard | Cosine/Angular |
| Comparison Time | O(k) | O(1) with SIMD |
| Best For | Set overlap | Vector similarity |
| Implementation | Simpler | Faster queries |
| **Quotey Choice** | - | **SimHash** |

**Rationale for SimHash:**
- Faster Hamming distance computation
- Better for weighted features (quantities, prices)
- Natural 128-bit representation fits SQLite

---

## 3. Feature Extraction Design

### 3.1 Quote Feature Vector

```rust
pub struct QuoteFeatures {
    // Core identifiers (high weight)
    pub product_ids: Vec<ProductId>,
    
    // Configuration attributes (medium weight)
    pub attributes: Vec<(String, String)>,
    
    // Quantities (normalized)
    pub total_quantity: u32,
    pub quantity_distribution: Vec<(ProductId, u32)>,
    
    // Segment/Context (high weight)
    pub customer_segment: String,
    pub industry: Option<String>,
    
    // Pricing hints (normalized)
    pub total_value_range: ValueRange,  // bucketed: 0-10k, 10-50k, etc.
    pub discount_range: DiscountRange,  // bucketed: 0-10%, 10-20%, etc.
}

pub enum ValueRange {
    Tier0,      // <$10k
    Tier1,      // $10k-50k
    Tier2,      // $50k-100k
    Tier3,      // $100k-500k
    Tier4,      // >$500k
}

pub enum DiscountRange {
    None,       // 0%
    Low,        // 1-10%
    Medium,     // 11-20%
    High,       // 21-30%
    Exceptional, // >30%
}
```

### 3.2 Feature Weighting

```rust
pub struct WeightedFeature {
    pub feature: Feature,
    pub weight: f64,  // 0.0 - 2.0
}

// Default weights
const WEIGHTS: &[(FeatureType, f64)] = &[
    (FeatureType::ProductId, 1.5),        // Core product mix
    (FeatureType::CustomerSegment, 1.2),  // Segment matters
    (FeatureType::Quantity, 1.0),         // Scale indicator
    (FeatureType::Attributes, 0.8),       // Configuration details
    (FeatureType::ValueRange, 0.7),       // Price tier
    (FeatureType::DiscountRange, 0.6),    // Pricing aggression
];
```

### 3.3 Canonical Feature Ordering

For consistent hashing:
```rust
pub fn canonical_features(features: &mut Vec<Feature>) {
    // Sort for deterministic ordering
    features.sort_by(|a, b| {
        a.feature_type().cmp(&b.feature_type())
            .then_with(|| a.key().cmp(&b.key()))
    });
    
    // Deduplicate
    features.dedup_by(|a, b| a.key() == b.key());
}
```

---

## 4. Database Schema Design

### 4.1 Core Tables

```sql
-- Configuration fingerprints table
CREATE TABLE configuration_fingerprints (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL REFERENCES quote(id),
    fingerprint BLOB NOT NULL,  -- 128-bit SimHash as 16 bytes
    fingerprint_hex TEXT NOT NULL,  -- For debugging
    feature_count INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    
    UNIQUE(quote_id)
);

CREATE INDEX idx_fingerprint_hash ON configuration_fingers(fingerprint_hex);

-- Deal outcomes for similarity results
CREATE TABLE deal_outcomes (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL REFERENCES quote(id),
    outcome_status TEXT NOT NULL,  -- 'won', 'lost', 'pending', 'expired'
    final_price REAL,
    final_discount_pct REAL,
    close_date TEXT,
    sales_cycle_days INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    
    UNIQUE(quote_id)
);

CREATE INDEX idx_outcome_status ON deal_outcomes(outcome_status);
CREATE INDEX idx_close_date ON deal_outcomes(close_date);

-- Precomputed similarity scores (optional optimization)
CREATE TABLE similarity_cache (
    id TEXT PRIMARY KEY,
    source_quote_id TEXT NOT NULL REFERENCES quote(id),
    target_quote_id TEXT NOT NULL REFERENCES quote(id),
    similarity_score REAL NOT NULL,  -- 0.0 - 1.0
    hamming_distance INTEGER NOT NULL,
    computed_at TEXT NOT NULL,
    
    UNIQUE(source_quote_id, target_quote_id)
);

CREATE INDEX idx_similarity_source ON similarity_cache(source_quote_id, similarity_score DESC);
CREATE INDEX idx_similarity_target ON similarity_cache(target_quote_id);
```

### 4.2 Query Patterns

**Find Similar Quotes:**
```sql
-- Hamming distance query (SQLite doesn't support hamming natively)
WITH target AS (
    SELECT fingerprint FROM configuration_fingerprints 
    WHERE quote_id = ?
)
SELECT 
    cf.quote_id,
    cf.fingerprint,
    -- Hamming distance via bit operations
    (
        LENGTH(REPLACE(HEX(cf.fingerprint & (SELECT fingerprint FROM target)), '0', '')) +
        LENGTH(REPLACE(HEX(~cf.fingerprint & (SELECT fingerprint FROM target)), '0', ''))
    ) / 2 as hamming_distance,
    do.outcome_status,
    do.final_price
FROM configuration_fingerprints cf
LEFT JOIN deal_outcomes do ON cf.quote_id = do.quote_id
WHERE cf.quote_id != ?
HAVING hamming_distance <= 15  -- Configurable threshold
ORDER BY hamming_distance ASC
LIMIT 5;
```

**Optimized with Hex Comparison:**
```sql
-- Faster query using hex prefix matching (first 32 bits)
SELECT 
    cf.quote_id,
    cf.fingerprint_hex,
    do.outcome_status,
    do.final_price
FROM configuration_fingerprints cf
LEFT JOIN deal_outcomes do ON cf.quote_id = do.quote_id
WHERE cf.fingerprint_hex LIKE ?  -- First 8 hex chars match
  AND cf.quote_id != ?
ORDER BY cf.fingerprint_hex
LIMIT 20;
```

---

## 5. Implementation Architecture

### 5.1 Core Components

```rust
// crates/core/src/dna/mod.rs

pub struct DealDnaEngine {
    fingerprint_generator: FingerprintGenerator,
    similarity_engine: SimilarityEngine,
    outcome_store: Arc<dyn OutcomeStore>,
}

pub struct FingerprintGenerator {
    hash_dimensions: usize,  // 128
    feature_hasher: FeatureHasher,
}

pub struct SimilarityEngine {
    threshold: f64,  // 0.8 = 80% similar
    max_results: usize,  // 5
}

pub struct SimilarDeal {
    pub quote_id: QuoteId,
    pub similarity_score: f64,
    pub hamming_distance: u32,
    pub outcome: Option<DealOutcome>,
}
```

### 5.2 Fingerprint Generation Flow

```rust
impl FingerprintGenerator {
    pub fn generate(&self, quote: &Quote) -> ConfigurationFingerprint {
        // 1. Extract features from quote
        let features = self.extract_features(quote);
        
        // 2. Weight features
        let weighted = self.apply_weights(features);
        
        // 3. Compute SimHash
        let simhash = self.compute_simhash(&weighted);
        
        // 4. Create fingerprint
        ConfigurationFingerprint {
            hash: simhash,
            feature_count: weighted.len(),
            quote_id: quote.id.clone(),
        }
    }
    
    fn extract_features(&self, quote: &Quote) -> Vec<Feature> {
        let mut features = Vec::new();
        
        // Product IDs
        for line in &quote.lines {
            features.push(Feature::Product(line.product_id.clone()));
            features.push(Feature::Quantity(line.product_id.clone(), line.quantity));
        }
        
        // Customer segment
        if let Some(ref segment) = quote.customer_segment {
            features.push(Feature::Segment(segment.clone()));
        }
        
        // Value tier
        let total = quote.calculate_subtotal();
        features.push(Feature::ValueTier(ValueRange::from_amount(total)));
        
        features
    }
}
```

### 5.3 Similarity Search Flow

```rust
impl SimilarityEngine {
    pub async fn find_similar(
        &self,
        fingerprint: &ConfigurationFingerprint,
        store: &dyn FingerprintStore,
    ) -> Result<Vec<SimilarDeal>, SimilarityError> {
        // 1. Load candidate fingerprints
        let candidates = store.load_all_fingerprints().await?;
        
        // 2. Compute similarities in parallel
        let mut similarities: Vec<_> = candidates
            .into_par_iter()
            .filter(|c| c.quote_id != fingerprint.quote_id)
            .map(|candidate| {
                let score = self.compute_similarity(fingerprint, &candidate);
                (candidate, score)
            })
            .filter(|(_, score)| *score >= self.threshold)
            .collect();
        
        // 3. Sort by similarity (descending)
        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        // 4. Take top N
        similarities.truncate(self.max_results);
        
        // 5. Enrich with outcome data
        self.enrich_with_outcomes(similarities).await
    }
    
    fn compute_similarity(
        &self,
        a: &ConfigurationFingerprint,
        b: &ConfigurationFingerprint,
    ) -> f64 {
        let distance = (a.hash ^ b.hash).count_ones();
        1.0 - (distance as f64 / 128.0)
    }
}
```

---

## 6. Performance Optimizations

### 6.1 Indexing Strategies

```rust
// Multi-level indexing for fast lookups
pub struct FingerprintIndex {
    // Level 1: First 32 bits (4 bytes) → Vec<QuoteId>
    prefix_index: HashMap<u32, Vec<QuoteId>>,
    
    // Level 2: Full fingerprints for exact candidates
    full_fingerprints: HashMap<QuoteId, u128>,
}

impl FingerprintIndex {
    pub fn insert(&mut self, quote_id: QuoteId, fingerprint: u128) {
        let prefix = (fingerprint >> 96) as u32;  // First 32 bits
        self.prefix_index.entry(prefix)
            .or_default()
            .push(quote_id.clone());
        self.full_fingerprints.insert(quote_id, fingerprint);
    }
    
    pub fn find_candidates(&self, fingerprint: u128) -> Vec<QuoteId> {
        let prefix = (fingerprint >> 96) as u32;
        self.prefix_index.get(&prefix)
            .cloned()
            .unwrap_or_default()
    }
}
```

### 6.2 Caching Strategy

```rust
pub struct SimilarityCache {
    lru: LruCache<(QuoteId, QuoteId), SimilarityScore>,
    ttl: Duration,  // 1 hour
}

impl SimilarityCache {
    pub async fn get_or_compute(
        &mut self,
        a: &QuoteId,
        b: &QuoteId,
        compute: impl FnOnce() -> SimilarityScore,
    ) -> SimilarityScore {
        let key = if a < b { (a.clone(), b.clone()) } else { (b.clone(), a.clone()) };
        
        if let Some(score) = self.lru.get(&key) {
            return *score;
        }
        
        let score = compute();
        self.lru.put(key, score);
        score
    }
}
```

---

## 7. Integration Points

### 7.1 Quote Finalization Hook

```rust
// In FlowEngine::transition_to(Finalized)
pub async fn on_quote_finalized(
    &self,
    quote: &Quote,
    dna_engine: &DealDnaEngine,
) -> Result<(), FlowError> {
    // Generate fingerprint
    let fingerprint = dna_engine.generate_fingerprint(quote).await?;
    
    // Store fingerprint
    dna_engine.store_fingerprint(fingerprint).await?;
    
    // Find similar deals for context
    let similar = dna_engine.find_similar(quote.id.clone()).await?;
    
    // Store similarity results for quick retrieval
    dna_engine.cache_similarity_results(quote.id.clone(), similar).await?;
    
    Ok(())
}
```

### 7.2 Slack Card Rendering

```rust
pub fn render_similarity_card(similar_deals: &[SimilarDeal]) -> MessageTemplate {
    let mut builder = MessageBuilder::new("Similar deals found");
    
    // Summary section
    let avg_price = similar_deals.iter()
        .filter_map(|d| d.outcome.as_ref()?.final_price)
        .fold(Decimal::ZERO, |acc, p| acc + p) 
        / Decimal::from(similar_deals.len());
    
    builder = builder.section("summary", |s| {
        s.mrkdwn(format!(
            "*Similar Deals Found*\n{} similar configurations\nAvg close price: ${}",
            similar_deals.len(),
            avg_price
        ))
    });
    
    // Individual deal cards
    for deal in similar_deals.iter().take(3) {
        builder = builder.context(&deal.quote_id.0, |c| {
            c.mrkdwn(format!(
                "{}% match | {} | ${}",
                (deal.similarity_score * 100.0) as u32,
                deal.outcome.as_ref().map(|o| o.status.to_string()).unwrap_or("Pending".to_string()),
                deal.outcome.as_ref().and_then(|o| o.final_price).unwrap_or(Decimal::ZERO)
            ))
        });
    }
    
    builder.build()
}
```

---

## 8. Testing Strategy

### 8.1 Determinism Tests

```rust
#[test]
fn fingerprint_is_deterministic() {
    let quote = test_quote();
    let gen = FingerprintGenerator::new();
    
    let fp1 = gen.generate(&quote);
    let fp2 = gen.generate(&quote);
    
    assert_eq!(fp1.hash, fp2.hash);
}

#[test]
fn similar_quotes_have_high_similarity() {
    let quote_a = test_quote_with_products(vec!["prod_1", "prod_2"]);
    let quote_b = test_quote_with_products(vec!["prod_1", "prod_2", "prod_3"]);  // One extra
    
    let gen = FingerprintGenerator::new();
    let fp_a = gen.generate(&quote_a);
    let fp_b = gen.generate(&quote_b);
    
    let similarity = SimilarityEngine::compute_similarity(&fp_a, &fp_b);
    assert!(similarity > 0.7, "Similar quotes should have >70% similarity");
}
```

### 8.2 Performance Tests

```rust
#[tokio::test]
async fn similarity_query_under_100ms() {
    let engine = setup_engine_with_10000_quotes().await;
    let target = test_fingerprint();
    
    let start = Instant::now();
    let results = engine.find_similar(&target).await.unwrap();
    let elapsed = start.elapsed();
    
    assert!(elapsed < Duration::from_millis(100));
    assert!(!results.is_empty());
}
```

---

## 9. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Fingerprint collisions | Medium | 128-bit hash + validation |
| Query performance degrades | High | Multi-level indexing + caching |
| Similarity threshold too strict | Medium | Configurable threshold per segment |
| Outcome data incomplete | Low | Graceful degradation to partial results |
| Feature extraction changes break determinism | High | Versioned feature extractors |

---

## 10. References

1. **MinHash Original Paper**: Broder, A.Z. (1997) "On the resemblance and containment of documents"
2. **SimHash**: Charikar, M.S. (2002) "Similarity estimation techniques from rounding algorithms"
3. **LSH Survey**: Wang et al. (2014) "Learning to Hash for Indexing Big Data"
4. **Rust fasthash crate**: github.com/flier/rust-fasthash

---

*Research compiled by ResearchAgent for the quotey project.*
