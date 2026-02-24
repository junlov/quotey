//! Explain Any Number - Deterministic Explanation Engine
//!
//! Provides deterministic explanation assembly for quote totals and line items,
//! sourced only from persisted pricing trace and policy artifacts.

use chrono::Utc;
use rust_decimal::Decimal;
use std::collections::HashMap;

use crate::domain::explanation::*;
use crate::domain::quote::{QuoteId, QuoteLineId};

/// Error types for explanation operations
#[derive(Clone, Debug, PartialEq)]
pub enum ExplanationError {
    MissingPricingSnapshot { quote_id: QuoteId },
    MissingPolicyEvaluation { quote_id: QuoteId },
    InvalidLineId { quote_id: QuoteId, line_id: QuoteLineId },
    QuoteNotFound { quote_id: QuoteId },
    VersionMismatch { expected: i32, actual: i32 },
    EvidenceGatheringFailed { reason: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum GuardrailedExplanation {
    Allowed {
        request_id: ExplanationRequestId,
        response: ExplanationResponse,
        audit_events: Vec<ExplanationAuditEvent>,
    },
    Denied {
        request_id: ExplanationRequestId,
        user_message: String,
        audit_events: Vec<ExplanationAuditEvent>,
    },
    Degraded {
        request_id: ExplanationRequestId,
        response: ExplanationResponse,
        user_message: String,
        audit_events: Vec<ExplanationAuditEvent>,
    },
}

impl std::fmt::Display for ExplanationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingPricingSnapshot { quote_id } => {
                write!(f, "Pricing snapshot not found for quote {}", quote_id.0)
            }
            Self::MissingPolicyEvaluation { quote_id } => {
                write!(f, "Policy evaluation not found for quote {}", quote_id.0)
            }
            Self::InvalidLineId { quote_id, line_id } => {
                write!(f, "Line {} not found in quote {}", line_id.0, quote_id.0)
            }
            Self::QuoteNotFound { quote_id } => {
                write!(f, "Quote {} not found", quote_id.0)
            }
            Self::VersionMismatch { expected, actual } => {
                write!(f, "Version mismatch: expected {}, got {}", expected, actual)
            }
            Self::EvidenceGatheringFailed { reason } => {
                write!(f, "Failed to gather evidence: {}", reason)
            }
        }
    }
}

/// Trait for pricing snapshot repository (abstract for testability)
pub trait PricingSnapshotProvider: Send + Sync {
    fn get_snapshot(
        &self,
        quote_id: &QuoteId,
        version: i32,
    ) -> Result<PricingSnapshot, ExplanationError>;
}

/// Trait for policy evaluation repository
pub trait PolicyEvaluationProvider: Send + Sync {
    fn get_evaluation(
        &self,
        quote_id: &QuoteId,
        version: i32,
    ) -> Result<PolicyEvaluation, ExplanationError>;
}

/// Pricing snapshot data (from CPQ pricing engine)
#[derive(Clone, Debug, PartialEq)]
pub struct PricingSnapshot {
    pub quote_id: QuoteId,
    pub version: i32,
    pub subtotal: Decimal,
    pub discount_total: Decimal,
    pub tax_total: Decimal,
    pub total: Decimal,
    pub currency: String,
    pub line_items: Vec<PricingLineSnapshot>,
    pub calculation_steps: Vec<CalculationStep>,
    pub created_at: String,
}

/// Individual line item pricing snapshot
#[derive(Clone, Debug, PartialEq)]
pub struct PricingLineSnapshot {
    pub line_id: String,
    pub product_id: String,
    pub product_name: String,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub discount_percent: Decimal,
    pub discount_amount: Decimal,
    pub line_subtotal: Decimal,
}

/// Calculation step in pricing trace
#[derive(Clone, Debug, PartialEq)]
pub struct CalculationStep {
    pub step_order: i32,
    pub step_name: String,
    pub input_values: HashMap<String, Decimal>,
    pub output_value: Decimal,
    pub formula: Option<String>,
}

/// Policy evaluation data
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyEvaluation {
    pub quote_id: QuoteId,
    pub version: i32,
    pub overall_status: String, // "approved", "violation", "waived"
    pub violations: Vec<PolicyViolation>,
    pub applied_rules: Vec<AppliedRule>,
    pub evaluated_at: String,
}

/// Policy violation record
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyViolation {
    pub policy_id: String,
    pub policy_name: String,
    pub severity: String, // "blocking", "warning", "info"
    pub threshold_value: Option<Decimal>,
    pub actual_value: Decimal,
    pub message: String,
    pub suggested_resolution: Option<String>,
}

/// Applied rule record
#[derive(Clone, Debug, PartialEq)]
pub struct AppliedRule {
    pub rule_id: String,
    pub rule_name: String,
    pub rule_section: String,
    pub rule_description: String,
}

/// Format a decimal value as currency string
fn format_currency_value(value: &Decimal, currency: &str) -> String {
    let symbol = match currency {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        _ => currency,
    };
    format!("{}{:.2}", symbol, value)
}

/// Deterministic explanation engine
pub struct ExplanationEngine<P, O> {
    pricing_provider: P,
    policy_provider: O,
}

