# Precedent Intelligence Graph (PRE) - Technical Research

**Feature:** Similar Deal Recommendations  
**Researcher:** ResearchAgent (kimi-k2)  **Date:** 2026-02-23  
**Priority:** P2

---

## 1. Technical Overview

ML-powered similarity matching for quote suggestions. Analyzes existing quotes to find "similar deals" and shows what was configured, priced, and won.

---

## 2. Deal Fingerprinting

### 2.1 Fingerprint Types

```rust
pub struct DealFingerprint {
    pub version: u8,
    pub customer_hash: u64,
    pub product_signature: ProductSignature,
    pub pricing_signature: PricingSignature,
    pub temporal_signature: TemporalSignature,
    pub outcome_signature: OutcomeSignature,
}

pub struct ProductSignature {
    pub product_ids: Vec<String>,
    pub category_vector: Vec<u32>,
    pub feature_flags: u64,
}

pub struct PricingSignature {
    pub total_list_price: Decimal,
    pub discount_tier: DiscountTier,
    pub term_months: u32,
}

pub enum DiscountTier {
    None,       // 0-5%
    Standard,   // 5-15%
    Preferred,  // 15-25%
    Strategic,  // 25-35%
    Exceptional, // 35%+
}

pub struct TemporalSignature {
    pub quarter: u8,
    pub month_of_quarter: u8,
    pub days_to_close: u32,
}

pub struct OutcomeSignature {
    pub won: bool,
    pub close_velocity_days: u32,
    pub discount_requested_pct: Decimal,
    pub final_discount_pct: Decimal,
}
```

### 2.2 Fingerprint Generation

```rust
pub struct FingerprintGenerator {
    hasher: XxHash64,
}

impl FingerprintGenerator {
    pub fn generate(&self, quote: &Quote, outcome: Option<&QuoteOutcome>) -> DealFingerprint {
        DealFingerprint {
            version: 1,
            customer_hash: self.hash_customer(&quote.customer),
            product_signature: self.generate_product_signature(&quote.lines),
            pricing_signature: self.generate_pricing_signature(quote),
            temporal_signature: self.generate_temporal_signature(quote, outcome),
            outcome_signature: outcome.map(|o| self.generate_outcome_signature(o)),
        }
    }
    
    fn hash_customer(&self, customer: &Customer) -> u64 {
        let mut hasher = XxHash64::default();
        customer.industry.hash(&mut hasher);
        customer.segment.hash(&mut hasher);
        customer.region.hash(&mut hasher);
        hasher.finish()
    }
    
    fn generate_product_signature(&self, lines: &[QuoteLine]) -> ProductSignature {
        let product_ids: Vec<String> = lines.iter()
            .map(|l| l.product_id.0.clone())
            .collect();
        
        let category_vector = self.vectorize_categories(&product_ids);
        let feature_flags = self.compute_feature_flags(lines);
        
        ProductSignature {
            product_ids,
            category_vector,
            feature_flags,
        }
    }
    
    fn vectorize_categories(&self, product_ids: &[String]) -> Vec<u32> {
        // One-hot encoding of product categories
        // Simplified - would use category taxonomy
        vec![product_ids.len() as u32]
    }
}
```

---

## 3. Similarity Engine

