use std::str::FromStr;

use sqlx::Row;

use quotey_core::domain::quote_comment::{AuthorType, QuoteComment};

use super::{QuoteCommentRepository, RepositoryError};
use crate::DbPool;

pub struct SqlQuoteCommentRepository {
    pool: DbPool,
}

impl SqlQuoteCommentRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl QuoteCommentRepository for SqlQuoteCommentRepository {
    async fn add_comment(&self, comment: QuoteComment) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO quote_comment (id, quote_id, author_type, author_id, body, metadata_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&comment.id)
        .bind(&comment.quote_id)
        .bind(comment.author_type.as_str())
        .bind(&comment.author_id)
        .bind(&comment.body)
        .bind(&comment.metadata_json)
        .bind(comment.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_by_quote(
        &self,
        quote_id: &str,
        limit: u32,
    ) -> Result<Vec<QuoteComment>, RepositoryError> {
        let rows: Vec<sqlx::sqlite::SqliteRow> = sqlx::query(
            "SELECT id, quote_id, author_type, author_id, body, metadata_json, created_at
             FROM quote_comment
             WHERE quote_id = ?
             ORDER BY created_at ASC
             LIMIT ?",
        )
        .bind(quote_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|row| row_to_comment(&row)).collect()
    }

    async fn count_by_quote(&self, quote_id: &str) -> Result<i64, RepositoryError> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM quote_comment WHERE quote_id = ?")
                .bind(quote_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(count)
    }
}

fn row_to_comment(row: &sqlx::sqlite::SqliteRow) -> Result<QuoteComment, RepositoryError> {
    let author_type_str: String = row
        .try_get("author_type")
        .map_err(|e| RepositoryError::Decode(format!("author_type: {e}")))?;
    let author_type = AuthorType::from_str(&author_type_str)
        .map_err(|e| RepositoryError::Decode(format!("author_type parse: {e}")))?;

    let created_at_str: String = row
        .try_get("created_at")
        .map_err(|e| RepositoryError::Decode(format!("created_at: {e}")))?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| RepositoryError::Decode(format!("created_at parse: {e}")))?;

