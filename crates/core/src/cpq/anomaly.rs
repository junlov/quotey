//! Pricing anomaly detection engine.
//!
//! Detects statistical outliers in quote pricing using configurable rules.
//! Each rule computes a z-score against a product/segment baseline, and the
//! overall anomaly score is a weighted combination of individual rule scores.
//!
//! Integration: anomaly flags surface as `PolicyViolation`s that can trigger
//! approval escalation through the existing policy engine.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::domain::quote::{Quote, QuoteLine};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Weights for anomaly scoring components.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AnomalyWeights {
    pub unit_price: f64,
    pub discount: f64,
    pub quantity: f64,
    pub deal_size: f64,
}

impl Default for AnomalyWeights {
    fn default() -> Self {
        Self { unit_price: 0.35, discount: 0.30, quantity: 0.15, deal_size: 0.20 }
    }
}

/// Thresholds at which anomaly severity levels trigger.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AnomalyThresholds {
    /// z-score threshold for flagging an individual component (default: 2.0)
    pub component_flag: f64,
    /// Overall score above which the anomaly is informational (default: 0.40)
    pub info: f64,
    /// Overall score above which the anomaly is a warning (default: 0.65)
    pub warning: f64,
    /// Overall score above which the anomaly is critical (default: 0.85)
    pub critical: f64,
}

impl Default for AnomalyThresholds {
    fn default() -> Self {
        Self { component_flag: 2.0, info: 0.40, warning: 0.65, critical: 0.85 }
    }
}

// ---------------------------------------------------------------------------
// Baseline statistics
// ---------------------------------------------------------------------------

/// Distribution statistics for a single metric (e.g., unit_price for a product).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DistributionStats {
    pub mean: f64,
    pub std_dev: f64,
    pub sample_count: u32,
}

impl DistributionStats {
    /// Compute z-score for a given value. Returns 0.0 if std_dev is zero or
    /// sample count is below the minimum threshold.
    pub fn z_score(&self, value: f64, min_samples: u32) -> f64 {
        if self.sample_count < min_samples || self.std_dev <= f64::EPSILON {
            return 0.0;
        }
        ((value - self.mean) / self.std_dev).abs()
    }
}

/// Baseline statistics for a product within an optional customer segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PricingBaseline {
    pub product_id: String,
    pub segment: Option<String>,
    pub unit_price: DistributionStats,
    pub discount_pct: DistributionStats,
    pub quantity: DistributionStats,
    pub deal_total: DistributionStats,
}

// ---------------------------------------------------------------------------
// Anomaly results
// ---------------------------------------------------------------------------

/// Severity of a detected anomaly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnomalySeverity {
    /// Below info threshold — no action needed.
    None,
    /// Informational — logged but no escalation.
    Info,
    /// Warning — may require review.
    Warning,
    /// Critical — requires approval escalation.
    Critical,
}

impl AnomalySeverity {
    /// The approval role that should handle this severity, if any.
    pub fn escalation_role(&self) -> Option<&'static str> {
        match self {
            Self::None | Self::Info => None,
            Self::Warning => Some("sales_manager"),
            Self::Critical => Some("vp_sales"),
        }
    }
}

/// Per-component anomaly flag with explanation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnomalyFlag {
    pub rule_name: String,
    pub z_score: f64,
    pub observed_value: f64,
    pub expected_mean: f64,
    pub explanation: String,
}

/// Full anomaly assessment for a quote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnomalyScore {
    /// Weighted overall anomaly score in [0.0, 1.0].
    pub score: f64,
    /// Severity determined by thresholds.
    pub severity: AnomalySeverity,
    /// Per-rule flags that contributed to the score.
    pub flags: Vec<AnomalyFlag>,
    /// Human-readable summary.
    pub summary: String,
}

// ---------------------------------------------------------------------------
// Detection engine
// ---------------------------------------------------------------------------

/// Minimum historical samples before a baseline is trusted.
const MIN_SAMPLES: u32 = 5;

/// Anomaly detection engine. Stateless — baselines are provided at call time.
#[derive(Debug, Clone, Default)]
pub struct AnomalyDetector {
    weights: AnomalyWeights,
    thresholds: AnomalyThresholds,
}

impl AnomalyDetector {
    pub fn new(weights: AnomalyWeights, thresholds: AnomalyThresholds) -> Self {
        Self { weights, thresholds }
    }

