# Win Probability Pricing Optimizer (FEAT-07) - Technical Research

**Feature:** ML-Powered Pricing Intelligence  
**Researcher:** ResearchAgent (kimi-k2)  
**Date:** 2026-02-23  
**Priority:** P2

---

## 1. Technical Overview

Deterministic pricing meets ML intelligence: calculates not just *can* we price at $X, but *should* we? Shows win probability curves and recommends optimal price for maximum expected value.

---

## 2. Model Architecture

### 2.1 Feature Engineering

```rust
pub struct QuoteFeatures {
    // Customer characteristics
    pub customer_segment: String,
    pub account_tier: String,
    pub industry: String,
    pub previous_deals_count: u32,
    pub avg_previous_discount: Decimal,
    
    // Deal characteristics
    pub deal_size_tier: String,  // SMB, Mid, Enterprise
    pub product_mix: Vec<String>,
    pub term_months: u32,
    pub has_competitor_mentioned: bool,
    pub timeline_urgency: UrgencyScore,
    
    // Pricing context
    pub list_price_total: Decimal,
    pub proposed_discount_pct: Decimal,
    pub competitor_price_estimate: Option<Decimal>,
    
    // Temporal features
    pub day_of_quarter: u32,
    pub days_until_close: i32,
    pub is_eoq: bool,
}

pub enum UrgencyScore {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}
```

### 2.2 Win Probability Model

```rust
pub struct WinProbabilityModel {
    coefficients: ModelCoefficients,
    threshold: f64,
}

pub struct ModelCoefficients {
    pub intercept: f64,
    pub discount_coefficient: f64,
    pub segment_weights: HashMap<String, f64>,
    pub competitor_presence_impact: f64,
    pub timeline_urgency_weights: [f64; 4],
    pub eoq_boost: f64,
}

impl WinProbabilityModel {
    /// Logistic regression for win probability
    pub fn predict(&self, features: &QuoteFeatures) -> f64 {
        let mut logit = self.coefficients.intercept;
        
        // Discount effect (negative: higher discount = lower probability of winning at that price)
        // Actually, we model win probability at given price point
        // Higher discount = lower price = higher win prob
        logit += self.coefficients.discount_coefficient * 
                 features.proposed_discount_pct.to_f64().unwrap();
        
        // Segment weight
        if let Some(weight) = self.coefficients.segment_weights.get(&features.customer_segment) {
            logit += weight;
        }
        
        // Competitor presence
        if features.has_competitor_mentioned {
            logit += self.coefficients.competitor_presence_impact;
        }
        
        // Timeline urgency
        logit += self.coefficients.timeline_urgency_weights[features.timeline_urgency as usize - 1];
        
        // EOQ boost
        if features.is_eoq {
            logit += self.coefficients.eoq_boost;
        }
        
        // Sigmoid function
        1.0 / (1.0 + (-logit).exp())
    }
    
    /// Generate probability curve across discount range
    pub fn generate_curve(
        &self,
        base_features: &QuoteFeatures,
        discount_range: (Decimal, Decimal),
        steps: u32,
    ) -> Vec<PricePoint> {
        let mut curve = Vec::new();
        let min_discount = discount_range.0.to_f64().unwrap();
        let max_discount = discount_range.1.to_f64().unwrap();
        let step = (max_discount - min_discount) / steps as f64;
        
        for i in 0..=steps {
            let discount = Decimal::from_f64(min_discount + step * i as f64).unwrap();
            let mut features = base_features.clone();
            features.proposed_discount_pct = discount;
            
            let win_prob = self.predict(&features);
            let expected_value = self.calculate_expected_value(&features, win_prob);
            
            curve.push(PricePoint {
                discount_pct: discount,
                win_probability: win_prob,
                expected_value,
            });
        }
        
        curve
    }
    
    fn calculate_expected_value(&self, features: &QuoteFeatures, win_prob: f64) -> Decimal {
        let list_price = features.list_price_total;
        let discount = features.proposed_discount_pct / Decimal::from(100);
        let price = list_price * (Decimal::ONE - discount);
        
        price * Decimal::from_f64(win_prob).unwrap()
    }
}

pub struct PricePoint {
    pub discount_pct: Decimal,
    pub win_probability: f64,
    pub expected_value: Decimal,
}
```

