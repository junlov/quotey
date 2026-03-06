use async_trait::async_trait;
use thiserror::Error;

use quotey_core::domain::approval::{ApprovalId, ApprovalRequest};
use quotey_core::domain::execution::{
    ExecutionTask, ExecutionTaskId, ExecutionTaskState, ExecutionTransitionEvent,
    IdempotencyRecord, OperationKey,
};
use quotey_core::domain::org_settings::OrgSetting;
use quotey_core::domain::product::{Product, ProductId};
use quotey_core::domain::quote::{Quote, QuoteId};
use quotey_core::domain::quote_comment::QuoteComment;
use quotey_core::domain::quote_lock::{LockConflict, LockInfo};
use quotey_core::domain::sales_rep::{SalesRep, SalesRepId};
use quotey_core::suggestions::{ProductAcceptanceRate, SuggestionFeedback};

pub mod analytics;
pub mod anomaly_override;
pub mod approval;
pub mod audit;
pub mod customer;
pub mod dialogue;
pub mod execution_queue;
pub mod explanation;
pub mod memory;
pub mod negotiation;
pub mod optimizer;
pub mod org_settings;
pub mod precedent;
pub mod pricing_snapshot;
pub mod product;
pub mod quote;
pub mod quote_comment;
pub mod quote_lock;
pub mod sales_rep;
pub mod simulation;
pub mod suggestion_feedback;

pub use analytics::{AnalyticsQueryError, SqlAnalyticsQueryBuilder};
pub use anomaly_override::SqlAnomalyOverrideRepository;
pub use approval::SqlApprovalRepository;
pub use audit::SqlAuditEventRepository;
pub use customer::SqlCustomerRepository;
pub use dialogue::{DialogueSessionRepository, SqlDialogueSessionRepository};
pub use execution_queue::SqlExecutionQueueRepository;
pub use explanation::{ExplanationRepository, SqlExplanationRepository};
pub use memory::{
    InMemoryApprovalRepository, InMemoryExecutionQueueRepository, InMemoryIdempotencyRepository,
    InMemoryPolicyOptimizerRepository, InMemoryProductRepository, InMemoryQuoteRepository,
    InMemorySuggestionFeedbackRepository,
};
pub use negotiation::SqlNegotiationRepository;
pub use optimizer::{PolicyOptimizerRepository, SqlPolicyOptimizerRepository};
pub use org_settings::SqlOrgSettingsRepository;
pub use precedent::{PrecedentRepository, SqlPrecedentRepository};
pub use pricing_snapshot::SqlPricingSnapshotRepository;
pub use product::SqlProductRepository;
pub use quote::SqlQuoteRepository;
pub use quote_comment::SqlQuoteCommentRepository;
pub use quote_lock::SqlQuoteLockRepository;
pub use sales_rep::SqlSalesRepRepository;
pub use simulation::{
    ScenarioAuditEventRecord, ScenarioDeltaRecord, ScenarioRepository, ScenarioRunRecord,
    ScenarioVariantRecord, SqlScenarioRepository,
};
pub use suggestion_feedback::SqlSuggestionFeedbackRepository;

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
    async fn list(
        &self,
        account_id: Option<&str>,
        status: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Quote>, RepositoryError>;
}

#[async_trait]
pub trait ProductRepository: Send + Sync {
    async fn find_by_id(&self, id: &ProductId) -> Result<Option<Product>, RepositoryError>;
    async fn save(&self, product: Product) -> Result<(), RepositoryError>;
    async fn search(
        &self,
        query: &str,
        active_only: bool,
        limit: u32,
    ) -> Result<Vec<Product>, RepositoryError>;
    async fn list_by_family(&self, family_id: &str) -> Result<Vec<Product>, RepositoryError>;
}

