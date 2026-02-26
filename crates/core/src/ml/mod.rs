//! Win Probability ML Model
//!
//! Provides deterministic logistic regression for predicting quote win probability
//! based on historical deal outcomes. All predictions are auditable and reproducible.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::domain::quote::{Quote, QuoteLine};
use crate::domain::product::ProductId;

/// Feature vector extracted from a quote for win probability prediction
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuoteFeatures {
    /// Total number of line items
    pub line_count: u32,
    /// Total quantity across all lines
    pub total_quantity: u32,
    /// Total value of the quote (subtotal)
    pub total_value: Decimal,
    /// Number of unique products
    pub unique_products: u32,
    /// Average unit price across lines
    pub avg_unit_price: Decimal,
    /// Has premium products (detected from product IDs)
    pub has_premium: bool,
    /// Has enterprise products (detected from product IDs)
    pub has_enterprise: bool,
    /// Quote has multiple product tiers
    pub mixed_tiers: bool,
    /// Customer segment (if available from external context)
    pub customer_segment: Option<String>,
}

impl QuoteFeatures {
    /// Extract features from a quote
    pub fn from_quote(quote: &Quote) -> Self {
        let line_count = quote.lines.len() as u32;
        let total_quantity: u32 = quote.lines.iter().map(|l| l.quantity).sum();
        let total_value: Decimal = quote.lines.iter()
            .map(|l| l.unit_price * Decimal::from(l.quantity))
            .sum();
        
        let unique_products = quote.lines.iter()
            .map(|l| &l.product_id)
            .collect::<std::collections::HashSet<_>>()
            .len() as u32;
        
        let avg_unit_price = if line_count > 0 {
            total_value / Decimal::from(line_count)
        } else {
            Decimal::ZERO
        };

        let has_premium = quote.lines.iter()
            .any(|l| l.product_id.0.to_lowercase().contains("premium"));
        
        let has_enterprise = quote.lines.iter()
            .any(|l| l.product_id.0.to_lowercase().contains("enterprise"));

        let has_standard = quote.lines.iter()
            .any(|l| l.product_id.0.to_lowercase().contains("standard") 
                || l.product_id.0.to_lowercase().contains("starter"));
        
        let mixed_tiers = (has_premium || has_enterprise) && has_standard;

        Self {
            line_count,
            total_quantity,
            total_value,
            unique_products,
            avg_unit_price,
            has_premium,
            has_enterprise,
            mixed_tiers,
            customer_segment: None,
        }
    }

    /// Convert features to a normalized feature vector for model input
    /// 
    /// Features are normalized to roughly [0, 1] range:
    /// - line_count: log(1 + x) / 3 (max ~20 lines -> ~1.0)
    /// - total_quantity: log(1 + x) / 5 (max ~150 qty -> ~1.0)
    /// - total_value: min(x / 100000, 1.0) ($100k max -> 1.0)
    /// - unique_products: x / 10.0 (max 10 products -> 1.0)
    /// - avg_unit_price: min(x / 1000, 1.0) ($1000 max -> 1.0)
    /// - has_premium: 1.0 or 0.0
    /// - has_enterprise: 1.0 or 0.0
    /// - mixed_tiers: 1.0 or 0.0
    pub fn to_normalized_vector(&self) -> Vec<f64> {
        let line_count_norm = ((1.0 + self.line_count as f64).ln()) / 3.0;
        let total_qty_norm = ((1.0 + self.total_quantity as f64).ln()) / 5.0;
        let total_value_f64: f64 = self.total_value.try_into().unwrap_or(0.0);
        let total_value_norm = (total_value_f64 / 100_000.0).min(1.0);
        let unique_products_norm = (self.unique_products as f64 / 10.0).min(1.0);
        let avg_price_f64: f64 = self.avg_unit_price.try_into().unwrap_or(0.0);
        let avg_price_norm = (avg_price_f64 / 1000.0).min(1.0);
        
        vec![
            1.0, // bias term
            line_count_norm.clamp(0.0, 1.0),
            total_qty_norm.clamp(0.0, 1.0),
            total_value_norm.clamp(0.0, 1.0),
            unique_products_norm.clamp(0.0, 1.0),
            avg_price_norm.clamp(0.0, 1.0),
            if self.has_premium { 1.0 } else { 0.0 },
            if self.has_enterprise { 1.0 } else { 0.0 },
            if self.mixed_tiers { 1.0 } else { 0.0 },
        ]
    }
}

