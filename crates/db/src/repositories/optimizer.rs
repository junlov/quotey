//! Repository layer for closed-loop policy optimizer persistence.
//!
//! Covers candidate lifecycle, replay evidence, approval decisions,
//! signed apply records, rollback chains, and immutable lifecycle audit events.

use async_trait::async_trait;
use quotey_core::chrono::{DateTime, Utc};
use quotey_core::domain::optimizer::{
    ApprovalDecisionKind, PolicyApplyRecord, PolicyApplyRecordId, PolicyApprovalDecision,
    PolicyApprovalDecisionId, PolicyCandidate, PolicyCandidateId, PolicyCandidateStatus,
    PolicyLifecycleAuditEvent, PolicyLifecycleAuditEventType, PolicyLifecycleAuditId,
    PolicyRollbackRecord, PolicyRollbackRecordId, ReplayEvaluation, ReplayEvaluationId,
};
use sqlx::{sqlite::SqliteRow, Row};

use super::RepositoryError;
use crate::DbPool;

#[async_trait]
pub trait PolicyOptimizerRepository: Send + Sync {
    async fn save_candidate(&self, candidate: PolicyCandidate) -> Result<(), RepositoryError>;

    async fn get_candidate(
        &self,
        id: &PolicyCandidateId,
    ) -> Result<Option<PolicyCandidate>, RepositoryError>;

    async fn list_candidates_by_status(
        &self,
        status: Option<PolicyCandidateStatus>,
        limit: i32,
    ) -> Result<Vec<PolicyCandidate>, RepositoryError>;

    async fn save_replay_evaluation(&self, replay: ReplayEvaluation)
        -> Result<(), RepositoryError>;

    async fn list_replay_evaluations_for_candidate(
        &self,
        candidate_id: &PolicyCandidateId,
    ) -> Result<Vec<ReplayEvaluation>, RepositoryError>;

    async fn find_replay_evaluation_by_checksum(
        &self,
        candidate_id: &PolicyCandidateId,
        replay_checksum: &str,
    ) -> Result<Option<ReplayEvaluation>, RepositoryError>;

    async fn save_approval_decision(
        &self,
        decision: PolicyApprovalDecision,
    ) -> Result<(), RepositoryError>;

    async fn list_approval_decisions_for_candidate(
        &self,
        candidate_id: &PolicyCandidateId,
    ) -> Result<Vec<PolicyApprovalDecision>, RepositoryError>;

    async fn list_stale_approval_decisions(
        &self,
        before: DateTime<Utc>,
    ) -> Result<Vec<PolicyApprovalDecision>, RepositoryError>;

    async fn save_apply_record(&self, apply: PolicyApplyRecord) -> Result<(), RepositoryError>;

    async fn get_apply_record(
        &self,
        id: &PolicyApplyRecordId,
    ) -> Result<Option<PolicyApplyRecord>, RepositoryError>;

    async fn save_rollback_record(
        &self,
        rollback: PolicyRollbackRecord,
    ) -> Result<(), RepositoryError>;

    async fn list_rollback_chain_for_apply(
        &self,
        apply_record_id: &PolicyApplyRecordId,
    ) -> Result<Vec<PolicyRollbackRecord>, RepositoryError>;

    async fn append_lifecycle_audit_event(
        &self,
        event: PolicyLifecycleAuditEvent,
    ) -> Result<(), RepositoryError>;

    async fn list_lifecycle_audit_for_candidate(
        &self,
        candidate_id: &PolicyCandidateId,
    ) -> Result<Vec<PolicyLifecycleAuditEvent>, RepositoryError>;
}

pub struct SqlPolicyOptimizerRepository {
    pool: DbPool,
}

impl SqlPolicyOptimizerRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PolicyOptimizerRepository for SqlPolicyOptimizerRepository {
    async fn save_candidate(&self, candidate: PolicyCandidate) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO policy_candidate (
                id, base_policy_version, proposed_policy_version, status, policy_diff_json,
                provenance_json, confidence_score, cohort_scope_json, latest_replay_checksum,
                idempotency_key, created_by_actor_id, created_at, updated_at,
                review_ready_at, approved_at, applied_at, monitoring_started_at, rolled_back_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                base_policy_version = excluded.base_policy_version,
                proposed_policy_version = excluded.proposed_policy_version,
                status = excluded.status,
                policy_diff_json = excluded.policy_diff_json,
                provenance_json = excluded.provenance_json,
                confidence_score = excluded.confidence_score,
                cohort_scope_json = excluded.cohort_scope_json,
                latest_replay_checksum = excluded.latest_replay_checksum,
                idempotency_key = excluded.idempotency_key,
                created_by_actor_id = excluded.created_by_actor_id,
                updated_at = excluded.updated_at,
                review_ready_at = excluded.review_ready_at,
                approved_at = excluded.approved_at,
                applied_at = excluded.applied_at,
                monitoring_started_at = excluded.monitoring_started_at,
                rolled_back_at = excluded.rolled_back_at
            "#,
        )
        .bind(&candidate.id.0)
        .bind(i64::from(candidate.base_policy_version))
        .bind(i64::from(candidate.proposed_policy_version))
        .bind(candidate.status.as_str())
        .bind(&candidate.policy_diff_json)
        .bind(&candidate.provenance_json)
        .bind(candidate.confidence_score)
        .bind(&candidate.cohort_scope_json)
        .bind(candidate.latest_replay_checksum.as_deref())
        .bind(&candidate.idempotency_key)
        .bind(&candidate.created_by_actor_id)
        .bind(candidate.created_at.to_rfc3339())
        .bind(candidate.updated_at.to_rfc3339())
        .bind(candidate.review_ready_at.map(|value| value.to_rfc3339()))
        .bind(candidate.approved_at.map(|value| value.to_rfc3339()))
        .bind(candidate.applied_at.map(|value| value.to_rfc3339()))
        .bind(candidate.monitoring_started_at.map(|value| value.to_rfc3339()))
        .bind(candidate.rolled_back_at.map(|value| value.to_rfc3339()))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_candidate(
        &self,
        id: &PolicyCandidateId,
    ) -> Result<Option<PolicyCandidate>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id, base_policy_version, proposed_policy_version, status, policy_diff_json,
                provenance_json, confidence_score, cohort_scope_json, latest_replay_checksum,
                idempotency_key, created_by_actor_id, created_at, updated_at,
                review_ready_at, approved_at, applied_at, monitoring_started_at, rolled_back_at
            FROM policy_candidate
            WHERE id = ?
            "#,
        )
        .bind(&id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(candidate_from_row).transpose()
    }

    async fn list_candidates_by_status(
        &self,
        status: Option<PolicyCandidateStatus>,
        limit: i32,
    ) -> Result<Vec<PolicyCandidate>, RepositoryError> {
        let rows = if let Some(status) = status {
            sqlx::query(
                r#"
                SELECT
                    id, base_policy_version, proposed_policy_version, status, policy_diff_json,
                    provenance_json, confidence_score, cohort_scope_json, latest_replay_checksum,
                    idempotency_key, created_by_actor_id, created_at, updated_at,
                    review_ready_at, approved_at, applied_at, monitoring_started_at, rolled_back_at
                FROM policy_candidate
                WHERE status = ?
                ORDER BY created_at DESC
                LIMIT ?
                "#,
            )
            .bind(status.as_str())
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT
                    id, base_policy_version, proposed_policy_version, status, policy_diff_json,
                    provenance_json, confidence_score, cohort_scope_json, latest_replay_checksum,
                    idempotency_key, created_by_actor_id, created_at, updated_at,
                    review_ready_at, approved_at, applied_at, monitoring_started_at, rolled_back_at
                FROM policy_candidate
                ORDER BY created_at DESC
                LIMIT ?
                "#,
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        rows.iter().map(candidate_from_row).collect()
    }

    async fn save_replay_evaluation(
        &self,
        replay: ReplayEvaluation,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO policy_replay_evaluation (
                id, candidate_id, replay_checksum, engine_version, cohort_scope_json,
                cohort_size, projected_margin_delta_bps, projected_win_rate_delta_bps,
                projected_approval_latency_delta_seconds, blast_radius_score,
                hard_violation_count, risk_flags_json, deterministic_pass,
                idempotency_key, replayed_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                candidate_id = excluded.candidate_id,
                replay_checksum = excluded.replay_checksum,
                engine_version = excluded.engine_version,
                cohort_scope_json = excluded.cohort_scope_json,
                cohort_size = excluded.cohort_size,
                projected_margin_delta_bps = excluded.projected_margin_delta_bps,
                projected_win_rate_delta_bps = excluded.projected_win_rate_delta_bps,
                projected_approval_latency_delta_seconds = excluded.projected_approval_latency_delta_seconds,
                blast_radius_score = excluded.blast_radius_score,
                hard_violation_count = excluded.hard_violation_count,
                risk_flags_json = excluded.risk_flags_json,
                deterministic_pass = excluded.deterministic_pass,
                idempotency_key = excluded.idempotency_key,
                replayed_at = excluded.replayed_at
            "#,
        )
        .bind(&replay.id.0)
        .bind(&replay.candidate_id.0)
        .bind(&replay.replay_checksum)
        .bind(&replay.engine_version)
        .bind(&replay.cohort_scope_json)
        .bind(i64::from(replay.cohort_size))
        .bind(i64::from(replay.projected_margin_delta_bps))
        .bind(i64::from(replay.projected_win_rate_delta_bps))
        .bind(i64::from(replay.projected_approval_latency_delta_seconds))
        .bind(replay.blast_radius_score)
        .bind(i64::from(replay.hard_violation_count))
        .bind(&replay.risk_flags_json)
        .bind(if replay.deterministic_pass { 1_i64 } else { 0_i64 })
        .bind(&replay.idempotency_key)
        .bind(replay.replayed_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_replay_evaluations_for_candidate(
        &self,
        candidate_id: &PolicyCandidateId,
    ) -> Result<Vec<ReplayEvaluation>, RepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, candidate_id, replay_checksum, engine_version, cohort_scope_json,
                cohort_size, projected_margin_delta_bps, projected_win_rate_delta_bps,
                projected_approval_latency_delta_seconds, blast_radius_score,
                hard_violation_count, risk_flags_json, deterministic_pass,
                idempotency_key, replayed_at
            FROM policy_replay_evaluation
            WHERE candidate_id = ?
            ORDER BY replayed_at ASC
            "#,
        )
        .bind(&candidate_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(replay_from_row).collect()
    }

    async fn find_replay_evaluation_by_checksum(
        &self,
        candidate_id: &PolicyCandidateId,
        replay_checksum: &str,
    ) -> Result<Option<ReplayEvaluation>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id, candidate_id, replay_checksum, engine_version, cohort_scope_json,
                cohort_size, projected_margin_delta_bps, projected_win_rate_delta_bps,
                projected_approval_latency_delta_seconds, blast_radius_score,
                hard_violation_count, risk_flags_json, deterministic_pass,
                idempotency_key, replayed_at
            FROM policy_replay_evaluation
            WHERE candidate_id = ? AND replay_checksum = ?
            "#,
        )
        .bind(&candidate_id.0)
        .bind(replay_checksum)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|value| replay_from_row(&value)).transpose()
    }

    async fn save_approval_decision(
        &self,
        decision: PolicyApprovalDecision,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO policy_approval_decision (
                id, candidate_id, replay_evaluation_id, decision, reason,
                decision_payload_json, actor_id, actor_role, channel_ref,
                signature, signature_key_id, idempotency_key, decided_at,
                expires_at, is_stale
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                candidate_id = excluded.candidate_id,
                replay_evaluation_id = excluded.replay_evaluation_id,
                decision = excluded.decision,
                reason = excluded.reason,
                decision_payload_json = excluded.decision_payload_json,
                actor_id = excluded.actor_id,
                actor_role = excluded.actor_role,
                channel_ref = excluded.channel_ref,
                signature = excluded.signature,
                signature_key_id = excluded.signature_key_id,
                idempotency_key = excluded.idempotency_key,
                decided_at = excluded.decided_at,
                expires_at = excluded.expires_at,
                is_stale = excluded.is_stale
            "#,
        )
        .bind(&decision.id.0)
        .bind(&decision.candidate_id.0)
        .bind(decision.replay_evaluation_id.as_ref().map(|value| value.0.as_str()))
        .bind(decision.decision.as_str())
        .bind(decision.reason.as_deref())
        .bind(&decision.decision_payload_json)
        .bind(&decision.actor_id)
        .bind(&decision.actor_role)
        .bind(decision.channel_ref.as_deref())
        .bind(decision.signature.as_deref())
        .bind(decision.signature_key_id.as_deref())
        .bind(&decision.idempotency_key)
        .bind(decision.decided_at.to_rfc3339())
        .bind(decision.expires_at.map(|value| value.to_rfc3339()))
        .bind(if decision.is_stale { 1_i64 } else { 0_i64 })
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_approval_decisions_for_candidate(
        &self,
        candidate_id: &PolicyCandidateId,
    ) -> Result<Vec<PolicyApprovalDecision>, RepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, candidate_id, replay_evaluation_id, decision, reason,
                decision_payload_json, actor_id, actor_role, channel_ref,
                signature, signature_key_id, idempotency_key, decided_at,
                expires_at, is_stale
            FROM policy_approval_decision
            WHERE candidate_id = ?
            ORDER BY decided_at ASC
            "#,
        )
        .bind(&candidate_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(approval_decision_from_row).collect()
    }

    async fn list_stale_approval_decisions(
        &self,
        before: DateTime<Utc>,
    ) -> Result<Vec<PolicyApprovalDecision>, RepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, candidate_id, replay_evaluation_id, decision, reason,
                decision_payload_json, actor_id, actor_role, channel_ref,
                signature, signature_key_id, idempotency_key, decided_at,
                expires_at, is_stale
            FROM policy_approval_decision
            WHERE is_stale = 1 AND expires_at IS NOT NULL AND expires_at <= ?
            ORDER BY expires_at ASC, decided_at ASC
            "#,
        )
        .bind(before.to_rfc3339())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(approval_decision_from_row).collect()
    }

    async fn save_apply_record(&self, apply: PolicyApplyRecord) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO policy_apply_record (
                id, candidate_id, approval_decision_id, prior_policy_version,
                applied_policy_version, replay_checksum, apply_signature,
                signature_key_id, actor_id, idempotency_key,
                verification_checksum, apply_audit_json, applied_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                candidate_id = excluded.candidate_id,
                approval_decision_id = excluded.approval_decision_id,
                prior_policy_version = excluded.prior_policy_version,
                applied_policy_version = excluded.applied_policy_version,
                replay_checksum = excluded.replay_checksum,
                apply_signature = excluded.apply_signature,
                signature_key_id = excluded.signature_key_id,
                actor_id = excluded.actor_id,
                idempotency_key = excluded.idempotency_key,
                verification_checksum = excluded.verification_checksum,
                apply_audit_json = excluded.apply_audit_json,
                applied_at = excluded.applied_at
            "#,
        )
        .bind(&apply.id.0)
        .bind(&apply.candidate_id.0)
        .bind(&apply.approval_decision_id.0)
        .bind(i64::from(apply.prior_policy_version))
        .bind(i64::from(apply.applied_policy_version))
        .bind(&apply.replay_checksum)
        .bind(&apply.apply_signature)
        .bind(&apply.signature_key_id)
        .bind(&apply.actor_id)
        .bind(&apply.idempotency_key)
        .bind(&apply.verification_checksum)
        .bind(&apply.apply_audit_json)
        .bind(apply.applied_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_apply_record(
        &self,
        id: &PolicyApplyRecordId,
    ) -> Result<Option<PolicyApplyRecord>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id, candidate_id, approval_decision_id, prior_policy_version,
                applied_policy_version, replay_checksum, apply_signature,
                signature_key_id, actor_id, idempotency_key,
                verification_checksum, apply_audit_json, applied_at
            FROM policy_apply_record
            WHERE id = ?
            "#,
        )
        .bind(&id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|value| apply_record_from_row(&value)).transpose()
    }

    async fn save_rollback_record(
        &self,
        rollback: PolicyRollbackRecord,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO policy_rollback_record (
                id, candidate_id, apply_record_id, rollback_target_version,
                rollback_reason, verification_checksum, rollback_signature,
                signature_key_id, actor_id, idempotency_key,
                parent_rollback_id, rollback_depth, rollback_metadata_json, rolled_back_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                candidate_id = excluded.candidate_id,
                apply_record_id = excluded.apply_record_id,
                rollback_target_version = excluded.rollback_target_version,
                rollback_reason = excluded.rollback_reason,
                verification_checksum = excluded.verification_checksum,
                rollback_signature = excluded.rollback_signature,
                signature_key_id = excluded.signature_key_id,
                actor_id = excluded.actor_id,
                idempotency_key = excluded.idempotency_key,
                parent_rollback_id = excluded.parent_rollback_id,
                rollback_depth = excluded.rollback_depth,
                rollback_metadata_json = excluded.rollback_metadata_json,
                rolled_back_at = excluded.rolled_back_at
            "#,
        )
        .bind(&rollback.id.0)
        .bind(&rollback.candidate_id.0)
        .bind(&rollback.apply_record_id.0)
        .bind(i64::from(rollback.rollback_target_version))
        .bind(&rollback.rollback_reason)
        .bind(&rollback.verification_checksum)
        .bind(&rollback.rollback_signature)
        .bind(&rollback.signature_key_id)
        .bind(&rollback.actor_id)
        .bind(&rollback.idempotency_key)
        .bind(rollback.parent_rollback_id.as_ref().map(|value| value.0.as_str()))
        .bind(i64::from(rollback.rollback_depth))
        .bind(&rollback.rollback_metadata_json)
        .bind(rollback.rolled_back_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_rollback_chain_for_apply(
        &self,
        apply_record_id: &PolicyApplyRecordId,
    ) -> Result<Vec<PolicyRollbackRecord>, RepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, candidate_id, apply_record_id, rollback_target_version,
                rollback_reason, verification_checksum, rollback_signature,
                signature_key_id, actor_id, idempotency_key,
                parent_rollback_id, rollback_depth, rollback_metadata_json, rolled_back_at
            FROM policy_rollback_record
            WHERE apply_record_id = ?
            ORDER BY rollback_depth ASC, rolled_back_at ASC
            "#,
        )
        .bind(&apply_record_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(rollback_record_from_row).collect()
    }

    async fn append_lifecycle_audit_event(
        &self,
        event: PolicyLifecycleAuditEvent,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO policy_lifecycle_audit (
                id, candidate_id, replay_evaluation_id, approval_decision_id,
                apply_record_id, rollback_record_id, event_type, event_payload_json,
                actor_type, actor_id, correlation_id, idempotency_key, occurred_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                candidate_id = excluded.candidate_id,
                replay_evaluation_id = excluded.replay_evaluation_id,
                approval_decision_id = excluded.approval_decision_id,
                apply_record_id = excluded.apply_record_id,
                rollback_record_id = excluded.rollback_record_id,
                event_type = excluded.event_type,
                event_payload_json = excluded.event_payload_json,
                actor_type = excluded.actor_type,
                actor_id = excluded.actor_id,
                correlation_id = excluded.correlation_id,
                idempotency_key = excluded.idempotency_key,
                occurred_at = excluded.occurred_at
            "#,
        )
        .bind(&event.id.0)
        .bind(&event.candidate_id.0)
        .bind(event.replay_evaluation_id.as_ref().map(|value| value.0.as_str()))
        .bind(event.approval_decision_id.as_ref().map(|value| value.0.as_str()))
        .bind(event.apply_record_id.as_ref().map(|value| value.0.as_str()))
        .bind(event.rollback_record_id.as_ref().map(|value| value.0.as_str()))
        .bind(event.event_type.as_str())
        .bind(&event.event_payload_json)
        .bind(&event.actor_type)
        .bind(&event.actor_id)
        .bind(&event.correlation_id)
        .bind(event.idempotency_key.as_deref())
        .bind(event.occurred_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_lifecycle_audit_for_candidate(
        &self,
        candidate_id: &PolicyCandidateId,
    ) -> Result<Vec<PolicyLifecycleAuditEvent>, RepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, candidate_id, replay_evaluation_id, approval_decision_id,
                apply_record_id, rollback_record_id, event_type, event_payload_json,
                actor_type, actor_id, correlation_id, idempotency_key, occurred_at
            FROM policy_lifecycle_audit
            WHERE candidate_id = ?
            ORDER BY occurred_at ASC
            "#,
        )
        .bind(&candidate_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(lifecycle_audit_from_row).collect()
    }
}

fn candidate_from_row(row: &SqliteRow) -> Result<PolicyCandidate, RepositoryError> {
    let status_raw: String = row.try_get("status")?;

    Ok(PolicyCandidate {
        id: PolicyCandidateId(row.try_get("id")?),
        base_policy_version: parse_i32("base_policy_version", row.try_get("base_policy_version")?)?,
        proposed_policy_version: parse_i32(
            "proposed_policy_version",
            row.try_get("proposed_policy_version")?,
        )?,
        status: PolicyCandidateStatus::parse(&status_raw).ok_or_else(|| {
            RepositoryError::Decode(format!("invalid candidate status: {status_raw}"))
        })?,
        policy_diff_json: row.try_get("policy_diff_json")?,
        provenance_json: row.try_get("provenance_json")?,
        confidence_score: row.try_get("confidence_score")?,
        cohort_scope_json: row.try_get("cohort_scope_json")?,
        latest_replay_checksum: row.try_get("latest_replay_checksum")?,
        idempotency_key: row.try_get("idempotency_key")?,
        created_by_actor_id: row.try_get("created_by_actor_id")?,
        created_at: parse_timestamp("created_at", row.try_get("created_at")?)?,
        updated_at: parse_timestamp("updated_at", row.try_get("updated_at")?)?,
        review_ready_at: parse_optional_timestamp(
            "review_ready_at",
            row.try_get("review_ready_at")?,
        )?,
        approved_at: parse_optional_timestamp("approved_at", row.try_get("approved_at")?)?,
        applied_at: parse_optional_timestamp("applied_at", row.try_get("applied_at")?)?,
        monitoring_started_at: parse_optional_timestamp(
            "monitoring_started_at",
            row.try_get("monitoring_started_at")?,
        )?,
        rolled_back_at: parse_optional_timestamp("rolled_back_at", row.try_get("rolled_back_at")?)?,
    })
}

fn replay_from_row(row: &SqliteRow) -> Result<ReplayEvaluation, RepositoryError> {
    Ok(ReplayEvaluation {
        id: ReplayEvaluationId(row.try_get("id")?),
        candidate_id: PolicyCandidateId(row.try_get("candidate_id")?),
        replay_checksum: row.try_get("replay_checksum")?,
        engine_version: row.try_get("engine_version")?,
        cohort_scope_json: row.try_get("cohort_scope_json")?,
        cohort_size: parse_i32("cohort_size", row.try_get("cohort_size")?)?,
        projected_margin_delta_bps: parse_i32(
            "projected_margin_delta_bps",
            row.try_get("projected_margin_delta_bps")?,
        )?,
        projected_win_rate_delta_bps: parse_i32(
            "projected_win_rate_delta_bps",
            row.try_get("projected_win_rate_delta_bps")?,
        )?,
        projected_approval_latency_delta_seconds: parse_i32(
            "projected_approval_latency_delta_seconds",
            row.try_get("projected_approval_latency_delta_seconds")?,
        )?,
        blast_radius_score: row.try_get("blast_radius_score")?,
        hard_violation_count: parse_i32(
            "hard_violation_count",
            row.try_get("hard_violation_count")?,
        )?,
        risk_flags_json: row.try_get("risk_flags_json")?,
        deterministic_pass: parse_bool_flag(
            "deterministic_pass",
            row.try_get("deterministic_pass")?,
        )?,
        idempotency_key: row.try_get("idempotency_key")?,
        replayed_at: parse_timestamp("replayed_at", row.try_get("replayed_at")?)?,
    })
}

fn approval_decision_from_row(row: &SqliteRow) -> Result<PolicyApprovalDecision, RepositoryError> {
    let decision_raw: String = row.try_get("decision")?;

    Ok(PolicyApprovalDecision {
        id: PolicyApprovalDecisionId(row.try_get("id")?),
        candidate_id: PolicyCandidateId(row.try_get("candidate_id")?),
        replay_evaluation_id: row
            .try_get::<Option<String>, _>("replay_evaluation_id")?
            .map(ReplayEvaluationId),
        decision: ApprovalDecisionKind::parse(&decision_raw).ok_or_else(|| {
            RepositoryError::Decode(format!("invalid approval decision: {decision_raw}"))
        })?,
        reason: row.try_get("reason")?,
        decision_payload_json: row.try_get("decision_payload_json")?,
        actor_id: row.try_get("actor_id")?,
        actor_role: row.try_get("actor_role")?,
        channel_ref: row.try_get("channel_ref")?,
        signature: row.try_get("signature")?,
        signature_key_id: row.try_get("signature_key_id")?,
        idempotency_key: row.try_get("idempotency_key")?,
        decided_at: parse_timestamp("decided_at", row.try_get("decided_at")?)?,
        expires_at: parse_optional_timestamp("expires_at", row.try_get("expires_at")?)?,
        is_stale: parse_bool_flag("is_stale", row.try_get("is_stale")?)?,
    })
}

fn apply_record_from_row(row: &SqliteRow) -> Result<PolicyApplyRecord, RepositoryError> {
    Ok(PolicyApplyRecord {
        id: PolicyApplyRecordId(row.try_get("id")?),
        candidate_id: PolicyCandidateId(row.try_get("candidate_id")?),
        approval_decision_id: PolicyApprovalDecisionId(row.try_get("approval_decision_id")?),
        prior_policy_version: parse_i32(
            "prior_policy_version",
            row.try_get("prior_policy_version")?,
        )?,
        applied_policy_version: parse_i32(
            "applied_policy_version",
            row.try_get("applied_policy_version")?,
        )?,
        replay_checksum: row.try_get("replay_checksum")?,
        apply_signature: row.try_get("apply_signature")?,
        signature_key_id: row.try_get("signature_key_id")?,
        actor_id: row.try_get("actor_id")?,
        idempotency_key: row.try_get("idempotency_key")?,
        verification_checksum: row.try_get("verification_checksum")?,
        apply_audit_json: row.try_get("apply_audit_json")?,
        applied_at: parse_timestamp("applied_at", row.try_get("applied_at")?)?,
    })
}

fn rollback_record_from_row(row: &SqliteRow) -> Result<PolicyRollbackRecord, RepositoryError> {
    Ok(PolicyRollbackRecord {
        id: PolicyRollbackRecordId(row.try_get("id")?),
        candidate_id: PolicyCandidateId(row.try_get("candidate_id")?),
        apply_record_id: PolicyApplyRecordId(row.try_get("apply_record_id")?),
        rollback_target_version: parse_i32(
            "rollback_target_version",
            row.try_get("rollback_target_version")?,
        )?,
        rollback_reason: row.try_get("rollback_reason")?,
        verification_checksum: row.try_get("verification_checksum")?,
        rollback_signature: row.try_get("rollback_signature")?,
        signature_key_id: row.try_get("signature_key_id")?,
        actor_id: row.try_get("actor_id")?,
        idempotency_key: row.try_get("idempotency_key")?,
        parent_rollback_id: row
            .try_get::<Option<String>, _>("parent_rollback_id")?
            .map(PolicyRollbackRecordId),
        rollback_depth: parse_i32("rollback_depth", row.try_get("rollback_depth")?)?,
        rollback_metadata_json: row.try_get("rollback_metadata_json")?,
        rolled_back_at: parse_timestamp("rolled_back_at", row.try_get("rolled_back_at")?)?,
    })
}

fn lifecycle_audit_from_row(row: &SqliteRow) -> Result<PolicyLifecycleAuditEvent, RepositoryError> {
    let event_raw: String = row.try_get("event_type")?;

    Ok(PolicyLifecycleAuditEvent {
        id: PolicyLifecycleAuditId(row.try_get("id")?),
        candidate_id: PolicyCandidateId(row.try_get("candidate_id")?),
        replay_evaluation_id: row
            .try_get::<Option<String>, _>("replay_evaluation_id")?
            .map(ReplayEvaluationId),
        approval_decision_id: row
            .try_get::<Option<String>, _>("approval_decision_id")?
            .map(PolicyApprovalDecisionId),
        apply_record_id: row
            .try_get::<Option<String>, _>("apply_record_id")?
            .map(PolicyApplyRecordId),
        rollback_record_id: row
            .try_get::<Option<String>, _>("rollback_record_id")?
            .map(PolicyRollbackRecordId),
        event_type: PolicyLifecycleAuditEventType::parse(&event_raw).ok_or_else(|| {
            RepositoryError::Decode(format!("invalid lifecycle event type: {event_raw}"))
        })?,
        event_payload_json: row.try_get("event_payload_json")?,
        actor_type: row.try_get("actor_type")?,
        actor_id: row.try_get("actor_id")?,
        correlation_id: row.try_get("correlation_id")?,
        idempotency_key: row.try_get("idempotency_key")?,
        occurred_at: parse_timestamp("occurred_at", row.try_get("occurred_at")?)?,
    })
}

fn parse_i32(column: &str, value: i64) -> Result<i32, RepositoryError> {
    i32::try_from(value).map_err(|_| {
        RepositoryError::Decode(format!(
            "invalid value for `{column}` (expected i32 range): {value}"
        ))
    })
}

fn parse_bool_flag(column: &str, value: i64) -> Result<bool, RepositoryError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        raw => Err(RepositoryError::Decode(format!("invalid boolean flag for `{column}`: {raw}"))),
    }
}