impl<P: PricingSnapshotProvider, O: PolicyEvaluationProvider> ExplanationEngine<P, O> {
    /// Create a new explanation engine
    pub fn new(pricing_provider: P, policy_provider: O) -> Self {
        Self { pricing_provider, policy_provider }
    }

    /// Explain the total amount for a quote
    pub fn explain_total(
        &self,
        quote_id: &QuoteId,
        version: i32,
    ) -> Result<ExplanationResponse, ExplanationError> {
        let pricing = self.pricing_provider.get_snapshot(quote_id, version)?;
        let policy = self.policy_provider.get_evaluation(quote_id, version)?;

        let arithmetic_chain = self.build_total_arithmetic_chain(&pricing);
        let policy_evidence = self.build_policy_evidence(&policy);
        let source_references = self.build_source_references(quote_id, version, &pricing, &policy);

        let user_summary = self.generate_total_summary(&pricing, &policy);

        Ok(ExplanationResponse {
            request_id: ExplanationRequestId(format!("exp-{}", Utc::now().timestamp_millis())),
            quote_id: quote_id.clone(),
            amount: pricing.total,
            amount_description: format!("Total for quote {}", quote_id.0),
            arithmetic_chain,
            policy_evidence,
            source_references,
            user_summary,
        })
    }

    /// Explain a specific line item
    pub fn explain_line(
        &self,
        quote_id: &QuoteId,
        line_id: &QuoteLineId,
        version: i32,
    ) -> Result<ExplanationResponse, ExplanationError> {
        let pricing = self.pricing_provider.get_snapshot(quote_id, version)?;
        let policy = self.policy_provider.get_evaluation(quote_id, version)?;

        let line = pricing.line_items.iter().find(|l| l.line_id == line_id.0).ok_or_else(|| {
            ExplanationError::InvalidLineId { quote_id: quote_id.clone(), line_id: line_id.clone() }
        })?;

        let arithmetic_chain = self.build_line_arithmetic_chain(line);
        let policy_evidence = self.build_policy_evidence(&policy);
        let source_references = self.build_source_references(quote_id, version, &pricing, &policy);

        let user_summary = self.generate_line_summary(line, &policy);

        Ok(ExplanationResponse {
            request_id: ExplanationRequestId(format!("exp-{}", Utc::now().timestamp_millis())),
            quote_id: quote_id.clone(),
            amount: line.line_subtotal,
            amount_description: format!("{} ({})", line.product_name, line.product_id),
            arithmetic_chain,
            policy_evidence,
            source_references,
            user_summary,
        })
    }

    /// Explain policy decisions for a quote
    pub fn explain_policy(
        &self,
        quote_id: &QuoteId,
        version: i32,
    ) -> Result<ExplanationResponse, ExplanationError> {
        let pricing = self.pricing_provider.get_snapshot(quote_id, version)?;
        let policy = self.policy_provider.get_evaluation(quote_id, version)?;

        let arithmetic_chain = vec![]; // Policy explanations don't have arithmetic
        let policy_evidence = self.build_policy_evidence(&policy);
        let source_references = self.build_source_references(quote_id, version, &pricing, &policy);

        let user_summary = self.generate_policy_summary(&policy);

        Ok(ExplanationResponse {
            request_id: ExplanationRequestId(format!("exp-{}", Utc::now().timestamp_millis())),
            quote_id: quote_id.clone(),
            amount: pricing.total,
            amount_description: format!("Policy evaluation for quote {}", quote_id.0),
            arithmetic_chain,
            policy_evidence,
            source_references,
            user_summary,
        })
    }

