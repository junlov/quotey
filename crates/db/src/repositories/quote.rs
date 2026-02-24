use quotey_core::chrono::{DateTime, Utc};
use quotey_core::domain::product::ProductId;
use quotey_core::domain::quote::{Quote, QuoteId};
use quotey_core::domain::quote::{QuoteLine, QuoteStatus};
use rust_decimal::Decimal;
use sqlx::Row;

use super::{QuoteRepository, RepositoryError};
use crate::DbPool;

const SYSTEM_CREATED_BY: &str = "system";

pub struct SqlQuoteRepository {
    pool: DbPool,
}

impl SqlQuoteRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl QuoteRepository for SqlQuoteRepository {
    async fn find_by_id(&self, id: &QuoteId) -> Result<Option<Quote>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                status,
                created_at
            FROM quote
            WHERE id = ?
            "#,
        )
        .bind(&id.0)
        .fetch_optional(&self.pool)
        .await?;

        let quote = match row {
            Some(row) => {
                let status_raw: String =
                    row.try_get("status").map_err(RepositoryError::Database)?;
                let created_at_raw: String =
                    row.try_get("created_at").map_err(RepositoryError::Database)?;
                let status = parse_quote_status(&status_raw)?;
                let created_at = parse_datetime("created_at", &created_at_raw)?;

                let rows = sqlx::query(
                    r#"
                    SELECT
                        product_id,
                        quantity,
                        CAST(COALESCE(unit_price, 0) AS TEXT) AS unit_price_text
                    FROM quote_line
                    WHERE quote_id = ?
                    ORDER BY created_at ASC, id ASC
                    "#,
                )
                .bind(&id.0)
                .fetch_all(&self.pool)
                .await?;

                let mut lines = Vec::with_capacity(rows.len());
                for row in rows {
                    let product_id: String =
                        row.try_get("product_id").map_err(RepositoryError::Database)?;
                    let quantity_raw: i64 =
                        row.try_get("quantity").map_err(RepositoryError::Database)?;
                    let unit_price_raw: String =
                        row.try_get("unit_price_text").map_err(RepositoryError::Database)?;

                    let quantity = u32::try_from(quantity_raw).map_err(|_| {
                        RepositoryError::Decode(format!(
                            "invalid quote line quantity `{quantity_raw}`"
                        ))
                    })?;
                    let unit_price = unit_price_raw.parse::<Decimal>().map_err(|error| {
                        RepositoryError::Decode(format!(
                            "invalid unit_price `{unit_price_raw}` for quote {} line `{product_id}`: {error}",
                            id.0
                        ))
                    })?;

                    lines.push(QuoteLine {
                        product_id: ProductId(product_id),
                        quantity,
                        unit_price,
                    });
                }

                Quote { id: QuoteId(id.0.clone()), status, lines, created_at }
            }
            None => return Ok(None),
        };

        Ok(Some(quote))
    }

    async fn save(&self, quote: Quote) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;

        let existing_created_by =
            sqlx::query_scalar::<_, String>("SELECT created_by FROM quote WHERE id = ?")
                .bind(&quote.id.0)
                .fetch_optional(&mut *tx)
                .await?;

        let created_by = existing_created_by.unwrap_or_else(|| SYSTEM_CREATED_BY.to_string());
        let created_at = quote.created_at.to_rfc3339();
        let status = quote_status_as_str(&quote.status);
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO quote (
                id,
                status,
                currency,
                start_date,
                end_date,
                term_months,
                valid_until,
                created_by,
                created_at,
                updated_at
            )
            VALUES (?, ?, 'USD', NULL, NULL, NULL, NULL, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&quote.id.0)
        .bind(status)
        .bind(&created_by)
        .bind(&created_at)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM quote_line WHERE quote_id = ?")
            .bind(&quote.id.0)
            .execute(&mut *tx)
            .await?;

        for (index, line) in quote.lines.iter().enumerate() {
            let line_id = format!("{}-ql-{}", quote.id.0, index + 1);
            let unit_price = line.unit_price.to_string();
            let quantity = i64::from(line.quantity);
            let subtotal = (line.unit_price * Decimal::from(line.quantity)).to_string();

            sqlx::query(
                r#"
                INSERT INTO quote_line (
                    id,
                    quote_id,
                    product_id,
                    quantity,
                    unit_price,
                    subtotal,
                    attributes_json,
                    created_at,
                    updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, NULL, ?, ?)
                "#,
            )
            .bind(&line_id)
            .bind(&quote.id.0)
            .bind(&line.product_id.0)
            .bind(quantity)
            .bind(&unit_price)
            .bind(&subtotal)
            .bind(&created_at)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

fn parse_quote_status(raw: &str) -> Result<QuoteStatus, RepositoryError> {
    match raw.to_lowercase().as_str() {
        "draft" => Ok(QuoteStatus::Draft),
        "validated" => Ok(QuoteStatus::Validated),
        "priced" => Ok(QuoteStatus::Priced),
        "approval" => Ok(QuoteStatus::Approval),
        "approved" => Ok(QuoteStatus::Approved),
        "rejected" => Ok(QuoteStatus::Rejected),
        "finalized" => Ok(QuoteStatus::Finalized),
        "sent" => Ok(QuoteStatus::Sent),
        "expired" => Ok(QuoteStatus::Expired),
        "cancelled" => Ok(QuoteStatus::Cancelled),
        "revised" => Ok(QuoteStatus::Revised),
        _ => Err(RepositoryError::Decode(format!("invalid quote status `{raw}`"))),
    }
}

fn quote_status_as_str(status: &QuoteStatus) -> &'static str {
    match status {
        QuoteStatus::Draft => "draft",
        QuoteStatus::Validated => "validated",
        QuoteStatus::Priced => "priced",
        QuoteStatus::Approval => "approval",
        QuoteStatus::Approved => "approved",
        QuoteStatus::Rejected => "rejected",
        QuoteStatus::Finalized => "finalized",
        QuoteStatus::Sent => "sent",
        QuoteStatus::Expired => "expired",
        QuoteStatus::Cancelled => "cancelled",
        QuoteStatus::Revised => "revised",
    }
}

