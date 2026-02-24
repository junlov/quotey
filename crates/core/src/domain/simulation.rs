use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::quote::QuoteId;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScenarioRunId(pub String);

impl fmt::Display for ScenarioRunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScenarioVariantId(pub String);

impl fmt::Display for ScenarioVariantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScenarioDeltaId(pub String);

impl fmt::Display for ScenarioDeltaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScenarioAuditEventId(pub String);

impl fmt::Display for ScenarioAuditEventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioRunStatus {
    Pending,
    Success,
    Failed,
    Promoted,
    Cancelled,
}

impl ScenarioRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Promoted => "promoted",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "success" => Some(Self::Success),
            "failed" => Some(Self::Failed),
            "promoted" => Some(Self::Promoted),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioDeltaType {
    Price,
    Policy,
    Approval,
    Configuration,
}

impl ScenarioDeltaType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Price => "price",
            Self::Policy => "policy",
            Self::Approval => "approval",
            Self::Configuration => "configuration",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "price" => Some(Self::Price),
            "policy" => Some(Self::Policy),
            "approval" => Some(Self::Approval),
            "configuration" => Some(Self::Configuration),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioAuditEventType {
    RequestReceived,
    VariantGenerated,
    ComparisonRendered,
    PromotionRequested,
    PromotionApplied,
    ErrorOccurred,
}

impl ScenarioAuditEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RequestReceived => "request_received",
            Self::VariantGenerated => "variant_generated",
            Self::ComparisonRendered => "comparison_rendered",
            Self::PromotionRequested => "promotion_requested",
            Self::PromotionApplied => "promotion_applied",
            Self::ErrorOccurred => "error_occurred",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "request_received" => Some(Self::RequestReceived),
            "variant_generated" => Some(Self::VariantGenerated),
            "comparison_rendered" => Some(Self::ComparisonRendered),
            "promotion_requested" => Some(Self::PromotionRequested),
            "promotion_applied" => Some(Self::PromotionApplied),
            "error_occurred" => Some(Self::ErrorOccurred),
            _ => None,
        }
    }
}

pub const COUNTER_SIM_REQUESTS_TOTAL: &str = "sim_requests_total";
pub const COUNTER_SIM_SUCCESS_TOTAL: &str = "sim_success_total";
pub const COUNTER_SIM_FAILURES_TOTAL: &str = "sim_failures_total";
pub const COUNTER_SIM_VARIANTS_GENERATED_TOTAL: &str = "sim_variants_generated_total";
pub const COUNTER_SIM_APPROVAL_REQUIRED_VARIANTS_TOTAL: &str =
    "sim_approval_required_variants_total";
pub const COUNTER_SIM_PROMOTIONS_REQUESTED_TOTAL: &str = "sim_promotions_requested_total";
pub const COUNTER_SIM_PROMOTIONS_APPLIED_TOTAL: &str = "sim_promotions_applied_total";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioTelemetryOutcome {
    Accepted,
    Success,
    GuardrailRejected,
    PromotionRequested,
    PromotionApplied,
}

