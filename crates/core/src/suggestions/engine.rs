//! Suggestion Engine implementation

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Datelike, Duration, Utc};

use super::scoring::{ScoreCalculator, ScoringWeights};
use super::types::*;
use super::SuggestionResult;

/// Lightweight catalog seed used by the current deterministic suggestion engine.
#[derive(Debug, Clone, Copy)]
struct ProductSeed {
    id: &'static str,
    sku: &'static str,
    name: &'static str,
    category: &'static str,
    unit_price: f64,
}

const PRODUCT_SEEDS: &[ProductSeed] = &[
    ProductSeed {
        id: "prod_pro_v2",
        sku: "PLAN-PRO-001",
        name: "Pro Plan",
        category: "plan",
        unit_price: 12_000.0,
    },
    ProductSeed {
        id: "prod_enterprise",
        sku: "PLAN-ENT-001",
        name: "Enterprise Plan",
        category: "plan",
        unit_price: 18_000.0,
    },
    ProductSeed {
        id: "prod_sso",
        sku: "ADDON-SSO-001",
        name: "SSO Add-on",
        category: "addon",
        unit_price: 2_400.0,
    },
    ProductSeed {
        id: "prod_support",
        sku: "ADDON-SUP-001",
        name: "Premium Support",
        category: "addon",
        unit_price: 6_500.0,
    },
    ProductSeed {
        id: "prod_onboarding",
        sku: "ADDON-ONB-001",
        name: "Onboarding Pack",
        category: "service",
        unit_price: 2_900.0,
    },
    ProductSeed {
        id: "prod_backup",
        sku: "ADDON-BKP-001",
        name: "Automated Backups",
        category: "addon",
        unit_price: 4_000.0,
    },
    ProductSeed {
        id: "prod_analytics",
        sku: "ANALYTICS-001",
        name: "Advanced Analytics",
        category: "addon",
        unit_price: 7_800.0,
    },
];

#[derive(Debug, Clone)]
struct CustomerSeed {
    id: &'static str,
    segment: &'static str,
    industry: Option<&'static str>,
    employee_count: Option<u32>,
    region: &'static str,
    avg_deal_size: f64,
}

const SIMILAR_CUSTOMER_SEEDS: &[CustomerSeed] = &[
    CustomerSeed {
        id: "acme_enterprise_saas",
        segment: "enterprise",
        industry: Some("fintech"),
        employee_count: Some(1200),
        region: "us",
        avg_deal_size: 82_000.0,
    },
    CustomerSeed {
        id: "northbridge_healthcare",
        segment: "enterprise",
        industry: Some("healthcare"),
        employee_count: Some(950),
        region: "us",
        avg_deal_size: 74_000.0,
    },
    CustomerSeed {
        id: "pivot_smb_saas",
        segment: "mid_market",
        industry: Some("saas"),
        employee_count: Some(180),
        region: "us",
        avg_deal_size: 31_000.0,
    },
    CustomerSeed {
        id: "aurora_mid_market",
        segment: "mid_market",
        industry: Some("education"),
        employee_count: Some(240),
        region: "eu",
        avg_deal_size: 33_000.0,
    },
    CustomerSeed {
        id: "founder_smb",
        segment: "smb",
        industry: Some("saas"),
        employee_count: Some(75),
        region: "us",
        avg_deal_size: 9_500.0,
    },
    CustomerSeed {
        id: "local_service_smb",
        segment: "smb",
        industry: Some("services"),
        employee_count: Some(40),
        region: "apac",
        avg_deal_size: 7_800.0,
    },
    CustomerSeed {
        id: "atlas_global",
        segment: "enterprise",
        industry: Some("technology"),
        employee_count: Some(2000),
        region: "eu",
        avg_deal_size: 98_000.0,
    },
];

#[derive(Debug, Clone)]
struct RelationshipSeed {
    source_product_id: &'static str,
    target_product_id: &'static str,
    relationship_type: RelationshipType,
    confidence: f64,
    co_occurrence_count: u32,
}