#[async_trait]
pub trait ApprovalRepository: Send + Sync {
    async fn find_by_id(&self, id: &ApprovalId)
        -> Result<Option<ApprovalRequest>, RepositoryError>;
    async fn save(&self, approval: ApprovalRequest) -> Result<(), RepositoryError>;
    async fn find_by_quote_id(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Vec<ApprovalRequest>, RepositoryError>;
    async fn list_pending(
        &self,
        approver_role: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ApprovalRequest>, RepositoryError>;
}

#[async_trait]
pub trait SalesRepRepository: Send + Sync {
    async fn find_by_id(&self, id: &SalesRepId) -> Result<Option<SalesRep>, RepositoryError>;
    async fn find_by_external_user_ref(
        &self,
        external_user_ref: &str,
    ) -> Result<Option<SalesRep>, RepositoryError>;
    async fn find_by_email(&self, email: &str) -> Result<Option<SalesRep>, RepositoryError>;
    async fn save(&self, sales_rep: SalesRep) -> Result<(), RepositoryError>;
    async fn list_by_role(
        &self,
        role: &str,
        active_only: bool,
    ) -> Result<Vec<SalesRep>, RepositoryError>;
    async fn list_by_team(
        &self,
        team_id: &str,
        active_only: bool,
    ) -> Result<Vec<SalesRep>, RepositoryError>;
    async fn list_active(&self, limit: u32) -> Result<Vec<SalesRep>, RepositoryError>;
}

#[async_trait]
pub trait QuoteCommentRepository: Send + Sync {
    async fn add_comment(&self, comment: QuoteComment) -> Result<(), RepositoryError>;
    async fn list_by_quote(
        &self,
        quote_id: &str,
        limit: u32,
    ) -> Result<Vec<QuoteComment>, RepositoryError>;
    async fn count_by_quote(&self, quote_id: &str) -> Result<i64, RepositoryError>;
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

#[async_trait]
pub trait SuggestionFeedbackRepository: Send + Sync {
    async fn record_shown(&self, feedbacks: Vec<SuggestionFeedback>)
        -> Result<(), RepositoryError>;
    async fn record_clicked(
        &self,
        request_id: &str,
        product_id: &str,
    ) -> Result<(), RepositoryError>;
    async fn record_added(&self, request_id: &str, product_id: &str)
        -> Result<(), RepositoryError>;
    async fn record_hidden(
        &self,
        request_id: &str,
        product_id: &str,
    ) -> Result<(), RepositoryError>;
    async fn find_by_product(
        &self,
        product_id: &str,
        limit: u32,
    ) -> Result<Vec<SuggestionFeedback>, RepositoryError>;
    async fn acceptance_rate(
        &self,
        product_id: &str,
    ) -> Result<Option<ProductAcceptanceRate>, RepositoryError>;
}

#[async_trait]
pub trait QuoteLockRepository: Send + Sync {
    /// Acquire a lock. Returns Err with LockConflict if already locked by someone else.
    async fn lock_quote(
        &self,
        quote_id: &str,
        actor_id: &str,
        duration_minutes: u32,
    ) -> Result<(), LockConflict>;

    /// Release a lock. Only the lock owner can release.
    async fn unlock_quote(&self, quote_id: &str, actor_id: &str) -> Result<(), RepositoryError>;

    /// Force-unlock regardless of owner (admin action).
    async fn force_unlock(&self, quote_id: &str) -> Result<(), RepositoryError>;

    /// Check current lock status (returns None for unlocked or expired locks).
    async fn check_lock(&self, quote_id: &str) -> Result<Option<LockInfo>, RepositoryError>;
}

#[async_trait]
pub trait OrgSettingsRepository: Send + Sync {
    /// Look up a single org setting by key.
    async fn get(&self, key: &str) -> Result<Option<OrgSetting>, RepositoryError>;
    /// Create or replace an org setting, recording the actor and timestamp.
    async fn set(
        &self,
        key: &str,
        value_json: &str,
        actor: Option<&str>,
    ) -> Result<(), RepositoryError>;
    /// Return all org settings ordered by key.
    async fn list_all(&self) -> Result<Vec<OrgSetting>, RepositoryError>;
}
