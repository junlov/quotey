use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::quote::QuoteId;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DealAutopsyId(pub String);

impl fmt::Display for DealAutopsyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DecisionForkId(pub String);

impl fmt::Display for DecisionForkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttributionScoreId(pub String);

impl fmt::Display for AttributionScoreId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttributionNodeId(pub String);

impl fmt::Display for AttributionNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttributionEdgeId(pub String);

impl fmt::Display for AttributionEdgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GenomeQueryId(pub String);

impl fmt::Display for GenomeQueryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CounterfactualSimulationId(pub String);

impl fmt::Display for CounterfactualSimulationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DealOutcomeType {
    Won,
    Lost,
    Expired,
    Cancelled,
}

impl DealOutcomeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Won => "won",
            Self::Lost => "lost",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "won" => Some(Self::Won),
            "lost" => Some(Self::Lost),
            "expired" => Some(Self::Expired),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionForkType {
    PricingPath,
    DiscountLevel,
    ConstraintResolution,
    ApprovalException,
    NegotiationConcession,
    ProductSelection,
    TermSelection,
    BundleChoice,
}

impl DecisionForkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PricingPath => "pricing_path",
            Self::DiscountLevel => "discount_level",
            Self::ConstraintResolution => "constraint_resolution",
            Self::ApprovalException => "approval_exception",
            Self::NegotiationConcession => "negotiation_concession",
            Self::ProductSelection => "product_selection",
            Self::TermSelection => "term_selection",
            Self::BundleChoice => "bundle_choice",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pricing_path" => Some(Self::PricingPath),
            "discount_level" => Some(Self::DiscountLevel),
            "constraint_resolution" => Some(Self::ConstraintResolution),
            "approval_exception" => Some(Self::ApprovalException),
            "negotiation_concession" => Some(Self::NegotiationConcession),
            "product_selection" => Some(Self::ProductSelection),
            "term_selection" => Some(Self::TermSelection),
            "bundle_choice" => Some(Self::BundleChoice),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStage {
    Configuration,
    Pricing,
    Policy,
    Approval,
    Negotiation,
    Finalization,
}

impl DecisionStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Configuration => "configuration",
            Self::Pricing => "pricing",
            Self::Policy => "policy",
            Self::Approval => "approval",
            Self::Negotiation => "negotiation",
            Self::Finalization => "finalization",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "configuration" => Some(Self::Configuration),
            "pricing" => Some(Self::Pricing),
            "policy" => Some(Self::Policy),
            "approval" => Some(Self::Approval),
            "negotiation" => Some(Self::Negotiation),
            "finalization" => Some(Self::Finalization),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditRefType {
    LedgerEntry,
    AuditEvent,
    PricingTrace,
    NegotiationTurn,
    ApprovalDecision,
}

impl AuditRefType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LedgerEntry => "ledger_entry",
            Self::AuditEvent => "audit_event",
            Self::PricingTrace => "pricing_trace",
            Self::NegotiationTurn => "negotiation_turn",
            Self::ApprovalDecision => "approval_decision",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ledger_entry" => Some(Self::LedgerEntry),
            "audit_event" => Some(Self::AuditEvent),
            "pricing_trace" => Some(Self::PricingTrace),
            "negotiation_turn" => Some(Self::NegotiationTurn),
            "approval_decision" => Some(Self::ApprovalDecision),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenomeQueryType {
    StrategyRecommendation,
    Counterfactual,
    PatternDetection,
    SegmentAnalysis,
    PolicyImpact,
    DecisionComparison,
}

impl GenomeQueryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::StrategyRecommendation => "strategy_recommendation",
            Self::Counterfactual => "counterfactual",
            Self::PatternDetection => "pattern_detection",
            Self::SegmentAnalysis => "segment_analysis",
            Self::PolicyImpact => "policy_impact",
            Self::DecisionComparison => "decision_comparison",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "strategy_recommendation" => Some(Self::StrategyRecommendation),
            "counterfactual" => Some(Self::Counterfactual),
            "pattern_detection" => Some(Self::PatternDetection),
            "segment_analysis" => Some(Self::SegmentAnalysis),
            "policy_impact" => Some(Self::PolicyImpact),
            "decision_comparison" => Some(Self::DecisionComparison),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectedOutcomeStatus {
    Won,
    Lost,
    Expired,
    Cancelled,
    Unknown,
}