---

## 3. Pricing Optimizer

```rust
pub struct PricingOptimizer {
    model: WinProbabilityModel,
    constraints: PricingConstraints,
}

pub struct PricingConstraints {
    pub min_discount: Decimal,
    pub max_discount: Decimal,
    pub min_margin_pct: Decimal,
    pub approval_thresholds: Vec<ApprovalThreshold>,
}

impl PricingOptimizer {
    /// Find optimal price for maximum expected value
    pub fn optimize(&self, features: &QuoteFeatures) -> OptimizationResult {
        let curve = self.model.generate_curve(
            features,
            (self.constraints.min_discount, self.constraints.max_discount),
            20,
        );
        
        // Filter by constraints
        let valid_points: Vec<_> = curve.into_iter()
            .filter(|p| self.satisfies_constraints(features, p))
            .collect();
        
        // Find maximum expected value
        let optimal = valid_points.iter()
            .max_by(|a, b| a.expected_value.partial_cmp(&b.expected_value).unwrap())
            .cloned();
        
        // Find highest win probability (if different)
        let highest_prob = valid_points.iter()
            .max_by(|a, b| a.win_probability.partial_cmp(&b.win_probability).unwrap())
            .cloned();
        
        OptimizationResult {
            optimal_point: optimal,
            highest_probability_point: highest_prob,
            all_points: valid_points,
            confidence_interval: self.calculate_confidence(features),
        }
    }
    
    fn satisfies_constraints(&self, features: &QuoteFeatures, point: &PricePoint) -> bool {
        // Check margin constraint
        let margin = self.calculate_margin(features, point.discount_pct);
        if margin < self.constraints.min_margin_pct {
            return false;
        }
        
        true
    }
    
    fn calculate_margin(&self, features: &QuoteFeatures, discount_pct: Decimal) -> Decimal {
        let list_price = features.list_price_total;
        let cost = list_price * Decimal::from_f64(0.4).unwrap(); // Assume 40% cost
        let discount = discount_pct / Decimal::from(100);
        let price = list_price * (Decimal::ONE - discount);
        
        (price - cost) / price * Decimal::from(100)
    }
}

pub struct OptimizationResult {
    pub optimal_point: Option<PricePoint>,
    pub highest_probability_point: Option<PricePoint>,
    pub all_points: Vec<PricePoint>,
    pub confidence_interval: (Decimal, Decimal),
}
```

---

## 4. Model Training

```rust
pub struct ModelTrainer {
    training_data: Vec<HistoricalDeal>,
}

pub struct HistoricalDeal {
    pub features: QuoteFeatures,
    pub outcome: DealOutcome,  // Won or Lost
    pub final_price: Decimal,
    pub close_date: DateTime<Utc>,
}

pub enum DealOutcome {
    Won,
    Lost,
}

impl ModelTrainer {
    /// Train model using logistic regression
    pub fn train(&self) -> Result<WinProbabilityModel, TrainingError> {
        // Prepare data
        let (xs, ys): (Vec<Vec<f64>>, Vec<f64>) = self.training_data.iter()
            .map(|deal| {
                let features = self.extract_feature_vector(&deal.features);
                let label = match deal.outcome {
                    DealOutcome::Won => 1.0,
                    DealOutcome::Lost => 0.0,
                };
                (features, label)
            })
            .unzip();
        
        // Fit logistic regression
        let coefficients = self.fit_logistic_regression(&xs, &ys)?;
        
        Ok(WinProbabilityModel {
            coefficients,
            threshold: 0.5,
        })
    }
    
    fn extract_feature_vector(&self, features: &QuoteFeatures) -> Vec<f64> {
        vec![
            features.proposed_discount_pct.to_f64().unwrap(),
            features.timeline_urgency as u32 as f64,
            if features.has_competitor_mentioned { 1.0 } else { 0.0 },
            if features.is_eoq { 1.0 } else { 0.0 },
            features.previous_deals_count as f64,
            features.avg_previous_discount.to_f64().unwrap(),
        ]
    }
    
    fn fit_logistic_regression(
        &self,
        xs: &[Vec<f64>],
        ys: &[f64],
    ) -> Result<ModelCoefficients, TrainingError> {
        // Simplified gradient descent
        // In practice, use a proper ML library or service
        let mut intercept = 0.0;
        let learning_rate = 0.01;
        let epochs = 1000;
        
        for _ in 0..epochs {
            let mut gradient = 0.0;
            
            for (x, y) in xs.iter().zip(ys.iter()) {
                let pred = 1.0 / (1.0 + (-intercept - dot_product(x, &vec![1.0; x.len()])).exp());
                gradient += (pred - y);
            }
            
            intercept -= learning_rate * gradient / xs.len() as f64;
        }
        
        Ok(ModelCoefficients {
            intercept,
            discount_coefficient: 0.05,  // Higher discount = higher win prob
            segment_weights: HashMap::new(),
            competitor_presence_impact: -0.1,  // Competitor hurts probability
            timeline_urgency_weights: [0.0, 0.05, 0.1, 0.15],
            eoq_boost: 0.08,
        })
    }
}

fn dot_product(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
```