    /// Explain with deterministic guardrail enforcement and explicit audit trail.
    pub fn explain_with_guardrails(
        &self,
        request: CreateExplanationRequest,
    ) -> GuardrailedExplanation {
        let request_id = ExplanationRequestId(format!("exp-{}", Utc::now().timestamp_millis()));
        let mut audit_events = vec![self.audit_event(
            &request_id,
            ExplanationEventType::RequestReceived,
            "{}".to_string(),
            &request.actor_id,
            &request.correlation_id,
        )];

        if request.quote_version < 1 {
            let message =
                "I can't explain this amount because the quote version is invalid. Refresh the quote and try again."
                    .to_string();
            audit_events.push(self.audit_event(
                &request_id,
                ExplanationEventType::ErrorOccurred,
                Self::guardrail_payload("denied", "invalid_quote_version", &message),
                &request.actor_id,
                &request.correlation_id,
            ));
            return GuardrailedExplanation::Denied {
                request_id,
                user_message: message,
                audit_events,
            };
        }

        let pricing =
            match self.pricing_provider.get_snapshot(&request.quote_id, request.quote_version) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    let message = Self::user_safe_message_for_error(&error);
                    audit_events.push(self.audit_event(
                        &request_id,
                        ExplanationEventType::ErrorOccurred,
                        Self::guardrail_payload("denied", "missing_pricing_snapshot", &message),
                        &request.actor_id,
                        &request.correlation_id,
                    ));
                    return GuardrailedExplanation::Denied {
                        request_id,
                        user_message: message,
                        audit_events,
                    };
                }
            };

        if pricing.version != request.quote_version {
            let message =
                "I can't explain this amount because the pricing snapshot version does not match the request."
                    .to_string();
            audit_events.push(self.audit_event(
                &request_id,
                ExplanationEventType::ErrorOccurred,
                Self::guardrail_payload("denied", "pricing_version_mismatch", &message),
                &request.actor_id,
                &request.correlation_id,
            ));
            return GuardrailedExplanation::Denied {
                request_id,
                user_message: message,
                audit_events,
            };
        }

        let policy =
            match self.policy_provider.get_evaluation(&request.quote_id, request.quote_version) {
                Ok(evaluation) => Some(evaluation),
                Err(ExplanationError::MissingPolicyEvaluation { .. }) => None,
                Err(error) => {
                    let message = Self::user_safe_message_for_error(&error);
                    audit_events.push(self.audit_event(
                        &request_id,
                        ExplanationEventType::ErrorOccurred,
                        Self::guardrail_payload("denied", "policy_evidence_error", &message),
                        &request.actor_id,
                        &request.correlation_id,
                    ));
                    return GuardrailedExplanation::Denied {
                        request_id,
                        user_message: message,
                        audit_events,
                    };
                }
            };

        let (mut response, degraded_message) = match (&request.request_type, policy.as_ref()) {
            (ExplanationRequestType::Total, Some(_)) => {
                match self.explain_total(&request.quote_id, request.quote_version) {
                    Ok(resp) => (resp, None),
                    Err(error) => {
                        let message = Self::user_safe_message_for_error(&error);
                        audit_events.push(self.audit_event(
                            &request_id,
                            ExplanationEventType::ErrorOccurred,
                            Self::guardrail_payload("denied", "explain_total_failed", &message),
                            &request.actor_id,
                            &request.correlation_id,
                        ));
                        return GuardrailedExplanation::Denied {
                            request_id,
                            user_message: message,
                            audit_events,
                        };
                    }
                }
            }
            (ExplanationRequestType::Line, Some(_)) => {
                let Some(line_id) = request.line_id.as_ref() else {
                    let message =
                        "I can't explain that line item yet. Select a specific quote line and try again."
                            .to_string();
                    audit_events.push(self.audit_event(
                        &request_id,
                        ExplanationEventType::ErrorOccurred,
                        Self::guardrail_payload("denied", "missing_line_id", &message),
                        &request.actor_id,
                        &request.correlation_id,
                    ));
                    return GuardrailedExplanation::Denied {
                        request_id,
                        user_message: message,
                        audit_events,
                    };
                };

                match self.explain_line(&request.quote_id, line_id, request.quote_version) {
                    Ok(resp) => (resp, None),
                    Err(error) => {
                        let message = Self::user_safe_message_for_error(&error);
                        audit_events.push(self.audit_event(
                            &request_id,
                            ExplanationEventType::ErrorOccurred,
                            Self::guardrail_payload("denied", "explain_line_failed", &message),
                            &request.actor_id,
                            &request.correlation_id,
                        ));
                        return GuardrailedExplanation::Denied {
                            request_id,
                            user_message: message,
                            audit_events,
                        };
                    }
                }
            }
            (ExplanationRequestType::Policy, Some(_)) => {
                match self.explain_policy(&request.quote_id, request.quote_version) {
                    Ok(resp) => (resp, None),
                    Err(error) => {
                        let message = Self::user_safe_message_for_error(&error);
                        audit_events.push(self.audit_event(
                            &request_id,
                            ExplanationEventType::ErrorOccurred,
                            Self::guardrail_payload("denied", "explain_policy_failed", &message),
                            &request.actor_id,
                            &request.correlation_id,
                        ));
                        return GuardrailedExplanation::Denied {
                            request_id,
                            user_message: message,
                            audit_events,
                        };
                    }
                }
            }
            (_, None) => {
                let degraded_message = "Policy evidence is temporarily unavailable. Showing deterministic pricing breakdown only."
                    .to_string();
                let response = self.fallback_pricing_only_response(&request_id, &request, &pricing);
                (response, Some(degraded_message))
            }
        };

        response.request_id = request_id.clone();

        audit_events.push(self.audit_event(
            &request_id,
            if degraded_message.is_some() {
                ExplanationEventType::EvidenceMissing
            } else {
                ExplanationEventType::EvidenceGathered
            },
            "{}".to_string(),
            &request.actor_id,
            &request.correlation_id,
        ));
        audit_events.push(self.audit_event(
            &request_id,
            ExplanationEventType::ExplanationGenerated,
            "{}".to_string(),
            &request.actor_id,
            &request.correlation_id,
        ));
        audit_events.push(self.audit_event(
            &request_id,
            ExplanationEventType::ExplanationDelivered,
            "{}".to_string(),
            &request.actor_id,
            &request.correlation_id,
        ));

        match degraded_message {
            Some(user_message) => GuardrailedExplanation::Degraded {
                request_id,
                response,
                user_message,
                audit_events,
            },
            None => GuardrailedExplanation::Allowed { request_id, response, audit_events },
        }
    }

    /// Build arithmetic chain for quote total
    fn build_total_arithmetic_chain(&self, pricing: &PricingSnapshot) -> Vec<ArithmeticStep> {
        let mut steps = vec![];
        let mut step_order = 1;

        // Step 1: Sum line items
        let line_items_subtotal: Decimal = pricing.line_items.iter().map(|l| l.line_subtotal).sum();
        steps.push(ArithmeticStep {
            step_order,
            operation: "sum".to_string(),
            input_values: pricing
                .line_items
                .iter()
                .map(|l| (format!("line_{}", l.line_id), l.line_subtotal))
                .collect(),
            result: line_items_subtotal,
            description: "Sum of all line item subtotals".to_string(),
        });
        step_order += 1;

        // Step 2: Apply discounts
        if pricing.discount_total > Decimal::ZERO {
            steps.push(ArithmeticStep {
                step_order,
                operation: "subtract".to_string(),
                input_values: vec![
                    ("subtotal".to_string(), line_items_subtotal),
                    ("discount_total".to_string(), pricing.discount_total),
                ]
                .into_iter()
                .collect(),
                result: line_items_subtotal - pricing.discount_total,
                description: "Apply total discounts".to_string(),
            });
            step_order += 1;
        }

        // Step 3: Add tax
        if pricing.tax_total > Decimal::ZERO {
            let after_discount = line_items_subtotal - pricing.discount_total;
            steps.push(ArithmeticStep {
                step_order,
                operation: "add".to_string(),
                input_values: vec![
                    ("after_discount".to_string(), after_discount),
                    ("tax_total".to_string(), pricing.tax_total),
                ]
                .into_iter()
                .collect(),
                result: after_discount + pricing.tax_total,
                description: "Add applicable taxes".to_string(),
            });
            step_order += 1;
        }

        // Step 4: Final total
        steps.push(ArithmeticStep {
            step_order,
            operation: "total".to_string(),
            input_values: vec![("total".to_string(), pricing.total)].into_iter().collect(),
            result: pricing.total,
            description: format!("Final total in {}", pricing.currency),
        });

        steps
    }

    /// Build arithmetic chain for a line item
    fn build_line_arithmetic_chain(&self, line: &PricingLineSnapshot) -> Vec<ArithmeticStep> {
        let mut steps = vec![];

        // Step 1: Calculate base price (quantity * unit_price)
        let base_price = line.unit_price * Decimal::from(line.quantity);
        steps.push(ArithmeticStep {
            step_order: 1,
            operation: "multiply".to_string(),
            input_values: vec![
                ("unit_price".to_string(), line.unit_price),
                ("quantity".to_string(), Decimal::from(line.quantity)),
            ]
            .into_iter()
            .collect(),
            result: base_price,
            description: format!(
                "Base price for {} ({} × {})",
                line.product_name, line.unit_price, line.quantity
            ),
        });

        // Step 2: Apply discount if any
        if line.discount_percent > Decimal::ZERO {
            let discount_amount = base_price * (line.discount_percent / Decimal::from(100));
            steps.push(ArithmeticStep {
                step_order: 2,
                operation: "discount".to_string(),
                input_values: vec![
                    ("base_price".to_string(), base_price),
                    ("discount_percent".to_string(), line.discount_percent),
                    ("discount_amount".to_string(), discount_amount),
                ]
                .into_iter()
                .collect(),
                result: line.line_subtotal,
                description: format!("Apply {:.1}% discount", line.discount_percent),
            });
        }

        // Step 3: Final line subtotal
        steps.push(ArithmeticStep {
            step_order: if line.discount_percent > Decimal::ZERO { 3 } else { 2 },
            operation: "line_total".to_string(),
            input_values: vec![("line_subtotal".to_string(), line.line_subtotal)]
                .into_iter()
                .collect(),
            result: line.line_subtotal,
            description: format!("{} line total", line.product_name),
        });

        steps
    }

    /// Build policy evidence from evaluation
    fn build_policy_evidence(&self, policy: &PolicyEvaluation) -> Vec<PolicyEvaluationEvidence> {
        policy
            .violations
            .iter()
            .map(|v| PolicyEvaluationEvidence {
                policy_id: v.policy_id.clone(),
                policy_name: v.policy_name.clone(),
                decision: if v.severity == "blocking" {
                    "violated".to_string()
                } else {
                    "warning".to_string()
                },
                threshold_value: v.threshold_value.map(|d| d.to_string()),
                actual_value: v.actual_value.to_string(),
                violation_message: Some(v.message.clone()),
            })
            .chain(policy.applied_rules.iter().map(|r| PolicyEvaluationEvidence {
                policy_id: r.rule_id.clone(),
                policy_name: r.rule_name.clone(),
                decision: "passed".to_string(),
                threshold_value: None,
                actual_value: "compliant".to_string(),
                violation_message: None,
            }))
            .collect()
    }

    /// Build source references for audit trail
    fn build_source_references(
        &self,
        quote_id: &QuoteId,
        version: i32,
        pricing: &PricingSnapshot,
        policy: &PolicyEvaluation,
    ) -> Vec<SourceReference> {
        vec![
            SourceReference {
                source_type: "pricing_snapshot".to_string(),
                source_id: pricing.quote_id.0.clone(),
                source_version: version.to_string(),
                field_path: "total".to_string(),
            },
            SourceReference {
                source_type: "policy_evaluation".to_string(),
                source_id: policy.quote_id.0.clone(),
                source_version: version.to_string(),
                field_path: "overall_status".to_string(),
            },
            SourceReference {
                source_type: "quote".to_string(),
                source_id: quote_id.0.clone(),
                source_version: version.to_string(),
                field_path: "lines".to_string(),
            },
        ]
    }

    /// Generate human-readable summary for total explanation
    fn generate_total_summary(
        &self,
        pricing: &PricingSnapshot,
        policy: &PolicyEvaluation,
    ) -> String {
        let violation_count = policy.violations.iter().filter(|v| v.severity == "blocking").count();
        let warning_count = policy.violations.iter().filter(|v| v.severity == "warning").count();

        let mut summary = format!(
            "Quote total of {} {} calculated from {} line item(s). ",
            format_currency_value(&pricing.total, &pricing.currency),
            pricing.currency,
            pricing.line_items.len()
        );

        if pricing.discount_total > Decimal::ZERO {
            summary.push_str(&format!(
                "Total discounts applied: {}. ",
                format_currency_value(&pricing.discount_total, &pricing.currency)
            ));
        }

        if violation_count > 0 {
            summary.push_str(&format!(
                "⚠️ {} policy violation(s) require attention. ",
                violation_count
            ));
        } else if warning_count > 0 {
            summary.push_str(&format!("⚡ {} policy warning(s) noted. ", warning_count));
        } else {
            summary.push_str("✅ All policy checks passed. ");
        }

        summary.push_str(&format!("Policy evaluation: {}.", policy.overall_status.to_uppercase()));

        summary
    }

    /// Generate human-readable summary for line explanation
    fn generate_line_summary(
        &self,
        line: &PricingLineSnapshot,
        policy: &PolicyEvaluation,
    ) -> String {
        let mut summary = format!(
            "{} ({}): {} × {} = {}. ",
            line.product_name, line.product_id, line.quantity, line.unit_price, line.line_subtotal
        );

        if line.discount_percent > Decimal::ZERO {
            summary.push_str(&format!(
                "Discount of {:.1}% applied ({} off). ",
                line.discount_percent, line.discount_amount
            ));
        }

        // Check for policy violations related to this product
        let product_violations: Vec<_> =
            policy.violations.iter().filter(|v| v.message.contains(&line.product_id)).collect();

        if !product_violations.is_empty() {
            summary.push_str(&format!(
                "⚠️ {} policy issue(s) related to this product.",
                product_violations.len()
            ));
        }

        summary
    }

    /// Generate human-readable summary for policy explanation
    fn generate_policy_summary(&self, policy: &PolicyEvaluation) -> String {
        let blocking = policy.violations.iter().filter(|v| v.severity == "blocking").count();
        let warnings = policy.violations.iter().filter(|v| v.severity == "warning").count();
        let rules = policy.applied_rules.len();

        let mut summary = format!("Policy evaluation: {}. ", policy.overall_status.to_uppercase());

        summary.push_str(&format!("{} rule(s) evaluated. ", rules));

        if blocking > 0 {
            summary.push_str(&format!("🚫 {} blocking violation(s) must be resolved. ", blocking));
        }

        if warnings > 0 {
            summary.push_str(&format!("⚡ {} warning(s) noted. ", warnings));
        }

        if blocking == 0 && warnings == 0 {
            summary.push_str("✅ All checks passed.");
        }

        summary
    }

    fn fallback_pricing_only_response(
        &self,
        request_id: &ExplanationRequestId,
        request: &CreateExplanationRequest,
        pricing: &PricingSnapshot,
    ) -> ExplanationResponse {
        match request.request_type {
            ExplanationRequestType::Line => {
                if let Some(line_id) = request.line_id.as_ref() {
                    if let Some(line) = pricing.line_items.iter().find(|line| line.line_id == line_id.0)
                    {
                        return ExplanationResponse {
                            request_id: request_id.clone(),
                            quote_id: request.quote_id.clone(),
                            amount: line.line_subtotal,
                            amount_description: format!("{} ({})", line.product_name, line.product_id),
                            arithmetic_chain: self.build_line_arithmetic_chain(line),
                            policy_evidence: vec![],
                            source_references: vec![
                                SourceReference {
                                    source_type: "pricing_snapshot".to_string(),
                                    source_id: pricing.quote_id.0.clone(),
                                    source_version: request.quote_version.to_string(),
                                    field_path: "line_items".to_string(),
                                },
                                SourceReference {
                                    source_type: "quote".to_string(),
                                    source_id: request.quote_id.0.clone(),
                                    source_version: request.quote_version.to_string(),
                                    field_path: "lines".to_string(),
                                },
                            ],
                            user_summary:
                                "Policy evidence unavailable. Showing pricing-only line explanation."
                                    .to_string(),
                        };
                    }
                }
            }
            ExplanationRequestType::Policy => {
                return ExplanationResponse {
                    request_id: request_id.clone(),
                    quote_id: request.quote_id.clone(),
                    amount: pricing.total,
                    amount_description: format!("Policy evaluation for quote {}", request.quote_id.0),
                    arithmetic_chain: vec![],
                    policy_evidence: vec![],
                    source_references: vec![SourceReference {
                        source_type: "pricing_snapshot".to_string(),
                        source_id: pricing.quote_id.0.clone(),
                        source_version: request.quote_version.to_string(),
                        field_path: "total".to_string(),
                    }],
                    user_summary:
                        "Policy evidence unavailable. No policy assertions can be provided right now."
                            .to_string(),
                };
            }
            ExplanationRequestType::Total => {}
        }

        ExplanationResponse {
            request_id: request_id.clone(),
            quote_id: request.quote_id.clone(),
            amount: pricing.total,
            amount_description: format!("Total for quote {}", request.quote_id.0),
            arithmetic_chain: self.build_total_arithmetic_chain(pricing),
            policy_evidence: vec![],
            source_references: vec![
                SourceReference {
                    source_type: "pricing_snapshot".to_string(),
                    source_id: pricing.quote_id.0.clone(),
                    source_version: request.quote_version.to_string(),
                    field_path: "total".to_string(),
                },
                SourceReference {
                    source_type: "quote".to_string(),
                    source_id: request.quote_id.0.clone(),
                    source_version: request.quote_version.to_string(),
                    field_path: "lines".to_string(),
                },
            ],
            user_summary:
                "Policy evidence unavailable. Showing deterministic pricing breakdown only."
                    .to_string(),
        }
    }

    fn audit_event(
        &self,
        request_id: &ExplanationRequestId,
        event_type: ExplanationEventType,
        event_payload_json: String,
        actor_id: &str,
        correlation_id: &str,
    ) -> ExplanationAuditEvent {
        ExplanationAuditEvent {
            id: format!("exp-audit-{}-{}", request_id.0, Utc::now().timestamp_millis()),
            explanation_request_id: request_id.clone(),
            event_type,
            event_payload_json,
            actor_type: "system".to_string(),
            actor_id: actor_id.to_string(),
            correlation_id: correlation_id.to_string(),
            occurred_at: Utc::now(),
        }
    }

    fn guardrail_payload(guardrail: &str, reason: &str, message: &str) -> String {
        serde_json::json!({
            "guardrail": guardrail,
            "reason": reason,
            "message": message
        })
        .to_string()
    }

    fn user_safe_message_for_error(error: &ExplanationError) -> String {
        match error {
            ExplanationError::MissingPricingSnapshot { .. } => {
                "I can't explain this amount yet because pricing evidence is missing. Refresh the quote and try again."
                    .to_string()
            }
            ExplanationError::MissingPolicyEvaluation { .. } => {
                "I can't include policy reasoning right now because policy evidence is missing."
                    .to_string()
            }
            ExplanationError::InvalidLineId { .. } => {
                "I can't find that line item in this quote version. Pick a valid line and retry."
                    .to_string()
            }
            ExplanationError::QuoteNotFound { .. } => {
                "I can't find that quote. Confirm the quote context and try again.".to_string()
            }
            ExplanationError::VersionMismatch { .. } => {
                "I can't explain this amount for that version. Refresh the quote and retry."
                    .to_string()
            }
            ExplanationError::EvidenceGatheringFailed { .. } => {
                "I couldn't gather deterministic evidence for this explanation. Retry shortly."
                    .to_string()
            }
        }
    }
}