    Ok(QuoteComment {
        id: row.try_get("id").map_err(|e| RepositoryError::Decode(format!("id: {e}")))?,
        quote_id: row
            .try_get("quote_id")
            .map_err(|e| RepositoryError::Decode(format!("quote_id: {e}")))?,
        author_type,
        author_id: row
            .try_get("author_id")
            .map_err(|e| RepositoryError::Decode(format!("author_id: {e}")))?,
        body: row.try_get("body").map_err(|e| RepositoryError::Decode(format!("body: {e}")))?,
        metadata_json: row
            .try_get("metadata_json")
            .map_err(|e| RepositoryError::Decode(format!("metadata_json: {e}")))?,
        created_at,
    })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use quotey_core::domain::product::ProductId;
    use quotey_core::domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};
    use quotey_core::domain::quote_comment::{AuthorType, QuoteComment};
    use rust_decimal::Decimal;

    use super::SqlQuoteCommentRepository;
    use crate::repositories::{QuoteCommentRepository, QuoteRepository, SqlQuoteRepository};

    type TestResult<T = ()> = Result<T, String>;

    async fn setup_pool() -> TestResult<crate::DbPool> {
        let pool = crate::connect_with_settings("sqlite::memory:", 1, 30)
            .await
            .map_err(|e| format!("connect: {e}"))?;
        crate::migrations::run_pending(&pool).await.map_err(|e| format!("migrations: {e}"))?;
        Ok(pool)
    }

    async fn seed_quote(pool: &crate::DbPool, id: &str) -> TestResult {
        let now = Utc::now();
        let quote = Quote {
            id: QuoteId(id.to_string()),
            version: 1,
            status: QuoteStatus::Draft,
            account_id: Some("acct-comment-test".to_string()),
            deal_id: None,
            currency: "USD".to_string(),
            term_months: None,
            start_date: None,
            end_date: None,
            valid_until: None,
            notes: None,
            created_by: "comment-tester".to_string(),
            lines: vec![QuoteLine {
                product_id: ProductId("plan-basic".to_string()),
                quantity: 1,
                unit_price: Decimal::new(5000, 2),
                discount_pct: 0.0,
                notes: None,
            }],
            created_at: now,
            updated_at: now,
        };
        let repo = SqlQuoteRepository::new(pool.clone());
        repo.save(quote).await.map_err(|e| format!("seed quote: {e}"))
    }

    fn make_comment(id: &str, quote_id: &str, author_type: AuthorType, body: &str) -> QuoteComment {
        QuoteComment {
            id: id.to_string(),
            quote_id: quote_id.to_string(),
            author_type,
            author_id: "user-1".to_string(),
            body: body.to_string(),
            metadata_json: None,
            created_at: Utc::now(),
        }
    }

    // ── Test 1: add_comment_and_list_round_trip ───────────────────────
    #[tokio::test]
    async fn add_comment_and_list_round_trip() -> TestResult {
        let pool = setup_pool().await?;
        seed_quote(&pool, "Q-CMT-001").await?;

        let repo = SqlQuoteCommentRepository::new(pool.clone());
        let comment = make_comment("cmt-001", "Q-CMT-001", AuthorType::Rep, "Looks good to me.");

        repo.add_comment(comment).await.map_err(|e| format!("add_comment: {e}"))?;

        let list = repo
            .list_by_quote("Q-CMT-001", 100)
            .await
            .map_err(|e| format!("list_by_quote: {e}"))?;

        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "cmt-001");
        assert_eq!(list[0].quote_id, "Q-CMT-001");
        assert_eq!(list[0].author_type, AuthorType::Rep);
        assert_eq!(list[0].author_id, "user-1");
        assert_eq!(list[0].body, "Looks good to me.");
        assert!(list[0].metadata_json.is_none());
        Ok(())
    }

    // ── Test 2: list_by_quote_ordered_by_created_at ───────────────────
    #[tokio::test]
    async fn list_by_quote_ordered_by_created_at() -> TestResult {
        let pool = setup_pool().await?;
        seed_quote(&pool, "Q-CMT-002").await?;

        let repo = SqlQuoteCommentRepository::new(pool.clone());

        // Insert comments with explicit timestamps to ensure ordering
        let earlier = Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap();
        let later = Utc.with_ymd_and_hms(2026, 1, 1, 11, 0, 0).unwrap();

        let c1 = QuoteComment {
            id: "cmt-002b".to_string(),
            quote_id: "Q-CMT-002".to_string(),
            author_type: AuthorType::Manager,
            author_id: "mgr-1".to_string(),
            body: "Second comment".to_string(),
            metadata_json: None,
            created_at: later,
        };
        let c2 = QuoteComment {
            id: "cmt-002a".to_string(),
            quote_id: "Q-CMT-002".to_string(),
            author_type: AuthorType::Rep,
            author_id: "rep-1".to_string(),
            body: "First comment".to_string(),
            metadata_json: None,
            created_at: earlier,
        };

        // Insert in reverse chronological order
        repo.add_comment(c1).await.map_err(|e| format!("add c1: {e}"))?;
        repo.add_comment(c2).await.map_err(|e| format!("add c2: {e}"))?;

        let list = repo.list_by_quote("Q-CMT-002", 100).await.map_err(|e| format!("list: {e}"))?;

        assert_eq!(list.len(), 2);
        // Should be ordered by created_at ASC
        assert_eq!(list[0].id, "cmt-002a", "earlier comment should come first");
        assert_eq!(list[1].id, "cmt-002b", "later comment should come second");
        Ok(())
    }

    // ── Test 3: count_by_quote ────────────────────────────────────────
    #[tokio::test]
    async fn count_by_quote() -> TestResult {
        let pool = setup_pool().await?;
        seed_quote(&pool, "Q-CMT-003").await?;

        let repo = SqlQuoteCommentRepository::new(pool.clone());

        let count_0 = repo.count_by_quote("Q-CMT-003").await.map_err(|e| format!("count: {e}"))?;
        assert_eq!(count_0, 0);

        for i in 1..=3 {
            let c = make_comment(
                &format!("cmt-003-{i}"),
                "Q-CMT-003",
                AuthorType::System,
                &format!("System event {i}"),
            );
            repo.add_comment(c).await.map_err(|e| format!("add {i}: {e}"))?;
        }

        let count_3 =
            repo.count_by_quote("Q-CMT-003").await.map_err(|e| format!("count after 3: {e}"))?;
        assert_eq!(count_3, 3);
        Ok(())
    }

    // ── Test 4: different_author_types ─────────────────────────────────
    #[tokio::test]
    async fn different_author_types() -> TestResult {
        let pool = setup_pool().await?;
        seed_quote(&pool, "Q-CMT-004").await?;

        let repo = SqlQuoteCommentRepository::new(pool.clone());

        let types = [
            ("cmt-004-rep", AuthorType::Rep),
            ("cmt-004-mgr", AuthorType::Manager),
            ("cmt-004-sys", AuthorType::System),
            ("cmt-004-ai", AuthorType::Ai),
            ("cmt-004-int", AuthorType::Integration),
        ];

        for (id, author_type) in &types {
            let c = QuoteComment {
                id: id.to_string(),
                quote_id: "Q-CMT-004".to_string(),
                author_type: *author_type,
                author_id: "author-x".to_string(),
                body: format!("Comment from {}", author_type.as_str()),
                metadata_json: None,
                created_at: Utc::now(),
            };
            repo.add_comment(c).await.map_err(|e| format!("add {id}: {e}"))?;
        }

        let list = repo.list_by_quote("Q-CMT-004", 100).await.map_err(|e| format!("list: {e}"))?;
        assert_eq!(list.len(), 5);

        // Verify each author_type round-tripped
        let stored_types: Vec<AuthorType> = list.iter().map(|c| c.author_type).collect();
        for (_, expected) in &types {
            assert!(stored_types.contains(expected), "missing author_type {:?}", expected);
        }
        Ok(())
    }

    // ── Test 5: empty_list_for_nonexistent_quote ──────────────────────
    #[tokio::test]
    async fn empty_list_for_nonexistent_quote() -> TestResult {
        let pool = setup_pool().await?;

        let repo = SqlQuoteCommentRepository::new(pool.clone());

        let list =
            repo.list_by_quote("Q-DOES-NOT-EXIST", 100).await.map_err(|e| format!("list: {e}"))?;
        assert!(list.is_empty());

        let count =
            repo.count_by_quote("Q-DOES-NOT-EXIST").await.map_err(|e| format!("count: {e}"))?;
        assert_eq!(count, 0);
        Ok(())
    }

    // ── Test 6: metadata_json_nullable ────────────────────────────────
    #[tokio::test]
    async fn metadata_json_nullable() -> TestResult {
        let pool = setup_pool().await?;
        seed_quote(&pool, "Q-CMT-006").await?;

        let repo = SqlQuoteCommentRepository::new(pool.clone());

        // Comment without metadata
        let c1 = QuoteComment {
            id: "cmt-006a".to_string(),
            quote_id: "Q-CMT-006".to_string(),
            author_type: AuthorType::Rep,
            author_id: "rep-1".to_string(),
            body: "No metadata".to_string(),
            metadata_json: None,
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 10, 0, 0).unwrap(),
        };

        // Comment with metadata
        let c2 = QuoteComment {
            id: "cmt-006b".to_string(),
            quote_id: "Q-CMT-006".to_string(),
            author_type: AuthorType::Ai,
            author_id: "ai-agent".to_string(),
            body: "With metadata".to_string(),
            metadata_json: Some(r#"{"source":"slack","thread_ts":"1234.5678"}"#.to_string()),
            created_at: Utc.with_ymd_and_hms(2026, 1, 1, 11, 0, 0).unwrap(),
        };

        repo.add_comment(c1).await.map_err(|e| format!("add c1: {e}"))?;
        repo.add_comment(c2).await.map_err(|e| format!("add c2: {e}"))?;

        let list = repo.list_by_quote("Q-CMT-006", 100).await.map_err(|e| format!("list: {e}"))?;
        assert_eq!(list.len(), 2);

        assert!(list[0].metadata_json.is_none(), "first comment should have no metadata");
        assert_eq!(
            list[1].metadata_json.as_deref(),
            Some(r#"{"source":"slack","thread_ts":"1234.5678"}"#),
            "second comment should have metadata"
        );
        Ok(())
    }
}
