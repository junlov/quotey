use quotey_core::domain::customer::{Customer, CustomerId};

use crate::DbPool;

pub struct SqlCustomerRepository {
    _pool: DbPool,
}

impl SqlCustomerRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { _pool: pool }
    }

    pub async fn find_by_id(&self, _id: &CustomerId) -> Result<Option<Customer>, sqlx::Error> {
        let _ = &_id;
        let _ = &self._pool;
        Err(sqlx::Error::RowNotFound)
    }
}