/// In-memory implementation of pricing snapshot provider (for testing)
pub struct InMemoryPricingProvider {
    snapshots: HashMap<(String, i32), PricingSnapshot>,
}

impl Default for InMemoryPricingProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryPricingProvider {
    pub fn new() -> Self {
        Self { snapshots: HashMap::new() }
    }

    pub fn add_snapshot(&mut self, quote_id: &QuoteId, version: i32, snapshot: PricingSnapshot) {
        self.snapshots.insert((quote_id.0.clone(), version), snapshot);
    }
}

impl PricingSnapshotProvider for InMemoryPricingProvider {
    fn get_snapshot(
        &self,
        quote_id: &QuoteId,
        version: i32,
    ) -> Result<PricingSnapshot, ExplanationError> {
        self.snapshots
            .get(&(quote_id.0.clone(), version))
            .cloned()
            .ok_or_else(|| ExplanationError::MissingPricingSnapshot { quote_id: quote_id.clone() })
    }
}

/// In-memory implementation of policy evaluation provider (for testing)
pub struct InMemoryPolicyProvider {
    evaluations: HashMap<(String, i32), PolicyEvaluation>,
}

impl Default for InMemoryPolicyProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryPolicyProvider {
    pub fn new() -> Self {
        Self { evaluations: HashMap::new() }
    }

