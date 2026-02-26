# Product Suggestion Engine - Similarity Scoring Algorithm

**Task:** quotey-002-1  
**Status:** In Progress  
**Created:** 2026-02-26

## Overview

This document describes the similarity scoring algorithm for the Smart Product Suggestions Engine (EPIC-V2-SUGGEST). The algorithm determines which products to recommend for a customer based on historical data, customer attributes, and product relationships.

## Algorithm Components

### 1. Customer Similarity

Finds customers most similar to the target customer based on attributes:

```rust
fn customer_similarity(target: &Customer, candidate: &Customer) -> f64 {
    let segment_score = if target.segment == candidate.segment { 1.0 } else { 0.0 };
    let industry_score = if target.industry == candidate.industry { 1.0 } else { 0.0 };
    let region_score = if target.region == candidate.region { 1.0 } else { 0.0 };
    let size_score = 1.0 - (target.employee_count - candidate.employee_count).abs() as f64 / MAX_EMPLOYEE_DIFF;
    
    (segment_score * 0.4) + (industry_score * 0.3) + (region_score * 0.2) + (size_score * 0.1)
}
```

**Weights:**
- Segment match: 40% (most indicative of similar needs)
- Industry match: 30% (similar pain points)
- Region match: 20% (similar pricing/tax considerations)
- Company size: 10% (rough proxy for scale needs)

### 2. Purchase Pattern Analysis

Analyzes what similar customers purchased:

```rust
fn purchase_pattern_score(
    target_customer: &CustomerId,
    product_id: &ProductId,
    historical_quotes: &[Quote],
) -> f64 {
    let similar_customers = find_similar_customers(target_customer);
    let total_similar = similar_customers.len() as f64;
    
    let purchase_count = similar_customers
        .iter()
        .filter(|c| c.purchased(product_id))
        .count() as f64;
    
    purchase_count / total_similar.max(1.0)
}
```

### 3. Product Relationship Scoring

Evaluates product relationships (bundles, add-ons, replacements):

```rust
fn product_relationship_score(
    existing_lines: &[QuoteLine],
    candidate_product: &Product,
    product_graph: &ProductGraph,
) -> f64 {
    let mut score = 0.0;
    
    // Bundle bonus: product is commonly bundled with existing selections
    for line in existing_lines {
        if product_graph.is_bundled_with(line.product_id, candidate_product.id) {
            score += 0.4;
        }
    }
    
    // Add-on bonus: product is commonly added to existing products
    if let Some(attributes) = &candidate_product.attributes {
        if attributes.is_addon {
            score += 0.3;
        }
    }
    
    // Upgrade path: product is an upgrade of existing
    if product_graph.is_upgrade_of(candidate_product.id, existing_lines.iter().map(|l| l.product_id)) {
        score += 0.3;
    }
    
    score.min(1.0)
}
```

### 4. Time-Decay Factor

Applies recency weighting to purchase patterns:

```rust
fn time_decay(quote_date: DateTime<Utc>, current_date: DateTime<Utc>) -> f64 {
    let days_ago = (current_date - quote_date).num_days() as f64;
    let half_life = 90.0; // 90-day half-life
    
    0.5_f64.powf(days_ago / half_life)
}
```

**Rationale:**
- Recent purchases more indicative of current needs
- 90-day half-life balances recency vs. historical pattern
- Q4 2025 deals weighted ~3x more than Q1 2025

### 5. Business Rule Boost

Applies configurable business rules:

```rust
fn business_rule_boost(
    customer: &Customer,
    product: &Product,
    rules: &[SuggestionRule],
) -> f64 {
    rules
        .iter()
        .filter(|r| r.matches(customer, product))
        .map(|r| r.boost)
        .sum::<f64>()
        .min(0.2) // Cap at 20% additional boost
}
```

**Example rules:**
- Always suggest onboarding for Enterprise segment (+0.1)
- Always suggest SSO for FinTech industry (+0.15)
- Suggest premium support for 100+ seat deals (+0.1)

