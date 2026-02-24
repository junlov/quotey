use std::collections::HashMap;

use tokio::sync::RwLock;

use quotey_core::chrono::{DateTime, Utc};
use quotey_core::domain::approval::{ApprovalId, ApprovalRequest};
use quotey_core::domain::execution::{
    ExecutionTask, ExecutionTaskId, ExecutionTaskState, ExecutionTransitionEvent,
    IdempotencyRecord, OperationKey,
};
use quotey_core::domain::optimizer::{
    PolicyApplyRecord, PolicyApplyRecordId, PolicyApprovalDecision, PolicyCandidate,
    PolicyCandidateId, PolicyCandidateStatus, PolicyLifecycleAuditEvent, PolicyRollbackRecord,
    ReplayEvaluation,
};
use quotey_core::domain::product::{Product, ProductId};
use quotey_core::domain::quote::{Quote, QuoteId};

use super::{
    ApprovalRepository, ExecutionQueueRepository, IdempotencyRepository, PolicyOptimizerRepository,
    ProductRepository, QuoteRepository, RepositoryError,
};

#[derive(Default)]
pub struct InMemoryQuoteRepository {
    quotes: RwLock<HashMap<String, Quote>>,
}

#[async_trait::async_trait]
impl QuoteRepository for InMemoryQuoteRepository {
    async fn find_by_id(&self, id: &QuoteId) -> Result<Option<Quote>, RepositoryError> {
        let quotes = self.quotes.read().await;
        Ok(quotes.get(&id.0).cloned())
    }

    async fn save(&self, quote: Quote) -> Result<(), RepositoryError> {
        let mut quotes = self.quotes.write().await;
        quotes.insert(quote.id.0.clone(), quote);
        Ok(())
    }
}

#[derive(Default)]
pub struct InMemoryProductRepository {
    products: RwLock<HashMap<String, Product>>,
}

#[async_trait::async_trait]
impl ProductRepository for InMemoryProductRepository {
    async fn find_by_id(&self, id: &ProductId) -> Result<Option<Product>, RepositoryError> {
        let products = self.products.read().await;
        Ok(products.get(&id.0).cloned())
    }

    async fn save(&self, product: Product) -> Result<(), RepositoryError> {
        let mut products = self.products.write().await;
        products.insert(product.id.0.clone(), product);
        Ok(())
    }
}

#[derive(Default)]
pub struct InMemoryApprovalRepository {
    approvals: RwLock<HashMap<String, ApprovalRequest>>,
}

#[async_trait::async_trait]
impl ApprovalRepository for InMemoryApprovalRepository {
    async fn find_by_id(
        &self,
        id: &ApprovalId,
    ) -> Result<Option<ApprovalRequest>, RepositoryError> {
        let approvals = self.approvals.read().await;
        Ok(approvals.get(&id.0).cloned())
    }

    async fn save(&self, approval: ApprovalRequest) -> Result<(), RepositoryError> {
        let mut approvals = self.approvals.write().await;
        approvals.insert(approval.id.0.clone(), approval);
        Ok(())
    }
}

#[derive(Default)]
pub struct InMemoryExecutionQueueRepository {
    tasks: RwLock<HashMap<String, ExecutionTask>>,
    transitions: RwLock<Vec<ExecutionTransitionEvent>>,
}

#[async_trait::async_trait]
impl ExecutionQueueRepository for InMemoryExecutionQueueRepository {
    async fn find_task_by_id(
        &self,
        id: &ExecutionTaskId,
    ) -> Result<Option<ExecutionTask>, RepositoryError> {
        let tasks = self.tasks.read().await;
        Ok(tasks.get(&id.0).cloned())
    }

    async fn list_tasks_for_quote(
        &self,
        quote_id: &QuoteId,
        state: Option<ExecutionTaskState>,
    ) -> Result<Vec<ExecutionTask>, RepositoryError> {
        let tasks = self.tasks.read().await;
        let mut entries: Vec<ExecutionTask> = tasks
            .values()
            .filter(|task| {
                task.quote_id == *quote_id
                    && match state.as_ref() {
                        Some(expected) => task.state == *expected,
                        None => true,
                    }
            })
            .cloned()
            .collect();
        entries.sort_by(|left, right| left.available_at.cmp(&right.available_at));
        Ok(entries)
    }

