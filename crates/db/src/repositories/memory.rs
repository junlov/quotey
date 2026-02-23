use std::collections::HashMap;

use tokio::sync::RwLock;

use quotey_core::domain::approval::{ApprovalId, ApprovalRequest};
use quotey_core::domain::product::{Product, ProductId};
use quotey_core::domain::quote::{Quote, QuoteId};

use super::{ApprovalRepository, ProductRepository, QuoteRepository, RepositoryError};

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

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;

    use quotey_core::domain::approval::{ApprovalId, ApprovalRequest, ApprovalStatus};
    use quotey_core::domain::product::{Product, ProductId};
    use quotey_core::domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};

    use crate::repositories::{
        ApprovalRepository, InMemoryApprovalRepository, InMemoryProductRepository,
        InMemoryQuoteRepository, ProductRepository, QuoteRepository,
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
}
