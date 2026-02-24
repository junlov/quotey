use async_trait::async_trait;
use thiserror::Error;

use quotey_core::domain::approval::{ApprovalId, ApprovalRequest};
use quotey_core::domain::execution::{
    ExecutionTask, ExecutionTaskId, ExecutionTaskState, ExecutionTransitionEvent,
    IdempotencyRecord, OperationKey,
};
use quotey_core::domain::product::{Product, ProductId};
use quotey_core::domain::quote::{Quote, QuoteId};

pub mod approval;
pub mod customer;
pub mod execution_queue;
pub mod explanation;
pub mod memory;
pub mod optimizer;
pub mod precedent;
pub mod product;
pub mod quote;
pub mod simulation;

pub use approval::SqlApprovalRepository;
pub use customer::SqlCustomerRepository;
pub use execution_queue::SqlExecutionQueueRepository;
pub use explanation::{ExplanationRepository, SqlExplanationRepository};
pub use memory::{
    InMemoryApprovalRepository, InMemoryExecutionQueueRepository, InMemoryIdempotencyRepository,
    InMemoryPolicyOptimizerRepository, InMemoryProductRepository, InMemoryQuoteRepository,
};
pub use optimizer::{PolicyOptimizerRepository, SqlPolicyOptimizerRepository};
pub use precedent::{PrecedentRepository, SqlPrecedentRepository};
pub use product::SqlProductRepository;
pub use quote::SqlQuoteRepository;
pub use simulation::{
    ScenarioAuditEventRecord, ScenarioDeltaRecord, ScenarioRepository, ScenarioRunRecord,
    ScenarioVariantRecord, SqlScenarioRepository,
};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("decode error: {0}")]
    Decode(String),
}

#[async_trait]
pub trait QuoteRepository: Send + Sync {
    async fn find_by_id(&self, id: &QuoteId) -> Result<Option<Quote>, RepositoryError>;
    async fn save(&self, quote: Quote) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait ProductRepository: Send + Sync {
    async fn find_by_id(&self, id: &ProductId) -> Result<Option<Product>, RepositoryError>;
    async fn save(&self, product: Product) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait ApprovalRepository: Send + Sync {
    async fn find_by_id(&self, id: &ApprovalId)
        -> Result<Option<ApprovalRequest>, RepositoryError>;
    async fn save(&self, approval: ApprovalRequest) -> Result<(), RepositoryError>;
}

#[async_trait]
pub trait ExecutionQueueRepository: Send + Sync {
    async fn find_task_by_id(
        &self,
        id: &ExecutionTaskId,
    ) -> Result<Option<ExecutionTask>, RepositoryError>;

    async fn list_tasks_for_quote(
        &self,
        quote_id: &QuoteId,
        state: Option<ExecutionTaskState>,
    ) -> Result<Vec<ExecutionTask>, RepositoryError>;

    async fn save_task(&self, task: ExecutionTask) -> Result<(), RepositoryError>;

    async fn append_transition(
        &self,
        transition: ExecutionTransitionEvent,
    ) -> Result<(), RepositoryError>;

    async fn list_transitions_for_task(
        &self,
        task_id: &ExecutionTaskId,
    ) -> Result<Vec<ExecutionTransitionEvent>, RepositoryError>;
}

#[async_trait]
pub trait IdempotencyRepository: Send + Sync {
    async fn find_operation(
        &self,
        operation_key: &OperationKey,
    ) -> Result<Option<IdempotencyRecord>, RepositoryError>;

    async fn save_operation(&self, record: IdempotencyRecord) -> Result<(), RepositoryError>;
}
