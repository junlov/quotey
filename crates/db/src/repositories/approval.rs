use chrono::{DateTime, Utc};
use sqlx::Row;

use quotey_core::domain::approval::{ApprovalId, ApprovalRequest, ApprovalStatus};
use quotey_core::domain::quote::QuoteId;

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

fn parse_status(s: &str) -> ApprovalStatus {
    match s {
        "approved" => ApprovalStatus::Approved,
        "rejected" => ApprovalStatus::Rejected,
        "escalated" => ApprovalStatus::Escalated,
        _ => ApprovalStatus::Pending,
    }
}

pub fn approval_status_as_str(status: &ApprovalStatus) -> &'static str {
    match status {
        ApprovalStatus::Pending => "pending",
        ApprovalStatus::Approved => "approved",
        ApprovalStatus::Rejected => "rejected",
        ApprovalStatus::Escalated => "escalated",
    }
}

fn row_to_approval(row: &sqlx::sqlite::SqliteRow) -> Result<ApprovalRequest, RepositoryError> {
    let id: String = row.try_get("id").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let quote_id: String =
        row.try_get("quote_id").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let approver_role: String =
        row.try_get("approver_role").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let reason: String =
        row.try_get("reason").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let justification: String =
        row.try_get("justification").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let status_str: String =
        row.try_get("status").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let requested_by: String =
        row.try_get("requested_by").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let expires_at_str: Option<String> =
        row.try_get("expires_at").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let created_at_str: String =
        row.try_get("created_at").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let updated_at_str: String =
        row.try_get("updated_at").map_err(|e| RepositoryError::Decode(e.to_string()))?;

    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let expires_at = expires_at_str
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    Ok(ApprovalRequest {
        id: ApprovalId(id),
        quote_id: QuoteId(quote_id),
        approver_role,
        reason,
        justification,
        status: parse_status(&status_str),
        requested_by,
        expires_at,
        created_at,
        updated_at,
    })
}

