use quotey_core::chrono::{DateTime, Utc};
use quotey_core::domain::product::ProductId;
use quotey_core::domain::quote::{Quote, QuoteId};
use quotey_core::domain::quote::{QuoteLine, QuoteStatus};
use rust_decimal::Decimal;
use sqlx::Row;

use super::{QuoteRepository, RepositoryError};
use crate::DbPool;

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
                currency,
                start_date,
                end_date,
                term_months,
                valid_until,
                created_by,
                created_at,
                updated_at,
                account_id,
                deal_id,
                notes,
                version
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
                let updated_at_raw: String =
                    row.try_get("updated_at").map_err(RepositoryError::Database)?;
                let currency: String =
                    row.try_get("currency").map_err(RepositoryError::Database)?;
                let start_date: Option<String> =
                    row.try_get("start_date").map_err(RepositoryError::Database)?;
                let end_date: Option<String> =
                    row.try_get("end_date").map_err(RepositoryError::Database)?;
                let term_months: Option<i32> =
                    row.try_get("term_months").map_err(RepositoryError::Database)?;
                let valid_until: Option<String> =
                    row.try_get("valid_until").map_err(RepositoryError::Database)?;
                let created_by: String =
                    row.try_get("created_by").map_err(RepositoryError::Database)?;
                let account_id: Option<String> =
                    row.try_get("account_id").map_err(RepositoryError::Database)?;
                let deal_id: Option<String> =
                    row.try_get("deal_id").map_err(RepositoryError::Database)?;
                let notes: Option<String> =
                    row.try_get("notes").map_err(RepositoryError::Database)?;
                let version: i32 = row.try_get("version").map_err(RepositoryError::Database)?;

                let status = parse_quote_status(&status_raw)?;
                let created_at = parse_datetime("created_at", &created_at_raw)?;
                let updated_at = parse_datetime("updated_at", &updated_at_raw)?;

                let lines = load_quote_lines(&self.pool, &id.0).await?;

                Quote {
                    id: QuoteId(id.0.clone()),
                    version: version as u32,
                    status,
                    account_id,
                    deal_id,
                    currency,
                    term_months: term_months.map(|v| v as u32),
                    start_date,
                    end_date,
                    valid_until,
                    notes,
                    created_by,
                    lines,
                    created_at,
                    updated_at,
                }
            }
            None => return Ok(None),
        };

        Ok(Some(quote))
    }

    async fn save(&self, quote: Quote) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;

        let status = quote_status_as_str(&quote.status);
        let now = Utc::now().to_rfc3339();
        let created_at = quote.created_at.to_rfc3339();
        let term_months = quote.term_months.map(|v| v as i32);

        sqlx::query(
            r#"
            INSERT INTO quote (
                id, status, currency, start_date, end_date,
                term_months, valid_until, created_by, created_at, updated_at,
                account_id, deal_id, notes, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                currency = excluded.currency,
                start_date = excluded.start_date,
                end_date = excluded.end_date,
                term_months = excluded.term_months,
                valid_until = excluded.valid_until,
                notes = excluded.notes,
                version = excluded.version,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&quote.id.0)
        .bind(status)
        .bind(&quote.currency)
        .bind(&quote.start_date)
        .bind(&quote.end_date)
        .bind(term_months)
        .bind(&quote.valid_until)
        .bind(&quote.created_by)
        .bind(&created_at)
        .bind(&now)
        .bind(&quote.account_id)
        .bind(&quote.deal_id)
        .bind(&quote.notes)
        .bind(quote.version as i32)
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
                    id, quote_id, product_id, quantity,
                    unit_price, subtotal, discount_pct, notes,
                    attributes_json, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
                "#,
            )
            .bind(&line_id)
            .bind(&quote.id.0)
            .bind(&line.product_id.0)
            .bind(quantity)
            .bind(&unit_price)
            .bind(&subtotal)
            .bind(line.discount_pct)
            .bind(&line.notes)
            .bind(&created_at)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn list(
        &self,
        account_id: Option<&str>,
        status: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Quote>, RepositoryError> {
        // Build dynamic query with optional filters
        let mut sql = String::from(
            r#"
            SELECT
                id, status, currency, start_date, end_date,
                term_months, valid_until, created_by, created_at, updated_at,
                account_id, deal_id, notes, version
            FROM quote
            WHERE 1=1
            "#,
        );
        if account_id.is_some() {
            sql.push_str(" AND account_id = ?");
        }
        if status.is_some() {
            sql.push_str(" AND status = ?");
        }
        sql.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");

        let mut query = sqlx::query(&sql);
        if let Some(aid) = account_id {
            query = query.bind(aid.to_string());
        }
        if let Some(s) = status {
            query = query.bind(s.to_string());
        }
        query = query.bind(limit as i64).bind(offset as i64);

        let rows = query.fetch_all(&self.pool).await?;

        let mut quotes = Vec::with_capacity(rows.len());
        for row in rows {
            let qid: String = row.try_get("id").map_err(RepositoryError::Database)?;
            let status_raw: String = row.try_get("status").map_err(RepositoryError::Database)?;
            let currency: String = row.try_get("currency").map_err(RepositoryError::Database)?;
            let start_date: Option<String> =
                row.try_get("start_date").map_err(RepositoryError::Database)?;
            let end_date: Option<String> =
                row.try_get("end_date").map_err(RepositoryError::Database)?;
            let term_months: Option<i32> =
                row.try_get("term_months").map_err(RepositoryError::Database)?;
            let valid_until: Option<String> =
                row.try_get("valid_until").map_err(RepositoryError::Database)?;
            let created_by: String =
                row.try_get("created_by").map_err(RepositoryError::Database)?;
            let created_at_raw: String =
                row.try_get("created_at").map_err(RepositoryError::Database)?;
            let updated_at_raw: String =
                row.try_get("updated_at").map_err(RepositoryError::Database)?;
            let acct_id: Option<String> =
                row.try_get("account_id").map_err(RepositoryError::Database)?;
            let d_id: Option<String> = row.try_get("deal_id").map_err(RepositoryError::Database)?;
            let qnotes: Option<String> = row.try_get("notes").map_err(RepositoryError::Database)?;
            let version: i32 = row.try_get("version").map_err(RepositoryError::Database)?;

            let status = parse_quote_status(&status_raw)?;
            let created_at = parse_datetime("created_at", &created_at_raw)?;
            let updated_at = parse_datetime("updated_at", &updated_at_raw)?;

            // For list, load lines per quote
            let lines = load_quote_lines(&self.pool, &qid).await?;

            quotes.push(Quote {
                id: QuoteId(qid),
                version: version as u32,
                status,
                account_id: acct_id,
                deal_id: d_id,
                currency,
                term_months: term_months.map(|v| v as u32),
                start_date,
                end_date,
                valid_until,
                notes: qnotes,
                created_by,
                lines,
                created_at,
                updated_at,
            });
        }

        Ok(quotes)
    }
}

