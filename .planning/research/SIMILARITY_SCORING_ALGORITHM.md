# Similarity Scoring Algorithm Design

**Task:** quotey-002-1: Design similarity scoring algorithm  
**Date:** 2026-02-26  
**Author:** Kimi (AI Agent)  
**Epic:** quotey-002: Smart Product Suggestions Engine

---

## Overview

This document defines the similarity scoring algorithm for the Smart Product Suggestions Engine. The algorithm determines which products to suggest to a sales rep based on customer context and historical patterns.

---

## Algorithm Goals

1. **Relevance**: Suggest products the customer is likely to need
2. **Diversity**: Surface different types of recommendations (not just the same products)
3. **Explainability**: Provide clear reasoning for each suggestion
4. **Performance**: Compute scores in <100ms for real-time suggestions

---

## Scoring Components

The final score is a weighted combination of four components:

```
score = (similar_customer_score × 0.40) +
        (product_relationship_score × 0.30) +
        (time_decay_score × 0.20) +
        (business_rule_boost × 0.10)
```

### 1. Similar Customer Score (40%)

Find customers similar to the current customer and score products based on what they purchased.

**Customer Similarity Factors:**
| Factor | Weight | How Measured |
|--------|--------|--------------|
| Segment | 35% | Exact match (Enterprise, SMB, etc.) |
| Industry | 25% | Category match (FinTech, Healthcare, etc.) |
| Company Size | 20% | Log-scale proximity (employee count) |
| Region | 15% | Exact match (US, EU, APAC) |
| Deal Size History | 5% | Average deal value percentile |

**Calculation:**
```rust
fn customer_similarity(c1: &Customer, c2: &Customer) -> f64 {
    let segment_score = if c1.segment == c2.segment { 1.0 } else { 0.0 };
    let industry_score = if c1.industry == c2.industry { 1.0 } else { 0.0 };
    let size_score = log_similarity(c1.employee_count, c2.employee_count);
    let region_score = if c1.region == c2.region { 1.0 } else { 0.0 };
    let deal_size_score = percentile_similarity(c1.avg_deal_size, c2.avg_deal_size);
    
    segment_score * 0.35 +
    industry_score * 0.25 +
    size_score * 0.20 +
    region_score * 0.15 +
    deal_size_score * 0.05
}
```

**Product Scoring from Similar Customers:**
```rust
fn similar_customer_score(customer: &Customer, product: &Product) -> f64 {
    // Find top 20 most similar customers who bought this product
    let similar_customers: Vec<(Customer, f64)> = find_similar_customers(customer, 20);
    
    let mut total_weight = 0.0;
    let mut weighted_purchases = 0.0;
    
    for (similar_customer, similarity) in similar_customers {
        let purchase_count = get_purchase_count(&similar_customer, product);
        weighted_purchases += similarity * purchase_count as f64;
        total_weight += similarity;
    }
    
    if total_weight == 0.0 {
        0.0
    } else {
        weighted_purchases / total_weight
    }
}
```

### 2. Product Relationship Score (30%)

Score based on relationships between products (bundles, add-ons, etc.).

**Relationship Types:**
| Type | Weight | Description |
|------|--------|-------------|
| Bundle | 40% | Products frequently bought together |
| Add-on | 30% | Natural add-ons to existing products |
| Upgrade | 20% | Higher-tier versions |
| Cross-sell | 10% | Related but different categories |

**Calculation:**
```rust
fn product_relationship_score(
    current_products: &[Product],
    candidate: &Product
) -> f64 {
    let mut max_score = 0.0;
    
    for current in current_products {
        let relationship = get_relationship(current, candidate);
        let score = match relationship.relationship_type {
            Bundle => 1.0 * relationship.confidence,
            AddOn => 0.8 * relationship.confidence,
            Upgrade => 0.6 * relationship.confidence,
            CrossSell => 0.4 * relationship.confidence,
            None => 0.0,
        };
        max_score = max_score.max(score);
    }
    
    max_score
}
```

**Relationship Confidence Calculation:**
```
confidence = co_occurrence_count / min(product_a_purchases, product_b_purchases)
```

### 3. Time Decay Score (20%)

Account for temporal trends and seasonality.

**Time Factors:**
| Factor | Weight | Description |
|--------|--------|-------------|
| Recency | 50% | Recent purchases weighted higher |
| Seasonality | 30% | Products popular in current quarter |
| Trending | 20% | Products with increasing purchase rate |

**Recency Decay:**
```rust
fn recency_score(purchase_date: DateTime<Utc>) -> f64 {
    let days_ago = (Utc::now() - purchase_date).num_days();
    // Exponential decay with 180-day half-life
    0.5f64.powf(days_ago as f64 / 180.0)
}
```