#[async_trait::async_trait]
impl ApprovalRepository for SqlApprovalRepository {
    async fn find_by_id(
        &self,
        id: &ApprovalId,
    ) -> Result<Option<ApprovalRequest>, RepositoryError> {
        let row = sqlx::query(
            "SELECT id, quote_id, approver_role, reason, justification, status,
                    requested_by, expires_at, created_at, updated_at
             FROM approval_request WHERE id = ?",
        )
        .bind(&id.0)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(ref r) => Ok(Some(row_to_approval(r)?)),
            None => Ok(None),
        }
    }

    async fn save(&self, approval: ApprovalRequest) -> Result<(), RepositoryError> {
        let status_str = approval_status_as_str(&approval.status);
        let expires_at_str = approval.expires_at.map(|dt| dt.to_rfc3339());

        sqlx::query(
            "INSERT INTO approval_request (id, quote_id, approver_role, reason, justification,
                                           status, requested_by, expires_at, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                 approver_role = excluded.approver_role,
                 reason = excluded.reason,
                 justification = excluded.justification,
                 status = excluded.status,
                 requested_by = excluded.requested_by,
                 expires_at = excluded.expires_at,
                 updated_at = excluded.updated_at",
        )
        .bind(&approval.id.0)
        .bind(&approval.quote_id.0)
        .bind(&approval.approver_role)
        .bind(&approval.reason)
        .bind(&approval.justification)
        .bind(status_str)
        .bind(&approval.requested_by)
        .bind(&expires_at_str)
        .bind(approval.created_at.to_rfc3339())
        .bind(approval.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn find_by_quote_id(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Vec<ApprovalRequest>, RepositoryError> {
        let rows: Vec<sqlx::sqlite::SqliteRow> = sqlx::query(
            "SELECT id, quote_id, approver_role, reason, justification, status,
                    requested_by, expires_at, created_at, updated_at
             FROM approval_request WHERE quote_id = ? ORDER BY created_at DESC",
        )
        .bind(&quote_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_approval).collect::<Result<Vec<_>, _>>()
    }

    async fn list_pending(
        &self,
        approver_role: Option<&str>,
        limit: u32,
    ) -> Result<Vec<ApprovalRequest>, RepositoryError> {
        let rows: Vec<sqlx::sqlite::SqliteRow> = if let Some(role) = approver_role {
            sqlx::query(
                "SELECT id, quote_id, approver_role, reason, justification, status,
                        requested_by, expires_at, created_at, updated_at
                 FROM approval_request
                 WHERE status = 'pending' AND approver_role = ?
                 ORDER BY created_at ASC
                 LIMIT ?",
            )
            .bind(role)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, quote_id, approver_role, reason, justification, status,
                        requested_by, expires_at, created_at, updated_at
                 FROM approval_request
                 WHERE status = 'pending'
                 ORDER BY created_at ASC
                 LIMIT ?",
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        rows.iter().map(row_to_approval).collect::<Result<Vec<_>, _>>()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;

    use quotey_core::domain::approval::{ApprovalId, ApprovalRequest, ApprovalStatus};
    use quotey_core::domain::product::ProductId;
    use quotey_core::domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};

    use super::SqlApprovalRepository;
    use crate::repositories::{ApprovalRepository, QuoteRepository, SqlQuoteRepository};
    use crate::{connect_with_settings, migrations};

    async fn setup() -> sqlx::SqlitePool {
        let pool = connect_with_settings("sqlite::memory:", 1, 30).await.expect("connect");
        migrations::run_pending(&pool).await.expect("migrations");
        pool
    }

    /// Insert a parent quote record so that FK constraints are satisfied.
    async fn insert_quote(pool: &sqlx::SqlitePool, quote_id: &str) {
        let repo = SqlQuoteRepository::new(pool.clone());
        let now = Utc::now();
        let quote = Quote {
            id: QuoteId(quote_id.to_string()),
            version: 1,
            status: QuoteStatus::Draft,
            account_id: None,
            deal_id: None,
            currency: "USD".to_string(),
            term_months: None,
            start_date: None,
            end_date: None,
            valid_until: None,
            notes: None,
            created_by: "test".to_string(),
            lines: vec![QuoteLine {
                product_id: ProductId("plan-pro".to_string()),
                quantity: 1,
                unit_price: Decimal::new(1000, 2),
                discount_pct: 0.0,
                notes: None,
            }],
            created_at: now,
            updated_at: now,
        };
        repo.save(quote).await.expect("insert parent quote");
    }

    fn sample_approval(id: &str, quote_id: &str) -> ApprovalRequest {
        let now = Utc::now();
        ApprovalRequest {
            id: ApprovalId(id.to_string()),
            quote_id: QuoteId(quote_id.to_string()),
            approver_role: "sales_manager".to_string(),
            reason: "Discount exceeds threshold".to_string(),
            justification: "Loyal customer, 3-year history".to_string(),
            status: ApprovalStatus::Pending,
            requested_by: "agent:mcp".to_string(),
            expires_at: Some(now + chrono::Duration::hours(4)),
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn save_and_find_by_id() {
        let pool = setup().await;
        insert_quote(&pool, "Q-2026-0001").await;

        let repo = SqlApprovalRepository::new(pool);
        let approval = sample_approval("APR-001", "Q-2026-0001");

        repo.save(approval.clone()).await.expect("save");
        let found = repo.find_by_id(&ApprovalId("APR-001".to_string())).await.expect("find");
        let found = found.expect("should exist");

        assert_eq!(found.id, approval.id);
        assert_eq!(found.quote_id, approval.quote_id);
        assert_eq!(found.approver_role, "sales_manager");
        assert_eq!(found.status, ApprovalStatus::Pending);
    }

    #[tokio::test]
    async fn find_by_quote_id_returns_related_approvals() {
        let pool = setup().await;
        insert_quote(&pool, "Q-100").await;
        insert_quote(&pool, "Q-200").await;

        let repo = SqlApprovalRepository::new(pool);

        repo.save(sample_approval("APR-001", "Q-100")).await.expect("save 1");
        repo.save(sample_approval("APR-002", "Q-100")).await.expect("save 2");
        repo.save(sample_approval("APR-003", "Q-200")).await.expect("save 3");

        let results =
            repo.find_by_quote_id(&QuoteId("Q-100".to_string())).await.expect("find by quote");
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn list_pending_filters_by_status_and_role() {
        let pool = setup().await;
        insert_quote(&pool, "Q-100").await;
        insert_quote(&pool, "Q-200").await;
        insert_quote(&pool, "Q-300").await;

        let repo = SqlApprovalRepository::new(pool);

        let mut apr1 = sample_approval("APR-001", "Q-100");
        apr1.approver_role = "sales_manager".to_string();
        repo.save(apr1).await.expect("save 1");

        let mut apr2 = sample_approval("APR-002", "Q-200");
        apr2.approver_role = "vp_finance".to_string();
        repo.save(apr2).await.expect("save 2");

        let mut apr3 = sample_approval("APR-003", "Q-300");
        apr3.status = ApprovalStatus::Approved;
        repo.save(apr3).await.expect("save 3");

        let all_pending = repo.list_pending(None, 100).await.expect("list all");
        assert_eq!(all_pending.len(), 2);

        let sm_pending = repo.list_pending(Some("sales_manager"), 100).await.expect("list sm");
        assert_eq!(sm_pending.len(), 1);
        assert_eq!(sm_pending[0].id.0, "APR-001");
    }

    #[tokio::test]
    async fn save_upserts_on_conflict() {
        let pool = setup().await;
        insert_quote(&pool, "Q-100").await;

        let repo = SqlApprovalRepository::new(pool);

        let approval = sample_approval("APR-001", "Q-100");
        repo.save(approval.clone()).await.expect("save");

        let mut updated = approval;
        updated.status = ApprovalStatus::Approved;
        updated.updated_at = Utc::now();
        repo.save(updated).await.expect("upsert");

        let found = repo.find_by_id(&ApprovalId("APR-001".to_string())).await.expect("find");
        assert_eq!(found.unwrap().status, ApprovalStatus::Approved);
    }
}