impl ScenarioTelemetryOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Success => "success",
            Self::GuardrailRejected => "guardrail_rejected",
            Self::PromotionRequested => "promotion_requested",
            Self::PromotionApplied => "promotion_applied",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "accepted" => Some(Self::Accepted),
            "success" => Some(Self::Success),
            "guardrail_rejected" => Some(Self::GuardrailRejected),
            "promotion_requested" => Some(Self::PromotionRequested),
            "promotion_applied" => Some(Self::PromotionApplied),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioTelemetryEvent {
    pub event_type: ScenarioAuditEventType,
    pub quote_id: QuoteId,
    pub correlation_id: String,
    pub scenario_run_id: Option<ScenarioRunId>,
    pub variant_key: Option<String>,
    pub variant_count: i32,
    pub approval_required_variant_count: i32,
    pub latency_ms: i64,
    pub outcome: ScenarioTelemetryOutcome,
    pub error_code: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

impl ScenarioTelemetryEvent {
    pub fn counter_deltas(&self) -> Vec<(&'static str, u64)> {
        fn to_counter(value: i32) -> u64 {
            if value <= 0 {
                0
            } else {
                value as u64
            }
        }

        match self.event_type {
            ScenarioAuditEventType::RequestReceived => {
                vec![(COUNTER_SIM_REQUESTS_TOTAL, 1)]
            }
            ScenarioAuditEventType::ComparisonRendered => vec![
                (COUNTER_SIM_SUCCESS_TOTAL, 1),
                (COUNTER_SIM_VARIANTS_GENERATED_TOTAL, to_counter(self.variant_count)),
                (
                    COUNTER_SIM_APPROVAL_REQUIRED_VARIANTS_TOTAL,
                    to_counter(self.approval_required_variant_count),
                ),
            ],
            ScenarioAuditEventType::ErrorOccurred => {
                vec![(COUNTER_SIM_FAILURES_TOTAL, 1)]
            }
            ScenarioAuditEventType::PromotionRequested => {
                vec![(COUNTER_SIM_PROMOTIONS_REQUESTED_TOTAL, 1)]
            }
            ScenarioAuditEventType::PromotionApplied => {
                vec![(COUNTER_SIM_PROMOTIONS_APPLIED_TOTAL, 1)]
            }
            ScenarioAuditEventType::VariantGenerated => {
                vec![(COUNTER_SIM_VARIANTS_GENERATED_TOTAL, to_counter(self.variant_count))]
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateScenarioRunRequest {
    pub quote_id: QuoteId,
    pub thread_id: String,
    pub actor_id: String,
    pub correlation_id: String,
    pub base_quote_version: i32,
    pub request_params_json: String,
    pub variant_count: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioRun {
    pub id: ScenarioRunId,
    pub quote_id: QuoteId,
    pub thread_id: String,
    pub actor_id: String,
    pub correlation_id: String,
    pub base_quote_version: i32,
    pub request_params_json: String,
    pub variant_count: i32,
    pub status: ScenarioRunStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScenarioVariant {
    pub id: ScenarioVariantId,
    pub scenario_run_id: ScenarioRunId,
    pub variant_key: String,
    pub variant_order: i32,
    pub params_json: String,
    pub pricing_result_json: String,
    pub policy_result_json: String,
    pub approval_route_json: String,
    pub configuration_result_json: String,
    pub rank_score: f64,
    pub rank_order: i32,
    pub selected_for_promotion: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioDelta {
    pub id: ScenarioDeltaId,
    pub scenario_variant_id: ScenarioVariantId,
    pub delta_type: ScenarioDeltaType,
    pub delta_payload_json: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioAuditEvent {
    pub id: ScenarioAuditEventId,
    pub scenario_run_id: ScenarioRunId,
    pub scenario_variant_id: Option<ScenarioVariantId>,
    pub event_type: ScenarioAuditEventType,
    pub event_payload_json: String,
    pub actor_type: String,
    pub actor_id: String,
    pub correlation_id: String,
    pub occurred_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::domain::quote::QuoteId;

    use super::{
        ScenarioAuditEventType, ScenarioDeltaType, ScenarioRunId, ScenarioRunStatus,
        ScenarioTelemetryEvent, ScenarioTelemetryOutcome,
        COUNTER_SIM_APPROVAL_REQUIRED_VARIANTS_TOTAL, COUNTER_SIM_REQUESTS_TOTAL,
        COUNTER_SIM_SUCCESS_TOTAL,
    };

    #[test]
    fn scenario_run_status_round_trips() {
        let all = [
            ScenarioRunStatus::Pending,
            ScenarioRunStatus::Success,
            ScenarioRunStatus::Failed,
            ScenarioRunStatus::Promoted,
            ScenarioRunStatus::Cancelled,
        ];

        for status in all {
            assert_eq!(ScenarioRunStatus::parse(status.as_str()), Some(status));
        }
    }

    #[test]
    fn scenario_delta_type_round_trips() {
        let all = [
            ScenarioDeltaType::Price,
            ScenarioDeltaType::Policy,
            ScenarioDeltaType::Approval,
            ScenarioDeltaType::Configuration,
        ];

        for delta_type in all {
            assert_eq!(ScenarioDeltaType::parse(delta_type.as_str()), Some(delta_type));
        }
    }

    #[test]
    fn scenario_audit_event_type_round_trips() {
        let all = [
            ScenarioAuditEventType::RequestReceived,
            ScenarioAuditEventType::VariantGenerated,
            ScenarioAuditEventType::ComparisonRendered,
            ScenarioAuditEventType::PromotionRequested,
            ScenarioAuditEventType::PromotionApplied,
            ScenarioAuditEventType::ErrorOccurred,
        ];

        for event_type in all {
            assert_eq!(ScenarioAuditEventType::parse(event_type.as_str()), Some(event_type));
        }
    }

    #[test]
    fn scenario_telemetry_outcome_round_trips() {
        let all = [
            ScenarioTelemetryOutcome::Accepted,
            ScenarioTelemetryOutcome::Success,
            ScenarioTelemetryOutcome::GuardrailRejected,
            ScenarioTelemetryOutcome::PromotionRequested,
            ScenarioTelemetryOutcome::PromotionApplied,
        ];

        for outcome in all {
            assert_eq!(ScenarioTelemetryOutcome::parse(outcome.as_str()), Some(outcome));
        }
    }

    #[test]
    fn scenario_telemetry_event_counter_deltas_are_deterministic() {
        let request_event = ScenarioTelemetryEvent {
            event_type: ScenarioAuditEventType::RequestReceived,
            quote_id: QuoteId("Q-2026-7001".to_string()),
            correlation_id: "req-telemetry-1".to_string(),
            scenario_run_id: Some(ScenarioRunId("sim-run-1".to_string())),
            variant_key: None,
            variant_count: 2,
            approval_required_variant_count: 0,
            latency_ms: 0,
            outcome: ScenarioTelemetryOutcome::Accepted,
            error_code: None,
            occurred_at: Utc::now(),
        };

        let completion_event = ScenarioTelemetryEvent {
            event_type: ScenarioAuditEventType::ComparisonRendered,
            quote_id: QuoteId("Q-2026-7001".to_string()),
            correlation_id: "req-telemetry-1".to_string(),
            scenario_run_id: Some(ScenarioRunId("sim-run-1".to_string())),
            variant_key: None,
            variant_count: 2,
            approval_required_variant_count: 1,
            latency_ms: 125,
            outcome: ScenarioTelemetryOutcome::Success,
            error_code: None,
            occurred_at: Utc::now(),
        };

        assert_eq!(request_event.counter_deltas(), vec![(COUNTER_SIM_REQUESTS_TOTAL, 1)]);
        assert_eq!(
            completion_event.counter_deltas(),
            vec![
                (COUNTER_SIM_SUCCESS_TOTAL, 1),
                ("sim_variants_generated_total", 2),
                (COUNTER_SIM_APPROVAL_REQUIRED_VARIANTS_TOTAL, 1),
            ]
        );
    }
}