impl ProjectedOutcomeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Won => "won",
            Self::Lost => "lost",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "won" => Some(Self::Won),
            "lost" => Some(Self::Lost),
            "expired" => Some(Self::Expired),
            "cancelled" => Some(Self::Cancelled),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DealAutopsy {
    pub id: DealAutopsyId,
    pub quote_id: QuoteId,
    pub outcome_status: DealOutcomeType,
    pub outcome_value_bps: i32,
    pub outcome_revenue_cents: i64,
    pub decision_fork_count: i32,
    pub attribution_checksum: String,
    pub audit_trail_refs_json: String,
    pub autopsy_version: String,
    pub idempotency_key: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionFork {
    pub id: DecisionForkId,
    pub autopsy_id: DealAutopsyId,
    pub fork_type: DecisionForkType,
    pub stage: DecisionStage,
    pub option_chosen_json: String,
    pub options_considered_json: String,
    pub audit_ref: String,
    pub audit_ref_type: AuditRefType,
    pub sequence_order: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributionScore {
    pub id: AttributionScoreId,
    pub autopsy_id: DealAutopsyId,
    pub fork_id: DecisionForkId,
    pub outcome_contribution_bps: i32,
    pub confidence_bps: i32,
    pub evidence_count: i32,
    pub evidence_refs_json: String,
    pub attribution_method: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributionNode {
    pub id: AttributionNodeId,
    pub fork_type: DecisionForkType,
    pub stage: DecisionStage,
    pub segment_key: String,
    pub option_value_hash: String,
    pub option_value_summary: String,
    pub sample_count: i32,
    pub first_seen_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributionEdge {
    pub id: AttributionEdgeId,
    pub source_node_id: AttributionNodeId,
    pub target_node_id: AttributionNodeId,
    pub outcome_weight_bps: i32,
    pub sample_count: i32,
    pub win_rate_bps: i32,
    pub avg_margin_delta_bps: i32,
    pub avg_revenue_cents: i64,
    pub first_seen_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenomeQueryAudit {
    pub id: GenomeQueryId,
    pub query_type: GenomeQueryType,
    pub query_params_json: String,
    pub result_checksum: String,
    pub result_summary_json: String,
    pub segments_analyzed: i32,
    pub evidence_count: i32,
    pub query_duration_ms: i64,
    pub queried_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterfactualSimulation {
    pub id: CounterfactualSimulationId,
    pub original_quote_id: QuoteId,
    pub original_autopsy_id: DealAutopsyId,
    pub alternative_decisions_json: String,
    pub replay_checksum: String,
    pub projected_outcome_status: ProjectedOutcomeStatus,
    pub projected_margin_delta_bps: i32,
    pub projected_revenue_delta_cents: i64,
    pub delta_vs_actual_json: String,
    pub evidence_chain_json: String,
    pub confidence_bps: i32,
    pub simulated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::{
        AuditRefType, DealOutcomeType, DecisionForkType, DecisionStage, GenomeQueryType,
        ProjectedOutcomeStatus,
    };

    #[test]
    fn deal_outcome_type_round_trips() {
        let all = [
            DealOutcomeType::Won,
            DealOutcomeType::Lost,
            DealOutcomeType::Expired,
            DealOutcomeType::Cancelled,
        ];

        for outcome in all {
            assert_eq!(DealOutcomeType::parse(outcome.as_str()), Some(outcome));
        }
    }

    #[test]
    fn decision_fork_type_round_trips() {
        let all = [
            DecisionForkType::PricingPath,
            DecisionForkType::DiscountLevel,
            DecisionForkType::ConstraintResolution,
            DecisionForkType::ApprovalException,
            DecisionForkType::NegotiationConcession,
            DecisionForkType::ProductSelection,
            DecisionForkType::TermSelection,
            DecisionForkType::BundleChoice,
        ];

        for fork_type in all {
            assert_eq!(DecisionForkType::parse(fork_type.as_str()), Some(fork_type));
        }
    }

    #[test]
    fn decision_stage_round_trips() {
        let all = [
            DecisionStage::Configuration,
            DecisionStage::Pricing,
            DecisionStage::Policy,
            DecisionStage::Approval,
            DecisionStage::Negotiation,
            DecisionStage::Finalization,
        ];

        for stage in all {
            assert_eq!(DecisionStage::parse(stage.as_str()), Some(stage));
        }
    }

    #[test]
    fn audit_ref_type_round_trips() {
        let all = [
            AuditRefType::LedgerEntry,
            AuditRefType::AuditEvent,
            AuditRefType::PricingTrace,
            AuditRefType::NegotiationTurn,
            AuditRefType::ApprovalDecision,
        ];

        for ref_type in all {
            assert_eq!(AuditRefType::parse(ref_type.as_str()), Some(ref_type));
        }
    }

    #[test]
    fn genome_query_type_round_trips() {
        let all = [
            GenomeQueryType::StrategyRecommendation,
            GenomeQueryType::Counterfactual,
            GenomeQueryType::PatternDetection,
            GenomeQueryType::SegmentAnalysis,
            GenomeQueryType::PolicyImpact,
            GenomeQueryType::DecisionComparison,
        ];

        for query_type in all {
            assert_eq!(GenomeQueryType::parse(query_type.as_str()), Some(query_type));
        }
    }

    #[test]
    fn projected_outcome_status_round_trips() {
        let all = [
            ProjectedOutcomeStatus::Won,
            ProjectedOutcomeStatus::Lost,
            ProjectedOutcomeStatus::Expired,
            ProjectedOutcomeStatus::Cancelled,
            ProjectedOutcomeStatus::Unknown,
        ];

        for status in all {
            assert_eq!(ProjectedOutcomeStatus::parse(status.as_str()), Some(status));
        }
    }
}