    /// Score a single quote line against its product baseline.
    pub fn score_line(&self, line: &QuoteLine, baseline: &PricingBaseline) -> Vec<AnomalyFlag> {
        let mut flags = Vec::new();
        let price_f = decimal_to_f64(line.unit_price);
        let qty_f = line.quantity as f64;

        // Unit price check
        let z_price = baseline.unit_price.z_score(price_f, MIN_SAMPLES);
        if z_price >= self.thresholds.component_flag {
            flags.push(AnomalyFlag {
                rule_name: "unit_price_outlier".to_string(),
                z_score: z_price,
                observed_value: price_f,
                expected_mean: baseline.unit_price.mean,
                explanation: format!(
                    "Unit price ${:.2} is {:.1}σ from typical ${:.2} for {}",
                    price_f, z_price, baseline.unit_price.mean, baseline.product_id
                ),
            });
        }

        // Discount check
        let z_discount = baseline.discount_pct.z_score(line.discount_pct, MIN_SAMPLES);
        if z_discount >= self.thresholds.component_flag {
            flags.push(AnomalyFlag {
                rule_name: "discount_outlier".to_string(),
                z_score: z_discount,
                observed_value: line.discount_pct,
                expected_mean: baseline.discount_pct.mean,
                explanation: format!(
                    "Discount {:.1}% is {:.1}σ from typical {:.1}% for {}",
                    line.discount_pct, z_discount, baseline.discount_pct.mean, baseline.product_id
                ),
            });
        }

        // Quantity check
        let z_qty = baseline.quantity.z_score(qty_f, MIN_SAMPLES);
        if z_qty >= self.thresholds.component_flag {
            flags.push(AnomalyFlag {
                rule_name: "quantity_outlier".to_string(),
                z_score: z_qty,
                observed_value: qty_f,
                expected_mean: baseline.quantity.mean,
                explanation: format!(
                    "Quantity {} is {:.1}σ from typical {:.0} for {}",
                    line.quantity, z_qty, baseline.quantity.mean, baseline.product_id
                ),
            });
        }

        flags
    }

    /// Score an entire quote given baselines for each product present.
    ///
    /// Products without baselines are silently skipped (insufficient data).
    pub fn score_quote(&self, quote: &Quote, baselines: &[PricingBaseline]) -> AnomalyScore {
        let mut all_flags = Vec::new();
        let mut weighted_z_sum = 0.0;
        let mut weight_total = 0.0;
        let mut matched_any_baseline = false;

        // Per-line anomaly checks
        for line in &quote.lines {
            let product_id = &line.product_id.0;
            let baseline = baselines.iter().find(|b| b.product_id == *product_id);

            if let Some(bl) = baseline {
                matched_any_baseline = true;
                let line_flags = self.score_line(line, bl);
                for flag in &line_flags {
                    let w = match flag.rule_name.as_str() {
                        "unit_price_outlier" => self.weights.unit_price,
                        "discount_outlier" => self.weights.discount,
                        "quantity_outlier" => self.weights.quantity,
                        _ => 0.0,
                    };
                    weighted_z_sum += flag.z_score * w;
                    weight_total += w;
                }
                all_flags.extend(line_flags);
            }
        }

        // Deal-size check — only if at least one product had a baseline
        let deal_total: f64 =
            quote.lines.iter().map(|l| decimal_to_f64(l.unit_price) * l.quantity as f64).sum();

        if matched_any_baseline {
            if let Some(bl) = baselines.first() {
                let z_deal = bl.deal_total.z_score(deal_total, MIN_SAMPLES);
                if z_deal >= self.thresholds.component_flag {
                    all_flags.push(AnomalyFlag {
                        rule_name: "deal_size_outlier".to_string(),
                        z_score: z_deal,
                        observed_value: deal_total,
                        expected_mean: bl.deal_total.mean,
                        explanation: format!(
                            "Deal total ${:.2} is {:.1}σ from typical ${:.2}",
                            deal_total, z_deal, bl.deal_total.mean
                        ),
                    });
                    weighted_z_sum += z_deal * self.weights.deal_size;
                    weight_total += self.weights.deal_size;
                }
            }
        }

        // Normalize to [0.0, 1.0] using sigmoid-like transformation
        let raw_score = if weight_total > 0.0 { weighted_z_sum / weight_total } else { 0.0 };
        let score = sigmoid_transform(raw_score);

        let severity = self.classify_severity(score);
        let summary = build_summary(&all_flags, score, severity);

        AnomalyScore { score, severity, flags: all_flags, summary }
    }