```rust
pub struct SimilarityEngine {
    index: Arc<AnnoyIndex>,
    fingerprint_store: Arc<dyn FingerprintStore>,
}

pub struct SimilarDeal {
    pub quote_id: String,
    pub fingerprint: DealFingerprint,
    pub similarity_score: f32,
    pub match_dimensions: Vec<MatchDimension>,
}

pub enum MatchDimension {
    ProductOverlap { score: f32, shared_products: Vec<String> },
    PricingSimilar { list_diff_pct: f32, discount_diff_pct: f32 },
    CustomerSimilar { industry_match: bool, segment_match: bool },
    TemporalSimilar { quarter_same: bool, close_velocity_similar: bool },
}

impl SimilarityEngine {
    /// Find similar deals
    pub fn find_similar(
        &self,
        reference: &DealFingerprint,
        filters: SimilarityFilters,
        limit: usize,
    ) -> Result<Vec<SimilarDeal>, SimilarityError> {
        // Vectorize reference fingerprint
        let reference_vector = self.vectorize_fingerprint(reference);
        
        // Query ANN index
        let nearest = self.index.get_nns_by_vector(
            &reference_vector,
            limit * 3,  // Over-fetch for filtering
            -1,
        )?;
        
        // Score and filter
        let mut scored: Vec<SimilarDeal> = nearest.iter()
            .filter_map(|id| self.score_similarity(reference, id, &filters))
            .filter(|d| d.similarity_score >= filters.min_score)
            .collect();
        
        // Sort by score descending
        scored.sort_by(|a, b| b.similarity_score.partial_cmp(&a.similarity_score).unwrap());
        
        Ok(scored.into_iter().take(limit).collect())
    }
    
    fn score_similarity(
        &self,
        reference: &DealFingerprint,
        candidate_id: &str,
        filters: &SimilarityFilters,
    ) -> Option<SimilarDeal> {
        let candidate = self.fingerprint_store.get(candidate_id)?;
        
        // Skip if outcome filter doesn't match
        if let Some(ref outcome_filter) = filters.outcome {
            match candidate.outcome_signature {
                Some(ref o) if o.won != *outcome_filter => return None,
                None => return None,
                _ => {}
            }
        }
        
        // Calculate match dimensions
        let product_match = self.score_product_similarity(
            &reference.product_signature,
            &candidate.product_signature,
        );
        
        let pricing_match = self.score_pricing_similarity(
            &reference.pricing_signature,
            &candidate.pricing_signature,
        );
        
        let customer_match = self.score_customer_similarity(
            reference.customer_hash,
            candidate.customer_hash,
        );
        
        // Weighted composite score
        let similarity_score = 
            product_match * 0.5 +
            pricing_match * 0.3 +
            customer_match * 0.2;
        
        Some(SimilarDeal {
            quote_id: candidate_id.to_string(),
            fingerprint: candidate,
            similarity_score,
            match_dimensions: vec![],  // Populated separately
        })
    }
    
    fn score_product_similarity(&self, a: &ProductSignature, b: &ProductSignature) -> f32 {
        // Jaccard similarity on product sets
        let set_a: HashSet<_> = a.product_ids.iter().collect();
        let set_b: HashSet<_> = b.product_ids.iter().collect();
        
        let intersection: HashSet<_> = set_a.intersection(&set_b).collect();
        let union: HashSet<_> = set_a.union(&set_b).collect();
        
        intersection.len() as f32 / union.len() as f32
    }
    
    fn score_pricing_similarity(&self, a: &PricingSignature, b: &PricingSignature) -> f32 {
        let list_ratio = std::cmp::min(a.total_list_price, b.total_list_price).to_f64().unwrap()
            / std::cmp::max(a.total_list_price, b.total_list_price).to_f64().unwrap();
        
        let discount_match = if a.discount_tier == b.discount_tier { 1.0 } else { 0.5 };
        
        (list_ratio as f32 * 0.7) + (discount_match * 0.3)
    }
}

pub struct SimilarityFilters {
    pub min_score: f32,
    pub outcome: Option<bool>,  // None = any, Some(true) = won only
    pub max_age_days: Option<u32>,
    pub exclude_quote_ids: Vec<String>,
}
```

---

## 4. Precedent Intelligence

```rust
pub struct PrecedentIntelligence {
    similarity_engine: Arc<SimilarityEngine>,
    outcome_analyzer: Arc<OutcomeAnalyzer>,
}

pub struct PrecedentAnalysis {
    pub similar_deals: Vec<SimilarDeal>,
    pub win_patterns: Vec<WinPattern>,
    pub pricing_baseline: PricingBaseline,
    pub recommendations: Vec<PrecedentRecommendation>,
}

pub struct WinPattern {
    pub pattern_type: PatternType,
    pub confidence: f32,
    pub evidence_count: u32,
    pub description: String,
}

pub enum PatternType {
    ProductBundle,
    DiscountStrategy,
    Timing,
    CustomerSegment,
}

pub struct PricingBaseline {
    pub avg_list_price: Decimal,
    pub avg_discount_pct: Decimal,
    pub discount_range: (Decimal, Decimal),
    pub win_rate_by_discount: Vec<(DiscountTier, f32)>,
}

pub struct PrecedentRecommendation {
    pub recommendation_type: RecommendationType,
    pub priority: u8,
    pub description: String,
    pub expected_impact: String,
}

pub enum RecommendationType {
    AddProduct { product_id: String },
    AdjustDiscount { suggested_pct: Decimal },
    AdjustTiming { suggested_close_date: DateTime<Utc> },
}

impl PrecedentIntelligence {
    pub fn analyze(&self, quote: &Quote) -> Result<PrecedentAnalysis, IntelligenceError> {
        let fingerprint = self.fingerprint_generator.generate(quote, None);
        
        // Find similar deals
        let similar = self.similarity_engine.find_similar(
            &fingerprint,
            SimilarityFilters {
                min_score: 0.5,
                outcome: None,
                max_age_days: Some(365),
                exclude_quote_ids: vec![quote.id.0.clone()],
            },
            10,
        )?;
        
        // Analyze patterns
        let patterns = self.outcome_analyzer.analyze_patterns(&similar);
        
        // Calculate pricing baseline
        let baseline = self.calculate_pricing_baseline(&similar);
        
        // Generate recommendations
        let recommendations = self.generate_recommendations(&similar, &patterns, &baseline);
        
        Ok(PrecedentAnalysis {
            similar_deals: similar,
            win_patterns: patterns,
            pricing_baseline: baseline,
            recommendations,
        })
    }
    
    fn calculate_pricing_baseline(&self, deals: &[SimilarDeal]) -> PricingBaseline {
        let outcomes: Vec<_> = deals.iter()
            .filter_map(|d| d.fingerprint.outcome_signature.as_ref())
            .collect();
        
        let total_discount: f64 = outcomes.iter()
            .map(|o| o.final_discount_pct.to_f64().unwrap())
            .sum();
        
        let avg_discount = total_discount / outcomes.len() as f64;
        
        PricingBaseline {
            avg_list_price: Decimal::ZERO,  // Calculate from deals
            avg_discount_pct: Decimal::from_f64(avg_discount).unwrap(),
            discount_range: (
                Decimal::from_f64(
                    outcomes.iter()
                        .map(|o| o.final_discount_pct.to_f64().unwrap())
                        .min_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap_or(0.0)
                ).unwrap(),
                Decimal::from_f64(
                    outcomes.iter()
                        .map(|o| o.final_discount_pct.to_f64().unwrap())
                        .max_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap_or(0.0)
                ).unwrap(),
            ),
            win_rate_by_discount: vec![],  // Calculate from outcomes
        }
    }
}
```

