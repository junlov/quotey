use quotey_core::domain::approval::{ApprovalId, ApprovalRequest};

use super::{ApprovalRepository, RepositoryError};
use crate::DbPool;

pub struct SqlApprovalRepository {
    pool: DbPool,
}

impl SqlApprovalRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ApprovalRepository for SqlApprovalRepository {
    async fn find_by_id(
        &self,
        _id: &ApprovalId,
    ) -> Result<Option<ApprovalRequest>, RepositoryError> {
        let _pool = &self.pool;
        Ok(None)
    }

    async fn save(&self, _approval: ApprovalRequest) -> Result<(), RepositoryError> {
        let _pool = &self.pool;
        Ok(())
    }
}
