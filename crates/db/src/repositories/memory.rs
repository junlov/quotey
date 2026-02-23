use std::collections::HashMap;

use tokio::sync::RwLock;

use quotey_core::domain::approval::{ApprovalId, ApprovalRequest};
use quotey_core::domain::execution::{
    ExecutionTask, ExecutionTaskId, ExecutionTaskState, ExecutionTransitionEvent,
    IdempotencyRecord, OperationKey,
};
use quotey_core::domain::product::{Product, ProductId};
use quotey_core::domain::quote::{Quote, QuoteId};

use super::{
    ApprovalRepository, ExecutionQueueRepository, IdempotencyRepository, ProductRepository,
    QuoteRepository, RepositoryError,
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

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;

    use quotey_core::domain::approval::{ApprovalId, ApprovalRequest, ApprovalStatus};
    use quotey_core::domain::execution::{
        ExecutionTask, ExecutionTaskId, ExecutionTaskState, ExecutionTransitionEvent,
        ExecutionTransitionId, IdempotencyRecord, IdempotencyRecordState, OperationKey,
    };
    use quotey_core::domain::product::{Product, ProductId};
    use quotey_core::domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};

    use crate::repositories::{
        ApprovalRepository, ExecutionQueueRepository, IdempotencyRepository,
        InMemoryApprovalRepository, InMemoryExecutionQueueRepository,
        InMemoryIdempotencyRepository, InMemoryProductRepository, InMemoryQuoteRepository,
        ProductRepository, QuoteRepository,
    };

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
}