    async fn save_task(&self, task: ExecutionTask) -> Result<(), RepositoryError> {
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.id.0.clone(), task);
        Ok(())
    }

    async fn append_transition(
        &self,
        transition: ExecutionTransitionEvent,
    ) -> Result<(), RepositoryError> {
        let mut transitions = self.transitions.write().await;
        transitions.push(transition);
        transitions.sort_by(|left, right| left.occurred_at.cmp(&right.occurred_at));
        Ok(())
    }

    async fn list_transitions_for_task(
        &self,
        task_id: &ExecutionTaskId,
    ) -> Result<Vec<ExecutionTransitionEvent>, RepositoryError> {
        let transitions = self.transitions.read().await;
        Ok(transitions
            .iter()
            .filter(|transition| transition.task_id == *task_id)
            .cloned()
            .collect())
    }
}

#[derive(Default)]
pub struct InMemoryIdempotencyRepository {
    operations: RwLock<HashMap<String, IdempotencyRecord>>,
}

#[async_trait::async_trait]
impl IdempotencyRepository for InMemoryIdempotencyRepository {
    async fn find_operation(
        &self,
        operation_key: &OperationKey,
    ) -> Result<Option<IdempotencyRecord>, RepositoryError> {
        let operations = self.operations.read().await;
        Ok(operations.get(&operation_key.0).cloned())
    }

    async fn save_operation(&self, record: IdempotencyRecord) -> Result<(), RepositoryError> {
        let mut operations = self.operations.write().await;
        operations.insert(record.operation_key.0.clone(), record);
        Ok(())
    }
}

#[derive(Default)]
pub struct InMemoryPolicyOptimizerRepository {
    candidates: RwLock<HashMap<String, PolicyCandidate>>,
    replay_evaluations: RwLock<HashMap<String, ReplayEvaluation>>,
    approval_decisions: RwLock<HashMap<String, PolicyApprovalDecision>>,
    apply_records: RwLock<HashMap<String, PolicyApplyRecord>>,
    rollback_records: RwLock<HashMap<String, PolicyRollbackRecord>>,
    lifecycle_events: RwLock<Vec<PolicyLifecycleAuditEvent>>,
}

#[async_trait::async_trait]
impl PolicyOptimizerRepository for InMemoryPolicyOptimizerRepository {
    async fn save_candidate(&self, candidate: PolicyCandidate) -> Result<(), RepositoryError> {
        let mut candidates = self.candidates.write().await;
        candidates.insert(candidate.id.0.clone(), candidate);
        Ok(())
    }

    async fn get_candidate(
        &self,
        id: &PolicyCandidateId,
    ) -> Result<Option<PolicyCandidate>, RepositoryError> {
        let candidates = self.candidates.read().await;
        Ok(candidates.get(&id.0).cloned())
    }

    async fn list_candidates_by_status(
        &self,
        status: Option<PolicyCandidateStatus>,
        limit: i32,
    ) -> Result<Vec<PolicyCandidate>, RepositoryError> {
        let candidates = self.candidates.read().await;
        let mut entries: Vec<PolicyCandidate> = candidates
            .values()
            .filter(|candidate| match status.as_ref() {
                Some(expected_status) => candidate.status == *expected_status,
                None => true,
            })
            .cloned()
            .collect();
        entries.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        entries.truncate(limit.max(0) as usize);
        Ok(entries)
    }

    async fn save_replay_evaluation(
        &self,
        replay: ReplayEvaluation,
    ) -> Result<(), RepositoryError> {
        let mut replay_evaluations = self.replay_evaluations.write().await;
        replay_evaluations.insert(replay.id.0.clone(), replay);
        Ok(())
    }

    async fn list_replay_evaluations_for_candidate(
        &self,
        candidate_id: &PolicyCandidateId,
    ) -> Result<Vec<ReplayEvaluation>, RepositoryError> {
        let replay_evaluations = self.replay_evaluations.read().await;
        let mut entries: Vec<ReplayEvaluation> = replay_evaluations
            .values()
            .filter(|replay| replay.candidate_id == *candidate_id)
            .cloned()
            .collect();
        entries.sort_by(|left, right| left.replayed_at.cmp(&right.replayed_at));
        Ok(entries)
    }

