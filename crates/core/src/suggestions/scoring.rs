//! Scoring algorithms for product suggestions

use chrono::{DateTime, Utc};

use super::types::*;
use super::{MAX_PER_CATEGORY, MIN_SUGGESTION_SCORE};

/// Weights for scoring components
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScoringWeights {
    /// Weight for similar customer score (default: 0.40)
    pub similar_customer: f64,
    /// Weight for product relationship score (default: 0.30)
    pub product_relationship: f64,
    /// Weight for time decay score (default: 0.20)
    pub time_decay: f64,
    /// Weight for business rule boost (default: 0.10)
    pub business_rule: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        super::DEFAULT_WEIGHTS
    }
}

/// Score calculator for product suggestions
#[derive(Debug, Clone)]
pub struct ScoreCalculator {
    weights: ScoringWeights,
}

impl ScoreCalculator {
    /// Create a new score calculator with default weights
    pub fn new() -> Self {
        Self { weights: ScoringWeights::default() }
    }

    /// Create with custom weights
    pub fn with_weights(weights: ScoringWeights) -> Self {
        Self { weights }
    }

    /// Calculate total score for a product
    pub fn calculate_total_score(&self, component_scores: &ComponentScores) -> f64 {
        let total = component_scores.similar_customer * self.weights.similar_customer
            + component_scores.product_relationship * self.weights.product_relationship
            + component_scores.time_decay * self.weights.time_decay
            + component_scores.business_rule * self.weights.business_rule;

        total.min(1.0) // Cap at 1.0
    }

    /// Calculate similar customer score
    pub fn similar_customer_score(
        &self,
        _customer: &CustomerProfile,
        _product: &ProductInfo,
        similar_customers: &[(CustomerProfile, f64)],
        purchase_counts: &std::collections::HashMap<String, u32>,
    ) -> f64 {
        if similar_customers.is_empty() {
            return 0.0;
        }

        let mut total_weight = 0.0;
        let mut weighted_purchases = 0.0;

        for (similar_customer, similarity) in similar_customers {
            let purchase_count = purchase_counts.get(&similar_customer.id).copied().unwrap_or(0);

            // Weight by similarity and normalize by purchase count
            let normalized = (purchase_count as f64).min(5.0) / 5.0; // Cap at 5 purchases
            weighted_purchases += similarity * normalized;
            total_weight += similarity;
        }

        if total_weight == 0.0 {
            0.0
        } else {
            (weighted_purchases / total_weight).min(1.0)
        }
    }

    /// Calculate product relationship score
    pub fn product_relationship_score(
        &self,
        current_products: &[ProductInfo],
        candidate: &ProductInfo,
        relationships: &[ProductRelationship],
    ) -> f64 {
        if current_products.is_empty() || relationships.is_empty() {
            return 0.0;
        }

        let mut max_score: f64 = 0.0;

        for current in current_products {
            // Find relationship between current and candidate
            for rel in relationships {
                if (rel.source_product_id == current.id && rel.target_product_id == candidate.id)
                    || (rel.source_product_id == candidate.id
                        && rel.target_product_id == current.id)
                {
                    let score = rel.relationship_type.base_score() * rel.confidence;
                    max_score = max_score.max(score);
                }
            }
        }

        max_score
    }

    /// Calculate time decay score
    pub fn time_decay_score(
        &self,
        _product: &ProductInfo,
        recent_purchases: &[DateTime<Utc>],
        seasonal_data: Option<&SeasonalPattern>,
    ) -> f64 {
        let now = Utc::now();

        // Recency score (exponential decay)
        let recency_score = if recent_purchases.is_empty() {
            0.5 // Neutral if no data
        } else {
            let avg_days_ago: f64 =
                recent_purchases.iter().map(|date| (now - *date).num_days() as f64).sum::<f64>()
                    / recent_purchases.len() as f64;

            // Exponential decay with 180-day half-life
            0.5f64.powf(avg_days_ago / 180.0)
        };

        // Seasonality score
        let seasonality_score = if let Some(seasonal) = seasonal_data {
            // Simple seasonal boost if product is popular this quarter
            if seasonal.avg_purchases > 1.0 {
                0.7 // Boost for seasonal products
            } else {
                0.5
            }
        } else {
            0.5 // Neutral if no seasonal data
        };

        // Combine scores (recency weighted more)
        recency_score * 0.7 + seasonality_score * 0.3
    }

    /// Calculate business rule boost
    pub fn business_rule_boost(
        &self,
        customer: &CustomerProfile,
        product: &ProductInfo,
        rules: &[BusinessRule],
    ) -> f64 {
        let mut total_boost = 0.0;

        for rule in rules {
            if !rule.active {
                continue;
            }

            let boost = match &rule.rule_type {
                BusinessRuleType::AlwaysSuggestForSegment { segment, product_id, boost }
                    if customer.segment == *segment && product.id == *product_id =>
                {
                    *boost
                }
                BusinessRuleType::MinimumDealSize { threshold, product_id, boost }
                    if product.unit_price >= *threshold && product.id == *product_id =>
                {
                    *boost
                }
                BusinessRuleType::NewProductPromotion {
                    product_id,
                    launch_date,
                    promotion_days,
                    boost,
                } if product.id == *product_id
                    && (Utc::now() - *launch_date).num_days() <= *promotion_days =>
                {
                    *boost
                }
                _ => 0.0,
            };

            total_boost += boost;
        }

        total_boost.min(1.0) // Cap at 1.0
    }