const PRODUCT_RELATIONSHIP_SEEDS: &[RelationshipSeed] = &[
    RelationshipSeed {
        source_product_id: "prod_enterprise",
        target_product_id: "prod_sso",
        relationship_type: RelationshipType::Bundle,
        confidence: 0.90,
        co_occurrence_count: 142,
    },
    RelationshipSeed {
        source_product_id: "prod_enterprise",
        target_product_id: "prod_support",
        relationship_type: RelationshipType::Bundle,
        confidence: 0.93,
        co_occurrence_count: 198,
    },
    RelationshipSeed {
        source_product_id: "prod_pro_v2",
        target_product_id: "prod_sso",
        relationship_type: RelationshipType::AddOn,
        confidence: 0.81,
        co_occurrence_count: 118,
    },
    RelationshipSeed {
        source_product_id: "prod_pro_v2",
        target_product_id: "prod_backup",
        relationship_type: RelationshipType::AddOn,
        confidence: 0.76,
        co_occurrence_count: 77,
    },
    RelationshipSeed {
        source_product_id: "prod_pro_v2",
        target_product_id: "prod_onboarding",
        relationship_type: RelationshipType::Upgrade,
        confidence: 0.84,
        co_occurrence_count: 95,
    },
    RelationshipSeed {
        source_product_id: "prod_support",
        target_product_id: "prod_analytics",
        relationship_type: RelationshipType::CrossSell,
        confidence: 0.58,
        co_occurrence_count: 64,
    },
    RelationshipSeed {
        source_product_id: "prod_enterprise",
        target_product_id: "prod_analytics",
        relationship_type: RelationshipType::CrossSell,
        confidence: 0.61,
        co_occurrence_count: 52,
    },
];

fn normalize_identifier(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .replace(['_', '-'], " ")
        .replace('/', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn infer_segment(normalized: &str) -> &'static str {
    if normalized.contains("mid") || normalized.contains("mm") {
        "mid_market"
    } else if normalized.contains("smb")
        || normalized.contains("small")
        || normalized.contains("startup")
        || normalized.contains("early")
    {
        "smb"
    } else if normalized.contains("ent") || normalized.contains("enterprise") {
        "enterprise"
    } else {
        "enterprise"
    }
}

fn infer_industry(normalized: &str) -> Option<String> {
    let keywords = [
        ("fintech", "fintech"),
        ("healthcare", "healthcare"),
        ("education", "education"),
        ("retail", "retail"),
        ("services", "services"),
        ("technology", "technology"),
    ];

    for (token, industry) in keywords {
        if normalized.contains(token) {
            return Some(industry.to_owned());
        }
    }

    if normalized.contains("saas") {
        Some("saas".to_owned())
    } else if normalized.contains("cloud") {
        Some("saas".to_owned())
    } else {
        None
    }
}

fn infer_region(normalized: &str) -> &'static str {
    if normalized.contains("eu") || normalized.contains("emea") {
        "eu"
    } else if normalized.contains("apac") || normalized.contains("ap") {
        "apac"
    } else if normalized.contains("ca") {
        "ca"
    } else {
        "us"
    }
}

fn infer_employee_count(segment: &str, normalized: &str) -> u32 {
    if normalized.contains("big") || segment == "enterprise" {
        1200
    } else if segment == "mid_market" {
        220
    } else if segment == "smb" {
        85
    } else {
        300
    }
}

fn infer_avg_deal_size(segment: &str, employee_count: u32, region: &str) -> f64 {
    let base = match segment {
        "enterprise" => 75_000.0,
        "mid_market" => 30_000.0,
        "smb" => 9_500.0,
        _ => 25_000.0,
    };

    let employee_factor = (employee_count as f64) / 1000.0;
    let region_factor = match region {
        "eu" => 1.02,
        "apac" => 0.95,
        "ca" => 1.08,
        _ => 1.0,
    };

    (base * (1.0 + employee_factor.min(0.35))).max(5_000.0) * region_factor
}

fn build_product_catalog() -> Vec<ProductInfo> {
    PRODUCT_SEEDS
        .iter()
        .map(|seed| ProductInfo {
            id: seed.id.to_owned(),
            sku: seed.sku.to_owned(),
            name: seed.name.to_owned(),
            category: seed.category.to_owned(),
            unit_price: seed.unit_price,
            active: true,
        })
        .collect()
}