fn parse_datetime(field: &str, value: &str) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| RepositoryError::Decode(format!("invalid {field}: {value}: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::run_pending;
    use quotey_core::chrono::Utc;
    use quotey_core::domain::{
        product::ProductId,
        quote::{Quote, QuoteId, QuoteLine, QuoteStatus},
    };
    use rust_decimal::Decimal;

    async fn in_memory_pool() -> Result<DbPool, RepositoryError> {
        crate::connect_with_settings("sqlite::memory:", 1, 30)
            .await
            .map_err(RepositoryError::Database)
    }

    #[tokio::test]
    async fn save_round_trip_quote_and_lines() -> Result<(), String> {
        let pool = in_memory_pool().await.map_err(|error| error.to_string())?;
        run_pending(&pool).await.map_err(|error| error.to_string())?;

        let repo = SqlQuoteRepository::new(pool.clone());
        let quote = Quote {
            id: QuoteId("Q-ROUND-001".to_string()),
            status: QuoteStatus::Draft,
            lines: vec![
                QuoteLine {
                    product_id: ProductId("prod-1".to_string()),
                    quantity: 3,
                    unit_price: Decimal::new(1999, 2),
                },
                QuoteLine {
                    product_id: ProductId("prod-2".to_string()),
                    quantity: 1,
                    unit_price: Decimal::new(5000, 2),
                },
            ],
            created_at: Utc::now(),
        };

        repo.save(quote.clone()).await.map_err(|error| error.to_string())?;
        let loaded = repo.find_by_id(&quote.id).await.map_err(|error| error.to_string())?;

        let loaded = loaded.expect("saved quote should be loadable");
        assert_eq!(loaded.id, quote.id);
        assert_eq!(loaded.status, quote.status);
        assert_eq!(loaded.lines, quote.lines);

        Ok(())
    }

    #[tokio::test]
    async fn updating_quote_replaces_lines() -> Result<(), String> {
        let pool = in_memory_pool().await.map_err(|error| error.to_string())?;
        run_pending(&pool).await.map_err(|error| error.to_string())?;

        let repo = SqlQuoteRepository::new(pool.clone());
        let quote_id = QuoteId("Q-ROUND-002".to_string());
        let initial = Quote {
            id: quote_id.clone(),
            status: QuoteStatus::Draft,
            lines: vec![QuoteLine {
                product_id: ProductId("prod-1".to_string()),
                quantity: 1,
                unit_price: Decimal::new(1000, 2),
            }],
            created_at: Utc::now(),
        };
        let updated = Quote {
            id: quote_id,
            status: QuoteStatus::Priced,
            lines: vec![QuoteLine {
                product_id: ProductId("prod-3".to_string()),
                quantity: 2,
                unit_price: Decimal::new(2500, 2),
            }],
            created_at: Utc::now(),
        };

        repo.save(initial).await.map_err(|error| error.to_string())?;
        repo.save(updated.clone()).await.map_err(|error| error.to_string())?;

        let loaded = repo.find_by_id(&updated.id).await.map_err(|error| error.to_string())?;
        let loaded = loaded.expect("updated quote should be loadable");
        assert_eq!(loaded.status, QuoteStatus::Priced);
        assert_eq!(loaded.lines.len(), 1);
        assert_eq!(loaded.lines[0].product_id, updated.lines[0].product_id);
        assert_eq!(loaded.lines[0].quantity, updated.lines[0].quantity);

        Ok(())
    }
}
