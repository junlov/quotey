//! Domain types for the "Explain Any Number" feature
//!
//! Provides deterministic explanation assembly for quote totals and line items,
//! sourced only from persisted pricing trace and policy artifacts.

use std::fmt;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::domain::quote::{QuoteId, QuoteLineId};

/// Unique identifier for an explanation request
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExplanationRequestId(pub String);

impl fmt::Display for ExplanationRequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for explanation evidence
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExplanationEvidenceId(pub String);

impl fmt::Display for ExplanationEvidenceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Types of explanation requests
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExplanationRequestType {
    /// Explain a quote total
    Total,
    /// Explain a specific line item
    Line,
    /// Explain a policy decision
    Policy,
}

impl ExplanationRequestType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Total => "total",
            Self::Line => "line",
            Self::Policy => "policy",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "total" => Some(Self::Total),
            "line" => Some(Self::Line),
            "policy" => Some(Self::Policy),
            _ => None,
        }
    }
}

/// Status of an explanation request
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExplanationStatus {
    /// Request received, processing pending
    Pending,
    /// Explanation successfully generated
    Success,
    /// Error occurred during explanation
    Error,
    /// Missing evidence to generate explanation
    MissingEvidence,
}

impl ExplanationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Success => "success",
            Self::Error => "error",
            Self::MissingEvidence => "missing_evidence",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "success" => Some(Self::Success),
            "error" => Some(Self::Error),
            "missing_evidence" => Some(Self::MissingEvidence),
            _ => None,
        }
    }
}

/// Types of evidence that can support an explanation
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceType {
    /// Pricing trace step
    PricingTrace,
    /// Policy evaluation result
    PolicyEvaluation,
    /// Specific rule citation
    RuleCitation,
    /// Line item detail
    LineItem,
}

impl EvidenceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PricingTrace => "pricing_trace",
            Self::PolicyEvaluation => "policy_evaluation",
            Self::RuleCitation => "rule_citation",
            Self::LineItem => "line_item",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pricing_trace" => Some(Self::PricingTrace),
            "policy_evaluation" => Some(Self::PolicyEvaluation),
            "rule_citation" => Some(Self::RuleCitation),
            "line_item" => Some(Self::LineItem),
            _ => None,
        }
    }
}

/// Explanation request from user
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplanationRequest {
    pub id: ExplanationRequestId,
    pub quote_id: QuoteId,
    pub line_id: Option<QuoteLineId>,
    pub request_type: ExplanationRequestType,
    pub thread_id: String,
    pub actor_id: String,
    pub correlation_id: String,
    pub quote_version: i32,
    pub pricing_snapshot_id: Option<String>,
    pub status: ExplanationStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub latency_ms: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Individual piece of evidence for an explanation
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplanationEvidence {
    pub id: ExplanationEvidenceId,
    pub explanation_request_id: ExplanationRequestId,
    pub evidence_type: EvidenceType,
    pub evidence_key: String,
    pub evidence_payload_json: String,
    pub source_reference: String,
    pub display_order: i32,
    pub created_at: DateTime<Utc>,
}

/// Pricing trace evidence payload
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PricingTraceEvidence {
    pub line_id: Option<String>,
    pub step_name: String,
    pub step_order: i32,
    pub input_values: serde_json::Value,
    pub output_value: Decimal,
    pub calculation_formula: Option<String>,
}

/// Policy evaluation evidence payload
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyEvaluationEvidence {
    pub policy_id: String,
    pub policy_name: String,
    pub decision: String, // "passed", "violated", "waived"
    pub threshold_value: Option<String>,
    pub actual_value: String,
    pub violation_message: Option<String>,
}

/// Rule citation evidence payload
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleCitationEvidence {
    pub rule_id: String,
    pub rule_name: String,
    pub rule_section: String,
    pub rule_text: String,
    pub documentation_url: Option<String>,
}

/// Line item evidence payload
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineItemEvidence {
    pub line_id: String,
    pub product_id: String,
    pub product_name: String,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub discount_percent: Decimal,
    pub subtotal: Decimal,
}

/// Audit event for explanation lifecycle
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplanationAuditEvent {
    pub id: String,
    pub explanation_request_id: ExplanationRequestId,
    pub event_type: ExplanationEventType,
    pub event_payload_json: String,
    pub actor_type: String,
    pub actor_id: String,
    pub correlation_id: String,
    pub occurred_at: DateTime<Utc>,
}

/// Types of explanation audit events
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExplanationEventType {
    RequestReceived,
    EvidenceGathered,
    ExplanationGenerated,
    ExplanationDelivered,
    ErrorOccurred,
    EvidenceMissing,
}

impl ExplanationEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RequestReceived => "request_received",
            Self::EvidenceGathered => "evidence_gathered",
            Self::ExplanationGenerated => "explanation_generated",
            Self::ExplanationDelivered => "explanation_delivered",
            Self::ErrorOccurred => "error_occurred",
            Self::EvidenceMissing => "evidence_missing",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "request_received" => Some(Self::RequestReceived),
            "evidence_gathered" => Some(Self::EvidenceGathered),
            "explanation_generated" => Some(Self::ExplanationGenerated),
            "explanation_delivered" => Some(Self::ExplanationDelivered),
            "error_occurred" => Some(Self::ErrorOccurred),
            "evidence_missing" => Some(Self::EvidenceMissing),
            _ => None,
        }
    }
}

