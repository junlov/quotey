use chrono::{DateTime, Utc};
use sqlx::Row;

use quotey_core::suggestions::{ProductAcceptanceRate, SuggestionFeedback};

use super::{RepositoryError, SuggestionFeedbackRepository};
use crate::DbPool;

pub struct SqlSuggestionFeedbackRepository {
    pool: DbPool,
}

impl SqlSuggestionFeedbackRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

fn row_to_feedback(row: &sqlx::sqlite::SqliteRow) -> Result<SuggestionFeedback, RepositoryError> {
    let id: String = row.try_get("id").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let request_id: String =
        row.try_get("request_id").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let customer_id: String =
        row.try_get("customer_id").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let product_id: String =
        row.try_get("product_id").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let product_sku: String =
        row.try_get("product_sku").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let score: f64 = row.try_get("score").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let confidence: String =
        row.try_get("confidence").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let category: String =
        row.try_get("category").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let quote_id: Option<String> =
        row.try_get("quote_id").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let suggested_at_str: String =
        row.try_get("suggested_at").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let was_shown: bool =
        row.try_get("was_shown").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let was_clicked: bool =
        row.try_get("was_clicked").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let was_added_to_quote: bool =
        row.try_get("was_added_to_quote").map_err(|e| RepositoryError::Decode(e.to_string()))?;
    let context_str: Option<String> =
        row.try_get("context").map_err(|e| RepositoryError::Decode(e.to_string()))?;

    let suggested_at = DateTime::parse_from_rfc3339(&suggested_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let context = context_str.and_then(|s| serde_json::from_str(&s).ok());

    Ok(SuggestionFeedback {
        id,
        request_id,
        customer_id,
        product_id,
        product_sku,
        score,
        confidence,
        category,
        quote_id,
        suggested_at,
        was_shown,
        was_clicked,
        was_added_to_quote,
        context,
    })
}

#[async_trait::async_trait]
impl SuggestionFeedbackRepository for SqlSuggestionFeedbackRepository {
    async fn record_shown(
        &self,
        feedbacks: Vec<SuggestionFeedback>,
    ) -> Result<(), RepositoryError> {
        for fb in &feedbacks {
            let context_json = fb.context.as_ref().map(|c| c.to_string());
            sqlx::query(
                "INSERT INTO suggestion_feedback
                    (id, request_id, customer_id, product_id, product_sku, score,
                     confidence, category, quote_id, suggested_at,
                     was_shown, was_clicked, was_added_to_quote, context)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, 0, 0, ?)
                 ON CONFLICT(id) DO NOTHING",
            )
            .bind(&fb.id)
            .bind(&fb.request_id)
            .bind(&fb.customer_id)
            .bind(&fb.product_id)
            .bind(&fb.product_sku)
            .bind(fb.score)
            .bind(&fb.confidence)
            .bind(&fb.category)
            .bind(&fb.quote_id)
            .bind(fb.suggested_at.to_rfc3339())
            .bind(&context_json)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    async fn record_clicked(
        &self,
        request_id: &str,
        product_id: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "UPDATE suggestion_feedback
             SET was_clicked = 1, updated_at = datetime('now')
             WHERE request_id = ? AND product_id = ?",
        )
        .bind(request_id)
        .bind(product_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn record_added(
        &self,
        request_id: &str,
        product_id: &str,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "UPDATE suggestion_feedback
             SET was_added_to_quote = 1, was_clicked = 1, updated_at = datetime('now')
             WHERE request_id = ? AND product_id = ?",
        )
        .bind(request_id)
        .bind(product_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn find_by_product(
        &self,
        product_id: &str,
        limit: u32,
    ) -> Result<Vec<SuggestionFeedback>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT id, request_id, customer_id, product_id, product_sku, score,
                    confidence, category, quote_id, suggested_at,
                    was_shown, was_clicked, was_added_to_quote, context
             FROM suggestion_feedback
             WHERE product_id = ?
             ORDER BY suggested_at DESC
             LIMIT ?",
        )
        .bind(product_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_feedback).collect::<Result<Vec<_>, _>>()
    }

    async fn acceptance_rate(
        &self,
        product_id: &str,
    ) -> Result<Option<ProductAcceptanceRate>, RepositoryError> {
        let row = sqlx::query(
            "SELECT
                 COUNT(*) AS shown_count,
                 COALESCE(SUM(was_clicked), 0) AS clicked_count,
                 COALESCE(SUM(was_added_to_quote), 0) AS added_count
             FROM suggestion_feedback
             WHERE product_id = ? AND was_shown = 1",
        )
        .bind(product_id)
        .fetch_one(&self.pool)
        .await?;

        let shown_count: i64 = row
            .try_get::<i64, _>("shown_count")
            .map_err(|e| RepositoryError::Decode(e.to_string()))?;
        if shown_count == 0 {
            return Ok(None);
        }

        let clicked_count: i64 = row
            .try_get::<i64, _>("clicked_count")
            .map_err(|e| RepositoryError::Decode(e.to_string()))?;
        let added_count: i64 = row
            .try_get::<i64, _>("added_count")
            .map_err(|e| RepositoryError::Decode(e.to_string()))?;

        let shown = shown_count as u32;
        let clicked = clicked_count as u32;
        let added = added_count as u32;

        Ok(Some(ProductAcceptanceRate {
            shown_count: shown,
            clicked_count: clicked,
            added_count: added,
            click_rate: clicked as f64 / shown as f64,
            add_rate: added as f64 / shown as f64,
        }))
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use quotey_core::suggestions::SuggestionFeedback;

    use super::SqlSuggestionFeedbackRepository;
    use crate::repositories::SuggestionFeedbackRepository;
    use crate::{connect_with_settings, migrations};

    async fn setup() -> sqlx::SqlitePool {
        let pool = connect_with_settings("sqlite::memory:", 1, 30).await.expect("connect");
        migrations::run_pending(&pool).await.expect("migrations");
        pool
    }

    fn sample_feedback(id: &str, request_id: &str, product_id: &str) -> SuggestionFeedback {
        SuggestionFeedback {
            id: id.to_string(),
            request_id: request_id.to_string(),
            customer_id: "cust-1".to_string(),
            product_id: product_id.to_string(),
            product_sku: format!("SKU-{product_id}"),
            score: 0.75,
            confidence: "Medium".to_string(),
            category: "SimilarCustomersBought".to_string(),
            quote_id: Some("Q-2026-001".to_string()),
            suggested_at: Utc::now(),
            was_shown: true,
            was_clicked: false,
            was_added_to_quote: false,
            context: None,
        }
    }

    #[tokio::test]
    async fn record_shown_and_find_by_product() {
        let pool = setup().await;
        let repo = SqlSuggestionFeedbackRepository::new(pool);

        let fb1 = sample_feedback("fb-1", "req-1", "prod_sso");
        let fb2 = sample_feedback("fb-2", "req-1", "prod_support");

        repo.record_shown(vec![fb1, fb2]).await.expect("record shown");

        let results = repo.find_by_product("prod_sso", 10).await.expect("find by product");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].product_id, "prod_sso");
        assert!(results[0].was_shown);
        assert!(!results[0].was_clicked);
    }

    #[tokio::test]
    async fn record_clicked_updates_flag() {
        let pool = setup().await;
        let repo = SqlSuggestionFeedbackRepository::new(pool);

        let fb = sample_feedback("fb-1", "req-1", "prod_sso");
        repo.record_shown(vec![fb]).await.expect("record shown");

        repo.record_clicked("req-1", "prod_sso").await.expect("record clicked");

        let results = repo.find_by_product("prod_sso", 10).await.expect("find");
        assert!(results[0].was_clicked);
        assert!(!results[0].was_added_to_quote);
    }

    #[tokio::test]
    async fn record_added_sets_both_clicked_and_added() {
        let pool = setup().await;
        let repo = SqlSuggestionFeedbackRepository::new(pool);

        let fb = sample_feedback("fb-1", "req-1", "prod_sso");
        repo.record_shown(vec![fb]).await.expect("record shown");

        repo.record_added("req-1", "prod_sso").await.expect("record added");

        let results = repo.find_by_product("prod_sso", 10).await.expect("find");
        assert!(results[0].was_clicked);
        assert!(results[0].was_added_to_quote);
    }

    #[tokio::test]
    async fn acceptance_rate_calculation() {
        let pool = setup().await;
        let repo = SqlSuggestionFeedbackRepository::new(pool);

        // 4 times shown, 2 clicked, 1 added
        repo.record_shown(vec![
            sample_feedback("fb-1", "req-1", "prod_sso"),
            sample_feedback("fb-2", "req-2", "prod_sso"),
            sample_feedback("fb-3", "req-3", "prod_sso"),
            sample_feedback("fb-4", "req-4", "prod_sso"),
        ])
        .await
        .expect("record shown");

        repo.record_clicked("req-1", "prod_sso").await.expect("click 1");
        repo.record_clicked("req-2", "prod_sso").await.expect("click 2");
        repo.record_added("req-1", "prod_sso").await.expect("add 1");

        let rate = repo.acceptance_rate("prod_sso").await.expect("rate").expect("some rate");
        assert_eq!(rate.shown_count, 4);
        assert_eq!(rate.clicked_count, 2);
        assert_eq!(rate.added_count, 1);
        assert!((rate.click_rate - 0.5).abs() < 0.01);
        assert!((rate.add_rate - 0.25).abs() < 0.01);
    }

    #[tokio::test]
    async fn acceptance_rate_returns_none_for_unknown_product() {
        let pool = setup().await;
        let repo = SqlSuggestionFeedbackRepository::new(pool);

        let rate = repo.acceptance_rate("nonexistent").await.expect("rate");
        assert!(rate.is_none());
    }
}