    pub fn add_evaluation(
        &mut self,
        quote_id: &QuoteId,
        version: i32,
        evaluation: PolicyEvaluation,
    ) {
        self.evaluations.insert((quote_id.0.clone(), version), evaluation);
    }
}

impl PolicyEvaluationProvider for InMemoryPolicyProvider {
    fn get_evaluation(
        &self,
        quote_id: &QuoteId,
        version: i32,
    ) -> Result<PolicyEvaluation, ExplanationError> {
        self.evaluations
            .get(&(quote_id.0.clone(), version))
            .cloned()
            .ok_or_else(|| ExplanationError::MissingPolicyEvaluation { quote_id: quote_id.clone() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_quote_id(id: &str) -> QuoteId {
        QuoteId(id.to_string())
    }

    fn create_test_line_id(id: &str) -> QuoteLineId {
        QuoteLineId(id.to_string())
    }

    fn create_test_pricing_snapshot(quote_id: &QuoteId) -> PricingSnapshot {
        PricingSnapshot {
            quote_id: quote_id.clone(),
            version: 1,
            subtotal: Decimal::new(20000, 2),      // $200.00
            discount_total: Decimal::new(2000, 2), // $20.00
            tax_total: Decimal::ZERO,
            total: Decimal::new(18000, 2), // $180.00
            currency: "USD".to_string(),
            line_items: vec![
                PricingLineSnapshot {
                    line_id: "line-1".to_string(),
                    product_id: "prod-1".to_string(),
                    product_name: "Enterprise Plan".to_string(),
                    quantity: 2,
                    unit_price: Decimal::new(5000, 2), // $50.00
                    discount_percent: Decimal::new(1000, 2), // 10%
                    discount_amount: Decimal::new(1000, 2), // $10.00
                    line_subtotal: Decimal::new(9000, 2), // $90.00
                },
                PricingLineSnapshot {
                    line_id: "line-2".to_string(),
                    product_id: "prod-2".to_string(),
                    product_name: "Support Add-on".to_string(),
                    quantity: 1,
                    unit_price: Decimal::new(10000, 2), // $100.00
                    discount_percent: Decimal::new(1000, 2), // 10%
                    discount_amount: Decimal::new(1000, 2), // $10.00
                    line_subtotal: Decimal::new(9000, 2), // $90.00
                },
            ],
            calculation_steps: vec![],
            created_at: Utc::now().to_rfc3339(),
        }
    }

    fn create_test_policy_evaluation(quote_id: &QuoteId) -> PolicyEvaluation {
        PolicyEvaluation {
            quote_id: quote_id.clone(),
            version: 1,
            overall_status: "approved".to_string(),
            violations: vec![],
            applied_rules: vec![AppliedRule {
                rule_id: "rule-1".to_string(),
                rule_name: "Discount Cap Check".to_string(),
                rule_section: "pricing.policy.discount".to_string(),
                rule_description: "Verify discount within allowed range".to_string(),
            }],
            evaluated_at: Utc::now().to_rfc3339(),
        }
    }

    #[test]
    fn explain_total_returns_complete_explanation() {
        let quote_id = create_test_quote_id("Q-2026-001");
        let pricing = create_test_pricing_snapshot(&quote_id);
        let policy = create_test_policy_evaluation(&quote_id);

        let mut pricing_provider = InMemoryPricingProvider::new();
        let mut policy_provider = InMemoryPolicyProvider::new();

        pricing_provider.add_snapshot(&quote_id, 1, pricing);
        policy_provider.add_evaluation(&quote_id, 1, policy);

        let engine = ExplanationEngine::new(pricing_provider, policy_provider);
        let explanation = engine.explain_total(&quote_id, 1).expect("should succeed");

        assert_eq!(explanation.quote_id.0, "Q-2026-001");
        assert_eq!(explanation.amount, Decimal::new(18000, 2));
        assert!(!explanation.arithmetic_chain.is_empty());
        assert!(!explanation.source_references.is_empty());
        assert!(explanation.user_summary.contains("$180.00"));
    }

    #[test]
    fn explain_line_returns_line_specific_explanation() {
        let quote_id = create_test_quote_id("Q-2026-002");
        let pricing = create_test_pricing_snapshot(&quote_id);
        let policy = create_test_policy_evaluation(&quote_id);

        let mut pricing_provider = InMemoryPricingProvider::new();
        let mut policy_provider = InMemoryPolicyProvider::new();

        pricing_provider.add_snapshot(&quote_id, 1, pricing);
        policy_provider.add_evaluation(&quote_id, 1, policy);

        let engine = ExplanationEngine::new(pricing_provider, policy_provider);
        let line_id = create_test_line_id("line-1");
        let explanation = engine.explain_line(&quote_id, &line_id, 1).expect("should succeed");

        assert!(explanation.amount_description.contains("Enterprise Plan"));
        assert!(!explanation.arithmetic_chain.is_empty());
    }

    #[test]
    fn explain_line_fails_for_invalid_line_id() {
        let quote_id = create_test_quote_id("Q-2026-003");
        let pricing = create_test_pricing_snapshot(&quote_id);
        let policy = create_test_policy_evaluation(&quote_id);

        let mut pricing_provider = InMemoryPricingProvider::new();
        let mut policy_provider = InMemoryPolicyProvider::new();

        pricing_provider.add_snapshot(&quote_id, 1, pricing);
        policy_provider.add_evaluation(&quote_id, 1, policy);

        let engine = ExplanationEngine::new(pricing_provider, policy_provider);
        let invalid_line_id = create_test_line_id("nonexistent");

        let result = engine.explain_line(&quote_id, &invalid_line_id, 1);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExplanationError::InvalidLineId { .. }));
    }

    #[test]
    fn explain_policy_returns_policy_focused_explanation() {
        let quote_id = create_test_quote_id("Q-2026-004");
        let pricing = create_test_pricing_snapshot(&quote_id);
        let policy = create_test_policy_evaluation(&quote_id);

        let mut pricing_provider = InMemoryPricingProvider::new();
        let mut policy_provider = InMemoryPolicyProvider::new();

        pricing_provider.add_snapshot(&quote_id, 1, pricing);
        policy_provider.add_evaluation(&quote_id, 1, policy);

        let engine = ExplanationEngine::new(pricing_provider, policy_provider);
        let explanation = engine.explain_policy(&quote_id, 1).expect("should succeed");

        assert!(explanation.user_summary.contains("APPROVED"));
        assert!(explanation.arithmetic_chain.is_empty()); // Policy explanations don't have arithmetic
    }

    #[test]
    fn missing_pricing_snapshot_returns_error() {
        let quote_id = create_test_quote_id("Q-2026-005");
        let policy = create_test_policy_evaluation(&quote_id);

        let pricing_provider = InMemoryPricingProvider::new(); // Empty
        let mut policy_provider = InMemoryPolicyProvider::new();
        policy_provider.add_evaluation(&quote_id, 1, policy);

        let engine = ExplanationEngine::new(pricing_provider, policy_provider);
        let result = engine.explain_total(&quote_id, 1);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ExplanationError::MissingPricingSnapshot { .. }));
    }

    #[test]
    fn arithmetic_chain_contains_all_steps() {
        let quote_id = create_test_quote_id("Q-2026-006");
        let pricing = create_test_pricing_snapshot(&quote_id);
        let policy = create_test_policy_evaluation(&quote_id);

        let mut pricing_provider = InMemoryPricingProvider::new();
        let mut policy_provider = InMemoryPolicyProvider::new();

        pricing_provider.add_snapshot(&quote_id, 1, pricing);
        policy_provider.add_evaluation(&quote_id, 1, policy);

        let engine = ExplanationEngine::new(pricing_provider, policy_provider);
        let explanation = engine.explain_total(&quote_id, 1).expect("should succeed");

        // Should have: sum lines, apply discount, final total
        assert!(explanation.arithmetic_chain.len() >= 2);

        // Check first step is sum
        assert_eq!(explanation.arithmetic_chain[0].operation, "sum");

        // Check last step is total
        let last = explanation.arithmetic_chain.last().unwrap();
        assert_eq!(last.operation, "total");
    }

    #[test]
    fn policy_violations_are_included_in_evidence() {
        let quote_id = create_test_quote_id("Q-2026-007");
        let pricing = create_test_pricing_snapshot(&quote_id);

        let mut policy = create_test_policy_evaluation(&quote_id);
        policy.violations.push(PolicyViolation {
            policy_id: "pol-1".to_string(),
            policy_name: "Max Discount".to_string(),
            severity: "warning".to_string(),
            threshold_value: Some(Decimal::new(2000, 2)),
            actual_value: Decimal::new(2000, 2),
            message: "Discount at maximum threshold".to_string(),
            suggested_resolution: None,
        });

        let mut pricing_provider = InMemoryPricingProvider::new();
        let mut policy_provider = InMemoryPolicyProvider::new();

        pricing_provider.add_snapshot(&quote_id, 1, pricing);
        policy_provider.add_evaluation(&quote_id, 1, policy);

        let engine = ExplanationEngine::new(pricing_provider, policy_provider);
        let explanation = engine.explain_total(&quote_id, 1).expect("should succeed");

        assert!(!explanation.policy_evidence.is_empty());
        assert!(explanation.user_summary.contains("⚡")); // Warning indicator
    }

    #[test]
    fn source_references_include_pricing_and_policy() {
        let quote_id = create_test_quote_id("Q-2026-008");
        let pricing = create_test_pricing_snapshot(&quote_id);
        let policy = create_test_policy_evaluation(&quote_id);

        let mut pricing_provider = InMemoryPricingProvider::new();
        let mut policy_provider = InMemoryPolicyProvider::new();

        pricing_provider.add_snapshot(&quote_id, 1, pricing);
        policy_provider.add_evaluation(&quote_id, 1, policy);

        let engine = ExplanationEngine::new(pricing_provider, policy_provider);
        let explanation = engine.explain_total(&quote_id, 1).expect("should succeed");

        let has_pricing_ref =
            explanation.source_references.iter().any(|r| r.source_type == "pricing_snapshot");
        let has_policy_ref =
            explanation.source_references.iter().any(|r| r.source_type == "policy_evaluation");

        assert!(has_pricing_ref, "should reference pricing snapshot");
        assert!(has_policy_ref, "should reference policy evaluation");
    }
}
