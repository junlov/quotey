use quotey_core::domain::customer::{Customer, CustomerId};

use crate::DbPool;

pub struct SqlCustomerRepository {
    pool: DbPool,
}

impl SqlCustomerRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn find_by_id(&self, _id: &CustomerId) -> Result<Option<Customer>, sqlx::Error> {
        let _pool = &self.pool;
        Ok(None)
    }
}