---

## 5. Slack Integration

```rust
pub fn render_similar_deals(analysis: &PrecedentAnalysis) -> MessageTemplate {
    let mut builder = MessageBuilder::new("Precedent Intelligence");
    
    // Header
    builder = builder.section("header", |s| {
        s.mrkdwn("📊 *Similar Deals Found*".to_string())
    });
    
    // Similar deals list
    let deals_text: Vec<String> = analysis.similar_deals.iter().take(5).enumerate()
        .map(|(i, deal)| {
            let outcome = deal.fingerprint.outcome_signature.as_ref()
                .map(|o| if o.won { "✅ Won" } else { "❌ Lost" })
                .unwrap_or("⏳ Open");
            
            format!(
                "{}. {} — {:.0}% match\n   {}",
                i + 1,
                deal.quote_id,
                deal.similarity_score * 100.0,
                outcome
            )
        })
        .collect();
    
    builder = builder.section("deals", |s| {
        s.mrkdwn(format!("*Top matches:*\n{}", deals_text.join("\n")))
    });
    
    // Pricing baseline
    builder = builder.section("pricing", |s| {
        s.mrkdwn(format!(
            "*Pricing baseline:*\nAvg discount: {:.1}% | Range: {:.0}%-{:.0}%",
            analysis.pricing_baseline.avg_discount_pct,
            analysis.pricing_baseline.discount_range.0,
            analysis.pricing_baseline.discount_range.1
        ))
    });
    
    // Recommendations
    if !analysis.recommendations.is_empty() {
        let recs_text: Vec<String> = analysis.recommendations.iter()
            .map(|r| format!(
                "• {} (impact: {})",
                r.description,
                r.expected_impact
            ))
            .collect();
        
        builder = builder.section("recommendations", |s| {
            s.mrkdwn(format!("*Recommendations:*\n{}", recs_text.join("\n")))
        });
    }
    
    builder.build()
}
```

---

## 6. Data Storage

```sql
-- Deal fingerprints
CREATE TABLE deal_fingerprints (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL UNIQUE REFERENCES quotes(id),
    fingerprint_hash TEXT NOT NULL,
    customer_hash TEXT NOT NULL,
    product_ids_json TEXT NOT NULL,
    category_vector_json TEXT NOT NULL,
    list_price_total REAL NOT NULL,
    discount_tier TEXT NOT NULL,
    quarter INTEGER NOT NULL,
    month_of_quarter INTEGER NOT NULL,
    outcome_won BOOLEAN,
    outcome_close_velocity_days INTEGER,
    final_discount_pct REAL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- ANN index metadata
CREATE TABLE fingerprint_index_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    index_version TEXT NOT NULL,
    vector_dimension INTEGER NOT NULL,
    total_vectors INTEGER NOT NULL,
    built_at TEXT NOT NULL
);

-- Similarity queries log
CREATE TABLE similarity_queries (
    id TEXT PRIMARY KEY,
    reference_quote_id TEXT,
    filters_json TEXT NOT NULL,
    results_count INTEGER NOT NULL,
    query_duration_ms INTEGER NOT NULL,
    created_at TEXT NOT NULL
);
```

---

*Research compiled by ResearchAgent for the quotey project.*