    fn classify_severity(&self, score: f64) -> AnomalySeverity {
        if score >= self.thresholds.critical {
            AnomalySeverity::Critical
        } else if score >= self.thresholds.warning {
            AnomalySeverity::Warning
        } else if score >= self.thresholds.info {
            AnomalySeverity::Info
        } else {
            AnomalySeverity::None
        }
    }
}

/// Map a raw z-score-based value into [0, 1] using a sigmoid: 2 / (1 + e^(-x)) - 1
/// This maps 0 → 0, and grows towards 1 for large positive values.
fn sigmoid_transform(x: f64) -> f64 {
    (2.0 / (1.0 + (-x).exp()) - 1.0).clamp(0.0, 1.0)
}

fn build_summary(flags: &[AnomalyFlag], score: f64, severity: AnomalySeverity) -> String {
    if flags.is_empty() {
        return "No pricing anomalies detected.".to_string();
    }
    let severity_label = match severity {
        AnomalySeverity::None => "negligible",
        AnomalySeverity::Info => "informational",
        AnomalySeverity::Warning => "warning",
        AnomalySeverity::Critical => "critical",
    };
    let flag_names: Vec<&str> = flags.iter().map(|f| f.rule_name.as_str()).collect();
    format!(
        "Pricing anomaly ({severity_label}, score {score:.2}): {} flagged — {}",
        flags.len(),
        flag_names.join(", ")
    )
}

