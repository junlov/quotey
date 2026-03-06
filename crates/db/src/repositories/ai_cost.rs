use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::Row;

use quotey_core::domain::ai_cost::{AiCostEvent, QuoteCostSummary};

use super::RepositoryError;
use crate::DbPool;

pub struct SqlAiCostRepository {
    pool: DbPool,
}

impl SqlAiCostRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl AiCostRepository for SqlAiCostRepository {
    async fn record(&self, event: AiCostEvent) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO ai_cost_event
                (id, quote_id, tool_name, model_name, input_tokens, output_tokens,
                 estimated_cost_cents, actor_id, metadata_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&event.id)
        .bind(&event.quote_id)
        .bind(&event.tool_name)
        .bind(&event.model_name)
        .bind(event.input_tokens)
        .bind(event.output_tokens)
        .bind(event.estimated_cost_cents)
        .bind(&event.actor_id)
        .bind(&event.metadata_json)
        .bind(event.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_by_quote(
        &self,
        quote_id: &str,
        limit: u32,
    ) -> Result<Vec<AiCostEvent>, RepositoryError> {
        let rows: Vec<sqlx::sqlite::SqliteRow> = sqlx::query(
            "SELECT id, quote_id, tool_name, model_name, input_tokens, output_tokens,
                    total_tokens, estimated_cost_cents, actor_id, metadata_json, created_at
             FROM ai_cost_event
             WHERE quote_id = ?
             ORDER BY created_at DESC
             LIMIT ?",
        )
        .bind(quote_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_event).collect()
    }

    async fn summary_by_quote(
        &self,
        quote_id: &str,
    ) -> Result<Option<QuoteCostSummary>, RepositoryError> {
        let row = sqlx::query(
            "SELECT
                quote_id,
                COALESCE(SUM(input_tokens), 0) AS total_input_tokens,
                COALESCE(SUM(output_tokens), 0) AS total_output_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COALESCE(SUM(estimated_cost_cents), 0.0) AS total_cost,
                COUNT(*) AS invocation_count
             FROM ai_cost_event
             WHERE quote_id = ?
             GROUP BY quote_id",
        )
        .bind(quote_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(QuoteCostSummary {
                quote_id: r.get("quote_id"),
                total_input_tokens: r.get("total_input_tokens"),
                total_output_tokens: r.get("total_output_tokens"),
                total_tokens: r.get("total_tokens"),
                total_estimated_cost_cents: r.get("total_cost"),
                invocation_count: r.get("invocation_count"),
            })),
            None => Ok(None),
        }
    }

    async fn list_recent(&self, limit: u32) -> Result<Vec<AiCostEvent>, RepositoryError> {
        let rows: Vec<sqlx::sqlite::SqliteRow> = sqlx::query(
            "SELECT id, quote_id, tool_name, model_name, input_tokens, output_tokens,
                    total_tokens, estimated_cost_cents, actor_id, metadata_json, created_at
             FROM ai_cost_event
             ORDER BY created_at DESC
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_event).collect()
    }
}

fn row_to_event(row: &sqlx::sqlite::SqliteRow) -> Result<AiCostEvent, RepositoryError> {
    let created_at_str: String = row.get("created_at");
    let created_at = DateTime::parse_from_rfc3339(&created_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            NaiveDateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S")
                .map(|ndt| ndt.and_utc())
        })
        .map_err(|e| RepositoryError::Decode(format!("invalid created_at: {e}")))?;

    Ok(AiCostEvent {
        id: row.get("id"),
        quote_id: row.get("quote_id"),
        tool_name: row.get("tool_name"),
        model_name: row.get("model_name"),
        input_tokens: row.get("input_tokens"),
        output_tokens: row.get("output_tokens"),
        total_tokens: row.get("total_tokens"),
        estimated_cost_cents: row.get("estimated_cost_cents"),
        actor_id: row.get("actor_id"),
        metadata_json: row.get("metadata_json"),
        created_at,
    })
}

#[async_trait::async_trait]
pub trait AiCostRepository: Send + Sync {
    async fn record(&self, event: AiCostEvent) -> Result<(), RepositoryError>;
    async fn list_by_quote(
        &self,
        quote_id: &str,
        limit: u32,
    ) -> Result<Vec<AiCostEvent>, RepositoryError>;
    async fn summary_by_quote(
        &self,
        quote_id: &str,
    ) -> Result<Option<QuoteCostSummary>, RepositoryError>;
    async fn list_recent(&self, limit: u32) -> Result<Vec<AiCostEvent>, RepositoryError>;
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use quotey_core::domain::ai_cost::AiCostEvent;

