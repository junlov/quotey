use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::cpq::policy::PolicyInput;
use crate::cpq::{CpqEvaluation, CpqEvaluationInput, CpqRuntime};
use crate::domain::product::ProductId;
use crate::domain::quote::{Quote, QuoteLine};
use crate::domain::simulation::{
    ScenarioAuditEventType, ScenarioRunId, ScenarioTelemetryEvent, ScenarioTelemetryOutcome,
};

const DEFAULT_MAX_VARIANTS: usize = 3;
const MAX_QUANTITY_DELTA: i32 = 1_000_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineVariation {
    pub product_id: ProductId,
    pub quantity_delta: i32,
    pub unit_price_override: Option<Decimal>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioVariation {
    pub variant_key: String,
    pub line_variations: Vec<LineVariation>,
    pub requested_discount_pct_override: Option<Decimal>,
    pub minimum_margin_pct_override: Option<Decimal>,
    pub deal_value_override: Option<Decimal>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedScenarioVariation {
    pub variant_key: String,
    pub line_variations: Vec<LineVariation>,
    pub requested_discount_pct_override: Option<Decimal>,
    pub minimum_margin_pct_override: Option<Decimal>,
    pub deal_value_override: Option<Decimal>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceDelta {
    pub subtotal_delta: Decimal,
    pub discount_total_delta: Decimal,
    pub tax_total_delta: Decimal,
    pub total_delta: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDelta {
    pub approval_required_changed: bool,
    pub violation_count_delta: i32,
    pub newly_added_reasons: Vec<String>,
    pub cleared_reasons: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalDelta {
    pub from_status: String,
    pub to_status: String,
    pub changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigurationLineDelta {
    pub product_id: ProductId,
    pub quantity_before: u32,
    pub quantity_after: u32,
    pub unit_price_before: Decimal,
    pub unit_price_after: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigurationDelta {
    pub added_product_ids: Vec<ProductId>,
    pub removed_product_ids: Vec<ProductId>,
    pub changed_lines: Vec<ConfigurationLineDelta>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioDeltaBundle {
    pub price: PriceDelta,
    pub policy: PolicyDelta,
    pub approval: ApprovalDelta,
    pub configuration: ConfigurationDelta,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimulatedScenario {
    pub variant_key: String,
    pub forked_quote: Quote,
    pub normalized_variation: NormalizedScenarioVariation,
    pub evaluation: CpqEvaluation,
    pub delta: ScenarioDeltaBundle,
    pub rank_order: i32,
    pub rank_score: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimulationComparison {
    pub baseline_quote: Quote,
    pub baseline_evaluation: CpqEvaluation,
    pub variants: Vec<SimulatedScenario>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationTelemetryContext {
    pub correlation_id: String,
    pub scenario_run_id: Option<ScenarioRunId>,
}

pub trait SimulationTelemetrySink: Send + Sync {
    fn emit(&self, event: ScenarioTelemetryEvent);
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum SimulatorGuardrailError {
    #[error("simulator requires at least one variation")]
    EmptyVariationSet,
    #[error("simulator received {requested} variations but max allowed is {max_allowed}")]
    TooManyVariations { requested: usize, max_allowed: usize },
    #[error("variant key cannot be empty")]
    EmptyVariantKey,
    #[error("duplicate variant key detected: {variant_key}")]
    DuplicateVariantKey { variant_key: String },
    #[error("variant {variant_key} has no effective changes")]
    EmptyVariantChanges { variant_key: String },
    #[error("line variation product_id cannot be empty")]
    EmptyProductId,
    #[error("quantity delta {quantity_delta} exceeds guardrail bounds")]
    QuantityDeltaOutOfBounds { quantity_delta: i32 },
    #[error("aggregated quantity delta {quantity_delta} for product {product_id} exceeds guardrail bounds")]
    AggregatedQuantityDeltaOutOfBounds { product_id: String, quantity_delta: i64 },
    #[error("conflicting unit price overrides for product {product_id}")]
    ConflictingUnitPriceOverride { product_id: String },
    #[error("quantity underflow for product {product_id} in variant {variant_key}")]
    QuantityUnderflow { variant_key: String, product_id: String },
    #[error("new product {product_id} in variant {variant_key} requires unit price override")]
    MissingUnitPriceForNewLine { variant_key: String, product_id: String },
    #[error("negative delta for missing product {product_id} in variant {variant_key}")]
    NegativeDeltaForMissingLine { variant_key: String, product_id: String },
}

impl SimulatorGuardrailError {
    pub fn user_safe_message(&self) -> String {
        match self {
            Self::EmptyVariationSet => "Add at least one scenario change to simulate.".to_string(),
            Self::TooManyVariations { max_allowed, .. } => {
                format!("You can simulate up to {max_allowed} scenarios at once.")
            }
            Self::EmptyVariantKey => "Each scenario must have a non-empty key.".to_string(),
            Self::DuplicateVariantKey { variant_key } => {
                format!("Scenario key '{variant_key}' is duplicated; use unique keys.")
            }
            Self::EmptyVariantChanges { variant_key } => {
                format!("Scenario '{variant_key}' has no effective changes.")
            }
            Self::EmptyProductId => "A scenario line is missing product identity.".to_string(),
            Self::QuantityDeltaOutOfBounds { .. } => {
                "Quantity changes are outside allowed bounds for safe simulation.".to_string()
            }
            Self::AggregatedQuantityDeltaOutOfBounds { product_id, .. } => {
                format!(
                    "Aggregated quantity changes for '{product_id}' exceed safe simulation bounds."
                )
            }
            Self::ConflictingUnitPriceOverride { product_id } => {
                format!("Conflicting unit-price overrides were provided for '{product_id}'.")
            }
            Self::QuantityUnderflow { variant_key, product_id } => {
                format!("Scenario '{variant_key}' would reduce '{product_id}' below zero quantity.")
            }
            Self::MissingUnitPriceForNewLine { variant_key, product_id } => {
                format!("Scenario '{variant_key}' adds '{product_id}' but omits unit price.")
            }
            Self::NegativeDeltaForMissingLine { variant_key, product_id } => {
                format!(
                    "Scenario '{variant_key}' subtracts '{product_id}', but it is not in the baseline quote."
                )
            }
        }
    }
}

pub fn normalize_variations(
    variations: Vec<ScenarioVariation>,
    max_variants: usize,
) -> Result<Vec<NormalizedScenarioVariation>, SimulatorGuardrailError> {
    if variations.is_empty() {
        return Err(SimulatorGuardrailError::EmptyVariationSet);
    }

    if variations.len() > max_variants {
        return Err(SimulatorGuardrailError::TooManyVariations {
            requested: variations.len(),
            max_allowed: max_variants,
        });
    }

    let mut seen_keys = BTreeSet::new();
    let mut normalized = Vec::with_capacity(variations.len());

    for variation in variations {
        let variant_key = variation.variant_key.trim().to_string();
        if variant_key.is_empty() {
            return Err(SimulatorGuardrailError::EmptyVariantKey);
        }

        let canonical_key = variant_key.to_ascii_lowercase();
        if !seen_keys.insert(canonical_key) {
            return Err(SimulatorGuardrailError::DuplicateVariantKey { variant_key });
        }

        let mut grouped: BTreeMap<String, (i64, Option<Decimal>)> = BTreeMap::new();
        for line in variation.line_variations {
            let product_id = line.product_id.0.trim().to_string();
            if product_id.is_empty() {
                return Err(SimulatorGuardrailError::EmptyProductId);
            }

            if line.quantity_delta.unsigned_abs() > MAX_QUANTITY_DELTA as u32 {
                return Err(SimulatorGuardrailError::QuantityDeltaOutOfBounds {
                    quantity_delta: line.quantity_delta,
                });
            }

            let entry =
                grouped.entry(product_id.clone()).or_insert((0_i64, line.unit_price_override));
            let updated_delta =
                entry.0.checked_add(i64::from(line.quantity_delta)).ok_or_else(|| {
                    SimulatorGuardrailError::AggregatedQuantityDeltaOutOfBounds {
                        product_id: product_id.clone(),
                        quantity_delta: if line.quantity_delta.is_negative() {
                            i64::MIN
                        } else {
                            i64::MAX
                        },
                    }
                })?;
            if updated_delta < -i64::from(MAX_QUANTITY_DELTA)
                || updated_delta > i64::from(MAX_QUANTITY_DELTA)
            {
                return Err(SimulatorGuardrailError::AggregatedQuantityDeltaOutOfBounds {
                    product_id: product_id.clone(),
                    quantity_delta: updated_delta,
                });
            }
            entry.0 = updated_delta;

            match (entry.1, line.unit_price_override) {
                (Some(existing), Some(candidate)) if existing != candidate => {
                    return Err(SimulatorGuardrailError::ConflictingUnitPriceOverride {
                        product_id,
                    });
                }
                (None, Some(candidate)) => {
                    entry.1 = Some(candidate);
                }
                _ => {}
            }
        }

        let mut line_variations = Vec::with_capacity(grouped.len());
        for (product_id, (quantity_delta, unit_price_override)) in grouped {
            if quantity_delta == 0 && unit_price_override.is_none() {
                continue;
            }

            let quantity_delta = i32::try_from(quantity_delta).map_err(|_| {
                SimulatorGuardrailError::AggregatedQuantityDeltaOutOfBounds {
                    product_id: product_id.clone(),
                    quantity_delta,
                }
            })?;
            line_variations.push(LineVariation {
                product_id: ProductId(product_id),
                quantity_delta,
                unit_price_override,
            });
        }

        if line_variations.is_empty()
            && variation.requested_discount_pct_override.is_none()
            && variation.minimum_margin_pct_override.is_none()
            && variation.deal_value_override.is_none()
        {
            return Err(SimulatorGuardrailError::EmptyVariantChanges { variant_key });
        }

        normalized.push(NormalizedScenarioVariation {
            variant_key,
            line_variations,
            requested_discount_pct_override: variation.requested_discount_pct_override,
            minimum_margin_pct_override: variation.minimum_margin_pct_override,
            deal_value_override: variation.deal_value_override,
        });
    }

    normalized.sort_by(|left, right| left.variant_key.cmp(&right.variant_key));
    Ok(normalized)
}

pub fn fork_quote_with_variation(
    baseline_quote: &Quote,
    variation: &NormalizedScenarioVariation,
) -> Result<Quote, SimulatorGuardrailError> {
    let mut lines: BTreeMap<String, QuoteLine> = BTreeMap::new();
    for line in &baseline_quote.lines {
        let product_id = line.product_id.0.trim().to_string();
        if product_id.is_empty() {
            return Err(SimulatorGuardrailError::EmptyProductId);
        }
        let mut canonical_line = line.clone();
        canonical_line.product_id = ProductId(product_id.clone());
        lines.insert(product_id, canonical_line);
    }

    for change in &variation.line_variations {
        if let Some(existing) = lines.get_mut(&change.product_id.0) {
            let next_quantity = i64::from(existing.quantity) + i64::from(change.quantity_delta);
            if next_quantity < 0 {
                return Err(SimulatorGuardrailError::QuantityUnderflow {
                    variant_key: variation.variant_key.clone(),
                    product_id: change.product_id.0.clone(),
                });
            }

            if next_quantity == 0 {
                lines.remove(&change.product_id.0);
                continue;
            }

            existing.quantity = next_quantity as u32;
            if let Some(unit_price_override) = change.unit_price_override {
                existing.unit_price = unit_price_override;
            }
            continue;
        }

        if change.quantity_delta < 0 {
            return Err(SimulatorGuardrailError::NegativeDeltaForMissingLine {
                variant_key: variation.variant_key.clone(),
                product_id: change.product_id.0.clone(),
            });
        }

        if change.quantity_delta == 0 {
            continue;
        }

        let unit_price = change.unit_price_override.ok_or_else(|| {
            SimulatorGuardrailError::MissingUnitPriceForNewLine {
                variant_key: variation.variant_key.clone(),
                product_id: change.product_id.0.clone(),
            }
        })?;

        lines.insert(
            change.product_id.0.clone(),
            QuoteLine {
                product_id: change.product_id.clone(),
                quantity: change.quantity_delta as u32,
                unit_price,
            },
        );
    }

    Ok(Quote {
        id: baseline_quote.id.clone(),
        status: baseline_quote.status.clone(),
        lines: lines.into_values().collect(),
        created_at: baseline_quote.created_at,
    })
}

pub fn compute_delta(
    baseline_quote: &Quote,
    baseline_eval: &CpqEvaluation,
    variant_quote: &Quote,
    variant_eval: &CpqEvaluation,
) -> ScenarioDeltaBundle {
    let price = PriceDelta {
        subtotal_delta: variant_eval.pricing.subtotal - baseline_eval.pricing.subtotal,
        discount_total_delta: variant_eval.pricing.discount_total
            - baseline_eval.pricing.discount_total,
        tax_total_delta: variant_eval.pricing.tax_total - baseline_eval.pricing.tax_total,
        total_delta: variant_eval.pricing.total - baseline_eval.pricing.total,
    };

    let baseline_reasons: BTreeSet<String> = baseline_eval.policy.reasons.iter().cloned().collect();
    let variant_reasons: BTreeSet<String> = variant_eval.policy.reasons.iter().cloned().collect();

    let policy = PolicyDelta {
        approval_required_changed: baseline_eval.policy.approval_required
            != variant_eval.policy.approval_required,
        violation_count_delta: variant_eval.policy.violations.len() as i32
            - baseline_eval.policy.violations.len() as i32,
        newly_added_reasons: variant_reasons.difference(&baseline_reasons).cloned().collect(),
        cleared_reasons: baseline_reasons.difference(&variant_reasons).cloned().collect(),
    };

    let approval = ApprovalDelta {
        from_status: format!("{:?}", baseline_eval.policy.approval_status),
        to_status: format!("{:?}", variant_eval.policy.approval_status),
        changed: baseline_eval.policy.approval_status != variant_eval.policy.approval_status,
    };

    let baseline_lines: BTreeMap<String, &QuoteLine> = baseline_quote
        .lines
        .iter()
        .map(|line| (line.product_id.0.trim().to_string(), line))
        .collect();
    let variant_lines: BTreeMap<String, &QuoteLine> = variant_quote
        .lines
        .iter()
        .map(|line| (line.product_id.0.trim().to_string(), line))
        .collect();

    let added_product_ids = variant_lines
        .keys()
        .filter(|product_id| !baseline_lines.contains_key(*product_id))
        .map(|product_id| ProductId(product_id.clone()))
        .collect();

    let removed_product_ids = baseline_lines
        .keys()
        .filter(|product_id| !variant_lines.contains_key(*product_id))
        .map(|product_id| ProductId(product_id.clone()))
        .collect();

    let mut changed_lines = Vec::new();
    for (product_id, baseline_line) in &baseline_lines {
        let Some(variant_line) = variant_lines.get(product_id) else {
            continue;
        };

        if baseline_line.quantity != variant_line.quantity
            || baseline_line.unit_price != variant_line.unit_price
        {
            changed_lines.push(ConfigurationLineDelta {
                product_id: ProductId(product_id.clone()),
                quantity_before: baseline_line.quantity,
                quantity_after: variant_line.quantity,
                unit_price_before: baseline_line.unit_price,
                unit_price_after: variant_line.unit_price,
            });
        }
    }

    let configuration =
        ConfigurationDelta { added_product_ids, removed_product_ids, changed_lines };

    ScenarioDeltaBundle { price, policy, approval, configuration }
}

pub struct DealFlightSimulator<R> {
    cpq_runtime: R,
    max_variants: usize,
}

impl<R: CpqRuntime> DealFlightSimulator<R> {
    pub fn new(cpq_runtime: R) -> Self {
        Self { cpq_runtime, max_variants: DEFAULT_MAX_VARIANTS }
    }

    pub fn with_limits(cpq_runtime: R, max_variants: usize) -> Self {
        Self { cpq_runtime, max_variants }
    }

    pub fn simulate(
        &self,
        baseline_quote: &Quote,
        currency: &str,
        baseline_policy_input: PolicyInput,
        variations: Vec<ScenarioVariation>,
    ) -> Result<SimulationComparison, SimulatorGuardrailError> {
        let normalized = normalize_variations(variations, self.max_variants)?;

        let baseline_evaluation = self.cpq_runtime.evaluate_quote(CpqEvaluationInput {
            quote: baseline_quote,
            currency,
            policy_input: baseline_policy_input.clone(),
        });

        let mut variants = Vec::with_capacity(normalized.len());
        for variation in normalized {
            let forked_quote = fork_quote_with_variation(baseline_quote, &variation)?;
            let policy_input = PolicyInput {
                requested_discount_pct: variation
                    .requested_discount_pct_override
                    .unwrap_or(baseline_policy_input.requested_discount_pct),
                deal_value: variation
                    .deal_value_override
                    .unwrap_or(baseline_policy_input.deal_value),
                minimum_margin_pct: variation
                    .minimum_margin_pct_override
                    .unwrap_or(baseline_policy_input.minimum_margin_pct),
            };

            let evaluation = self.cpq_runtime.evaluate_quote(CpqEvaluationInput {
                quote: &forked_quote,
                currency,
                policy_input,
            });
            let delta =
                compute_delta(baseline_quote, &baseline_evaluation, &forked_quote, &evaluation);

            variants.push(SimulatedScenario {
                variant_key: variation.variant_key.clone(),
                forked_quote,
                normalized_variation: variation,
                evaluation,
                delta,
                rank_order: -1,
                rank_score: 0.0,
            });
        }

        rank_variants(&mut variants);

        Ok(SimulationComparison {
            baseline_quote: baseline_quote.clone(),
            baseline_evaluation,
            variants,
        })
    }

    pub fn simulate_with_telemetry<S: SimulationTelemetrySink>(
        &self,
        baseline_quote: &Quote,
        currency: &str,
        baseline_policy_input: PolicyInput,
        variations: Vec<ScenarioVariation>,
        telemetry_context: &SimulationTelemetryContext,
        telemetry_sink: &S,
    ) -> Result<SimulationComparison, SimulatorGuardrailError> {
        let requested_variant_count = variations.len() as i32;
        telemetry_sink.emit(ScenarioTelemetryEvent {
            event_type: ScenarioAuditEventType::RequestReceived,
            quote_id: baseline_quote.id.clone(),
            correlation_id: telemetry_context.correlation_id.clone(),
            scenario_run_id: telemetry_context.scenario_run_id.clone(),
            variant_key: None,
            variant_count: requested_variant_count,
            approval_required_variant_count: 0,
            latency_ms: 0,
            outcome: ScenarioTelemetryOutcome::Accepted,
            error_code: None,
            occurred_at: Utc::now(),
        });

        let started = Instant::now();
        let result = self.simulate(baseline_quote, currency, baseline_policy_input, variations);
        let latency_ms = duration_to_millis_i64(started.elapsed());

        match &result {
            Ok(comparison) => {
                let approval_required_variant_count = comparison
                    .variants
                    .iter()
                    .filter(|variant| variant.evaluation.policy.approval_required)
                    .count() as i32;

                telemetry_sink.emit(ScenarioTelemetryEvent {
                    event_type: ScenarioAuditEventType::ComparisonRendered,
                    quote_id: baseline_quote.id.clone(),
                    correlation_id: telemetry_context.correlation_id.clone(),
                    scenario_run_id: telemetry_context.scenario_run_id.clone(),
                    variant_key: None,
                    variant_count: comparison.variants.len() as i32,
                    approval_required_variant_count,
                    latency_ms,
                    outcome: ScenarioTelemetryOutcome::Success,
                    error_code: None,
                    occurred_at: Utc::now(),
                });
            }
            Err(error) => {
                telemetry_sink.emit(ScenarioTelemetryEvent {
                    event_type: ScenarioAuditEventType::ErrorOccurred,
                    quote_id: baseline_quote.id.clone(),
                    correlation_id: telemetry_context.correlation_id.clone(),
                    scenario_run_id: telemetry_context.scenario_run_id.clone(),
                    variant_key: None,
                    variant_count: requested_variant_count,
                    approval_required_variant_count: 0,
                    latency_ms,
                    outcome: ScenarioTelemetryOutcome::GuardrailRejected,
                    error_code: Some(simulator_guardrail_error_code(error).to_string()),
                    occurred_at: Utc::now(),
                });
            }
        }

        result
    }
}

fn simulator_guardrail_error_code(error: &SimulatorGuardrailError) -> &'static str {
    match error {
        SimulatorGuardrailError::EmptyVariationSet => "empty_variation_set",
        SimulatorGuardrailError::TooManyVariations { .. } => "too_many_variations",
        SimulatorGuardrailError::EmptyVariantKey => "empty_variant_key",
        SimulatorGuardrailError::DuplicateVariantKey { .. } => "duplicate_variant_key",
        SimulatorGuardrailError::EmptyVariantChanges { .. } => "empty_variant_changes",
        SimulatorGuardrailError::EmptyProductId => "empty_product_id",
        SimulatorGuardrailError::QuantityDeltaOutOfBounds { .. } => "quantity_delta_out_of_bounds",
        SimulatorGuardrailError::AggregatedQuantityDeltaOutOfBounds { .. } => {
            "aggregated_quantity_delta_out_of_bounds"
        }
        SimulatorGuardrailError::ConflictingUnitPriceOverride { .. } => {
            "conflicting_unit_price_override"
        }
        SimulatorGuardrailError::QuantityUnderflow { .. } => "quantity_underflow",
        SimulatorGuardrailError::MissingUnitPriceForNewLine { .. } => {
            "missing_unit_price_for_new_line"
        }
        SimulatorGuardrailError::NegativeDeltaForMissingLine { .. } => {
            "negative_delta_for_missing_line"
        }
    }
}

fn duration_to_millis_i64(duration: Duration) -> i64 {
    duration.as_millis().min(i64::MAX as u128) as i64
}

fn rank_variants(variants: &mut [SimulatedScenario]) {
    variants.sort_by(|left, right| {
        let left_key = (
            left.evaluation.policy.approval_required,
            left.evaluation.policy.violations.len(),
            left.evaluation.pricing.total,
            left.variant_key.clone(),
        );
        let right_key = (
            right.evaluation.policy.approval_required,
            right.evaluation.policy.violations.len(),
            right.evaluation.pricing.total,
            right.variant_key.clone(),
        );

        left_key.cmp(&right_key)
    });

    for (index, variant) in variants.iter_mut().enumerate() {
        variant.rank_order = index as i32;
        variant.rank_score = score_variant(variant);
    }
}

fn score_variant(variant: &SimulatedScenario) -> f64 {
    let approval_penalty = if variant.evaluation.policy.approval_required { 10_000.0 } else { 0.0 };
    let violation_penalty = (variant.evaluation.policy.violations.len() as f64) * 1_000.0;
    let total_cents =
        (variant.evaluation.pricing.total * Decimal::from(100)).round_dp(0).to_f64().unwrap_or(0.0);

    approval_penalty + violation_penalty + total_cents
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::Utc;
    use rust_decimal::Decimal;

    use crate::cpq::policy::PolicyInput;
    use crate::cpq::DeterministicCpqRuntime;
    use crate::domain::product::ProductId;
    use crate::domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};
    use crate::domain::simulation::{
        ScenarioAuditEventType, ScenarioDeltaType, ScenarioRunId, ScenarioTelemetryEvent,
        ScenarioTelemetryOutcome,
    };

    use super::{
        normalize_variations, DealFlightSimulator, LineVariation, ScenarioVariation,
        SimulationTelemetryContext, SimulationTelemetrySink, SimulatorGuardrailError,
    };

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn normalizer_merges_and_orders_line_variations() {
        let variations = vec![ScenarioVariation {
            variant_key: "B".to_string(),
            line_variations: vec![
                LineVariation {
                    product_id: ProductId("b".to_string()),
                    quantity_delta: 2,
                    unit_price_override: None,
                },
                LineVariation {
                    product_id: ProductId("a".to_string()),
                    quantity_delta: 1,
                    unit_price_override: Some(Decimal::new(1200, 2)),
                },
                LineVariation {
                    product_id: ProductId("b".to_string()),
                    quantity_delta: 3,
                    unit_price_override: None,
                },
            ],
            requested_discount_pct_override: None,
            minimum_margin_pct_override: None,
            deal_value_override: None,
        }];

        let normalized = normalize_variations(variations, 3).expect("normalize");
        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0].line_variations.len(), 2);
        assert_eq!(normalized[0].line_variations[0].product_id.0, "a");
        assert_eq!(normalized[0].line_variations[1].product_id.0, "b");
        assert_eq!(normalized[0].line_variations[1].quantity_delta, 5);
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn normalizer_trims_product_id_whitespace_before_grouping() {
        let variations = vec![ScenarioVariation {
            variant_key: "trim-test".to_string(),
            line_variations: vec![
                LineVariation {
                    product_id: ProductId(" plan-pro ".to_string()),
                    quantity_delta: 2,
                    unit_price_override: None,
                },
                LineVariation {
                    product_id: ProductId("plan-pro".to_string()),
                    quantity_delta: 3,
                    unit_price_override: None,
                },
            ],
            requested_discount_pct_override: None,
            minimum_margin_pct_override: None,
            deal_value_override: None,
        }];

        let normalized = normalize_variations(variations, 3).expect("normalize");
        assert_eq!(normalized.len(), 1);
        assert_eq!(normalized[0].line_variations.len(), 1);
        assert_eq!(normalized[0].line_variations[0].product_id.0, "plan-pro");
        assert_eq!(normalized[0].line_variations[0].quantity_delta, 5);
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn normalizer_rejects_duplicate_variant_keys() {
        let variations = vec![
            ScenarioVariation {
                variant_key: "v1".to_string(),
                line_variations: vec![LineVariation {
                    product_id: ProductId("plan".to_string()),
                    quantity_delta: 1,
                    unit_price_override: Some(Decimal::new(1000, 2)),
                }],
                requested_discount_pct_override: None,
                minimum_margin_pct_override: None,
                deal_value_override: None,
            },
            ScenarioVariation {
                variant_key: "V1".to_string(),
                line_variations: vec![LineVariation {
                    product_id: ProductId("plan".to_string()),
                    quantity_delta: 2,
                    unit_price_override: Some(Decimal::new(1000, 2)),
                }],
                requested_discount_pct_override: None,
                minimum_margin_pct_override: None,
                deal_value_override: None,
            },
        ];

        let error = normalize_variations(variations, 3).expect_err("must fail");
        assert!(matches!(error, SimulatorGuardrailError::DuplicateVariantKey { .. }));
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn normalizer_rejects_aggregated_quantity_delta_above_guardrail() {
        let variations = vec![ScenarioVariation {
            variant_key: "burst".to_string(),
            line_variations: vec![
                LineVariation {
                    product_id: ProductId("plan-pro".to_string()),
                    quantity_delta: 700_000,
                    unit_price_override: None,
                },
                LineVariation {
                    product_id: ProductId("plan-pro".to_string()),
                    quantity_delta: 700_000,
                    unit_price_override: None,
                },
            ],
            requested_discount_pct_override: None,
            minimum_margin_pct_override: None,
            deal_value_override: None,
        }];

        let error = normalize_variations(variations, 3).expect_err("must fail");
        assert!(matches!(
            error,
            SimulatorGuardrailError::AggregatedQuantityDeltaOutOfBounds { .. }
        ));
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn normalizer_rejects_extreme_negative_delta_without_panicking() {
        let variations = vec![ScenarioVariation {
            variant_key: "extreme-neg".to_string(),
            line_variations: vec![LineVariation {
                product_id: ProductId("plan-pro".to_string()),
                quantity_delta: i32::MIN,
                unit_price_override: None,
            }],
            requested_discount_pct_override: None,
            minimum_margin_pct_override: None,
            deal_value_override: None,
        }];

        let error = normalize_variations(variations, 3).expect_err("must fail");
        assert!(matches!(error, SimulatorGuardrailError::QuantityDeltaOutOfBounds { .. }));
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn simulator_replay_and_parity_are_deterministic() {
        let runtime = DeterministicCpqRuntime::default();
        let simulator = DealFlightSimulator::new(runtime);
        let quote = quote_fixture();
        let policy_input = PolicyInput {
            requested_discount_pct: Decimal::new(1000, 2),
            deal_value: Decimal::new(100_000, 2),
            minimum_margin_pct: Decimal::new(4000, 2),
        };

        let variations = vec![
            ScenarioVariation {
                variant_key: "keep-parity".to_string(),
                line_variations: vec![LineVariation {
                    product_id: ProductId("plan-pro".to_string()),
                    quantity_delta: 0,
                    unit_price_override: Some(Decimal::new(9999, 2)),
                }],
                requested_discount_pct_override: None,
                minimum_margin_pct_override: None,
                deal_value_override: None,
            },
            ScenarioVariation {
                variant_key: "add-seats".to_string(),
                line_variations: vec![LineVariation {
                    product_id: ProductId("plan-pro".to_string()),
                    quantity_delta: 5,
                    unit_price_override: None,
                }],
                requested_discount_pct_override: None,
                minimum_margin_pct_override: None,
                deal_value_override: None,
            },
        ];

        let run_a = simulator
            .simulate(&quote, "USD", policy_input.clone(), variations.clone())
            .expect("simulate run a");
        let run_b =
            simulator.simulate(&quote, "USD", policy_input, variations).expect("simulate run b");

        assert_eq!(run_a.variants.len(), 2);
        assert_eq!(run_a.variants[0].variant_key, run_b.variants[0].variant_key);
        assert_eq!(run_a.variants[1].variant_key, run_b.variants[1].variant_key);
        assert_eq!(
            run_a
                .variants
                .iter()
                .find(|variant| variant.variant_key == "keep-parity")
                .expect("parity variant")
                .delta
                .price
                .total_delta,
            Decimal::ZERO
        );
        assert_eq!(run_a.variants[0].rank_order, 0);
        assert_eq!(run_a.variants[1].rank_order, 1);
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn simulator_guards_new_line_without_price_override() {
        let runtime = DeterministicCpqRuntime::default();
        let simulator = DealFlightSimulator::new(runtime);
        let quote = quote_fixture();
        let policy_input = PolicyInput {
            requested_discount_pct: Decimal::ZERO,
            deal_value: Decimal::new(100_000, 2),
            minimum_margin_pct: Decimal::new(4000, 2),
        };

        let result = simulator.simulate(
            &quote,
            "USD",
            policy_input,
            vec![ScenarioVariation {
                variant_key: "new-line-missing-price".to_string(),
                line_variations: vec![LineVariation {
                    product_id: ProductId("addon-support".to_string()),
                    quantity_delta: 1,
                    unit_price_override: None,
                }],
                requested_discount_pct_override: None,
                minimum_margin_pct_override: None,
                deal_value_override: None,
            }],
        );

        assert!(matches!(
            result.expect_err("must guard"),
            SimulatorGuardrailError::MissingUnitPriceForNewLine { .. }
        ));
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn simulator_normalizes_whitespace_product_id_for_existing_baseline_line() {
        let runtime = DeterministicCpqRuntime::default();
        let simulator = DealFlightSimulator::new(runtime);
        let quote = Quote {
            id: QuoteId("Q-SIM-WHITESPACE-1".to_string()),
            status: QuoteStatus::Draft,
            lines: vec![QuoteLine {
                product_id: ProductId(" plan-pro ".to_string()),
                quantity: 10,
                unit_price: Decimal::new(9999, 2),
            }],
            created_at: Utc::now(),
        };
        let policy_input = PolicyInput {
            requested_discount_pct: Decimal::ZERO,
            deal_value: Decimal::new(100_000, 2),
            minimum_margin_pct: Decimal::new(4000, 2),
        };

        let comparison = simulator
            .simulate(
                &quote,
                "USD",
                policy_input,
                vec![ScenarioVariation {
                    variant_key: "trimmed-update".to_string(),
                    line_variations: vec![LineVariation {
                        product_id: ProductId("plan-pro".to_string()),
                        quantity_delta: 2,
                        unit_price_override: None,
                    }],
                    requested_discount_pct_override: None,
                    minimum_margin_pct_override: None,
                    deal_value_override: None,
                }],
            )
            .expect("simulate variation against whitespace baseline product id");

        let variant = &comparison.variants[0];
        assert_eq!(variant.forked_quote.lines.len(), 1);
        assert_eq!(variant.forked_quote.lines[0].product_id.0, "plan-pro");
        assert_eq!(variant.forked_quote.lines[0].quantity, 12);
        assert!(variant.delta.configuration.added_product_ids.is_empty());
        assert!(variant.delta.configuration.removed_product_ids.is_empty());
        assert_eq!(variant.delta.configuration.changed_lines.len(), 1);
        assert_eq!(variant.delta.configuration.changed_lines[0].product_id.0, "plan-pro");
        assert_eq!(variant.delta.configuration.changed_lines[0].quantity_before, 10);
        assert_eq!(variant.delta.configuration.changed_lines[0].quantity_after, 12);
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn delta_type_enum_has_expected_storage_value() {
        assert_eq!(ScenarioDeltaType::Price.as_str(), "price");
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn simulate_with_telemetry_emits_usage_latency_and_outcome_events() {
        #[derive(Default)]
        struct RecordingTelemetrySink {
            events: Mutex<Vec<ScenarioTelemetryEvent>>,
        }

        impl SimulationTelemetrySink for RecordingTelemetrySink {
            fn emit(&self, event: ScenarioTelemetryEvent) {
                self.events.lock().expect("lock telemetry sink").push(event);
            }
        }

        let runtime = DeterministicCpqRuntime::default();
        let simulator = DealFlightSimulator::new(runtime);
        let quote = quote_fixture();
        let policy_input = PolicyInput {
            requested_discount_pct: Decimal::new(500, 2),
            deal_value: Decimal::new(90_000, 2),
            minimum_margin_pct: Decimal::new(3500, 2),
        };
        let telemetry_sink = RecordingTelemetrySink::default();

        let result = simulator
            .simulate_with_telemetry(
                &quote,
                "USD",
                policy_input,
                vec![ScenarioVariation {
                    variant_key: "price-down".to_string(),
                    line_variations: vec![LineVariation {
                        product_id: ProductId("plan-pro".to_string()),
                        quantity_delta: 2,
                        unit_price_override: None,
                    }],
                    requested_discount_pct_override: None,
                    minimum_margin_pct_override: None,
                    deal_value_override: None,
                }],
                &SimulationTelemetryContext {
                    correlation_id: "req-sim-telemetry-ok".to_string(),
                    scenario_run_id: Some(ScenarioRunId("sim-run-telemetry-1".to_string())),
                },
                &telemetry_sink,
            )
            .expect("simulate with telemetry");

        assert_eq!(result.variants.len(), 1);

        let events = telemetry_sink.events.lock().expect("lock telemetry events").clone();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, ScenarioAuditEventType::RequestReceived);
        assert_eq!(events[0].outcome, ScenarioTelemetryOutcome::Accepted);
        assert_eq!(events[0].variant_count, 1);
        assert_eq!(events[1].event_type, ScenarioAuditEventType::ComparisonRendered);
        assert_eq!(events[1].outcome, ScenarioTelemetryOutcome::Success);
        assert_eq!(events[1].variant_count, 1);
        assert!(events[1].latency_ms >= 0);
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn simulate_with_telemetry_emits_failure_event_for_guardrail_rejection() {
        #[derive(Default)]
        struct RecordingTelemetrySink {
            events: Mutex<Vec<ScenarioTelemetryEvent>>,
        }

        impl SimulationTelemetrySink for RecordingTelemetrySink {
            fn emit(&self, event: ScenarioTelemetryEvent) {
                self.events.lock().expect("lock telemetry sink").push(event);
            }
        }

        let runtime = DeterministicCpqRuntime::default();
        let simulator = DealFlightSimulator::new(runtime);
        let quote = quote_fixture();
        let policy_input = PolicyInput {
            requested_discount_pct: Decimal::ZERO,
            deal_value: Decimal::new(50_000, 2),
            minimum_margin_pct: Decimal::new(4000, 2),
        };
        let telemetry_sink = RecordingTelemetrySink::default();

        let result = simulator.simulate_with_telemetry(
            &quote,
            "USD",
            policy_input,
            vec![ScenarioVariation {
                variant_key: "bad-new-line".to_string(),
                line_variations: vec![LineVariation {
                    product_id: ProductId("addon-support".to_string()),
                    quantity_delta: 1,
                    unit_price_override: None,
                }],
                requested_discount_pct_override: None,
                minimum_margin_pct_override: None,
                deal_value_override: None,
            }],
            &SimulationTelemetryContext {
                correlation_id: "req-sim-telemetry-fail".to_string(),
                scenario_run_id: Some(ScenarioRunId("sim-run-telemetry-2".to_string())),
            },
            &telemetry_sink,
        );

        assert!(matches!(
            result.expect_err("must fail"),
            SimulatorGuardrailError::MissingUnitPriceForNewLine { .. }
        ));

        let events = telemetry_sink.events.lock().expect("lock telemetry events").clone();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, ScenarioAuditEventType::RequestReceived);
        assert_eq!(events[1].event_type, ScenarioAuditEventType::ErrorOccurred);
        assert_eq!(events[1].outcome, ScenarioTelemetryOutcome::GuardrailRejected);
        assert_eq!(events[1].error_code.as_deref(), Some("missing_unit_price_for_new_line"));
    }

    fn quote_fixture() -> Quote {
        Quote {
            id: QuoteId("Q-SIM-CORE-1".to_string()),
            status: QuoteStatus::Draft,
            lines: vec![QuoteLine {
                product_id: ProductId("plan-pro".to_string()),
                quantity: 10,
                unit_price: Decimal::new(9999, 2),
            }],
            created_at: Utc::now(),
        }
    }
}