fn decimal_to_f64(d: Decimal) -> f64 {
    use rust_decimal::prelude::ToPrimitive;
    d.to_f64().unwrap_or(0.0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::*;
    use crate::domain::product::ProductId;
    use crate::domain::quote::{QuoteId, QuoteStatus};

    fn baseline_for(product: &str) -> PricingBaseline {
        PricingBaseline {
            product_id: product.to_string(),
            segment: None,
            unit_price: DistributionStats { mean: 100.0, std_dev: 10.0, sample_count: 20 },
            discount_pct: DistributionStats { mean: 5.0, std_dev: 3.0, sample_count: 20 },
            quantity: DistributionStats { mean: 10.0, std_dev: 5.0, sample_count: 20 },
            deal_total: DistributionStats { mean: 1000.0, std_dev: 200.0, sample_count: 20 },
        }
    }

    fn test_quote(lines: Vec<QuoteLine>) -> Quote {
        let now = chrono::Utc::now();
        Quote {
            id: QuoteId("Q-TEST".to_string()),
            version: 1,
            status: QuoteStatus::Draft,
            account_id: None,
            deal_id: None,
            currency: "USD".to_string(),
            term_months: None,
            start_date: None,
            end_date: None,
            valid_until: None,
            notes: None,
            created_by: "test".to_string(),
            lines,
            created_at: now,
            updated_at: now,
        }
    }

    fn line(product: &str, price: i64, qty: u32, discount: f64) -> QuoteLine {
        QuoteLine {
            product_id: ProductId(product.to_string()),
            quantity: qty,
            unit_price: Decimal::new(price, 2),
            discount_pct: discount,
            notes: None,
        }
    }

    #[test]
    fn normal_pricing_has_no_anomalies() {
        let detector = AnomalyDetector::default();
        let quote = test_quote(vec![line("prod-a", 10000, 10, 5.0)]); // $100.00, qty 10, 5%
        let baselines = vec![baseline_for("prod-a")];

        let result = detector.score_quote(&quote, &baselines);

        assert_eq!(result.severity, AnomalySeverity::None);
        assert!(result.flags.is_empty());
        assert!(result.score < 0.1);
    }

    #[test]
    fn high_price_triggers_unit_price_outlier() {
        let detector = AnomalyDetector::default();
        // $150.00 = 5σ above mean of $100 (std_dev=10)
        let quote = test_quote(vec![line("prod-a", 15000, 10, 5.0)]);
        let baselines = vec![baseline_for("prod-a")];

        let result = detector.score_quote(&quote, &baselines);

        assert!(result.flags.iter().any(|f| f.rule_name == "unit_price_outlier"));
        assert!(result.score > 0.4);
    }

    #[test]
    fn extreme_discount_triggers_discount_outlier() {
        let detector = AnomalyDetector::default();
        // 25% discount = (25-5)/3 = 6.67σ
        let quote = test_quote(vec![line("prod-a", 10000, 10, 25.0)]);
        let baselines = vec![baseline_for("prod-a")];

        let result = detector.score_quote(&quote, &baselines);

        assert!(result.flags.iter().any(|f| f.rule_name == "discount_outlier"));
    }

    #[test]
    fn huge_quantity_triggers_quantity_outlier() {
        let detector = AnomalyDetector::default();
        // qty 100 = (100-10)/5 = 18σ
        let quote = test_quote(vec![line("prod-a", 10000, 100, 5.0)]);
        let baselines = vec![baseline_for("prod-a")];

        let result = detector.score_quote(&quote, &baselines);

        assert!(result.flags.iter().any(|f| f.rule_name == "quantity_outlier"));
    }

    #[test]
    fn critical_severity_from_multiple_anomalies() {
        let detector = AnomalyDetector::default();
        // $200 price (10σ), 40% discount (11.67σ), qty 200 (38σ)
        let quote = test_quote(vec![line("prod-a", 20000, 200, 40.0)]);
        let baselines = vec![baseline_for("prod-a")];

        let result = detector.score_quote(&quote, &baselines);

        assert_eq!(result.severity, AnomalySeverity::Critical);
        assert!(result.flags.len() >= 3);
        assert_eq!(result.severity.escalation_role(), Some("vp_sales"));
    }

    #[test]
    fn missing_baseline_skips_product() {
        let detector = AnomalyDetector::default();
        // Product "unknown" has no baseline
        let quote = test_quote(vec![line("unknown", 99999, 999, 50.0)]);
        let baselines = vec![baseline_for("prod-a")]; // different product

        let result = detector.score_quote(&quote, &baselines);

        assert_eq!(result.severity, AnomalySeverity::None);
        assert!(result.flags.is_empty());
    }

    #[test]
    fn insufficient_samples_suppresses_flags() {
        let detector = AnomalyDetector::default();
        let quote = test_quote(vec![line("prod-a", 50000, 10, 5.0)]); // $500 = 40σ

        let mut bl = baseline_for("prod-a");
        bl.unit_price.sample_count = 3; // Below MIN_SAMPLES of 5

        let result = detector.score_quote(&quote, &[bl]);

        // The price flag should not fire because sample count is too low
        assert!(!result.flags.iter().any(|f| f.rule_name == "unit_price_outlier"));
    }

    #[test]
    fn zero_std_dev_suppresses_flags() {
        let detector = AnomalyDetector::default();
        let quote = test_quote(vec![line("prod-a", 15000, 10, 5.0)]);

        let mut bl = baseline_for("prod-a");
        bl.unit_price.std_dev = 0.0;

        let result = detector.score_quote(&quote, &[bl]);

        assert!(!result.flags.iter().any(|f| f.rule_name == "unit_price_outlier"));
    }

    #[test]
    fn custom_thresholds_change_severity() {
        let detector = AnomalyDetector::new(
            AnomalyWeights::default(),
            AnomalyThresholds { component_flag: 1.0, info: 0.1, warning: 0.2, critical: 0.3 },
        );
        // $130 = 3σ, normally just info with defaults
        let quote = test_quote(vec![line("prod-a", 13000, 10, 5.0)]);
        let baselines = vec![baseline_for("prod-a")];

        let result = detector.score_quote(&quote, &baselines);

        // With lower thresholds, even moderate anomalies are more severe
        assert!(result.severity != AnomalySeverity::None);
    }

    #[test]
    fn sigmoid_transform_maps_zero_to_zero() {
        assert!((sigmoid_transform(0.0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn sigmoid_transform_approaches_one_for_large_values() {
        assert!(sigmoid_transform(10.0) > 0.99);
    }

    #[test]
    fn anomaly_score_summary_describes_flags() {
        let detector = AnomalyDetector::default();
        let quote = test_quote(vec![line("prod-a", 20000, 200, 40.0)]);
        let baselines = vec![baseline_for("prod-a")];

        let result = detector.score_quote(&quote, &baselines);

        assert!(result.summary.contains("unit_price_outlier"));
        assert!(result.summary.contains("flagged"));
    }

    #[test]
    fn distribution_stats_z_score_calculation() {
        let stats = DistributionStats { mean: 100.0, std_dev: 10.0, sample_count: 20 };

        assert!((stats.z_score(100.0, 5) - 0.0).abs() < 1e-10); // at mean
        assert!((stats.z_score(120.0, 5) - 2.0).abs() < 1e-10); // 2σ above
        assert!((stats.z_score(80.0, 5) - 2.0).abs() < 1e-10); // 2σ below (abs)
    }
}