    use super::*;
    use crate::DbPool;

    async fn setup() -> DbPool {
        let pool = crate::connect_with_settings("sqlite::memory:", 1, 30).await.expect("connect");
        crate::migrations::run_pending(&pool).await.expect("migrate");
        pool
    }

    fn sample_event(quote_id: Option<&str>, tool: &str) -> AiCostEvent {
        AiCostEvent {
            id: format!("COST-TEST-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
            quote_id: quote_id.map(str::to_string),
            tool_name: tool.to_string(),
            model_name: "claude-sonnet-4-20250514".to_string(),
            input_tokens: 1500,
            output_tokens: 300,
            total_tokens: 1800,
            estimated_cost_cents: 0.54,
            actor_id: Some("mcp:test".to_string()),
            metadata_json: "{}".to_string(),
            created_at: Utc::now(),
        }
    }

    async fn seed_quote(pool: &DbPool, id: &str) {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO quote (id, status, currency, created_by, created_at, updated_at)
             VALUES (?, 'draft', 'USD', 'test', ?, ?)",
        )
        .bind(id)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .expect("seed quote");
    }

    #[tokio::test]
    async fn record_and_list_by_quote() {
        let pool = setup().await;
        seed_quote(&pool, "Q-COST-001").await;
        let repo = SqlAiCostRepository::new(pool);

        let ev1 = sample_event(Some("Q-COST-001"), "quote_price");
        let ev2 = sample_event(Some("Q-COST-001"), "catalog_search");
        repo.record(ev1).await.expect("record 1");
        repo.record(ev2).await.expect("record 2");

        let events = repo.list_by_quote("Q-COST-001", 10).await.expect("list");
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.quote_id.as_deref() == Some("Q-COST-001")));
    }

    #[tokio::test]
    async fn summary_by_quote_aggregates() {
        let pool = setup().await;
        seed_quote(&pool, "Q-COST-002").await;
        let repo = SqlAiCostRepository::new(pool);

        let mut ev1 = sample_event(Some("Q-COST-002"), "quote_price");
        ev1.input_tokens = 1000;
        ev1.output_tokens = 200;
        ev1.estimated_cost_cents = 0.36;
        repo.record(ev1).await.expect("record 1");

        let mut ev2 = sample_event(Some("Q-COST-002"), "approval_request");
        ev2.input_tokens = 500;
        ev2.output_tokens = 100;
        ev2.estimated_cost_cents = 0.18;
        repo.record(ev2).await.expect("record 2");

        let summary = repo.summary_by_quote("Q-COST-002").await.expect("summary");
        let summary = summary.expect("should have summary");
        assert_eq!(summary.invocation_count, 2);
        assert_eq!(summary.total_input_tokens, 1500);
        assert_eq!(summary.total_output_tokens, 300);
        assert!((summary.total_estimated_cost_cents - 0.54).abs() < 0.01);
    }

    #[tokio::test]
    async fn summary_returns_none_for_unknown_quote() {
        let pool = setup().await;
        let repo = SqlAiCostRepository::new(pool);

        let summary = repo.summary_by_quote("Q-NONEXISTENT").await.expect("summary");
        assert!(summary.is_none());
    }

    #[tokio::test]
    async fn list_recent_returns_across_quotes() {
        let pool = setup().await;
        seed_quote(&pool, "Q-COST-003").await;
        let repo = SqlAiCostRepository::new(pool);

        repo.record(sample_event(Some("Q-COST-003"), "catalog_search")).await.expect("r1");
        repo.record(sample_event(None, "settings_list")).await.expect("r2");

        let recent = repo.list_recent(10).await.expect("recent");
        assert_eq!(recent.len(), 2);
    }

    #[tokio::test]
    async fn record_without_quote_id() {
        let pool = setup().await;
        let repo = SqlAiCostRepository::new(pool);

        let ev = sample_event(None, "settings_get");
        repo.record(ev.clone()).await.expect("record");

        let recent = repo.list_recent(10).await.expect("list");
        assert_eq!(recent.len(), 1);
        assert!(recent[0].quote_id.is_none());
    }
}
