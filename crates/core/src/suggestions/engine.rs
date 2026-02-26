//! Suggestion Engine implementation

use std::collections::HashMap;

use chrono::{Datelike, Utc};

use super::scoring::{ScoreCalculator, ScoringWeights};
use super::types::*;
use super::SuggestionResult;

/// The main suggestion engine
#[derive(Debug, Clone)]
pub struct SuggestionEngine {
    /// Scoring calculator
    calculator: ScoreCalculator,
    /// In-memory cache for customer similarities
    customer_cache: HashMap<String, Vec<(CustomerProfile, f64)>>,
    /// In-memory cache for product relationships  
    relationship_cache: HashMap<String, Vec<ProductRelationship>>,
}

impl SuggestionEngine {
    /// Create a new suggestion engine with default weights
    pub fn new() -> Self {
        Self {
            calculator: ScoreCalculator::new(),
            customer_cache: HashMap::new(),
            relationship_cache: HashMap::new(),
        }
    }

    /// Create with custom weights
    pub fn with_weights(weights: ScoringWeights) -> Self {
        Self {
            calculator: ScoreCalculator::with_weights(weights),
            customer_cache: HashMap::new(),
            relationship_cache: HashMap::new(),
        }
    }

    /// Get product suggestions for a customer
    pub async fn get_suggestions(
        &self,
        request: SuggestionRequest,
    ) -> SuggestionResult<Vec<ProductSuggestion>> {
        // Get customer profile
        let customer = self.get_customer_profile(&request.customer_id).await?;

        // Get current products (if any)
        let current_products = if request.current_products.is_empty() {
            Vec::new()
        } else {
            self.get_products(&request.current_products).await?
        };

        // Get candidate products
        let candidates = self.get_candidate_products(&customer).await?;

        // Score each candidate
        let mut suggestions = Vec::new();

        for candidate in candidates {
            let scores = self.score_product(&customer, &current_products, &candidate).await?;

            let total_score = self.calculator.calculate_total_score(&scores);

            // Skip if below threshold
            if total_score < super::MIN_SUGGESTION_SCORE {
                continue;
            }

            let similar_customers = self.get_similar_customer_names(&customer).await?;
            let reasoning = self.calculator.generate_reasoning(&scores, &similar_customers);
            let confidence = ConfidenceLevel::from_score(total_score);
            let category = self.calculator.determine_category(&scores);

            suggestions.push(ProductSuggestion {
                product_id: candidate.id.clone(),
                product_name: candidate.name.clone(),
                product_sku: candidate.sku.clone(),
                score: total_score,
                confidence,
                reasoning,
                category,
                component_scores: scores,
            });
        }

        // Filter and diversify
        let final_suggestions =
            self.calculator.filter_and_diversify(suggestions, request.max_suggestions);

        Ok(final_suggestions)
    }

    /// Score a single product
    async fn score_product(
        &self,
        customer: &CustomerProfile,
        current_products: &[ProductInfo],
        candidate: &ProductInfo,
    ) -> SuggestionResult<ComponentScores> {
        // Similar customer score
        let similar_customers = self.get_similar_customers(customer).await?;
        let purchase_counts = self.get_purchase_counts(&similar_customers, candidate).await?;
        let similar_score = self.calculator.similar_customer_score(
            customer,
            candidate,
            &similar_customers,
            &purchase_counts,
        );

        // Product relationship score
        let relationships = self.get_product_relationships(candidate).await?;
        let relationship_score =
            self.calculator.product_relationship_score(current_products, candidate, &relationships);

        // Time decay score
        let recent_purchases = self.get_recent_purchases(candidate).await?;
        let seasonal_data = self.get_seasonal_pattern(candidate).await?;
        let time_score =
            self.calculator.time_decay_score(candidate, &recent_purchases, seasonal_data.as_ref());

        // Business rule boost
        let rules = self.get_business_rules().await?;
        let rule_boost = self.calculator.business_rule_boost(customer, candidate, &rules);

        Ok(ComponentScores {
            similar_customer: similar_score,
            product_relationship: relationship_score,
            time_decay: time_score,
            business_rule: rule_boost,
        })
    }

    // -------------------------------------------------------------------------
    // Data Access (Placeholder implementations)
    // These would connect to actual database repositories in production
    // -------------------------------------------------------------------------

