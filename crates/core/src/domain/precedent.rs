use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::quote::QuoteId;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrecedentApprovalPathId(pub String);

impl fmt::Display for PrecedentApprovalPathId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrecedentSimilarityEvidenceId(pub String);

impl fmt::Display for PrecedentSimilarityEvidenceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrecedentOutcomeStatus {
    Won,
    Lost,
    Pending,
}

impl PrecedentOutcomeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Won => "won",
            Self::Lost => "lost",
            Self::Pending => "pending",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "won" => Some(Self::Won),
            "lost" => Some(Self::Lost),
            "pending" => Some(Self::Pending),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrecedentDecisionStatus {
    Pending,
    Approved,
    Rejected,
    Escalated,
}

impl PrecedentDecisionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Escalated => "escalated",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            "escalated" => Some(Self::Escalated),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrecedentQuery {
    pub quote_id: QuoteId,
    pub customer_segment: Option<String>,
    pub region: Option<String>,
    pub product_family: Option<String>,
    pub limit: i32,
}

impl PrecedentQuery {
    pub fn normalized_limit(&self) -> i32 {
        self.limit.clamp(1, 20)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PrecedentResult {
    pub candidate_quote_id: QuoteId,
    pub similarity_score: f64,
    pub outcome_status: PrecedentOutcomeStatus,
    pub outcome_final_price: f64,
    pub approval_decision_status: Option<PrecedentDecisionStatus>,
    pub approval_route_version: Option<i32>,
    pub evidence: PrecedentEvidence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrecedentEvidence {
    pub source_fingerprint_id: String,
    pub candidate_fingerprint_id: String,
    pub strategy_version: String,
    pub score_components_json: String,
    pub evidence_payload_json: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PrecedentFingerprint {
    pub id: String,
    pub quote_id: QuoteId,
    pub fingerprint_hash: String,
    pub configuration_vector: Vec<u8>,
    pub outcome_status: PrecedentOutcomeStatus,
    pub final_price: f64,
    pub close_date: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PrecedentDealOutcome {
    pub id: String,
    pub quote_id: QuoteId,
    pub outcome_status: PrecedentOutcomeStatus,
    pub final_price: f64,
    pub close_date: String,
    pub customer_segment: Option<String>,
    pub product_mix_json: String,
    pub sales_cycle_days: Option<i32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrecedentApprovalPathEvidence {
    pub id: PrecedentApprovalPathId,
    pub quote_id: QuoteId,
    pub route_version: i32,
    pub route_payload_json: String,
    pub decision_status: PrecedentDecisionStatus,
    pub decision_actor_id: Option<String>,
    pub decision_reason: Option<String>,
    pub routed_by_actor_id: String,
    pub idempotency_key: String,
    pub correlation_id: String,
    pub routed_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PrecedentSimilarityEvidence {
    pub id: PrecedentSimilarityEvidenceId,
    pub source_quote_id: QuoteId,
    pub source_fingerprint_id: String,
    pub candidate_quote_id: QuoteId,
    pub candidate_fingerprint_id: String,
    pub similarity_score: f64,
    pub strategy_version: String,
    pub score_components_json: String,
    pub evidence_payload_json: String,
    pub idempotency_key: String,
    pub correlation_id: String,
    pub computed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::{
        PrecedentDecisionStatus, PrecedentOutcomeStatus, PrecedentQuery,
        PrecedentSimilarityEvidenceId,
    };
    use crate::domain::quote::QuoteId;

    #[test]
    fn precedent_outcome_status_round_trips() {
        let all = [
            PrecedentOutcomeStatus::Won,
            PrecedentOutcomeStatus::Lost,
            PrecedentOutcomeStatus::Pending,
        ];

        for status in all {
            assert_eq!(PrecedentOutcomeStatus::parse(status.as_str()), Some(status));
        }
    }

    #[test]
    fn precedent_decision_status_round_trips() {
        let all = [
            PrecedentDecisionStatus::Pending,
            PrecedentDecisionStatus::Approved,
            PrecedentDecisionStatus::Rejected,
            PrecedentDecisionStatus::Escalated,
        ];

        for status in all {
            assert_eq!(PrecedentDecisionStatus::parse(status.as_str()), Some(status));
        }
    }

    #[test]
    fn precedent_query_limit_is_clamped() {
        let query = PrecedentQuery {
            quote_id: QuoteId("Q-900".to_string()),
            customer_segment: None,
            region: None,
            product_family: None,
            limit: 500,
        };

        assert_eq!(query.normalized_limit(), 20);
    }

    #[test]
    fn precedent_similarity_id_display_round_trip() {
        let id = PrecedentSimilarityEvidenceId("pre-sim-1".to_string());
        assert_eq!(id.to_string(), "pre-sim-1");
    }
}