**Seasonality:**
```rust
fn seasonality_score(product: &Product, current_month: u32) -> f64 {
    let seasonal_data = get_seasonal_pattern(product);
    let current_quarter = (current_month - 1) / 3;
    
    // Compare current quarter to historical averages
    let q_avg = seasonal_data.quarterly_averages[current_quarter as usize];
    let overall_avg = seasonal_data.overall_average;
    
    if overall_avg == 0.0 {
        0.5 // Neutral if no data
    } else {
        (q_avg / overall_avg).clamp(0.0, 1.0)
    }
}
```

### 4. Business Rule Boost (10%)

Configurable rules for business priorities.

**Rule Types:**
```rust
enum BusinessRule {
    /// Always suggest for specific segment
    AlwaysSuggestForSegment {
        segment: String,
        product_id: String,
        boost: f64, // 0.0 to 1.0
    },
    /// Minimum deal size threshold
    MinimumDealSize {
        threshold: f64,
        product_id: String,
        boost: f64,
    },
    /// New product promotion
    NewProductPromotion {
        product_id: String,
        launch_date: DateTime<Utc>,
        promotion_days: i64,
        boost: f64,
    },
}
```

**Boost Application:**
```rust
fn business_rule_boost(customer: &Customer, product: &Product) -> f64 {
    let rules = get_active_business_rules();
    let mut total_boost = 0.0;
    
    for rule in rules {
        let boost = match rule {
            AlwaysSuggestForSegment { segment, product_id, boost } 
                if customer.segment == *segment && product.id == *product_id => *boost,
            MinimumDealSize { threshold, product_id, boost }
                if product.unit_price >= *threshold && product.id == *product_id => *boost,
            NewProductPromotion { product_id, launch_date, promotion_days, boost }
                if product.id == *product_id 
                    && (Utc::now() - *launch_date).num_days() <= *promotion_days => *boost,
            _ => 0.0,
        };
        total_boost += boost;
    }
    
    total_boost.min(1.0) // Cap at 1.0
}
```

---

## Final Scoring Algorithm

```rust
pub struct SuggestionRequest {
    pub customer_id: String,
    pub current_products: Vec<String>,
    pub quote_context: Option<QuoteContext>,
    pub max_suggestions: usize,
}

pub struct ProductSuggestion {
    pub product_id: String,
    pub product_name: String,
    pub score: f64,
    pub confidence: ConfidenceLevel, // High, Medium, Low
    pub reasoning: Vec<String>,
    pub category: SuggestionCategory,
}

pub enum SuggestionCategory {
    SimilarCustomersBought,
    ComplementaryProduct,
    BundleRecommendation,
    SeasonalTrend,
    BusinessRule,
}

pub struct SuggestionEngine {
    db_pool: DbPool,
    weights: ScoringWeights,
}

impl SuggestionEngine {
    pub async fn get_suggestions(
        &self,
        request: SuggestionRequest,
    ) -> Result<Vec<ProductSuggestion>, Error> {
        let customer = self.get_customer(&request.customer_id).await?;
        let current_products = self.get_products(&request.current_products).await?;
        let candidates = self.get_candidate_products(&customer).await?;
        
        let mut scored_products: Vec<ProductSuggestion> = Vec::new();
        
        for candidate in candidates {
            let similar_score = self.similar_customer_score(&customer, &candidate).await;
            let relationship_score = self.product_relationship_score(&current_products, &candidate);
            let time_score = self.time_decay_score(&candidate).await;
            let rule_boost = self.business_rule_boost(&customer, &candidate).await;
            
            let total_score = 
                similar_score * self.weights.similar_customer +
                relationship_score * self.weights.product_relationship +
                time_score * self.weights.time_decay +
                rule_boost * self.weights.business_rule;
            
            let reasoning = self.generate_reasoning(
                &customer, &candidate, similar_score, relationship_score, time_score, rule_boost
            ).await;
            
            let confidence = self.calculate_confidence(total_score);
            let category = self.determine_category(
                similar_score, relationship_score, time_score, rule_boost
            );
            
            scored_products.push(ProductSuggestion {
                product_id: candidate.id.clone(),
                product_name: candidate.name.clone(),
                score: total_score,
                confidence,
                reasoning,
                category,
            });
        }
        
        // Sort by score descending and take top N
        scored_products.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        scored_products.truncate(request.max_suggestions);
        
        // Ensure diversity - don't show only one category
        scored_products = self.ensure_diversity(scored_products, 5);
        
        Ok(scored_products)
    }
    
    fn ensure_diversity(
        &self,
        mut suggestions: Vec<ProductSuggestion>,
        max_per_category: usize,
    ) -> Vec<ProductSuggestion> {
        let mut category_counts: HashMap<SuggestionCategory, usize> = HashMap::new();
        let mut diverse_suggestions: Vec<ProductSuggestion> = Vec::new();
        let mut overflow: Vec<ProductSuggestion> = Vec::new();
        
        for suggestion in suggestions {
            let count = category_counts.entry(suggestion.category.clone()).or_insert(0);
            if *count < max_per_category {
                diverse_suggestions.push(suggestion);
                *count += 1;
            } else {
                overflow.push(suggestion);
            }
        }
        
        // Fill remaining slots with overflow if needed
        let remaining = suggestions.len() - diverse_suggestions.len();
        diverse_suggestions.extend(overflow.into_iter().take(remaining));
        
        diverse_suggestions
    }
}
```

