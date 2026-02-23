use quotey_core::domain::product::{Product, ProductId};

use super::{ProductRepository, RepositoryError};
use crate::DbPool;

pub struct SqlProductRepository {
    pool: DbPool,
}

impl SqlProductRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ProductRepository for SqlProductRepository {
    async fn find_by_id(&self, _id: &ProductId) -> Result<Option<Product>, RepositoryError> {
        let _pool = &self.pool;
        Ok(None)
    }

    async fn save(&self, _product: Product) -> Result<(), RepositoryError> {
        let _pool = &self.pool;
        Ok(())
    }
}