---

## 5. Slack Integration

```rust
pub fn render_probability_curve(result: &OptimizationResult) -> MessageTemplate {
    let mut builder = MessageBuilder::new("Pricing Intelligence");
    
    // Header
    builder = builder.section("header", |s| {
        s.mrkdwn("💰 *Win Probability Analysis*".to_string())
    });
    
    // Curve visualization
    let mut curve_text = "*Win Probability Curve:*\n```\n".to_string();
    
    for point in result.all_points.iter().step_by(2) {
        let bar_length = (point.win_probability * 20.0) as usize;
        let bar = "█".repeat(bar_length);
        curve_text.push_str(&format!(
            "{:.0}% {:20} {:.0}%\n",
            point.discount_pct,
            bar,
            point.win_probability * 100.0
        ));
    }
    
    curve_text.push_str("```");
    
    builder = builder.section("curve", |s| {
        s.mrkdwn(curve_text)
    });
    
    // Optimal recommendation
    if let Some(ref optimal) = result.optimal_point {
        builder = builder.section("optimal", |s| {
            s.mrkdwn(format!(
                "💡 *Optimal: {:.0}% discount*\nWin probability: {:.0}% | Expected value: ${}",
                optimal.discount_pct,
                optimal.win_probability * 100.0,
                optimal.expected_value
            ))
        });
    }
    
    // Actions
    builder = builder.actions("actions", |a| {
        a.button(ButtonElement::new("apply_optimal", "Apply Optimal").style(ButtonStyle::Primary))
         .button(ButtonElement::new("view_details", "View Details"))
         .button(ButtonElement::new("recalculate", "Recalculate"))
    });
    
    builder.build()
}
```

---

## 6. Data Requirements

```sql
-- Historical deals for model training
CREATE TABLE deal_outcomes (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    customer_segment TEXT NOT NULL,
    account_tier TEXT NOT NULL,
    industry TEXT,
    deal_size_tier TEXT NOT NULL,
    list_price_total REAL NOT NULL,
    final_discount_pct REAL NOT NULL,
    final_price REAL NOT NULL,
    has_competitor_mentioned BOOLEAN,
    timeline_urgency INTEGER,
    term_months INTEGER,
    day_of_quarter INTEGER,
    days_until_close INTEGER,
    is_eoq BOOLEAN,
    outcome TEXT NOT NULL,  -- 'won' or 'lost'
    closed_at TEXT,
    created_at TEXT NOT NULL
);

-- Model versioning
CREATE TABLE win_probability_models (
    id TEXT PRIMARY KEY,
    version TEXT NOT NULL UNIQUE,
    coefficients_json TEXT NOT NULL,
    training_data_count INTEGER NOT NULL,
    accuracy_score REAL,
    precision_score REAL,
    recall_score REAL,
    deployed_at TEXT,
    created_at TEXT NOT NULL
);
```

---

## 7. Safety Considerations

1. **Model is advisory only**: Reps make final pricing decisions
2. **Deterministic fallback**: If model unavailable, use standard pricing
3. **Confidence thresholds**: Show warning if prediction confidence low
4. **Audit trail**: Log all model recommendations
5. **Bias monitoring**: Regular checks for segment bias

---

*Research compiled by ResearchAgent for the quotey project.*
