use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PolicyCandidateId(pub String);

impl fmt::Display for PolicyCandidateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReplayEvaluationId(pub String);

impl fmt::Display for ReplayEvaluationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PolicyApprovalDecisionId(pub String);

impl fmt::Display for PolicyApprovalDecisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PolicyApplyRecordId(pub String);

impl fmt::Display for PolicyApplyRecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PolicyRollbackRecordId(pub String);

impl fmt::Display for PolicyRollbackRecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PolicyLifecycleAuditId(pub String);

impl fmt::Display for PolicyLifecycleAuditId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyCandidateStatus {
    Draft,
    Replayed,
    ReviewReady,
    Approved,
    Rejected,
    ChangesRequested,
    Applied,
    Monitoring,
    RolledBack,
}

impl PolicyCandidateStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Replayed => "replayed",
            Self::ReviewReady => "review_ready",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::ChangesRequested => "changes_requested",
            Self::Applied => "applied",
            Self::Monitoring => "monitoring",
            Self::RolledBack => "rolled_back",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "draft" => Some(Self::Draft),
            "replayed" => Some(Self::Replayed),
            "review_ready" => Some(Self::ReviewReady),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            "changes_requested" => Some(Self::ChangesRequested),
            "applied" => Some(Self::Applied),
            "monitoring" => Some(Self::Monitoring),
            "rolled_back" => Some(Self::RolledBack),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecisionKind {
    Approved,
    Rejected,
    ChangesRequested,
}

impl ApprovalDecisionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::ChangesRequested => "changes_requested",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            "changes_requested" => Some(Self::ChangesRequested),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyLifecycleAuditEventType {
    CandidateCreated,
    ReplayCompleted,
    ReviewPacketBuilt,
    Approved,
    Rejected,
    ChangesRequested,
    Applied,
    MonitoringStarted,
    RolledBack,
    StaleApprovalDetected,
}

impl PolicyLifecycleAuditEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CandidateCreated => "candidate_created",
            Self::ReplayCompleted => "replay_completed",
            Self::ReviewPacketBuilt => "review_packet_built",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::ChangesRequested => "changes_requested",
            Self::Applied => "applied",
            Self::MonitoringStarted => "monitoring_started",
            Self::RolledBack => "rolled_back",
            Self::StaleApprovalDetected => "stale_approval_detected",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "candidate_created" => Some(Self::CandidateCreated),
            "replay_completed" => Some(Self::ReplayCompleted),
            "review_packet_built" => Some(Self::ReviewPacketBuilt),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            "changes_requested" => Some(Self::ChangesRequested),
            "applied" => Some(Self::Applied),
            "monitoring_started" => Some(Self::MonitoringStarted),
            "rolled_back" => Some(Self::RolledBack),
            "stale_approval_detected" => Some(Self::StaleApprovalDetected),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolicyCandidate {
    pub id: PolicyCandidateId,
    pub base_policy_version: i32,
    pub proposed_policy_version: i32,
    pub status: PolicyCandidateStatus,
    pub policy_diff_json: String,
    pub provenance_json: String,
    pub confidence_score: f64,
    pub cohort_scope_json: String,
    pub latest_replay_checksum: Option<String>,
    pub idempotency_key: String,
    pub created_by_actor_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub review_ready_at: Option<DateTime<Utc>>,
    pub approved_at: Option<DateTime<Utc>>,
    pub applied_at: Option<DateTime<Utc>>,
    pub monitoring_started_at: Option<DateTime<Utc>>,
    pub rolled_back_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReplayEvaluation {
    pub id: ReplayEvaluationId,
    pub candidate_id: PolicyCandidateId,
    pub replay_checksum: String,
    pub engine_version: String,
    pub cohort_scope_json: String,
    pub cohort_size: i32,
    pub projected_margin_delta_bps: i32,
    pub projected_win_rate_delta_bps: i32,
    pub projected_approval_latency_delta_seconds: i32,
    pub blast_radius_score: f64,
    pub hard_violation_count: i32,
    pub risk_flags_json: String,
    pub deterministic_pass: bool,
    pub idempotency_key: String,
    pub replayed_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PolicyApprovalDecision {
    pub id: PolicyApprovalDecisionId,
    pub candidate_id: PolicyCandidateId,
    pub replay_evaluation_id: Option<ReplayEvaluationId>,
    pub decision: ApprovalDecisionKind,
    pub reason: Option<String>,
    pub decision_payload_json: String,
    pub actor_id: String,
    pub actor_role: String,
    pub channel_ref: Option<String>,
    pub signature: Option<String>,
    pub signature_key_id: Option<String>,
    pub idempotency_key: String,
    pub decided_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub is_stale: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyApplyRecord {
    pub id: PolicyApplyRecordId,
    pub candidate_id: PolicyCandidateId,
    pub approval_decision_id: PolicyApprovalDecisionId,
    pub prior_policy_version: i32,
    pub applied_policy_version: i32,
    pub replay_checksum: String,
    pub apply_signature: String,
    pub signature_key_id: String,
    pub actor_id: String,
    pub idempotency_key: String,
    pub verification_checksum: String,
    pub apply_audit_json: String,
    pub applied_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRollbackRecord {
    pub id: PolicyRollbackRecordId,
    pub candidate_id: PolicyCandidateId,
    pub apply_record_id: PolicyApplyRecordId,
    pub rollback_target_version: i32,
    pub rollback_reason: String,
    pub verification_checksum: String,
    pub rollback_signature: String,
    pub signature_key_id: String,
    pub actor_id: String,
    pub idempotency_key: String,
    pub parent_rollback_id: Option<PolicyRollbackRecordId>,
    pub rollback_depth: i32,
    pub rollback_metadata_json: String,
    pub rolled_back_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyLifecycleAuditEvent {
    pub id: PolicyLifecycleAuditId,
    pub candidate_id: PolicyCandidateId,
    pub replay_evaluation_id: Option<ReplayEvaluationId>,
    pub approval_decision_id: Option<PolicyApprovalDecisionId>,
    pub apply_record_id: Option<PolicyApplyRecordId>,
    pub rollback_record_id: Option<PolicyRollbackRecordId>,
    pub event_type: PolicyLifecycleAuditEventType,
    pub event_payload_json: String,
    pub actor_type: String,
    pub actor_id: String,
    pub correlation_id: String,
    pub idempotency_key: Option<String>,
    pub occurred_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::{ApprovalDecisionKind, PolicyCandidateStatus, PolicyLifecycleAuditEventType};

    #[test]
    fn policy_candidate_status_round_trips() {
        let all = [
            PolicyCandidateStatus::Draft,
            PolicyCandidateStatus::Replayed,
            PolicyCandidateStatus::ReviewReady,
            PolicyCandidateStatus::Approved,
            PolicyCandidateStatus::Rejected,
            PolicyCandidateStatus::ChangesRequested,
            PolicyCandidateStatus::Applied,
            PolicyCandidateStatus::Monitoring,
            PolicyCandidateStatus::RolledBack,
        ];

        for status in all {
            assert_eq!(PolicyCandidateStatus::parse(status.as_str()), Some(status));
        }
    }

    #[test]
    fn approval_decision_kind_round_trips() {
        let all = [
            ApprovalDecisionKind::Approved,
            ApprovalDecisionKind::Rejected,
            ApprovalDecisionKind::ChangesRequested,
        ];

        for decision in all {
            assert_eq!(ApprovalDecisionKind::parse(decision.as_str()), Some(decision));
        }
    }

    #[test]
    fn policy_lifecycle_event_round_trips() {
        let all = [
            PolicyLifecycleAuditEventType::CandidateCreated,
            PolicyLifecycleAuditEventType::ReplayCompleted,
            PolicyLifecycleAuditEventType::ReviewPacketBuilt,
            PolicyLifecycleAuditEventType::Approved,
            PolicyLifecycleAuditEventType::Rejected,
            PolicyLifecycleAuditEventType::ChangesRequested,
            PolicyLifecycleAuditEventType::Applied,
            PolicyLifecycleAuditEventType::MonitoringStarted,
            PolicyLifecycleAuditEventType::RolledBack,
            PolicyLifecycleAuditEventType::StaleApprovalDetected,
        ];

        for event in all {
            assert_eq!(PolicyLifecycleAuditEventType::parse(event.as_str()), Some(event));
        }
    }
}