---

## Reasoning Generation

Each suggestion includes human-readable reasoning:

```rust
async fn generate_reasoning(
    &self,
    customer: &Customer,
    product: &Product,
    similar_score: f64,
    relationship_score: f64,
    time_score: f64,
    rule_boost: f64,
) -> Vec<String> {
    let mut reasons = Vec::new();
    
    if similar_score > 0.6 {
        let similar_customers = self.get_top_similar_customers(customer, 3).await;
        let customer_names: Vec<_> = similar_customers.iter()
            .map(|c| c.name.clone())
            .collect();
        reasons.push(format!(
            "Similar customers ({}) purchased this product",
            customer_names.join(", ")
        ));
    }
    
    if relationship_score > 0.5 {
        reasons.push("Complements products in your current quote".to_string());
    }
    
    if time_score > 0.6 {
        reasons.push("Popular choice in current quarter".to_string());
    }
    
    if rule_boost > 0.0 {
        reasons.push("Recommended for your segment".to_string());
    }
    
    // Ensure at least one reason
    if reasons.is_empty() {
        reasons.push("Based on your customer profile".to_string());
    }
    
    reasons
}
```

---

## Confidence Levels

| Score Range | Confidence | Display |
|-------------|-----------|---------|
| 0.80 - 1.00 | High | "95% match" |
| 0.60 - 0.79 | Medium | "72% match" |
| 0.40 - 0.59 | Low | "48% match" |
| < 0.40 | Filtered | (Don't show) |

---

## Database Schema Additions

```sql
-- Customer similarity data (pre-computed)
CREATE TABLE customer_similarity (
    customer_id TEXT NOT NULL,
    similar_customer_id TEXT NOT NULL,
    similarity_score REAL NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (customer_id, similar_customer_id)
);

-- Product relationship data
CREATE TABLE product_relationship (
    id TEXT PRIMARY KEY,
    source_product_id TEXT NOT NULL,
    target_product_id TEXT NOT NULL,
    relationship_type TEXT NOT NULL, -- 'bundle', 'addon', 'upgrade', 'cross_sell'
    confidence REAL NOT NULL,
    co_occurrence_count INTEGER NOT NULL,
    updated_at TEXT NOT NULL
);

-- Seasonal patterns
CREATE TABLE product_seasonality (
    product_id TEXT NOT NULL,
    quarter INTEGER NOT NULL, -- 1, 2, 3, 4
    avg_purchases REAL NOT NULL,
    year INTEGER NOT NULL,
    PRIMARY KEY (product_id, quarter, year)
);

-- Business rules
CREATE TABLE suggestion_business_rule (
    id TEXT PRIMARY KEY,
    rule_type TEXT NOT NULL,
    rule_config_json TEXT NOT NULL,
    active BOOLEAN DEFAULT TRUE,
    priority INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Suggestion feedback for learning
CREATE TABLE suggestion_feedback (
    id TEXT PRIMARY KEY,
    customer_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    suggested_at TEXT NOT NULL,
    was_shown BOOLEAN NOT NULL,
    was_clicked BOOLEAN NOT NULL,
    was_added_to_quote BOOLEAN NOT NULL,
    context_json TEXT,
    feedback_score REAL -- Calculated value based on actions
);
```

---

## Performance Considerations

1. **Pre-computation**: Customer similarities computed nightly
2. **Caching**: Hot product relationships cached in memory
3. **Indexing**: All lookup fields indexed
4. **Batching**: Similar customer lookups batched

**Target Performance:**
- Cold start: <200ms
- Warm cache: <50ms
- Database queries: <30ms total

---

## Next Steps

1. **quotey-002-2**: Implement suggestion engine with this algorithm
2. **quotey-002-3**: Add suggestion UI to Slack
3. **quotey-002-4**: Add feedback loop for continuous improvement
