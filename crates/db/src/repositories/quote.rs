use quotey_core::domain::quote::{Quote, QuoteId};

use super::{QuoteRepository, RepositoryError};
use crate::DbPool;

pub struct SqlQuoteRepository {
    pool: DbPool,
}

impl SqlQuoteRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl QuoteRepository for SqlQuoteRepository {
    async fn find_by_id(&self, _id: &QuoteId) -> Result<Option<Quote>, RepositoryError> {
        let _pool = &self.pool;
        Ok(None)
    }

    async fn save(&self, _quote: Quote) -> Result<(), RepositoryError> {
        let _pool = &self.pool;
        Ok(())
    }
}
