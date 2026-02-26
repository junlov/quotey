//! Types for the Suggestion Engine

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Request for product suggestions
#[derive(Debug, Clone)]
pub struct SuggestionRequest {
    /// Customer to get suggestions for
    pub customer_id: String,
    /// Products already in the quote (for relationship scoring)
    pub current_products: Vec<String>,
    /// Optional quote context
    pub quote_context: Option<QuoteContext>,
    /// Maximum number of suggestions to return
    pub max_suggestions: usize,
}

impl SuggestionRequest {
    /// Create a new suggestion request
    pub fn new(customer_id: impl Into<String>) -> Self {
        Self {
            customer_id: customer_id.into(),
            current_products: Vec::new(),
            quote_context: None,
            max_suggestions: super::DEFAULT_MAX_SUGGESTIONS,
        }
    }

    /// Add current products to the request
    pub fn with_current_products(mut self, products: Vec<String>) -> Self {
        self.current_products = products;
        self
    }

    /// Set quote context
    pub fn with_quote_context(mut self, context: QuoteContext) -> Self {
        self.quote_context = Some(context);
        self
    }

    /// Set max suggestions
    pub fn with_max_suggestions(mut self, max: usize) -> Self {
        self.max_suggestions = max;
        self
    }
}

/// Quote context for suggestions
#[derive(Debug, Clone)]
pub struct QuoteContext {
    /// Current quote total value
    pub quote_value: Option<f64>,
    /// Quote status
    pub quote_status: String,
    /// Term in months
    pub term_months: Option<u32>,
}

/// A product suggestion with scoring and reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductSuggestion {
    /// Product ID
    pub product_id: String,
    /// Product name
    pub product_name: String,
    /// Product SKU
    pub product_sku: String,
    /// Total score (0.0 - 1.0)
    pub score: f64,
    /// Confidence level
    pub confidence: ConfidenceLevel,
    /// Human-readable reasoning
    pub reasoning: Vec<String>,
    /// Suggestion category
    pub category: SuggestionCategory,
    /// Individual component scores
    pub component_scores: ComponentScores,
}

/// Individual scoring components
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ComponentScores {
    /// Similar customer score (0.0 - 1.0)
    pub similar_customer: f64,
    /// Product relationship score (0.0 - 1.0)
    pub product_relationship: f64,
    /// Time decay score (0.0 - 1.0)
    pub time_decay: f64,
    /// Business rule boost (0.0 - 1.0)
    pub business_rule: f64,
}

/// Confidence level for a suggestion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfidenceLevel {
    /// High confidence (score >= 0.80)
    High,
    /// Medium confidence (score 0.60 - 0.79)
    Medium,
    /// Low confidence (score 0.40 - 0.59)
    Low,
}

impl ConfidenceLevel {
    /// Get confidence level from score
    pub fn from_score(score: f64) -> Self {
        if score >= 0.80 {
            ConfidenceLevel::High
        } else if score >= 0.60 {
            ConfidenceLevel::Medium
        } else {
            ConfidenceLevel::Low
        }
    }

    /// Get display percentage
    pub fn display_percentage(&self, score: f64) -> String {
        format!("{:.0}% match", score * 100.0)
    }
}

/// Category of suggestion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SuggestionCategory {
    /// Similar customers bought this
    SimilarCustomersBought,
    /// Complements current products
    ComplementaryProduct,
    /// Part of a bundle
    BundleRecommendation,
    /// Seasonal trend
    SeasonalTrend,
    /// Business rule match
    BusinessRule,
}

impl SuggestionCategory {
    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            SuggestionCategory::SimilarCustomersBought => "Similar customers purchased this",
            SuggestionCategory::ComplementaryProduct => "Complements your current selection",
            SuggestionCategory::BundleRecommendation => "Frequently purchased together",
            SuggestionCategory::SeasonalTrend => "Popular choice this quarter",
            SuggestionCategory::BusinessRule => "Recommended for your profile",
        }
    }
}

/// Customer similarity data
#[derive(Debug, Clone)]
pub struct CustomerSimilarity {
    pub customer_id: String,
    pub similar_customer_id: String,
    pub similarity_score: f64,
    pub updated_at: DateTime<Utc>,
}

/// Product relationship data
#[derive(Debug, Clone)]
pub struct ProductRelationship {
    pub id: String,
    pub source_product_id: String,
    pub target_product_id: String,
    pub relationship_type: RelationshipType,
    pub confidence: f64,
    pub co_occurrence_count: u32,
}

/// Type of product relationship
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipType {
    Bundle,
    AddOn,
    Upgrade,
    CrossSell,
}

impl RelationshipType {
    /// Get base score weight for this relationship type
    pub fn base_score(&self) -> f64 {
        match self {
            RelationshipType::Bundle => 1.0,
            RelationshipType::AddOn => 0.8,
            RelationshipType::Upgrade => 0.6,
            RelationshipType::CrossSell => 0.4,
        }
    }
}

/// Seasonal pattern data
#[derive(Debug, Clone)]
pub struct SeasonalPattern {
    pub product_id: String,
    pub quarter: u8,
    pub avg_purchases: f64,
    pub year: u32,
}

/// Business rule for suggestion boosting
#[derive(Debug, Clone)]
pub struct BusinessRule {
    pub id: String,
    pub rule_type: BusinessRuleType,
    pub active: bool,
    pub priority: i32,
}

/// Types of business rules
#[derive(Debug, Clone)]
pub enum BusinessRuleType {
    AlwaysSuggestForSegment {
        segment: String,
        product_id: String,
        boost: f64,
    },
    MinimumDealSize {
        threshold: f64,
        product_id: String,
        boost: f64,
    },
    NewProductPromotion {
        product_id: String,
        launch_date: DateTime<Utc>,
        promotion_days: i64,
        boost: f64,
    },
}

/// Feedback on a suggestion (for learning)
#[derive(Debug, Clone)]
pub struct SuggestionFeedback {
    pub id: String,
    pub request_id: String,
    pub customer_id: String,
    pub product_id: String,
    pub product_sku: String,
    pub score: f64,
    pub confidence: String,
    pub category: String,
    pub quote_id: Option<String>,
    pub suggested_at: DateTime<Utc>,
    pub was_shown: bool,
    pub was_clicked: bool,
    pub was_added_to_quote: bool,
    pub context: Option<serde_json::Value>,
}

/// Event extracted from a suggestion block action
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuggestionFeedbackEvent {
    /// User clicked "Add to Quote" on a suggestion
    Added { request_id: String, product_id: String, product_sku: String, quote_id: Option<String> },
    /// User clicked "View Details" on a suggestion
    Clicked { request_id: String, product_id: String },
}

/// Acceptance rate for a product across all suggestion feedback
#[derive(Debug, Clone, Copy)]
pub struct ProductAcceptanceRate {
    pub shown_count: u32,
    pub clicked_count: u32,
    pub added_count: u32,
    pub click_rate: f64,
    pub add_rate: f64,
}

/// Customer data for similarity calculation
#[derive(Debug, Clone)]
pub struct CustomerProfile {
    pub id: String,
    pub segment: String,
    pub industry: Option<String>,
    pub employee_count: Option<u32>,
    pub region: String,
    pub avg_deal_size: f64,
}

/// Product data for suggestion scoring
#[derive(Debug, Clone)]
pub struct ProductInfo {
    pub id: String,
    pub sku: String,
    pub name: String,
    pub category: String,
    pub unit_price: f64,
    pub active: bool,
}
