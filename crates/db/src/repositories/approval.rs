use quotey_core::domain::approval::{ApprovalId, ApprovalRequest};

use super::{ApprovalRepository, RepositoryError};
use crate::DbPool;

pub struct SqlApprovalRepository {
    _pool: DbPool,
}

impl SqlApprovalRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { _pool: pool }
    }
}

#[async_trait::async_trait]
impl ApprovalRepository for SqlApprovalRepository {
    async fn find_by_id(
        &self,
        _id: &ApprovalId,
    ) -> Result<Option<ApprovalRequest>, RepositoryError> {
        Err(RepositoryError::Decode(
            "SqlApprovalRepository is unavailable: approval tables are not present in current schema".to_string(),
        ))
    }

    async fn save(&self, _approval: ApprovalRequest) -> Result<(), RepositoryError> {
        Err(RepositoryError::Decode(
            "SqlApprovalRepository is unavailable: approval tables are not present in current schema".to_string(),
        ))
    }
}