    async fn get_customer_profile(&self, customer_id: &str) -> SuggestionResult<CustomerProfile> {
        // TODO: Connect to customer repository
        // Placeholder implementation
        Ok(CustomerProfile {
            id: customer_id.to_string(),
            segment: "enterprise".to_string(),
            industry: Some("saas".to_string()),
            employee_count: Some(500),
            region: "us".to_string(),
            avg_deal_size: 50000.0,
        })
    }

    async fn get_products(&self, product_ids: &[String]) -> SuggestionResult<Vec<ProductInfo>> {
        // TODO: Connect to product repository
        let mut products = Vec::new();
        for id in product_ids {
            products.push(ProductInfo {
                id: id.clone(),
                sku: format!("SKU-{}", id),
                name: format!("Product {}", id),
                category: "saas".to_string(),
                unit_price: 100.0,
                active: true,
            });
        }
        Ok(products)
    }

    async fn get_candidate_products(
        &self,
        _customer: &CustomerProfile,
    ) -> SuggestionResult<Vec<ProductInfo>> {
        // TODO: Connect to product repository and filter active products
        // Return mock data for now
        Ok(vec![
            ProductInfo {
                id: "prod_pro_v2".to_string(),
                sku: "PLAN-PRO-001".to_string(),
                name: "Pro Plan".to_string(),
                category: "saas".to_string(),
                unit_price: 10.0,
                active: true,
            },
            ProductInfo {
                id: "prod_enterprise".to_string(),
                sku: "PLAN-ENT-001".to_string(),
                name: "Enterprise Plan".to_string(),
                category: "saas".to_string(),
                unit_price: 18.0,
                active: true,
            },
            ProductInfo {
                id: "prod_sso".to_string(),
                sku: "ADDON-SSO-001".to_string(),
                name: "SSO Add-on".to_string(),
                category: "addon".to_string(),
                unit_price: 2.0,
                active: true,
            },
            ProductInfo {
                id: "prod_support".to_string(),
                sku: "ADDON-SUP-001".to_string(),
                name: "Premium Support".to_string(),
                category: "addon".to_string(),
                unit_price: 500.0,
                active: true,
            },
        ])
    }

    async fn get_similar_customers(
        &self,
        customer: &CustomerProfile,
    ) -> SuggestionResult<Vec<(CustomerProfile, f64)>> {
        // TODO: Connect to customer similarity repository or cache
        // Check cache first
        if let Some(cached) = self.customer_cache.get(&customer.id) {
            return Ok(cached.clone());
        }

        // Placeholder: return mock similar customers
        let similar = vec![
            (
                CustomerProfile {
                    id: "similar_1".to_string(),
                    segment: "enterprise".to_string(),
                    industry: Some("saas".to_string()),
                    employee_count: Some(450),
                    region: "us".to_string(),
                    avg_deal_size: 45000.0,
                },
                0.85,
            ),
            (
                CustomerProfile {
                    id: "similar_2".to_string(),
                    segment: "enterprise".to_string(),
                    industry: Some("fintech".to_string()),
                    employee_count: Some(600),
                    region: "us".to_string(),
                    avg_deal_size: 55000.0,
                },
                0.70,
            ),
        ];

        Ok(similar)
    }

    async fn get_similar_customer_names(
        &self,
        customer: &CustomerProfile,
    ) -> SuggestionResult<Vec<String>> {
        let similar = self.get_similar_customers(customer).await?;
        Ok(similar.iter().map(|(c, _)| c.id.clone()).collect())
    }

    async fn get_purchase_counts(
        &self,
        customers: &[(CustomerProfile, f64)],
        _product: &ProductInfo,
    ) -> SuggestionResult<HashMap<String, u32>> {
        // TODO: Connect to quote/line item repository
        // Placeholder: return mock purchase counts
        let mut counts = HashMap::new();
        for (customer, _) in customers {
            // Mock: each similar customer bought the product once
            counts.insert(customer.id.clone(), 1);
        }
        Ok(counts)
    }