/// Historical deal outcome for training
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DealOutcome {
    pub quote_id: String,
    pub features: QuoteFeatures,
    pub outcome: bool, // true = won, false = lost
    pub final_price: Decimal,
    pub close_date: DateTime<Utc>,
}

/// Trained win probability model with version and metadata
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WinProbabilityModel {
    /// Model version (semantic versioning)
    pub version: String,
    /// Training timestamp
    pub trained_at: DateTime<Utc>,
    /// Model weights (logistic regression coefficients)
    pub weights: Vec<f64>,
    /// Training accuracy on validation set
    pub accuracy: f64,
    /// Number of training samples
    pub training_samples: usize,
    /// Feature names for interpretability
    pub feature_names: Vec<String>,
}

impl WinProbabilityModel {
    /// Feature dimension (including bias)
    pub const FEATURE_DIM: usize = 9;
    
    /// Learning rate for gradient descent
    pub const LEARNING_RATE: f64 = 0.1;
    /// Number of training epochs
    pub const EPOCHS: usize = 1000;
    /// L2 regularization parameter
    pub const REGULARIZATION: f64 = 0.01;

    /// Create a new untrained model with random initialization
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            trained_at: Utc::now(),
            weights: vec![0.0; Self::FEATURE_DIM],
            accuracy: 0.0,
            training_samples: 0,
            feature_names: vec![
                "bias".to_string(),
                "line_count".to_string(),
                "total_quantity".to_string(),
                "total_value".to_string(),
                "unique_products".to_string(),
                "avg_unit_price".to_string(),
                "has_premium".to_string(),
                "has_enterprise".to_string(),
                "mixed_tiers".to_string(),
            ],
        }
    }

    /// Create a model with pre-trained weights (for testing/loading persisted models)
    pub fn with_weights(version: impl Into<String>, weights: Vec<f64>) -> Result<Self, String> {
        if weights.len() != Self::FEATURE_DIM {
            return Err(format!(
                "Expected {} weights, got {}",
                Self::FEATURE_DIM,
                weights.len()
            ));
        }
        
        Ok(Self {
            version: version.into(),
            trained_at: Utc::now(),
            weights,
            accuracy: 0.0,
            training_samples: 0,
            feature_names: vec![
                "bias".to_string(),
                "line_count".to_string(),
                "total_quantity".to_string(),
                "total_value".to_string(),
                "unique_products".to_string(),
                "avg_unit_price".to_string(),
                "has_premium".to_string(),
                "has_enterprise".to_string(),
                "mixed_tiers".to_string(),
            ],
        })
    }

    /// Sigmoid activation function: 1 / (1 + e^(-z))
    fn sigmoid(z: f64) -> f64 {
        // Clamp to avoid overflow
        let z = z.clamp(-500.0, 500.0);
        1.0 / (1.0 + (-z).exp())
    }

    /// Predict win probability for a quote (0.0 to 1.0)
    pub fn predict_win_probability(&self, quote: &Quote) -> f64 {
        let features = QuoteFeatures::from_quote(quote);
        self.predict_from_features(&features)
    }

    /// Predict from pre-extracted features
    pub fn predict_from_features(&self, features: &QuoteFeatures) -> f64 {
        let x = features.to_normalized_vector();
        let z: f64 = self.weights.iter().zip(x.iter()).map(|(w, xi)| w * xi).sum();
        Self::sigmoid(z)
    }

    /// Train the model on historical deal outcomes
    /// 
    /// Uses batch gradient descent with L2 regularization
    pub fn train(&mut self, outcomes: &[DealOutcome]) -> Result<f64, String> {
        if outcomes.is_empty() {
            return Err("Cannot train on empty dataset".to_string());
        }

        let n = outcomes.len() as f64;
        self.training_samples = outcomes.len();
        
        // Prepare training data
        let x: Vec<Vec<f64>> = outcomes.iter()
            .map(|o| o.features.to_normalized_vector())
            .collect();
        let y: Vec<f64> = outcomes.iter()
            .map(|o| if o.outcome { 1.0 } else { 0.0 })
            .collect();

        // Gradient descent
        for epoch in 0..Self::EPOCHS {
            let mut gradients = vec![0.0; Self::FEATURE_DIM];
            
            // Compute gradients
            for i in 0..outcomes.len() {
                let z: f64 = self.weights.iter().zip(x[i].iter()).map(|(w, xi)| w * xi).sum();
                let pred = Self::sigmoid(z);
                let error = pred - y[i];
                
                for j in 0..Self::FEATURE_DIM {
                    gradients[j] += error * x[i][j];
                }
            }
            
            // Average and add regularization (skip bias term)
            for j in 0..Self::FEATURE_DIM {
                gradients[j] /= n;
                if j > 0 { // Don't regularize bias
                    gradients[j] += Self::REGULARIZATION * self.weights[j];
                }
            }
            
            // Update weights
            for j in 0..Self::FEATURE_DIM {
                self.weights[j] -= Self::LEARNING_RATE * gradients[j];
            }
            
            // Log progress every 100 epochs (only in debug builds)
            #[cfg(debug_assertions)]
            if epoch % 100 == 0 {
                let _loss = self.compute_loss(&x, &y);
                // Training progress logged at epoch intervals
            }
        }

        // Compute final accuracy
        self.accuracy = self.compute_accuracy(&x, &y);
        self.trained_at = Utc::now();
        
        Ok(self.accuracy)
    }

    /// Compute binary cross-entropy loss
    fn compute_loss(&self, x: &[Vec<f64>], y: &[f64]) -> f64 {
        let n = x.len() as f64;
        let mut loss = 0.0;
        
        for i in 0..x.len() {
            let z: f64 = self.weights.iter().zip(x[i].iter()).map(|(w, xi)| w * xi).sum();
            let pred = Self::sigmoid(z);
            // Add small epsilon to avoid log(0)
            let pred = pred.clamp(1e-15, 1.0 - 1e-15);
            loss -= y[i] * pred.ln() + (1.0 - y[i]) * (1.0 - pred).ln();
        }
        
        loss / n
    }

    /// Compute classification accuracy
    fn compute_accuracy(&self, x: &[Vec<f64>], y: &[f64]) -> f64 {
        if x.is_empty() {
            return 0.0;
        }

        let mut correct = 0;

        for i in 0..x.len() {
            let z: f64 = self.weights.iter().zip(x[i].iter()).map(|(w, xi)| w * xi).sum();
            let pred = Self::sigmoid(z);
            let pred_class = pred >= 0.5;
            let true_class = y[i] >= 0.5;

            if pred_class == true_class {
                correct += 1;
            }
        }

        correct as f64 / x.len() as f64
    }

    /// Evaluate model on test set
    pub fn evaluate(&self, outcomes: &[DealOutcome]) -> ModelMetrics {
        if outcomes.is_empty() {
            return ModelMetrics {
                accuracy: 0.0,
                precision: 0.0,
                recall: 0.0,
                f1_score: 0.0,
                sample_count: 0,
            };
        }

        let x: Vec<Vec<f64>> = outcomes.iter()
            .map(|o| o.features.to_normalized_vector())
            .collect();
        let y: Vec<f64> = outcomes.iter()
            .map(|o| if o.outcome { 1.0 } else { 0.0 })
            .collect();

        let accuracy = self.compute_accuracy(&x, &y);
        
        // Compute precision and recall
        let mut true_positives = 0;
        let mut false_positives = 0;
        let mut false_negatives = 0;
        
        for i in 0..outcomes.len() {
            let prob = self.predict_from_features(&outcomes[i].features);
            let pred = prob >= 0.5;
            let actual = outcomes[i].outcome;
            
            if pred && actual {
                true_positives += 1;
            } else if pred && !actual {
                false_positives += 1;
            } else if !pred && actual {
                false_negatives += 1;
            }
        }
        
        let precision = if true_positives + false_positives > 0 {
            true_positives as f64 / (true_positives + false_positives) as f64
        } else {
            0.0
        };
        
        let recall = if true_positives + false_negatives > 0 {
            true_positives as f64 / (true_positives + false_negatives) as f64
        } else {
            0.0
        };
        
        let f1 = if precision + recall > 0.0 {
            2.0 * precision * recall / (precision + recall)
        } else {
            0.0
        };

        ModelMetrics {
            accuracy,
            precision,
            recall,
            f1_score: f1,
            sample_count: outcomes.len(),
        }
    }
}