fn parse_timestamp(column: &str, value: String) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(&value).map(|timestamp| timestamp.with_timezone(&Utc)).map_err(
        |error| {
            RepositoryError::Decode(format!("invalid timestamp in `{column}`: `{value}` ({error})"))
        },
    )
}

fn parse_optional_timestamp(
    column: &str,
    value: Option<String>,
) -> Result<Option<DateTime<Utc>>, RepositoryError> {
    value.map(|timestamp| parse_timestamp(column, timestamp)).transpose()
}

#[cfg(test)]
mod tests {
    use quotey_core::chrono::{DateTime, Utc};
    use quotey_core::domain::optimizer::{
        ApprovalDecisionKind, PolicyApplyRecord, PolicyApplyRecordId, PolicyApprovalDecision,
        PolicyApprovalDecisionId, PolicyCandidate, PolicyCandidateId, PolicyCandidateStatus,
        PolicyLifecycleAuditEvent, PolicyLifecycleAuditEventType, PolicyLifecycleAuditId,
        PolicyRollbackRecord, PolicyRollbackRecordId, ReplayEvaluation, ReplayEvaluationId,
    };

    use super::{PolicyOptimizerRepository, SqlPolicyOptimizerRepository};
    use crate::{connect_with_settings, migrations, DbPool};
    type TestResult<T> = Result<T, String>;