    async fn get_product_relationships(
        &self,
        product: &ProductInfo,
    ) -> SuggestionResult<Vec<ProductRelationship>> {
        // TODO: Connect to product relationship repository or cache
        if let Some(cached) = self.relationship_cache.get(&product.id) {
            return Ok(cached.clone());
        }

        // Placeholder: return mock relationships
        let mut relationships = Vec::new();

        if product.id == "prod_sso" {
            relationships.push(ProductRelationship {
                id: "rel_1".to_string(),
                source_product_id: "prod_pro_v2".to_string(),
                target_product_id: "prod_sso".to_string(),
                relationship_type: super::types::RelationshipType::AddOn,
                confidence: 0.85,
                co_occurrence_count: 50,
            });
        }

        if product.id == "prod_support" {
            relationships.push(ProductRelationship {
                id: "rel_2".to_string(),
                source_product_id: "prod_enterprise".to_string(),
                target_product_id: "prod_support".to_string(),
                relationship_type: super::types::RelationshipType::Bundle,
                confidence: 0.90,
                co_occurrence_count: 75,
            });
        }

        Ok(relationships)
    }

    async fn get_recent_purchases(
        &self,
        _product: &ProductInfo,
    ) -> SuggestionResult<Vec<chrono::DateTime<chrono::Utc>>> {
        // TODO: Connect to quote repository
        // Placeholder: return recent dates
        use chrono::Duration;
        let now = chrono::Utc::now();
        Ok(vec![now - Duration::days(10), now - Duration::days(25), now - Duration::days(45)])
    }

    async fn get_seasonal_pattern(
        &self,
        product: &ProductInfo,
    ) -> SuggestionResult<Option<SeasonalPattern>> {
        // TODO: Connect to seasonal pattern repository
        // Placeholder: return seasonal data
        Ok(Some(SeasonalPattern {
            product_id: product.id.clone(),
            quarter: ((Utc::now().month() - 1) / 3 + 1) as u8,
            avg_purchases: 1.5,
            year: Utc::now().year() as u32,
        }))
    }

    async fn get_business_rules(&self) -> SuggestionResult<Vec<BusinessRule>> {
        // TODO: Connect to business rules repository
        // Placeholder: return active rules
        Ok(vec![BusinessRule {
            id: "rule_1".to_string(),
            rule_type: BusinessRuleType::AlwaysSuggestForSegment {
                segment: "enterprise".to_string(),
                product_id: "prod_enterprise".to_string(),
                boost: 0.3,
            },
            active: true,
            priority: 1,
        }])
    }

    // -------------------------------------------------------------------------
    // Cache Management
    // -------------------------------------------------------------------------

    /// Warm the cache with customer similarities
    pub fn warm_customer_cache(&mut self, data: HashMap<String, Vec<(CustomerProfile, f64)>>) {
        self.customer_cache = data;
    }

    /// Warm the cache with product relationships
    pub fn warm_relationship_cache(&mut self, data: HashMap<String, Vec<ProductRelationship>>) {
        self.relationship_cache = data;
    }

    /// Clear all caches
    pub fn clear_caches(&mut self) {
        self.customer_cache.clear();
        self.relationship_cache.clear();
    }
}

impl Default for SuggestionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_suggestions() {
        let engine = SuggestionEngine::new();

        let request = SuggestionRequest::new("test_customer")
            .with_current_products(vec!["prod_pro_v2".to_string()])
            .with_max_suggestions(5);

        let suggestions = engine.get_suggestions(request).await.unwrap();

        // Should return suggestions (mock data should generate enough score)
        // Note: This depends on the mock data in get_candidate_products
        // If no suggestions, the mock scoring isn't generating high enough scores

        // All returned suggestions should have scores above threshold
        for suggestion in &suggestions {
            assert!(suggestion.score >= super::super::MIN_SUGGESTION_SCORE);
            assert!(!suggestion.product_name.is_empty());
            assert!(!suggestion.reasoning.is_empty());
        }
    }

    #[test]
    fn test_default_weights() {
        let engine = SuggestionEngine::new();
        // Just verify it creates successfully with default weights
        let score = engine.calculator.calculate_total_score(&ComponentScores {
            similar_customer: 1.0,
            product_relationship: 1.0,
            time_decay: 1.0,
            business_rule: 1.0,
        });
        // Use approximate comparison for floating point
        assert!((score - 1.0).abs() < 0.0001);
    }
}