## Final Scoring Formula

```rust
fn calculate_suggestion_score(
    customer: &Customer,
    product: &Product,
    context: &QuoteContext,
    data: &SuggestionData,
) -> ProductSuggestion {
    let customer_sim = customer_similarity(&context.customer, &customer);
    let purchase_score = purchase_pattern_score(&context.customer.id, &product.id, &data.quotes);
    let relationship_score = product_relationship_score(&context.lines, &product, &data.product_graph);
    let time_weighted_purchases = data.recent_quotes
        .iter()
        .map(|q| (purchase_pattern_score(&q.customer_id, &product.id, &data.quotes), time_decay(q.created_at, Utc::now())))
        .map(|(score, decay)| score * decay)
        .sum::<f64>() / data.recent_quotes.len().max(1) as f64;
    let rule_boost = business_rule_boost(&context.customer, &product, &data.rules);
    
    let final_score = 
        (customer_sim * purchase_score * 0.4) +
        (relationship_score * 0.3) +
        (time_weighted_purchases * 0.2) +
        rule_boost;
    
    ProductSuggestion {
        product: product.clone(),
        score: final_score,
        reasoning: generate_reasoning(customer_sim, purchase_score, relationship_score, rule_boost),
    }
}
```

## Scoring Weights Rationale

| Component | Weight | Rationale |
|-----------|--------|-----------|
| Customer similarity + Purchase pattern | 40% | Core collaborative filtering signal |
| Product relationships | 30% | Domain expertise encoded in product graph |
| Time-weighted purchases | 20% | Recency matters, but historical patterns still valuable |
| Business rules | 10% | Override/boost for known patterns |

## Examples

### Example 1: Enterprise FinTech Customer

**Input:**
- Customer: Acme Corp, Enterprise segment, FinTech industry, 500 employees
- Context: New quote started, no products selected yet

**Scoring for "SOC2 Compliance Add-on":**
- Customer similarity: Matches Enterprise (0.4) + FinTech (0.3) = 0.7
- Purchase pattern: 8/10 similar customers purchased = 0.8
- Product relationship: Common add-on for Enterprise = 0.3
- Time decay: Recent purchases weighted 1.0x
- Business rule: FinTech industry rule (+0.15)

**Final score:** (0.7 × 0.8 × 0.4) + (0.3 × 0.3) + (0.8 × 0.2) + 0.15 = **0.58**

### Example 2: SMB Retail Customer

**Input:**
- Customer: ShopLocal, SMB segment, Retail industry, 25 employees
- Context: Existing quote has "Basic Plan" selected

**Scoring for "Basic Support":**
- Customer similarity: Matches SMB (0.4) + Retail (0.3) = 0.7
- Purchase pattern: 3/10 similar = 0.3
- Product relationship: Bundled with Basic Plan = 0.4
- Time decay: Older purchases 0.5x weight
- Business rule: None matched = 0

**Final score:** (0.7 × 0.3 × 0.4) + (0.4 × 0.3) + (0.3 × 0.5 × 0.2) + 0 = **0.29**

## Implementation Notes

### Data Requirements

1. **Customer similarity lookup:** Indexed queries on segment, industry, region
2. **Purchase history:** Join quotes → quote_lines → products
3. **Product graph:** Precomputed adjacency list for bundles/add-ons/upgrades
4. **Business rules:** Configurable rules stored in SQLite

### Caching Strategy

- Cache suggestions per customer + context hash
- Refresh hourly or on quote state change
- Invalidate on: new closed-won quote, product catalog change, rule change

### Privacy Considerations

- All recommendations based on aggregate anonymized data
- No individual customer data leaked in suggestions
- Configurable privacy mode: disable purchase pattern analysis

## Future Enhancements

1. **ML-based scoring:** Replace heuristic weights with learned model
2. **Negative signals:** Learn from rejected suggestions
3. **A/B testing framework:** Test different weight configurations
4. **Multi-objective optimization:** Balance relevance vs. margin

---

*Design document for quotey-002-1*