fn resolve_product(product_id: &str) -> Option<ProductInfo> {
    PRODUCT_SEEDS
        .iter()
        .find(|seed| seed.id.eq_ignore_ascii_case(product_id))
        .map(|seed| ProductInfo {
            id: seed.id.to_owned(),
            sku: seed.sku.to_owned(),
            name: seed.name.to_owned(),
            category: seed.category.to_owned(),
            unit_price: seed.unit_price,
            active: true,
        })
}

fn synthetic_peer_profiles() -> Vec<CustomerProfile> {
    SIMILAR_CUSTOMER_SEEDS
        .iter()
        .map(|seed| CustomerProfile {
            id: seed.id.to_owned(),
            segment: seed.segment.to_owned(),
            industry: seed.industry.map(std::string::ToString::to_string),
            employee_count: seed.employee_count,
            region: seed.region.to_owned(),
            avg_deal_size: seed.avg_deal_size,
        })
        .collect()
}

fn profile_similarity(a: &CustomerProfile, b: &CustomerProfile) -> f64 {
    let mut score = 0.0;

    if a.segment == b.segment {
        score += 0.45;
    }
    if a.industry == b.industry {
        score += 0.20;
    }
    if a.region == b.region {
        score += 0.10;
    }

    let a_employees = a.employee_count.unwrap_or(100) as f64;
    let b_employees = b.employee_count.unwrap_or(100) as f64;
    if a_employees > 0.0 {
        let employee_delta = ((a_employees - b_employees).abs() / a_employees).min(1.0);
        score += 0.15 * (1.0 - employee_delta);
    }

    let avg_delta = (a.avg_deal_size - b.avg_deal_size).abs();
    let scale = a.avg_deal_size.max(b.avg_deal_size).max(1.0);
    score += 0.20 * (1.0 - (avg_delta / scale).min(1.0));

    score.min(1.0)
}

fn customer_profile_preferences(product_id: &str) -> (Vec<&'static str>, Vec<&'static str>) {
    match product_id {
        "prod_enterprise" => (vec!["enterprise"], vec!["enterprise", "high-compliance"]),
        "prod_sso" => (vec!["enterprise", "mid_market"], vec!["security", "identity"]),
        "prod_support" => (vec!["enterprise", "mid_market", "smb"], vec!["service", "retention"]),
        "prod_onboarding" => (vec!["enterprise", "mid_market"], vec!["growth", "new"]),
        "prod_backup" => (vec!["enterprise", "mid_market", "smb"], vec!["operations"]),
        "prod_analytics" => (vec!["enterprise", "mid_market"], vec!["insight"]),
        _ => (vec!["smb", "mid_market", "enterprise"], vec!["baseline"]),
    }
}

fn recent_purchase_days(product_id: &str) -> [i64; 3] {
    match product_id {
        "prod_analytics" => [11, 49, 89],
        "prod_support" => [14, 28, 63],
        "prod_sso" => [17, 32, 77],
        "prod_enterprise" => [20, 75, 140],
        "prod_backup" => [31, 58, 108],
        "prod_onboarding" => [9, 27, 92],
        _ => [20, 55, 95],
    }
}

fn seasonal_intensity(product_id: &str, quarter: u8) -> f64 {
    // Tuned synthetic trend: Q4 is strongest for onboarding, Q2 for security/add-ons.
    match (product_id, quarter) {
        ("prod_enterprise", 1) => 1.15,
        ("prod_enterprise", 4) => 1.28,
        ("prod_sso", 2) => 1.16,
        ("prod_sso", 4) => 1.20,
        ("prod_support", 3) => 1.18,
        ("prod_support", 4) => 1.04,
        ("prod_analytics", 4) => 1.35,
        ("prod_onboarding", 1) => 1.22,
        ("prod_backup", 2) => 1.20,
        ("prod_backup", 3) => 0.95,
        _ => 1.0,
    }
}

