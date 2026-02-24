use quotey_core::domain::product::{Product, ProductId};

use super::{ProductRepository, RepositoryError};
use crate::DbPool;

pub struct SqlProductRepository {
    _pool: DbPool,
}

impl SqlProductRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { _pool: pool }
    }
}

#[async_trait::async_trait]
impl ProductRepository for SqlProductRepository {
    async fn find_by_id(&self, _id: &ProductId) -> Result<Option<Product>, RepositoryError> {
        Err(RepositoryError::Decode(
            "SqlProductRepository is unavailable: product catalog tables are not present in current schema"
                .to_string(),
        ))
    }

    async fn save(&self, _product: Product) -> Result<(), RepositoryError> {
        Err(RepositoryError::Decode(
            "SqlProductRepository is unavailable: product catalog tables are not present in current schema"
                .to_string(),
        ))
    }
}