/// Explanation response payload (deterministic)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExplanationResponse {
    pub request_id: ExplanationRequestId,
    pub quote_id: QuoteId,
    pub amount: Decimal,
    pub amount_description: String,
    pub arithmetic_chain: Vec<ArithmeticStep>,
    pub policy_evidence: Vec<PolicyEvaluationEvidence>,
    pub source_references: Vec<SourceReference>,
    pub user_summary: String,
}

/// Single step in arithmetic explanation
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArithmeticStep {
    pub step_order: i32,
    pub operation: String,
    pub input_values: Vec<(String, Decimal)>,
    pub result: Decimal,
    pub description: String,
}

/// Reference to source data for auditability
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceReference {
    pub source_type: String,
    pub source_id: String,
    pub source_version: String,
    pub field_path: String,
}

/// Explanation cache entry
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplanationCache {
    pub id: String,
    pub cache_key: String,
    pub quote_id: QuoteId,
    pub line_id: Option<QuoteLineId>,
    pub quote_version: i32,
    pub pricing_snapshot_id: String,
    pub explanation_summary: String,
    pub evidence_refs_json: String,
    pub hit_count: i32,
    pub last_hit_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Explanation statistics (from materialized view)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplanationStats {
    pub total_requests: i32,
    pub success_count: i32,
    pub error_count: i32,
    pub missing_evidence_count: i32,
    pub avg_latency_ms: Option<i32>,
    pub p95_latency_ms: Option<i32>,
    pub last_updated_at: DateTime<Utc>,
}

/// Input for creating an explanation request
#[derive(Clone, Debug)]
pub struct CreateExplanationRequest {
    pub quote_id: QuoteId,
    pub line_id: Option<QuoteLineId>,
    pub request_type: ExplanationRequestType,
    pub thread_id: String,
    pub actor_id: String,
    pub correlation_id: String,
    pub quote_version: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explanation_request_type_round_trips() {
        let cases = [
            ExplanationRequestType::Total,
            ExplanationRequestType::Line,
            ExplanationRequestType::Policy,
        ];

        for case in cases {
            let decoded = ExplanationRequestType::parse(case.as_str());
            assert_eq!(decoded, Some(case));
        }
    }

    #[test]
    fn explanation_status_round_trips() {
        let cases = [
            ExplanationStatus::Pending,
            ExplanationStatus::Success,
            ExplanationStatus::Error,
            ExplanationStatus::MissingEvidence,
        ];

        for case in cases {
            let decoded = ExplanationStatus::parse(case.as_str());
            assert_eq!(decoded, Some(case));
        }
    }

    #[test]
    fn evidence_type_round_trips() {
        let cases = [
            EvidenceType::PricingTrace,
            EvidenceType::PolicyEvaluation,
            EvidenceType::RuleCitation,
            EvidenceType::LineItem,
        ];

        for case in cases {
            let decoded = EvidenceType::parse(case.as_str());
            assert_eq!(decoded, Some(case));
        }
    }

    #[test]
    fn explanation_event_type_round_trips() {
        let cases = [
            ExplanationEventType::RequestReceived,
            ExplanationEventType::EvidenceGathered,
            ExplanationEventType::ExplanationGenerated,
            ExplanationEventType::ExplanationDelivered,
            ExplanationEventType::ErrorOccurred,
            ExplanationEventType::EvidenceMissing,
        ];

        for case in cases {
            let decoded = ExplanationEventType::parse(case.as_str());
            assert_eq!(decoded, Some(case));
        }
    }

    #[test]
    fn display_traits_format_ids() {
        let req_id = ExplanationRequestId("exp-req-001".to_string());
        let ev_id = ExplanationEvidenceId("exp-ev-001".to_string());

        assert_eq!(format!("{}", req_id), "exp-req-001");
        assert_eq!(format!("{}", ev_id), "exp-ev-001");
    }
}