fn build_business_rules() -> Vec<BusinessRule> {
    let now = Utc::now();

    vec![
        BusinessRule {
            id: "rule_always_enterprise_support".to_owned(),
            rule_type: BusinessRuleType::AlwaysSuggestForSegment {
                segment: "enterprise".to_owned(),
                product_id: "prod_support".to_owned(),
                boost: 0.24,
            },
            active: true,
            priority: 100,
        },
        BusinessRule {
            id: "rule_mid_market_onboarding".to_owned(),
            rule_type: BusinessRuleType::AlwaysSuggestForSegment {
                segment: "mid_market".to_owned(),
                product_id: "prod_onboarding".to_owned(),
                boost: 0.16,
            },
            active: true,
            priority: 90,
        },
        BusinessRule {
            id: "rule_smb_sso".to_owned(),
            rule_type: BusinessRuleType::AlwaysSuggestForSegment {
                segment: "smb".to_owned(),
                product_id: "prod_sso".to_owned(),
                boost: 0.10,
            },
            active: true,
            priority: 80,
        },
        BusinessRule {
            id: "rule_min_deal_size_backup".to_owned(),
            rule_type: BusinessRuleType::MinimumDealSize {
                threshold: 20_000.0,
                product_id: "prod_backup".to_owned(),
                boost: 0.11,
            },
            active: true,
            priority: 70,
        },
        BusinessRule {
            id: "rule_analytics_promo".to_owned(),
            rule_type: BusinessRuleType::NewProductPromotion {
                product_id: "prod_analytics".to_owned(),
                launch_date: now - Duration::days(14),
                promotion_days: 45,
                boost: 0.12,
            },
            active: true,
            priority: 95,
        },
    ]
}

fn now_quarter(now: DateTime<Utc>) -> u8 {
    ((now.month() - 1) / 3 + 1) as u8
}

