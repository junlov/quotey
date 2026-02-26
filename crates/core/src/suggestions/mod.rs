//! Smart Product Suggestions Engine
//!
//! Provides AI-powered product recommendations based on customer similarity,
//! product relationships, temporal patterns, and business rules.

mod engine;
mod scoring;
mod types;

pub use engine::SuggestionEngine;
pub use scoring::{ScoringWeights, ScoreCalculator};
pub use types::*;

use crate::errors::DomainError;

/// Result type for suggestion operations
pub type SuggestionResult<T> = Result<T, DomainError>;

/// Default scoring weights
pub const DEFAULT_WEIGHTS: ScoringWeights = ScoringWeights {
    similar_customer: 0.40,
    product_relationship: 0.30,
    time_decay: 0.20,
    business_rule: 0.10,
};

/// Minimum score threshold for suggestions
pub const MIN_SUGGESTION_SCORE: f64 = 0.40;

/// Maximum suggestions to return
pub const DEFAULT_MAX_SUGGESTIONS: usize = 5;

/// Maximum suggestions per category for diversity
pub const MAX_PER_CATEGORY: usize = 2;
