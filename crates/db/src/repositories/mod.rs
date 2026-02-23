use async_trait::async_trait;
use thiserror::Error;

use quotey_core::domain::approval::{ApprovalId, ApprovalRequest};
use quotey_core::domain::product::{Product, ProductId};
use quotey_core::domain::quote::{Quote, QuoteId};

pub mod approval;
pub mod customer;
pub mod memory;
pub mod product;
pub mod quote;

pub use approval::SqlApprovalRepository;
pub use customer::SqlCustomerRepository;
pub use memory::{InMemoryApprovalRepository, InMemoryProductRepository, InMemoryQuoteRepository};
pub use product::SqlProductRepository;
pub use quote::SqlQuoteRepository;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
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