fn apply_context_boosts(catalog: Vec<ProductInfo>, customer: &CustomerProfile, context: Option<&QuoteContext>)
-> Vec<ProductInfo> {
    if let Some(context) = context {
        let quote_value = context.quote_value.unwrap_or(customer.avg_deal_size);

        catalog
            .into_iter()
            .map(|product| {
                let is_enterprise = product.id == "prod_enterprise";
                let is_backup = product.id == "prod_backup";
                let is_analytics = product.id == "prod_analytics";

                // Prefer higher ACV products for larger deals and keep core service fit.
                let unit_price = if quote_value >= 60_000.0 {
                    if is_enterprise || is_analytics || is_backup {
                        product.unit_price * 1.0
                    } else {
                        product.unit_price * 1.1
                    }
                } else if quote_value < 20_000.0 {
                    if is_backup {
                        product.unit_price * 0.9
                    } else {
                        product.unit_price
                    }
                } else {
                    product.unit_price
                };

                ProductInfo { unit_price, ..product }
            })
            .collect()
    } else {
        catalog
    }
}

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

        // Similar customer pool is reused across candidates to keep scoring consistent and cheap.
        let similar_customers = self.get_similar_customers(&customer).await?;
        let similar_customer_names = self.get_similar_customer_names(&similar_customers).await?;

        // Score candidate products
        let mut suggestions = Vec::new();
        let candidate_products = self
            .get_candidate_products(&customer, request.quote_context.as_ref(), &current_products)
            .await?;

        for candidate in candidate_products {
            let scores =
                self.score_product(&customer, &current_products, &candidate, &similar_customers).await?;
            let total_score = self.calculator.calculate_total_score(&scores);

            // Skip if below threshold
            if total_score < super::MIN_SUGGESTION_SCORE {
                continue;
            }

            let confidence = ConfidenceLevel::from_score(total_score);
            let category = self.calculator.determine_category(&scores);
            let reasoning = self.calculator.generate_reasoning(&scores, &similar_customer_names);

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

        // Filter duplicates, diversify, and keep the requested count.
        let mut deduped: Vec<ProductSuggestion> = Vec::new();
        let mut seen = HashSet::new();
        for suggestion in suggestions {
            if seen.insert(suggestion.product_id.clone()) {
                deduped.push(suggestion);
            }
        }

        let max_suggestions = request.max_suggestions.max(1);
        let final_suggestions = self
            .calculator
            .filter_and_diversify(deduped, max_suggestions);

        Ok(final_suggestions)
    }

    /// Score a single product
    async fn score_product(
        &self,
        customer: &CustomerProfile,
        current_products: &[ProductInfo],
        candidate: &ProductInfo,
        similar_customers: &[(CustomerProfile, f64)],
    ) -> SuggestionResult<ComponentScores> {
        // Similar customer score
        let purchase_counts = self.get_purchase_counts(similar_customers, candidate).await?;
        let similar_score =
            self.calculator.similar_customer_score(customer, candidate, similar_customers, &purchase_counts);

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
    // Data Access (Deterministic in-memory implementation)
    // -------------------------------------------------------------------------

    async fn get_customer_profile(&self, customer_id: &str) -> SuggestionResult<CustomerProfile> {
        let normalized = normalize_identifier(customer_id);
        let segment = infer_segment(&normalized).to_owned();
        let region = infer_region(&normalized);

        let employee_count = if normalized.contains("large") {
            2200
        } else {
            infer_employee_count(&segment, &normalized)
        };

        let industry = if let Some(override_industry) = infer_industry(&normalized) {
            Some(override_industry)
        } else {
            match segment.as_str() {
                "enterprise" => Some("saas".to_owned()),
                "mid_market" => Some("saas".to_owned()),
                _ => None,
            }
        };

        let avg_deal_size = infer_avg_deal_size(&segment, employee_count, region);

        Ok(CustomerProfile {
            id: customer_id.to_string(),
            segment,
            industry,
            employee_count: Some(employee_count),
            region: region.to_owned(),
            avg_deal_size,
        })
    }

    async fn get_products(&self, product_ids: &[String]) -> SuggestionResult<Vec<ProductInfo>> {
        let mut products = Vec::with_capacity(product_ids.len());

        for id in product_ids {
            if let Some(product) = resolve_product(id) {
                products.push(product);
                continue;
            }

            // Deterministic fallback for unknown product identifiers.
            products.push(ProductInfo {
                id: id.clone(),
                sku: format!("SKU-{id}"),
                name: format!("Product {id}"),
                category: "addon".to_string(),
                unit_price: 0.0,
                active: true,
            });
        }

        Ok(products)
    }

    async fn get_candidate_products(
        &self,
        customer: &CustomerProfile,
        quote_context: Option<&QuoteContext>,
        current_products: &[ProductInfo],
    ) -> SuggestionResult<Vec<ProductInfo>> {
        let mut candidates = build_product_catalog();
        let current_ids: HashSet<&str> = current_products.iter().map(|product| product.id.as_str()).collect();

        candidates.retain(|product| current_ids.is_empty() || !current_ids.contains(product.id.as_str()));

        let quote_value = quote_context.and_then(|context| context.quote_value).unwrap_or(customer.avg_deal_size);
        let term = quote_context.and_then(|context| context.term_months).unwrap_or(12);

        // Region-aware and segment-aware ranking:
        //  - enterprise customers are suggested enterprise-first
        //  - mid-market customers favor onboarding and security add-ons
        //  - SMB customers prefer lighter add-ons first
        candidates.sort_by(|a, b| {
            let (a_preferred, a_tags) = customer_profile_preferences(&a.id);
            let (b_preferred, b_tags) = customer_profile_preferences(&b.id);

            let a_match = a_preferred.iter().filter(|segment| **segment == customer.segment).count() as i32
                + a_tags.iter().filter(|tag| tag == &&"growth").count() as i32;
            let b_match = b_preferred.iter().filter(|segment| **segment == customer.segment).count() as i32
                + b_tags.iter().filter(|tag| tag == &&"growth").count() as i32;

            // Prefer products with a price that aligns with the quote context.
            let a_price_signal = if quote_value < 20_000.0 {
                (a.unit_price <= 5_000.0) as i32
            } else if quote_value > 60_000.0 {
                (a.unit_price >= 4_000.0) as i32
            } else {
                1
            };
            let b_price_signal = if quote_value < 20_000.0 {
                (b.unit_price <= 5_000.0) as i32
            } else if quote_value > 60_000.0 {
                (b.unit_price >= 4_000.0) as i32
            } else {
                1
            };

            let a_term_signal = if term >= 12 {
                (a.id == "prod_support") as i32 + (a.id == "prod_analytics") as i32
            } else {
                0
            };
            let b_term_signal = if term >= 12 {
                (b.id == "prod_support") as i32 + (b.id == "prod_analytics") as i32
            } else {
                0
            };

            let a_score = a_match + a_price_signal + a_term_signal;
            let b_score = b_match + b_price_signal + b_term_signal;

            b_score
                .cmp(&a_score)
                .then_with(|| a.id.cmp(&b.id))
        });

        Ok(apply_context_boosts(candidates, customer, quote_context))
    }

    async fn get_similar_customers(
        &self,
        customer: &CustomerProfile,
    ) -> SuggestionResult<Vec<(CustomerProfile, f64)>> {
        if let Some(cached) = self.customer_cache.get(&customer.id) {
            return Ok(cached.clone());
        }

        let mut peers = synthetic_peer_profiles()
            .into_iter()
            .filter(|peer| peer.id != customer.id)
            .map(|peer| {
                let similarity = profile_similarity(customer, &peer);
                (peer, similarity)
            })
            .filter(|(_, similarity)| *similarity >= 0.40)
            .collect::<Vec<_>>();

        peers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        peers.truncate(4);
        Ok(peers)
    }

    async fn get_similar_customer_names(
        &self,
        similar_customers: &[(CustomerProfile, f64)],
    ) -> SuggestionResult<Vec<String>> {
        Ok(similar_customers.iter().map(|(customer, _)| customer.id.clone()).collect())
    }

    async fn get_purchase_counts(
        &self,
        customers: &[(CustomerProfile, f64)],
        product: &ProductInfo,
    ) -> SuggestionResult<HashMap<String, u32>> {
        let mut counts = HashMap::new();

        for (similar_customer, similarity) in customers {
            let mut count = 1.0f64;

            if customer_profile_preferences(&product.id).0.iter().any(|segment| *segment == similar_customer.segment)
            {
                count += 1.5;
            }

            if similar_customer.industry.is_some()
                && customer_profile_preferences(&product.id)
                    .1
                    .iter()
                    .any(|tag| tag == &&"growth")
            {
                count += 0.5;
            }

            if let Some(employees) = similar_customer.employee_count {
                count += (employees as f64 / 1000.0).min(1.0);
            }

            count *= similarity.max(0.25);
            counts.insert(similar_customer.id.clone(), (count.round() as u32).max(1).min(5));
        }

        Ok(counts)
    }

    async fn get_product_relationships(
        &self,
        product: &ProductInfo,
    ) -> SuggestionResult<Vec<ProductRelationship>> {
        if let Some(cached) = self.relationship_cache.get(&product.id) {
            return Ok(cached.clone());
        }

        let mut relationships = PRODUCT_RELATIONSHIP_SEEDS
            .iter()
            .filter(|seed| {
                seed.source_product_id == product.id || seed.target_product_id == product.id
            })
            .map(|seed| ProductRelationship {
                id: format!("rel:{}:{}", seed.source_product_id, seed.target_product_id),
                source_product_id: seed.source_product_id.to_string(),
                target_product_id: seed.target_product_id.to_string(),
                relationship_type: seed.relationship_type,
                confidence: seed.confidence,
                co_occurrence_count: seed.co_occurrence_count,
            })
            .collect::<Vec<_>>();

        if relationships.is_empty() {
            return Ok(Vec::new());
        }

        relationships.sort_by(|a, b| {
            b.co_occurrence_count.cmp(&a.co_occurrence_count).then_with(|| {
                b.relationship_type.base_score().partial_cmp(&a.relationship_type.base_score()).unwrap_or(std::cmp::Ordering::Equal)
            })
        });

        Ok(relationships)
    }

    async fn get_recent_purchases(
        &self,
        product: &ProductInfo,
    ) -> SuggestionResult<Vec<chrono::DateTime<chrono::Utc>>> {
        let now = chrono::Utc::now();
        let mut purchases = Vec::new();
        for age_days in recent_purchase_days(&product.id) {
            purchases.push(now - Duration::days(age_days));
        }

        // Add mild regional recency spread for non-US regions.
        if product.id == "prod_analytics" {
            purchases.push(now - Duration::days(120));
        }

        Ok(purchases)
    }

    async fn get_seasonal_pattern(
        &self,
        product: &ProductInfo,
    ) -> SuggestionResult<Option<SeasonalPattern>> {
        let now = Utc::now();
        let quarter = now_quarter(now);
        let avg = seasonal_intensity(&product.id, quarter) * 1.0;

        Ok(Some(SeasonalPattern {
            product_id: product.id.clone(),
            quarter,
            avg_purchases: avg,
            year: now.year() as u32,
        }))
    }

    async fn get_business_rules(&self) -> SuggestionResult<Vec<BusinessRule>> {
        Ok(build_business_rules())
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

        // Should return suggestions from deterministic engine input.
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
