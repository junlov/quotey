# FEAT-07 Win Probability Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-70d.7`
(`Win Probability Pricing Optimizer`) so pricing recommendations include win probability curves and expected value optimization.

## Scope
### In Scope
- Logistic regression model for win probability prediction.
- Feature engineering from quote, customer, and temporal data.
- Win probability curves across discount ranges.
- Expected value calculation and optimal price recommendation.
- Model training pipeline from historical deal outcomes.
- Slack integration for probability visualization.

### Out of Scope (for Wave 1)
- Deep learning or neural network models.
- Real-time model updates during quote editing.
- External data feeds (market conditions, competitor pricing).
- Guaranteed win rate commitments to customers.

## Rollout Slices
- `Slice A` (contracts): feature schema, probability model interface, recommendation format.
- `Slice B` (model): logistic regression engine, feature extraction, training pipeline.
- `Slice C` (runtime): probability service, curve generation, optimization engine.
- `Slice D` (integration): Slack visualization, model monitoring, feedback loop.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Model accuracy (AUC-ROC) | N/A | >= 0.75 | ML owner | area under ROC curve on holdout data |
| Prediction latency | N/A | <= 100ms | Platform owner | feature extraction to probability output |
| Recommendation adoption | N/A | >= 50% | Product owner | optimal price accepted / recommendations shown |
| Actual win rate vs predicted | N/A | +/- 10% | ML owner | predicted win rate - actual win rate |
| Model drift detection | N/A | <= 7 days | ML owner | time to detect performance degradation |

## Deterministic Safety Constraints
- Win probability is advisory only; reps make final pricing decisions.
- Model predictions never override policy constraints or approval requirements.
- Training data excludes deals where pricing was determined by the model (feedback loop prevention).
- Feature values computed deterministically from quote state; no external API calls during prediction.
- Model versioning ensures reproducibility of historical predictions.

## Interface Boundaries (Draft)
### Domain Contracts
- `QuoteFeatures`: customer_segment, deal_size_tier, discount_pct, term_months, etc.
- `WinProbabilityResult`: probability, confidence_interval, model_version.
- `PricePoint`: discount_pct, win_probability, expected_value.
- `OptimizationResult`: optimal_point, highest_prob_point, all_points, confidence.

### Service Contracts
- `WinProbabilityModel::predict(features) -> WinProbabilityResult`
- `PricingOptimizer::optimize(features, constraints) -> OptimizationResult`
- `PricingOptimizer::generate_curve(features, range, steps) -> Vec<PricePoint>`
- `ModelTrainer::train(historical_deals) -> TrainedModel`
- `ModelRegistry::get_active_model() -> ModelVersion`

### Persistence Contracts
- `WinProbabilityModelRepo`: model coefficients and version metadata.
- `HistoricalDealRepo`: training data with outcomes.
- `PredictionAuditRepo`: prediction requests and results for monitoring.

### Slack Contract
- `/quote optimize` command shows win probability curve.
- Visualization: ASCII chart or bar graph of probability vs discount.
- Recommendation card highlights optimal price and expected value.
- Confidence interval displayed for transparency.
- Manual override always available with reason capture.

### Crate Boundaries
- `quotey-core`: probability model, optimization engine, feature extraction.
- `quotey-db`: model storage, training data, prediction audit.
- `quotey-slack`: visualization rendering, command handling.
- `quotey-agent`: recommendation presentation, feedback collection.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Model bias against certain segments | High | Medium | bias monitoring + fairness metrics | ML owner |
| Over-reliance on model recommendations | Medium | Medium | explicit advisory labeling + override tracking | Product owner |
| Model performance degradation | Medium | Medium | drift detection + automated retraining | ML owner |
| Data leakage in features | High | Low | feature validation + exclusion of post-hoc data | Data owner |
| Inaccurate probability calibration | Medium | Medium | calibration plots + confidence intervals | ML owner |

## Guardrail Checklist (Pre-implementation Exit)
- [ ] Scope and non-goals agreed.
- [ ] KPI owner and metric formula explicitly documented.
- [ ] Deterministic constraints copied into implementation task templates.
- [ ] Interface contracts reviewed against existing crate boundaries.
- [ ] Risks and mitigations acknowledged by feature owner.

## Migration Contract
### Schema Additions (`0024_win_probability`)
- `win_probability_models`: model versions and coefficients.
- `historical_deal_outcomes`: training data with features and outcomes.
- `prediction_audit`: prediction requests and results.
- `model_performance_metrics`: accuracy tracking per model version.

### Version and Audit Semantics
- Models versioned; old predictions remain valid under their model version.
- Feature schemas versioned for reproducibility.
- Prediction audit enables model performance tracking.

### Migration Behavior and Rollback
- Migration adds model tables; no changes to quote schema.
- Model training requires historical data backfill.
- Rollback removes model tables; core pricing unaffected.