    async fn find_replay_evaluation_by_checksum(
        &self,
        candidate_id: &PolicyCandidateId,
        replay_checksum: &str,
    ) -> Result<Option<ReplayEvaluation>, RepositoryError> {
        let replay_evaluations = self.replay_evaluations.read().await;
        Ok(replay_evaluations
            .values()
            .find(|replay| {
                replay.candidate_id == *candidate_id && replay.replay_checksum == replay_checksum
            })
            .cloned())
    }

    async fn save_approval_decision(
        &self,
        decision: PolicyApprovalDecision,
    ) -> Result<(), RepositoryError> {
        let mut approval_decisions = self.approval_decisions.write().await;
        approval_decisions.insert(decision.id.0.clone(), decision);
        Ok(())
    }

    async fn list_approval_decisions_for_candidate(
        &self,
        candidate_id: &PolicyCandidateId,
    ) -> Result<Vec<PolicyApprovalDecision>, RepositoryError> {
        let approval_decisions = self.approval_decisions.read().await;
        let mut entries: Vec<PolicyApprovalDecision> = approval_decisions
            .values()
            .filter(|decision| decision.candidate_id == *candidate_id)
            .cloned()
            .collect();
        entries.sort_by(|left, right| left.decided_at.cmp(&right.decided_at));
        Ok(entries)
    }

    async fn list_stale_approval_decisions(
        &self,
        before: DateTime<Utc>,
    ) -> Result<Vec<PolicyApprovalDecision>, RepositoryError> {
        let approval_decisions = self.approval_decisions.read().await;
        let mut entries: Vec<PolicyApprovalDecision> = approval_decisions
            .values()
            .filter(|decision| {
                decision.is_stale
                    && decision.expires_at.as_ref().is_some_and(|expires_at| *expires_at <= before)
            })
            .cloned()
            .collect();
        entries.sort_by(|left, right| left.decided_at.cmp(&right.decided_at));
        Ok(entries)
    }

    async fn save_apply_record(&self, apply: PolicyApplyRecord) -> Result<(), RepositoryError> {
        let mut apply_records = self.apply_records.write().await;
        apply_records.insert(apply.id.0.clone(), apply);
        Ok(())
    }

    async fn get_apply_record(
        &self,
        id: &PolicyApplyRecordId,
    ) -> Result<Option<PolicyApplyRecord>, RepositoryError> {
        let apply_records = self.apply_records.read().await;
        Ok(apply_records.get(&id.0).cloned())
    }

    async fn save_rollback_record(
        &self,
        rollback: PolicyRollbackRecord,
    ) -> Result<(), RepositoryError> {
        let mut rollback_records = self.rollback_records.write().await;
        rollback_records.insert(rollback.id.0.clone(), rollback);
        Ok(())
    }

    async fn list_rollback_chain_for_apply(
        &self,
        apply_record_id: &PolicyApplyRecordId,
    ) -> Result<Vec<PolicyRollbackRecord>, RepositoryError> {
        let rollback_records = self.rollback_records.read().await;
        let mut entries: Vec<PolicyRollbackRecord> = rollback_records
            .values()
            .filter(|rollback| rollback.apply_record_id == *apply_record_id)
            .cloned()
            .collect();
        entries.sort_by(|left, right| {
            left.rollback_depth
                .cmp(&right.rollback_depth)
                .then(left.rolled_back_at.cmp(&right.rolled_back_at))
        });
        Ok(entries)
    }

    async fn append_lifecycle_audit_event(
        &self,
        event: PolicyLifecycleAuditEvent,
    ) -> Result<(), RepositoryError> {
        let mut lifecycle_events = self.lifecycle_events.write().await;
        lifecycle_events.push(event);
        lifecycle_events.sort_by(|left, right| left.occurred_at.cmp(&right.occurred_at));
        Ok(())
    }