    /// Determine suggestion category based on component scores
    pub fn determine_category(&self, component_scores: &ComponentScores) -> SuggestionCategory {
        // Find the dominant factor
        let scores = [
            (component_scores.similar_customer, SuggestionCategory::SimilarCustomersBought),
            (component_scores.product_relationship, SuggestionCategory::ComplementaryProduct),
            (component_scores.time_decay, SuggestionCategory::SeasonalTrend),
            (component_scores.business_rule, SuggestionCategory::BusinessRule),
        ];

        // Return category with highest score
        scores
            .iter()
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .map(|(_, cat)| *cat)
            .unwrap_or(SuggestionCategory::SimilarCustomersBought)
    }

    /// Generate human-readable reasoning
    pub fn generate_reasoning(
        &self,
        component_scores: &ComponentScores,
        similar_customers: &[String],
    ) -> Vec<String> {
        let mut reasons = Vec::new();

        if component_scores.similar_customer > 0.5 && !similar_customers.is_empty() {
            let names: Vec<_> = similar_customers.iter().take(3).cloned().collect();
            reasons.push(format!("Similar customers ({}) purchased this", names.join(", ")));
        }

        if component_scores.product_relationship > 0.4 {
            reasons.push("Complements products in your current quote".to_string());
        }

        if component_scores.time_decay > 0.5 {
            reasons.push("Popular choice in current quarter".to_string());
        }

        if component_scores.business_rule > 0.0 {
            reasons.push("Recommended for your segment".to_string());
        }

        // Ensure at least one reason
        if reasons.is_empty() {
            reasons.push("Based on your customer profile".to_string());
        }

        reasons
    }

    /// Filter and sort suggestions, ensuring diversity
    pub fn filter_and_diversify(
        &self,
        mut suggestions: Vec<ProductSuggestion>,
        max_suggestions: usize,
    ) -> Vec<ProductSuggestion> {
        // Filter by minimum score
        suggestions.retain(|s| s.score >= MIN_SUGGESTION_SCORE);

        // Sort by score descending
        suggestions
            .sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // Ensure diversity - limit per category
        let mut category_counts: std::collections::HashMap<SuggestionCategory, usize> =
            std::collections::HashMap::new();
        let mut diverse: Vec<ProductSuggestion> = Vec::new();
        let mut overflow: Vec<ProductSuggestion> = Vec::new();

        for suggestion in suggestions {
            let count = category_counts.entry(suggestion.category).or_insert(0);
            if *count < MAX_PER_CATEGORY {
                diverse.push(suggestion);
                *count += 1;
            } else {
                overflow.push(suggestion);
            }
        }

        // Fill remaining slots with overflow
        let needed = max_suggestions.saturating_sub(diverse.len());
        diverse.extend(overflow.into_iter().take(needed));

        // Final limit
        diverse.truncate(max_suggestions);
        diverse
    }
}

impl Default for ScoreCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_level_from_score() {
        assert_eq!(ConfidenceLevel::from_score(0.85), ConfidenceLevel::High);
        assert_eq!(ConfidenceLevel::from_score(0.70), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::from_score(0.45), ConfidenceLevel::Low);
    }

    #[test]
    fn test_total_score_calculation() {
        let calculator = ScoreCalculator::new();
        let components = ComponentScores {
            similar_customer: 0.8,
            product_relationship: 0.6,
            time_decay: 0.5,
            business_rule: 0.2,
        };

        let total = calculator.calculate_total_score(&components);
        // (0.8 * 0.4) + (0.6 * 0.3) + (0.5 * 0.2) + (0.2 * 0.1) = 0.32 + 0.18 + 0.10 + 0.02 = 0.62
        assert!((total - 0.62).abs() < 0.01);
    }

    #[test]
    fn test_diversity_filtering() {
        let calculator = ScoreCalculator::new();

        let suggestions = vec![
            ProductSuggestion {
                product_id: "1".to_string(),
                product_name: "A".to_string(),
                product_sku: "SKU1".to_string(),
                score: 0.9,
                confidence: ConfidenceLevel::High,
                reasoning: vec![],
                category: SuggestionCategory::SimilarCustomersBought,
                component_scores: ComponentScores::default(),
            },
            ProductSuggestion {
                product_id: "2".to_string(),
                product_name: "B".to_string(),
                product_sku: "SKU2".to_string(),
                score: 0.85,
                confidence: ConfidenceLevel::High,
                reasoning: vec![],
                category: SuggestionCategory::SimilarCustomersBought,
                component_scores: ComponentScores::default(),
            },
            ProductSuggestion {
                product_id: "3".to_string(),
                product_name: "C".to_string(),
                product_sku: "SKU3".to_string(),
                score: 0.8,
                confidence: ConfidenceLevel::High,
                reasoning: vec![],
                category: SuggestionCategory::ComplementaryProduct,
                component_scores: ComponentScores::default(),
            },
        ];

        let filtered = calculator.filter_and_diversify(suggestions, 5);

        // Should limit SimilarCustomersBought to 2, include ComplementaryProduct
        let similar_count = filtered
            .iter()
            .filter(|s| s.category == SuggestionCategory::SimilarCustomersBought)
            .count();
        assert!(similar_count <= MAX_PER_CATEGORY);
    }

    #[test]
    fn test_relationship_type_scores() {
        assert_eq!(RelationshipType::Bundle.base_score(), 1.0);
        assert_eq!(RelationshipType::AddOn.base_score(), 0.8);
        assert_eq!(RelationshipType::Upgrade.base_score(), 0.6);
        assert_eq!(RelationshipType::CrossSell.base_score(), 0.4);
    }
}