async fn load_quote_lines(
    pool: &DbPool,
    quote_id: &str,
) -> Result<Vec<QuoteLine>, RepositoryError> {
    let rows = sqlx::query(
        r#"
        SELECT
            product_id,
            quantity,
            CAST(COALESCE(unit_price, 0) AS TEXT) AS unit_price_text,
            COALESCE(discount_pct, 0.0) AS discount_pct,
            notes
        FROM quote_line
        WHERE quote_id = ?
        ORDER BY created_at ASC, id ASC
        "#,
    )
    .bind(quote_id)
    .fetch_all(pool)
    .await?;

    let mut lines = Vec::with_capacity(rows.len());
    for row in rows {
        let product_id: String = row.try_get("product_id").map_err(RepositoryError::Database)?;
        let quantity_raw: i64 = row.try_get("quantity").map_err(RepositoryError::Database)?;
        let unit_price_raw: String =
            row.try_get("unit_price_text").map_err(RepositoryError::Database)?;
        let discount_pct: f64 = row.try_get("discount_pct").map_err(RepositoryError::Database)?;
        let notes: Option<String> = row.try_get("notes").map_err(RepositoryError::Database)?;

        let quantity = u32::try_from(quantity_raw).map_err(|_| {
            RepositoryError::Decode(format!("invalid quote line quantity `{quantity_raw}`"))
        })?;
        let unit_price = unit_price_raw.parse::<Decimal>().map_err(|error| {
            RepositoryError::Decode(format!(
                "invalid unit_price `{unit_price_raw}` for quote {quote_id} line `{product_id}`: {error}"
            ))
        })?;

        lines.push(QuoteLine {
            product_id: ProductId(product_id),
            quantity,
            unit_price,
            discount_pct,
            notes,
        });
    }

    Ok(lines)
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

pub fn quote_status_as_str(status: &QuoteStatus) -> &'static str {
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

    fn test_quote(id: &str, account: Option<&str>) -> Quote {
        let now = Utc::now();
        Quote {
            id: QuoteId(id.to_string()),
            version: 1,
            status: QuoteStatus::Draft,
            account_id: account.map(|s| s.to_string()),
            deal_id: None,
            currency: "USD".to_string(),
            term_months: Some(12),
            start_date: None,
            end_date: None,
            valid_until: None,
            notes: None,
            created_by: "test".to_string(),
            lines: vec![
                QuoteLine {
                    product_id: ProductId("prod-1".to_string()),
                    quantity: 3,
                    unit_price: Decimal::new(1999, 2),
                    discount_pct: 0.0,
                    notes: None,
                },
                QuoteLine {
                    product_id: ProductId("prod-2".to_string()),
                    quantity: 1,
                    unit_price: Decimal::new(5000, 2),
                    discount_pct: 10.0,
                    notes: Some("Enterprise discount".to_string()),
                },
            ],
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn save_round_trip_quote_and_lines() -> Result<(), String> {
        let pool = in_memory_pool().await.map_err(|error| error.to_string())?;
        run_pending(&pool).await.map_err(|error| error.to_string())?;

        let repo = SqlQuoteRepository::new(pool.clone());
        let quote = test_quote("Q-ROUND-001", Some("acct_acme"));

        repo.save(quote.clone()).await.map_err(|error| error.to_string())?;
        let loaded = repo.find_by_id(&quote.id).await.map_err(|error| error.to_string())?;

        let loaded = loaded.expect("saved quote should be loadable");
        assert_eq!(loaded.id, quote.id);
        assert_eq!(loaded.status, quote.status);
        assert_eq!(loaded.account_id, quote.account_id);
        assert_eq!(loaded.currency, "USD");
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.lines.len(), 2);
        assert_eq!(loaded.lines[0].product_id, quote.lines[0].product_id);
        assert_eq!(loaded.lines[1].discount_pct, 10.0);

        Ok(())
    }

    #[tokio::test]
    async fn updating_quote_replaces_lines() -> Result<(), String> {
        let pool = in_memory_pool().await.map_err(|error| error.to_string())?;
        run_pending(&pool).await.map_err(|error| error.to_string())?;

        let repo = SqlQuoteRepository::new(pool.clone());
        let initial = test_quote("Q-ROUND-002", None);
        let now = Utc::now();
        let updated = Quote {
            id: QuoteId("Q-ROUND-002".to_string()),
            version: 2,
            status: QuoteStatus::Priced,
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
                product_id: ProductId("prod-3".to_string()),
                quantity: 2,
                unit_price: Decimal::new(2500, 2),
                discount_pct: 0.0,
                notes: None,
            }],
            created_at: now,
            updated_at: now,
        };

        repo.save(initial).await.map_err(|error| error.to_string())?;
        repo.save(updated.clone()).await.map_err(|error| error.to_string())?;

        let loaded = repo.find_by_id(&updated.id).await.map_err(|error| error.to_string())?;
        let loaded = loaded.expect("updated quote should be loadable");
        assert_eq!(loaded.status, QuoteStatus::Priced);
        assert_eq!(loaded.version, 2);
        assert_eq!(loaded.lines.len(), 1);
        assert_eq!(loaded.lines[0].product_id, updated.lines[0].product_id);
        assert_eq!(loaded.lines[0].quantity, updated.lines[0].quantity);

        Ok(())
    }

    #[tokio::test]
    async fn list_quotes_filters_by_account() -> Result<(), String> {
        let pool = in_memory_pool().await.map_err(|error| error.to_string())?;
        run_pending(&pool).await.map_err(|error| error.to_string())?;

        let repo = SqlQuoteRepository::new(pool.clone());
        let q1 = test_quote("Q-LIST-001", Some("acct_acme"));
        let q2 = test_quote("Q-LIST-002", Some("acct_acme"));
        let q3 = test_quote("Q-LIST-003", Some("acct_globex"));

        repo.save(q1).await.map_err(|e| e.to_string())?;
        repo.save(q2).await.map_err(|e| e.to_string())?;
        repo.save(q3).await.map_err(|e| e.to_string())?;

        let acme_quotes =
            repo.list(Some("acct_acme"), None, 20, 0).await.map_err(|e| e.to_string())?;
        assert_eq!(acme_quotes.len(), 2);

        let all_quotes = repo.list(None, None, 20, 0).await.map_err(|e| e.to_string())?;
        assert_eq!(all_quotes.len(), 3);

        Ok(())
    }
}