    async fn list_lifecycle_audit_for_candidate(
        &self,
        candidate_id: &PolicyCandidateId,
    ) -> Result<Vec<PolicyLifecycleAuditEvent>, RepositoryError> {
        let lifecycle_events = self.lifecycle_events.read().await;
        Ok(lifecycle_events
            .iter()
            .filter(|event| event.candidate_id == *candidate_id)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;

    use quotey_core::domain::approval::{ApprovalId, ApprovalRequest, ApprovalStatus};
    use quotey_core::domain::execution::{
        ExecutionTask, ExecutionTaskId, ExecutionTaskState, ExecutionTransitionEvent,
        ExecutionTransitionId, IdempotencyRecord, IdempotencyRecordState, OperationKey,
    };
    use quotey_core::domain::optimizer::{
        ApprovalDecisionKind, PolicyApplyRecord, PolicyApplyRecordId, PolicyApprovalDecision,
        PolicyApprovalDecisionId, PolicyCandidate, PolicyCandidateId, PolicyCandidateStatus,
        PolicyLifecycleAuditEvent, PolicyLifecycleAuditEventType, PolicyLifecycleAuditId,
        PolicyRollbackRecord, PolicyRollbackRecordId, ReplayEvaluation, ReplayEvaluationId,
    };
    use quotey_core::domain::product::{Product, ProductId};
    use quotey_core::domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};

    use crate::repositories::{
        ApprovalRepository, ExecutionQueueRepository, IdempotencyRepository,
        InMemoryApprovalRepository, InMemoryExecutionQueueRepository,
        InMemoryIdempotencyRepository, InMemoryPolicyOptimizerRepository,
        InMemoryProductRepository, InMemoryQuoteRepository, PolicyOptimizerRepository,
        ProductRepository, QuoteRepository,
    };

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[tokio::test]
    async fn in_memory_quote_repo_round_trip() {
        let repo = InMemoryQuoteRepository::default();
        let quote = Quote {
            id: QuoteId("Q-1".to_string()),
            status: QuoteStatus::Draft,
            lines: vec![QuoteLine {
                product_id: ProductId("plan-pro".to_string()),
                quantity: 3,
                unit_price: Decimal::new(1000, 2),
            }],
            created_at: Utc::now(),
        };

        repo.save(quote.clone()).await.expect("save quote");
        let found = repo.find_by_id(&quote.id).await.expect("find quote");

        assert_eq!(found, Some(quote));
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[tokio::test]
    async fn in_memory_product_repo_round_trip() {
        let repo = InMemoryProductRepository::default();
        let product = Product {
            id: ProductId("plan-pro".to_string()),
            sku: "PRO-001".to_string(),
            name: "Pro Plan".to_string(),
            active: true,
        };

        repo.save(product.clone()).await.expect("save product");
        let found = repo.find_by_id(&product.id).await.expect("find product");

        assert_eq!(found, Some(product));
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[tokio::test]
    async fn in_memory_approval_repo_round_trip() {
        let repo = InMemoryApprovalRepository::default();
        let approval = ApprovalRequest {
            id: ApprovalId("APR-1".to_string()),
            quote_id: QuoteId("Q-1".to_string()),
            approver_role: "sales_manager".to_string(),
            reason: "Discount above threshold".to_string(),
            status: ApprovalStatus::Pending,
            created_at: Utc::now(),
        };

        repo.save(approval.clone()).await.expect("save approval");
        let found = repo.find_by_id(&approval.id).await.expect("find approval");

        assert_eq!(found, Some(approval));
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[tokio::test]
    async fn in_memory_execution_queue_repo_round_trip() {
        let repo = InMemoryExecutionQueueRepository::default();
        let task = ExecutionTask {
            id: ExecutionTaskId("task-1".to_string()),
            quote_id: QuoteId("Q-1".to_string()),
            operation_kind: "crm.write_quote".to_string(),
            payload_json: "{\"deal_id\":\"D-1\"}".to_string(),
            idempotency_key: OperationKey("op-1".to_string()),
            state: ExecutionTaskState::Queued,
            retry_count: 0,
            max_retries: 3,
            available_at: Utc::now(),
            claimed_by: None,
            claimed_at: None,
            last_error: None,
            result_fingerprint: None,
            state_version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        repo.save_task(task.clone()).await.expect("save task");
        let found = repo.find_task_by_id(&task.id).await.expect("find task");
        assert_eq!(found, Some(task.clone()));

        let transition = ExecutionTransitionEvent {
            id: ExecutionTransitionId("transition-1".to_string()),
            task_id: task.id.clone(),
            quote_id: task.quote_id.clone(),
            from_state: Some(ExecutionTaskState::Queued),
            to_state: ExecutionTaskState::Running,
            transition_reason: "claimed".to_string(),
            error_class: None,
            decision_context_json: "{}".to_string(),
            actor_type: "system".to_string(),
            actor_id: "worker-1".to_string(),
            idempotency_key: Some(task.idempotency_key.clone()),
            correlation_id: "corr-1".to_string(),
            state_version: 2,
            occurred_at: Utc::now(),
        };

        repo.append_transition(transition.clone()).await.expect("append transition");
        let transitions = repo.list_transitions_for_task(&task.id).await.expect("list transitions");
        assert_eq!(transitions, vec![transition]);
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[tokio::test]
    async fn in_memory_idempotency_repo_round_trip() {
        let repo = InMemoryIdempotencyRepository::default();
        let record = IdempotencyRecord {
            operation_key: OperationKey("op-1".to_string()),
            quote_id: QuoteId("Q-1".to_string()),
            operation_kind: "slack.update_message".to_string(),
            payload_hash: "sha256:abcd".to_string(),
            state: IdempotencyRecordState::Reserved,
            attempt_count: 1,
            first_seen_at: Utc::now(),
            last_seen_at: Utc::now(),
            result_snapshot_json: None,
            error_snapshot_json: None,
            expires_at: None,
            correlation_id: "corr-2".to_string(),
            created_by_component: "socket".to_string(),
            updated_by_component: "socket".to_string(),
        };

        repo.save_operation(record.clone()).await.expect("save operation");
        let found = repo.find_operation(&record.operation_key).await.expect("find operation");

        assert_eq!(found, Some(record));
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[tokio::test]
    async fn in_memory_policy_optimizer_repo_supports_lifecycle_queries() {
        let repo = InMemoryPolicyOptimizerRepository::default();
        let candidate = PolicyCandidate {
            id: PolicyCandidateId("cand-1".to_string()),
            base_policy_version: 10,
            proposed_policy_version: 11,
            status: PolicyCandidateStatus::Draft,
            policy_diff_json: "{\"patch\":\"discount_cap=0.17\"}".to_string(),
            provenance_json: "{\"source\":\"outcome_window\"}".to_string(),
            confidence_score: 0.8,
            cohort_scope_json: "{\"segment\":\"enterprise\"}".to_string(),
            latest_replay_checksum: None,
            idempotency_key: "idem-cand-1".to_string(),
            created_by_actor_id: "agent-1".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            review_ready_at: None,
            approved_at: None,
            applied_at: None,
            monitoring_started_at: None,
            rolled_back_at: None,
        };
        repo.save_candidate(candidate.clone()).await.expect("save candidate");

        let replay = ReplayEvaluation {
            id: ReplayEvaluationId("replay-1".to_string()),
            candidate_id: candidate.id.clone(),
            replay_checksum: "sha256:replay-1".to_string(),
            engine_version: "core-1.0.0".to_string(),
            cohort_scope_json: "{}".to_string(),
            cohort_size: 5,
            projected_margin_delta_bps: 25,
            projected_win_rate_delta_bps: 10,
            projected_approval_latency_delta_seconds: -60,
            blast_radius_score: 0.2,
            hard_violation_count: 0,
            risk_flags_json: "[]".to_string(),
            deterministic_pass: true,
            idempotency_key: "idem-replay-1".to_string(),
            replayed_at: Utc::now(),
        };
        repo.save_replay_evaluation(replay.clone()).await.expect("save replay");

        let approval = PolicyApprovalDecision {
            id: PolicyApprovalDecisionId("approval-1".to_string()),
            candidate_id: candidate.id.clone(),
            replay_evaluation_id: Some(replay.id.clone()),
            decision: ApprovalDecisionKind::Approved,
            reason: Some("looks good".to_string()),
            decision_payload_json: "{}".to_string(),
            actor_id: "U-APPROVER".to_string(),
            actor_role: "vp_sales".to_string(),
            channel_ref: Some("C1/T1".to_string()),
            signature: Some("sig-approval".to_string()),
            signature_key_id: Some("kms-1".to_string()),
            idempotency_key: "idem-approval-1".to_string(),
            decided_at: Utc::now(),
            expires_at: Some(Utc::now()),
            is_stale: true,
        };
        repo.save_approval_decision(approval.clone()).await.expect("save approval");

        let apply_record = PolicyApplyRecord {
            id: PolicyApplyRecordId("apply-1".to_string()),
            candidate_id: candidate.id.clone(),
            approval_decision_id: approval.id.clone(),
            prior_policy_version: 10,
            applied_policy_version: 11,
            replay_checksum: replay.replay_checksum.clone(),
            apply_signature: "sig-apply".to_string(),
            signature_key_id: "kms-2".to_string(),
            actor_id: "U-OPS".to_string(),
            idempotency_key: "idem-apply-1".to_string(),
            verification_checksum: "sha256:apply".to_string(),
            apply_audit_json: "{}".to_string(),
            applied_at: Utc::now(),
        };
        repo.save_apply_record(apply_record.clone()).await.expect("save apply");

        let rollback = PolicyRollbackRecord {
            id: PolicyRollbackRecordId("rollback-1".to_string()),
            candidate_id: candidate.id.clone(),
            apply_record_id: apply_record.id.clone(),
            rollback_target_version: 10,
            rollback_reason: "drift alert".to_string(),
            verification_checksum: "sha256:rb".to_string(),
            rollback_signature: "sig-rb".to_string(),
            signature_key_id: "kms-3".to_string(),
            actor_id: "U-OPS".to_string(),
            idempotency_key: "idem-rb-1".to_string(),
            parent_rollback_id: None,
            rollback_depth: 0,
            rollback_metadata_json: "{}".to_string(),
            rolled_back_at: Utc::now(),
        };
        repo.save_rollback_record(rollback.clone()).await.expect("save rollback");

        let audit_event = PolicyLifecycleAuditEvent {
            id: PolicyLifecycleAuditId("audit-1".to_string()),
            candidate_id: candidate.id.clone(),
            replay_evaluation_id: Some(replay.id.clone()),
            approval_decision_id: Some(approval.id.clone()),
            apply_record_id: Some(apply_record.id.clone()),
            rollback_record_id: Some(rollback.id.clone()),
            event_type: PolicyLifecycleAuditEventType::Applied,
            event_payload_json: "{}".to_string(),
            actor_type: "agent".to_string(),
            actor_id: "optimizer".to_string(),
            correlation_id: "corr-1".to_string(),
            idempotency_key: Some("idem-audit-1".to_string()),
            occurred_at: Utc::now(),
        };
        repo.append_lifecycle_audit_event(audit_event.clone()).await.expect("append audit event");

        let candidates = repo
            .list_candidates_by_status(Some(PolicyCandidateStatus::Draft), 10)
            .await
            .expect("list candidates");
        assert_eq!(candidates, vec![candidate]);

        let replay_lookup = repo
            .find_replay_evaluation_by_checksum(&replay.candidate_id, &replay.replay_checksum)
            .await
            .expect("find replay by checksum");
        assert_eq!(replay_lookup, Some(replay));

        let stale_approvals =
            repo.list_stale_approval_decisions(Utc::now()).await.expect("list stale approvals");
        assert_eq!(stale_approvals, vec![approval]);

        let apply_lookup = repo.get_apply_record(&apply_record.id).await.expect("get apply record");
        assert_eq!(apply_lookup, Some(apply_record.clone()));

        let rollback_chain = repo
            .list_rollback_chain_for_apply(&apply_record.id)
            .await
            .expect("list rollback chain");
        assert_eq!(rollback_chain, vec![rollback]);

        let lifecycle_events = repo
            .list_lifecycle_audit_for_candidate(&PolicyCandidateId("cand-1".to_string()))
            .await
            .expect("list lifecycle events");
        assert_eq!(lifecycle_events, vec![audit_event]);
    }
}