/// Model performance metrics
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelMetrics {
    pub accuracy: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub sample_count: usize,
}

/// Model versioning and persistence
#[derive(Clone, Debug, Default)]
pub struct ModelRegistry {
    models: HashMap<String, WinProbabilityModel>,
    current_version: Option<String>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a trained model
    pub fn register(&mut self, model: WinProbabilityModel) {
        self.current_version = Some(model.version.clone());
        self.models.insert(model.version.clone(), model);
    }

    /// Get a model by version
    pub fn get(&self, version: &str) -> Option<&WinProbabilityModel> {
        self.models.get(version)
    }

    /// Get the current (latest) model
    pub fn current(&self) -> Option<&WinProbabilityModel> {
        self.current_version.as_ref()
            .and_then(|v| self.models.get(v))
    }

    /// List all registered versions
    pub fn versions(&self) -> Vec<&String> {
        self.models.keys().collect()
    }

    /// Load a model from serialized JSON
    pub fn load(&mut self, json: &str) -> Result<(), String> {
        let model: WinProbabilityModel = serde_json::from_str(json)
            .map_err(|e| format!("Failed to deserialize model: {}", e))?;
        self.register(model);
        Ok(())
    }

    /// Save a model to JSON
    pub fn save(&self, version: &str) -> Result<String, String> {
        let model = self.models.get(version)
            .ok_or_else(|| format!("Model version {} not found", version))?;
        serde_json::to_string_pretty(model)
            .map_err(|e| format!("Failed to serialize model: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;

    fn create_test_quote(product_id: &str, quantity: u32, unit_price: i64) -> Quote {
        let now = Utc::now();
        Quote {
            id: crate::domain::quote::QuoteId("Q-TEST-001".to_string()),
            version: 1,
            status: crate::domain::quote::QuoteStatus::Draft,
            account_id: None,
            deal_id: None,
            currency: "USD".to_string(),
            term_months: None,
            start_date: None,
            end_date: None,
            valid_until: None,
            notes: None,
            created_by: "system".to_string(),
            lines: vec![QuoteLine {
                product_id: ProductId(product_id.to_string()),
                quantity,
                unit_price: Decimal::new(unit_price, 2),
                discount_pct: 0.0,
                notes: None,
            }],
            created_at: now,
            updated_at: now,
        }
    }

    fn create_test_outcome(product_id: &str, quantity: u32, price: i64, won: bool) -> DealOutcome {
        let quote = create_test_quote(product_id, quantity, price);
        DealOutcome {
            quote_id: "Q-TEST-001".to_string(),
            features: QuoteFeatures::from_quote(&quote),
            outcome: won,
            final_price: Decimal::new(price * quantity as i64, 2),
            close_date: Utc::now(),
        }
    }

    #[test]
    fn quote_features_extracts_correctly() {
        let quote = create_test_quote("enterprise-plan", 10, 50000); // $500.00
        let features = QuoteFeatures::from_quote(&quote);
        
        assert_eq!(features.line_count, 1);
        assert_eq!(features.total_quantity, 10);
        assert!(features.has_enterprise);
        assert!(!features.has_premium);
        assert!(!features.mixed_tiers);
    }

    #[test]
    fn feature_normalization_produces_correct_dimensions() {
        let quote = create_test_quote("premium-plan", 5, 25000);
        let features = QuoteFeatures::from_quote(&quote);
        let vector = features.to_normalized_vector();
        
        assert_eq!(vector.len(), WinProbabilityModel::FEATURE_DIM);
        assert_eq!(vector[0], 1.0); // bias
        assert!(vector[1] >= 0.0 && vector[1] <= 1.0); // line_count
        assert!(vector[6] == 1.0); // has_premium
    }

    #[test]
    fn sigmoid_computes_correctly() {
        assert!((WinProbabilityModel::sigmoid(0.0) - 0.5).abs() < 0.001);
        assert!(WinProbabilityModel::sigmoid(5.0) > 0.99);
        assert!(WinProbabilityModel::sigmoid(-5.0) < 0.01);
    }

    #[test]
    fn model_prediction_returns_probability_between_0_and_1() {
        let model = WinProbabilityModel::new("v1.0.0-test");
        let quote = create_test_quote("enterprise", 10, 50000);
        
        let prob = model.predict_win_probability(&quote);
        assert!(prob >= 0.0 && prob <= 1.0);
    }

    #[test]
    fn model_trains_and_achieves_minimum_accuracy() {
        // Create synthetic training data
        // Pattern: enterprise + high quantity = more likely to win
        let mut outcomes = vec![];
        
        // Won deals: enterprise with high quantity
        for i in 0..30 {
            outcomes.push(create_test_outcome("enterprise-plan", 20 + i as u32, 100000, true));
        }
        
        // Lost deals: starter with low quantity  
        for i in 0..30 {
            outcomes.push(create_test_outcome("starter-plan", 2 + i as u32, 10000, false));
        }
        
        let mut model = WinProbabilityModel::new("v1.0.0-test");
        let accuracy = model.train(&outcomes).expect("Training should succeed");
        
        // Model should achieve >70% accuracy on this separable data
        assert!(
            accuracy >= 0.70,
            "Model accuracy {:.2}% should be >= 70%", 
            accuracy * 100.0
        );
        
        println!("Training accuracy: {:.2}%", accuracy * 100.0);
    }

    #[test]
    fn model_registry_manages_versions() {
        let mut registry = ModelRegistry::new();
        
        let model1 = WinProbabilityModel::new("v1.0.0");
        let model2 = WinProbabilityModel::new("v1.1.0");
        
        registry.register(model1);
        assert_eq!(registry.current().unwrap().version, "v1.0.0");
        
        registry.register(model2);
        assert_eq!(registry.current().unwrap().version, "v1.1.0");
        
        assert!(registry.get("v1.0.0").is_some());
        assert!(registry.get("v1.1.0").is_some());
        assert_eq!(registry.versions().len(), 2);
    }

    #[test]
    fn model_serialization_roundtrip() {
        let mut registry = ModelRegistry::new();
        let model = WinProbabilityModel::new("v1.0.0-test");
        registry.register(model);
        
        let json = registry.save("v1.0.0-test").expect("Save should succeed");
        
        let mut new_registry = ModelRegistry::new();
        new_registry.load(&json).expect("Load should succeed");
        
        let loaded = new_registry.get("v1.0.0-test").unwrap();
        assert_eq!(loaded.version, "v1.0.0-test");
        assert_eq!(loaded.weights.len(), WinProbabilityModel::FEATURE_DIM);
    }

    #[test]
    fn mixed_tiers_detection_works() {
        let now = Utc::now();
        let quote = Quote {
            id: crate::domain::quote::QuoteId("Q-MIXED".to_string()),
            version: 1,
            status: crate::domain::quote::QuoteStatus::Draft,
            account_id: None,
            deal_id: None,
            currency: "USD".to_string(),
            term_months: None,
            start_date: None,
            end_date: None,
            valid_until: None,
            notes: None,
            created_by: "system".to_string(),
            lines: vec![
                QuoteLine {
                    product_id: ProductId("enterprise".to_string()),
                    quantity: 1,
                    unit_price: Decimal::new(100000, 2),
                    discount_pct: 0.0,
                    notes: None,
                },
                QuoteLine {
                    product_id: ProductId("starter".to_string()),
                    quantity: 1,
                    unit_price: Decimal::new(10000, 2),
                    discount_pct: 0.0,
                    notes: None,
                },
            ],
            created_at: now,
            updated_at: now,
        };
        
        let features = QuoteFeatures::from_quote(&quote);
        assert!(features.has_enterprise);
        assert!(features.mixed_tiers);
    }
}