    #[tokio::test]
    async fn sql_optimizer_repo_supports_conflicting_candidates_and_replay_lookup() -> TestResult<()>
    {
        let pool = setup_pool().await?;
        let repo = SqlPolicyOptimizerRepository::new(pool.clone());

        let candidate_a =
            sample_candidate("cand-conflict-a", "idem-cand-a", "U-A", "2026-02-24T01:00:00Z")?;
        let candidate_b =
            sample_candidate("cand-conflict-b", "idem-cand-b", "U-B", "2026-02-24T01:01:00Z")?;

        repo.save_candidate(candidate_a.clone())
            .await
            .map_err(|error| format!("save candidate a: {error}"))?;
        repo.save_candidate(candidate_b.clone())
            .await
            .map_err(|error| format!("save candidate b: {error}"))?;

        let draft_candidates = repo
            .list_candidates_by_status(Some(PolicyCandidateStatus::Draft), 10)
            .await
            .map_err(|error| format!("list draft candidates: {error}"))?;
        if draft_candidates.len() != 2 {
            return Err("conflicting candidates: expected exactly two candidates".to_string());
        }

        let replay = ReplayEvaluation {
            id: ReplayEvaluationId("replay-cand-a-v1".to_string()),
            candidate_id: candidate_a.id.clone(),
            replay_checksum: "sha256:replay-a-1".to_string(),
            engine_version: "core-1.0.0".to_string(),
            cohort_scope_json: "{\"segment\":\"enterprise\"}".to_string(),
            cohort_size: 42,
            projected_margin_delta_bps: 120,
            projected_win_rate_delta_bps: 45,
            projected_approval_latency_delta_seconds: -900,
            blast_radius_score: 0.18,
            hard_violation_count: 0,
            risk_flags_json: "[]".to_string(),
            deterministic_pass: true,
            idempotency_key: "idem-replay-a-1".to_string(),
            replayed_at: parse_ts("2026-02-24T01:02:00Z")?,
        };

        repo.save_replay_evaluation(replay.clone())
            .await
            .map_err(|error| format!("save replay: {error}"))?;

        let replay_lookup = repo
            .find_replay_evaluation_by_checksum(&candidate_a.id, &replay.replay_checksum)
            .await
            .map_err(|error| format!("lookup replay by checksum: {error}"))?;
        if replay_lookup != Some(replay) {
            return Err("replay lookup mismatch".to_string());
        }

        pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn sql_optimizer_repo_lists_stale_approval_decisions() -> TestResult<()> {
        let pool = setup_pool().await?;
        let repo = SqlPolicyOptimizerRepository::new(pool.clone());

        let candidate =
            sample_candidate("cand-stale", "idem-cand-stale", "U-C", "2026-02-24T01:10:00Z")?;
        repo.save_candidate(candidate.clone())
            .await
            .map_err(|error| format!("save candidate: {error}"))?;

        let fresh = PolicyApprovalDecision {
            id: PolicyApprovalDecisionId("apr-fresh".to_string()),
            candidate_id: candidate.id.clone(),
            replay_evaluation_id: None,
            decision: ApprovalDecisionKind::Approved,
            reason: Some("within policy".to_string()),
            decision_payload_json: "{}".to_string(),
            actor_id: "U-MGR-1".to_string(),
            actor_role: "sales_manager".to_string(),
            channel_ref: Some("C123/T123".to_string()),
            signature: Some("sig-fresh".to_string()),
            signature_key_id: Some("kms-key-1".to_string()),
            idempotency_key: "idem-apr-fresh".to_string(),
            decided_at: parse_ts("2026-02-24T01:11:00Z")?,
            expires_at: Some(parse_ts("2026-02-25T01:11:00Z")?),
            is_stale: false,
        };

        let stale = PolicyApprovalDecision {
            id: PolicyApprovalDecisionId("apr-stale".to_string()),
            candidate_id: candidate.id.clone(),
            replay_evaluation_id: None,
            decision: ApprovalDecisionKind::ChangesRequested,
            reason: Some("requires more cohort evidence".to_string()),
            decision_payload_json: "{}".to_string(),
            actor_id: "U-MGR-2".to_string(),
            actor_role: "deal_desk".to_string(),
            channel_ref: Some("C124/T124".to_string()),
            signature: None,
            signature_key_id: None,
            idempotency_key: "idem-apr-stale".to_string(),
            decided_at: parse_ts("2026-02-24T01:12:00Z")?,
            expires_at: Some(parse_ts("2026-02-24T01:20:00Z")?),
            is_stale: true,
        };

        repo.save_approval_decision(fresh)
            .await
            .map_err(|error| format!("save fresh approval: {error}"))?;
        repo.save_approval_decision(stale.clone())
            .await
            .map_err(|error| format!("save stale approval: {error}"))?;

        let stale_rows = repo
            .list_stale_approval_decisions(parse_ts("2026-02-24T02:00:00Z")?)
            .await
            .map_err(|error| format!("list stale approvals: {error}"))?;
        if stale_rows != vec![stale] {
            return Err("stale approval rows mismatch".to_string());
        }

        pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn sql_optimizer_repo_persists_apply_and_rollback_chains() -> TestResult<()> {
        let pool = setup_pool().await?;
        let repo = SqlPolicyOptimizerRepository::new(pool.clone());

        let candidate =
            sample_candidate("cand-chain", "idem-cand-chain", "U-CHAIN", "2026-02-24T02:00:00Z")?;
        repo.save_candidate(candidate.clone())
            .await
            .map_err(|error| format!("save candidate: {error}"))?;

        let approval = PolicyApprovalDecision {
            id: PolicyApprovalDecisionId("apr-chain".to_string()),
            candidate_id: candidate.id.clone(),
            replay_evaluation_id: None,
            decision: ApprovalDecisionKind::Approved,
            reason: Some("signed-off".to_string()),
            decision_payload_json: "{}".to_string(),
            actor_id: "U-VP-1".to_string(),
            actor_role: "vp_sales".to_string(),
            channel_ref: None,
            signature: Some("sig-approval".to_string()),
            signature_key_id: Some("kms-key-2".to_string()),
            idempotency_key: "idem-apr-chain".to_string(),
            decided_at: parse_ts("2026-02-24T02:05:00Z")?,
            expires_at: None,
            is_stale: false,
        };
        repo.save_approval_decision(approval.clone())
            .await
            .map_err(|error| format!("save approval: {error}"))?;

        let apply = PolicyApplyRecord {
            id: PolicyApplyRecordId("apply-chain".to_string()),
            candidate_id: candidate.id.clone(),
            approval_decision_id: approval.id.clone(),
            prior_policy_version: 12,
            applied_policy_version: 13,
            replay_checksum: "sha256:chain-replay".to_string(),
            apply_signature: "sig-apply".to_string(),
            signature_key_id: "kms-key-3".to_string(),
            actor_id: "U-OPS-1".to_string(),
            idempotency_key: "idem-apply-chain".to_string(),
            verification_checksum: "sha256:verify-apply".to_string(),
            apply_audit_json: "{}".to_string(),
            applied_at: parse_ts("2026-02-24T02:10:00Z")?,
        };
        repo.save_apply_record(apply.clone())
            .await
            .map_err(|error| format!("save apply: {error}"))?;

        let rollback_one = PolicyRollbackRecord {
            id: PolicyRollbackRecordId("rb-chain-1".to_string()),
            candidate_id: candidate.id.clone(),
            apply_record_id: apply.id.clone(),
            rollback_target_version: 12,
            rollback_reason: "forecast drift".to_string(),
            verification_checksum: "sha256:verify-rb-1".to_string(),
            rollback_signature: "sig-rb-1".to_string(),
            signature_key_id: "kms-key-rb".to_string(),
            actor_id: "U-OPS-2".to_string(),
            idempotency_key: "idem-rb-1".to_string(),
            parent_rollback_id: None,
            rollback_depth: 0,
            rollback_metadata_json: "{}".to_string(),
            rolled_back_at: parse_ts("2026-02-24T02:15:00Z")?,
        };

        let rollback_two = PolicyRollbackRecord {
            id: PolicyRollbackRecordId("rb-chain-2".to_string()),
            candidate_id: candidate.id.clone(),
            apply_record_id: apply.id.clone(),
            rollback_target_version: 11,
            rollback_reason: "secondary rollback drill".to_string(),
            verification_checksum: "sha256:verify-rb-2".to_string(),
            rollback_signature: "sig-rb-2".to_string(),
            signature_key_id: "kms-key-rb".to_string(),
            actor_id: "U-OPS-3".to_string(),
            idempotency_key: "idem-rb-2".to_string(),
            parent_rollback_id: Some(rollback_one.id.clone()),
            rollback_depth: 1,
            rollback_metadata_json: "{}".to_string(),
            rolled_back_at: parse_ts("2026-02-24T02:20:00Z")?,
        };

        repo.save_rollback_record(rollback_one.clone())
            .await
            .map_err(|error| format!("save rollback one: {error}"))?;
        repo.save_rollback_record(rollback_two.clone())
            .await
            .map_err(|error| format!("save rollback two: {error}"))?;

        let rollback_chain = repo
            .list_rollback_chain_for_apply(&apply.id)
            .await
            .map_err(|error| format!("list rollback chain: {error}"))?;
        if rollback_chain != vec![rollback_one, rollback_two] {
            return Err("rollback chain mismatch".to_string());
        }

        let apply_lookup = repo
            .get_apply_record(&apply.id)
            .await
            .map_err(|error| format!("get apply record: {error}"))?;
        if apply_lookup != Some(apply) {
            return Err("apply record mismatch".to_string());
        }

        pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn sql_optimizer_repo_appends_and_lists_lifecycle_audit_events() -> TestResult<()> {
        let pool = setup_pool().await?;
        let repo = SqlPolicyOptimizerRepository::new(pool.clone());

        let candidate =
            sample_candidate("cand-audit", "idem-cand-audit", "U-AUD", "2026-02-24T03:00:00Z")?;
        repo.save_candidate(candidate.clone())
            .await
            .map_err(|error| format!("save candidate: {error}"))?;

        let created_event = PolicyLifecycleAuditEvent {
            id: PolicyLifecycleAuditId("audit-created".to_string()),
            candidate_id: candidate.id.clone(),
            replay_evaluation_id: None,
            approval_decision_id: None,
            apply_record_id: None,
            rollback_record_id: None,
            event_type: PolicyLifecycleAuditEventType::CandidateCreated,
            event_payload_json: "{}".to_string(),
            actor_type: "agent".to_string(),
            actor_id: "optimizer".to_string(),
            correlation_id: "corr-audit-1".to_string(),
            idempotency_key: Some("idem-audit-created".to_string()),
            occurred_at: parse_ts("2026-02-24T03:01:00Z")?,
        };

        let stale_event = PolicyLifecycleAuditEvent {
            id: PolicyLifecycleAuditId("audit-stale".to_string()),
            candidate_id: candidate.id.clone(),
            replay_evaluation_id: None,
            approval_decision_id: None,
            apply_record_id: None,
            rollback_record_id: None,
            event_type: PolicyLifecycleAuditEventType::StaleApprovalDetected,
            event_payload_json: "{\"approval_id\":\"apr-1\"}".to_string(),
            actor_type: "system".to_string(),
            actor_id: "policy-monitor".to_string(),
            correlation_id: "corr-audit-2".to_string(),
            idempotency_key: Some("idem-audit-stale".to_string()),
            occurred_at: parse_ts("2026-02-24T03:02:00Z")?,
        };

        repo.append_lifecycle_audit_event(created_event.clone())
            .await
            .map_err(|error| format!("append created event: {error}"))?;
        repo.append_lifecycle_audit_event(stale_event.clone())
            .await
            .map_err(|error| format!("append stale event: {error}"))?;

        let events = repo
            .list_lifecycle_audit_for_candidate(&candidate.id)
            .await
            .map_err(|error| format!("list lifecycle events: {error}"))?;

        if events != vec![created_event, stale_event] {
            return Err("lifecycle audit events mismatch".to_string());
        }

        pool.close().await;
        Ok(())
    }

    async fn setup_pool() -> TestResult<DbPool> {
        let pool = connect_with_settings("sqlite::memory:?cache=shared", 1, 30)
            .await
            .map_err(|error| format!("connect test pool: {error}"))?;
        migrations::run_pending(&pool).await.map_err(|error| format!("run migrations: {error}"))?;
        Ok(pool)
    }

    fn sample_candidate(
        id: &str,
        idempotency_key: &str,
        actor: &str,
        created_at: &str,
    ) -> TestResult<PolicyCandidate> {
        Ok(PolicyCandidate {
            id: PolicyCandidateId(id.to_string()),
            base_policy_version: 7,
            proposed_policy_version: 8,
            status: PolicyCandidateStatus::Draft,
            policy_diff_json: "{\"patch\":\"set discount cap 17%\"}".to_string(),
            provenance_json: "{\"source\":\"won/lost outcomes\"}".to_string(),
            confidence_score: 0.72,
            cohort_scope_json: "{\"segment\":\"enterprise\",\"region\":\"us\"}".to_string(),
            latest_replay_checksum: None,
            idempotency_key: idempotency_key.to_string(),
            created_by_actor_id: actor.to_string(),
            created_at: parse_ts(created_at)?,
            updated_at: parse_ts(created_at)?,
            review_ready_at: None,
            approved_at: None,
            applied_at: None,
            monitoring_started_at: None,
            rolled_back_at: None,
        })
    }

    fn parse_ts(value: &str) -> TestResult<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(value)
            .map(|timestamp| timestamp.with_timezone(&Utc))
            .map_err(|error| format!("invalid timestamp `{value}`: {error}"))
    }
}
