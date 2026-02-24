use std::{collections::HashMap, str::FromStr};

use async_trait::async_trait;
use quotey_core::domain::quote::QuoteId;
use quotey_core::{
    CalculationStep, ExplanationError, PricingLineSnapshot, PricingSnapshot,
    PricingSnapshotProvider,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteRow, Row};

use crate::DbPool;

/// SQLite-backed pricing snapshot provider for Explain Any Number.
///
/// Lookup order:
/// 1. Validate quote exists.
/// 2. Validate requested quote version exists in quote_ledger.
/// 3. Return cached row from quote_pricing_snapshot when present.
/// 4. Fallback-build snapshot from current quote_line rows and cache it.
pub struct SqlPricingSnapshotRepository {
    pool: DbPool,
    priced_by: String,
}

impl SqlPricingSnapshotRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool, priced_by: "system".to_string() }
    }

    pub fn with_priced_by(pool: DbPool, priced_by: impl Into<String>) -> Self {
        Self { pool, priced_by: priced_by.into() }
    }

    async fn ensure_quote_exists(&self, quote_id: &QuoteId) -> Result<String, ExplanationError> {
        let row = sqlx::query("SELECT currency FROM quote WHERE id = ?")
            .bind(&quote_id.0)
            .fetch_optional(&self.pool)
            .await
            .map_err(Self::db_error)?;

        let row =
            row.ok_or_else(|| ExplanationError::QuoteNotFound { quote_id: quote_id.clone() })?;
        row.try_get("currency").map_err(Self::db_error)
    }

    async fn load_ledger_version(
        &self,
        quote_id: &QuoteId,
        version: i32,
    ) -> Result<LedgerVersion, ExplanationError> {
        let row = sqlx::query(
            r#"
            SELECT entry_id, content_hash, timestamp, version_number
            FROM quote_ledger
            WHERE quote_id = ? AND version_number = ?
            "#,
        )
        .bind(&quote_id.0)
        .bind(version)
        .fetch_optional(&self.pool)
        .await
        .map_err(Self::db_error)?;

        if let Some(row) = row {
            let version_number_raw: i64 = row.try_get("version_number").map_err(Self::db_error)?;
            i32::try_from(version_number_raw).map_err(|_| {
                ExplanationError::EvidenceGatheringFailed {
                    reason: format!(
                        "ledger version_number `{version_number_raw}` does not fit in i32"
                    ),
                }
            })?;
            return Ok(LedgerVersion {
                entry_id: row.try_get("entry_id").map_err(Self::db_error)?,
                content_hash: row.try_get("content_hash").map_err(Self::db_error)?,
                timestamp: row.try_get("timestamp").map_err(Self::db_error)?,
            });
        }

        let latest_raw: Option<i64> =
            sqlx::query_scalar("SELECT MAX(version_number) FROM quote_ledger WHERE quote_id = ?")
                .bind(&quote_id.0)
                .fetch_one(&self.pool)
                .await
                .map_err(Self::db_error)?;

        let actual = latest_raw.and_then(|value| i32::try_from(value).ok()).unwrap_or_default();
        Err(ExplanationError::VersionMismatch { expected: version, actual })
    }

    async fn load_cached_snapshot_row(
        &self,
        quote_id: &QuoteId,
        version: i32,
    ) -> Result<Option<SqliteRow>, ExplanationError> {
        sqlx::query(
            r#"
            SELECT
                id,
                quote_id,
                version,
                ledger_entry_id,
                ledger_content_hash,
                CAST(subtotal AS TEXT) AS subtotal_text,
                CAST(discount_total AS TEXT) AS discount_total_text,
                CAST(tax_total AS TEXT) AS tax_total_text,
                CAST(total AS TEXT) AS total_text,
                currency,
                pricing_trace_json,
                priced_at
            FROM quote_pricing_snapshot
            WHERE quote_id = ? AND version = ?
            LIMIT 1
            "#,
        )
        .bind(&quote_id.0)
        .bind(version)
        .fetch_optional(&self.pool)
        .await
        .map_err(Self::db_error)
    }

    fn validate_ledger_linkage(
        row: &SqliteRow,
        quote_id: &QuoteId,
        expected_version: i32,
        ledger: &LedgerVersion,
    ) -> Result<(), ExplanationError> {
        let cached_ledger_entry_id: Option<String> =
            row.try_get("ledger_entry_id").map_err(Self::db_error)?;
        if let Some(cached_ledger_entry_id) = cached_ledger_entry_id {
            if cached_ledger_entry_id != ledger.entry_id {
                return Err(ExplanationError::EvidenceGatheringFailed {
                    reason: format!(
                        "ledger mismatch for quote {} version {}: snapshot entry {} != ledger entry {}",
                        quote_id.0, expected_version, cached_ledger_entry_id, ledger.entry_id
                    ),
                });
            }
        }

        let cached_ledger_content_hash: Option<String> =
            row.try_get("ledger_content_hash").map_err(Self::db_error)?;
        if let Some(cached_ledger_content_hash) = cached_ledger_content_hash {
            if cached_ledger_content_hash != ledger.content_hash {
                return Err(ExplanationError::EvidenceGatheringFailed {
                    reason: format!(
                        "ledger mismatch for quote {} version {}: snapshot hash {} != ledger hash {}",
                        quote_id.0, expected_version, cached_ledger_content_hash, ledger.content_hash
                    ),
                });
            }
        }

        Ok(())
    }

    fn snapshot_from_row(row: &SqliteRow) -> Result<PricingSnapshot, ExplanationError> {
        let quote_id: String = row.try_get("quote_id").map_err(Self::db_error)?;
        let version: i32 = row.try_get("version").map_err(Self::db_error)?;
        let subtotal_text: String = row.try_get("subtotal_text").map_err(Self::db_error)?;
        let discount_total_text: String =
            row.try_get("discount_total_text").map_err(Self::db_error)?;
        let tax_total_text: String = row.try_get("tax_total_text").map_err(Self::db_error)?;
        let total_text: String = row.try_get("total_text").map_err(Self::db_error)?;
        let currency: String = row.try_get("currency").map_err(Self::db_error)?;
        let pricing_trace_json: String =
            row.try_get("pricing_trace_json").map_err(Self::db_error)?;
        let priced_at: String = row.try_get("priced_at").map_err(Self::db_error)?;

        let payload: PersistedPricingTrace =
            serde_json::from_str(&pricing_trace_json).map_err(|error| {
                ExplanationError::EvidenceGatheringFailed {
                    reason: format!("failed to decode pricing_trace_json: {error}"),
                }
            })?;

        let line_items = payload.try_into_line_items()?;
        let calculation_steps = payload.try_into_steps()?;

        Ok(PricingSnapshot {
            quote_id: QuoteId(quote_id),
            version,
            subtotal: Self::parse_decimal("subtotal", &subtotal_text)?,
            discount_total: Self::parse_decimal("discount_total", &discount_total_text)?,
            tax_total: Self::parse_decimal("tax_total", &tax_total_text)?,
            total: Self::parse_decimal("total", &total_text)?,
            currency,
            line_items,
            calculation_steps,
            created_at: priced_at,
        })
    }

    async fn build_fallback_snapshot(
        &self,
        quote_id: &QuoteId,
        version: i32,
        currency: String,
        priced_at: String,
    ) -> Result<PricingSnapshot, ExplanationError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                product_id,
                quantity,
                CAST(COALESCE(unit_price, 0) AS TEXT) AS unit_price_text,
                CAST(COALESCE(subtotal, COALESCE(unit_price, 0) * quantity) AS TEXT) AS line_subtotal_text
            FROM quote_line
            WHERE quote_id = ?
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .bind(&quote_id.0)
        .fetch_all(&self.pool)
        .await
        .map_err(Self::db_error)?;

        let mut line_items = Vec::with_capacity(rows.len());
        let mut subtotal = Decimal::ZERO;
        let mut input_values = HashMap::new();

        for row in rows {
            let line_id: String = row.try_get("id").map_err(Self::db_error)?;
            let product_id: String = row.try_get("product_id").map_err(Self::db_error)?;
            let quantity_raw: i64 = row.try_get("quantity").map_err(Self::db_error)?;
            let quantity = i32::try_from(quantity_raw).map_err(|_| {
                ExplanationError::EvidenceGatheringFailed {
                    reason: format!(
                        "quantity `{quantity_raw}` on quote {} line {} exceeds i32",
                        quote_id.0, line_id
                    ),
                }
            })?;
            let unit_price_text: String = row.try_get("unit_price_text").map_err(Self::db_error)?;
            let line_subtotal_text: String =
                row.try_get("line_subtotal_text").map_err(Self::db_error)?;

            let unit_price = Self::parse_decimal("unit_price", &unit_price_text)?;
            let line_subtotal = Self::parse_decimal("line_subtotal", &line_subtotal_text)?;
            subtotal += line_subtotal;

            input_values.insert(format!("line_{line_id}"), line_subtotal);
            line_items.push(PricingLineSnapshot {
                line_id,
                product_id: product_id.clone(),
                product_name: product_id,
                quantity,
                unit_price,
                discount_percent: Decimal::ZERO,
                discount_amount: Decimal::ZERO,
                line_subtotal,
            });
        }

        let discount_total = Decimal::ZERO;
        let tax_total = Decimal::ZERO;
        let total = subtotal - discount_total + tax_total;

        let calculation_steps = vec![CalculationStep {
            step_order: 1,
            step_name: "fallback_subtotal".to_string(),
            input_values,
            output_value: subtotal,
            formula: Some("sum(line_subtotal)".to_string()),
        }];

        Ok(PricingSnapshot {
            quote_id: quote_id.clone(),
            version,
            subtotal,
            discount_total,
            tax_total,
            total,
            currency,
            line_items,
            calculation_steps,
            created_at: priced_at,
        })
    }

    async fn persist_snapshot(
        &self,
        snapshot: &PricingSnapshot,
        ledger: &LedgerVersion,
    ) -> Result<(), ExplanationError> {
        let payload = PersistedPricingTrace::from_snapshot(snapshot);
        let payload_json = serde_json::to_string(&payload).map_err(|error| {
            ExplanationError::EvidenceGatheringFailed {
                reason: format!("failed to encode pricing trace payload: {error}"),
            }
        })?;
        let snapshot_id = format!("psnap-{}", sqlx::types::Uuid::new_v4());

        sqlx::query(
            r#"
            INSERT INTO quote_pricing_snapshot (
                id,
                quote_id,
                version,
                ledger_entry_id,
                ledger_content_hash,
                subtotal,
                discount_total,
                tax_total,
                total,
                currency,
                price_book_id,
                pricing_trace_json,
                policy_evaluation_json,
                priced_at,
                priced_by
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (quote_id, version) DO NOTHING
            "#,
        )
        .bind(snapshot_id)
        .bind(&snapshot.quote_id.0)
        .bind(snapshot.version)
        .bind(&ledger.entry_id)
        .bind(&ledger.content_hash)
        .bind(snapshot.subtotal.to_string())
        .bind(snapshot.discount_total.to_string())
        .bind(snapshot.tax_total.to_string())
        .bind(snapshot.total.to_string())
        .bind(&snapshot.currency)
        .bind(Option::<String>::None)
        .bind(payload_json)
        .bind(Option::<String>::None)
        .bind(&snapshot.created_at)
        .bind(&self.priced_by)
        .execute(&self.pool)
        .await
        .map_err(Self::db_error)?;

        Ok(())
    }

    fn parse_decimal(field: &str, value: &str) -> Result<Decimal, ExplanationError> {
        Decimal::from_str(value).map_err(|error| ExplanationError::EvidenceGatheringFailed {
            reason: format!("invalid decimal value for {field}: {error}"),
        })
    }

    fn db_error(error: sqlx::Error) -> ExplanationError {
        ExplanationError::EvidenceGatheringFailed { reason: format!("database error: {error}") }
    }
}

#[async_trait]
impl PricingSnapshotProvider for SqlPricingSnapshotRepository {
    async fn get_snapshot(
        &self,
        quote_id: &QuoteId,
        version: i32,
    ) -> Result<PricingSnapshot, ExplanationError> {
        let currency = self.ensure_quote_exists(quote_id).await?;
        let ledger = self.load_ledger_version(quote_id, version).await?;

        if let Some(row) = self.load_cached_snapshot_row(quote_id, version).await? {
            Self::validate_ledger_linkage(&row, quote_id, version, &ledger)?;
            return Self::snapshot_from_row(&row);
        }

        let snapshot = self
            .build_fallback_snapshot(quote_id, version, currency, ledger.timestamp.clone())
            .await?;
        self.persist_snapshot(&snapshot, &ledger).await?;

        let persisted =
            self.load_cached_snapshot_row(quote_id, version).await?.ok_or_else(|| {
                ExplanationError::MissingPricingSnapshot { quote_id: quote_id.clone() }
            })?;
        Self::validate_ledger_linkage(&persisted, quote_id, version, &ledger)?;
        Self::snapshot_from_row(&persisted)
    }
}

#[derive(Clone, Debug)]
struct LedgerVersion {
    entry_id: String,
    content_hash: String,
    timestamp: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedPricingTrace {
    line_items: Vec<PersistedPricingLineItem>,
    calculation_steps: Vec<PersistedCalculationStep>,
}

impl PersistedPricingTrace {
    fn from_snapshot(snapshot: &PricingSnapshot) -> Self {
        Self {
            line_items: snapshot
                .line_items
                .iter()
                .map(PersistedPricingLineItem::from_pricing_line)
                .collect(),
            calculation_steps: snapshot
                .calculation_steps
                .iter()
                .map(PersistedCalculationStep::from_calculation_step)
                .collect(),
        }
    }

    fn try_into_line_items(&self) -> Result<Vec<PricingLineSnapshot>, ExplanationError> {
        self.line_items
            .iter()
            .cloned()
            .map(PersistedPricingLineItem::try_into_pricing_line)
            .collect()
    }

    fn try_into_steps(&self) -> Result<Vec<CalculationStep>, ExplanationError> {
        self.calculation_steps
            .iter()
            .map(PersistedCalculationStep::try_into_calculation_step)
            .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedPricingLineItem {
    line_id: String,
    product_id: String,
    product_name: String,
    quantity: i32,
    unit_price: String,
    discount_percent: String,
    discount_amount: String,
    line_subtotal: String,
}

impl PersistedPricingLineItem {
    fn from_pricing_line(line: &PricingLineSnapshot) -> Self {
        Self {
            line_id: line.line_id.clone(),
            product_id: line.product_id.clone(),
            product_name: line.product_name.clone(),
            quantity: line.quantity,
            unit_price: line.unit_price.to_string(),
            discount_percent: line.discount_percent.to_string(),
            discount_amount: line.discount_amount.to_string(),
            line_subtotal: line.line_subtotal.to_string(),
        }
    }

    fn try_into_pricing_line(self) -> Result<PricingLineSnapshot, ExplanationError> {
        Ok(PricingLineSnapshot {
            line_id: self.line_id,
            product_id: self.product_id,
            product_name: self.product_name,
            quantity: self.quantity,
            unit_price: SqlPricingSnapshotRepository::parse_decimal(
                "line.unit_price",
                &self.unit_price,
            )?,
            discount_percent: SqlPricingSnapshotRepository::parse_decimal(
                "line.discount_percent",
                &self.discount_percent,
            )?,
            discount_amount: SqlPricingSnapshotRepository::parse_decimal(
                "line.discount_amount",
                &self.discount_amount,
            )?,
            line_subtotal: SqlPricingSnapshotRepository::parse_decimal(
                "line.line_subtotal",
                &self.line_subtotal,
            )?,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedCalculationStep {
    step_order: i32,
    step_name: String,
    input_values: HashMap<String, String>,
    output_value: String,
    formula: Option<String>,
}

impl PersistedCalculationStep {
    fn from_calculation_step(step: &CalculationStep) -> Self {
        let input_values =
            step.input_values.iter().map(|(key, value)| (key.clone(), value.to_string())).collect();
        Self {
            step_order: step.step_order,
            step_name: step.step_name.clone(),
            input_values,
            output_value: step.output_value.to_string(),
            formula: step.formula.clone(),
        }
    }

    fn try_into_calculation_step(&self) -> Result<CalculationStep, ExplanationError> {
        let mut input_values = HashMap::with_capacity(self.input_values.len());
        for (key, value) in &self.input_values {
            input_values.insert(
                key.clone(),
                SqlPricingSnapshotRepository::parse_decimal("step.input_values", value)?,
            );
        }

        Ok(CalculationStep {
            step_order: self.step_order,
            step_name: self.step_name.clone(),
            input_values,
            output_value: SqlPricingSnapshotRepository::parse_decimal(
                "step.output_value",
                &self.output_value,
            )?,
            formula: self.formula.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use quotey_core::{PricingSnapshotProvider, QuoteId};
    use rust_decimal::Decimal;

    use super::{
        PersistedPricingTrace, PricingLineSnapshot, PricingSnapshot, SqlPricingSnapshotRepository,
    };
    use crate::{connect_with_settings, migrations, DbPool};

    #[tokio::test]
    async fn get_snapshot_returns_cached_snapshot_when_present() {
        let pool = setup_pool().await;
        let quote_id = QuoteId("Q-PS-CACHED-001".to_string());
        insert_quote(&pool, &quote_id, "USD").await;
        insert_ledger_entry(&pool, &quote_id, 1, "led-1", "hash-1", None).await;

        let snapshot = sample_snapshot(&quote_id, 1, "USD", "2026-02-24T00:00:00Z");
        insert_snapshot(&pool, &snapshot, "led-1", "hash-1").await;

        let repo = SqlPricingSnapshotRepository::new(pool.clone());
        let fetched = repo.get_snapshot(&quote_id, 1).await.expect("fetch cached snapshot");

        assert_eq!(fetched.quote_id, quote_id);
        assert_eq!(fetched.version, 1);
        assert_eq!(fetched.total, Decimal::new(9000, 2));
        assert_eq!(fetched.line_items.len(), 1);
        assert_eq!(fetched.line_items[0].line_id, "line-1");

        pool.close().await;
    }

    #[tokio::test]
    async fn get_snapshot_builds_and_caches_fallback_when_missing() {
        let pool = setup_pool().await;
        let quote_id = QuoteId("Q-PS-FALLBACK-001".to_string());
        insert_quote(&pool, &quote_id, "USD").await;
        insert_quote_line(&pool, &quote_id, "line-1", "plan-pro", 2, "50.00", Some("100.00")).await;
        insert_quote_line(&pool, &quote_id, "line-2", "support", 1, "25.00", None).await;
        insert_ledger_entry(&pool, &quote_id, 1, "led-fallback-1", "hash-fallback-1", None).await;

        let repo = SqlPricingSnapshotRepository::new(pool.clone());
        let first = repo.get_snapshot(&quote_id, 1).await.expect("fallback snapshot");
        assert_eq!(first.subtotal, Decimal::new(12500, 2));
        assert_eq!(first.discount_total, Decimal::ZERO);
        assert_eq!(first.tax_total, Decimal::ZERO);
        assert_eq!(first.total, Decimal::new(12500, 2));
        assert_eq!(first.line_items.len(), 2);

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM quote_pricing_snapshot WHERE quote_id = ? AND version = 1",
        )
        .bind(&quote_id.0)
        .fetch_one(&pool)
        .await
        .expect("snapshot count");
        assert_eq!(count, 1);

        let second = repo.get_snapshot(&quote_id, 1).await.expect("cached snapshot");
        assert_eq!(second.total, first.total);
        assert_eq!(second.line_items.len(), first.line_items.len());

        pool.close().await;
    }

    #[tokio::test]
    async fn get_snapshot_returns_version_mismatch_for_missing_ledger_version() {
        let pool = setup_pool().await;
        let quote_id = QuoteId("Q-PS-VERSION-001".to_string());
        insert_quote(&pool, &quote_id, "USD").await;
        insert_ledger_entry(&pool, &quote_id, 1, "led-version-1", "hash-version-1", None).await;

        let repo = SqlPricingSnapshotRepository::new(pool.clone());
        let error = repo.get_snapshot(&quote_id, 2).await.expect_err("version mismatch");
        assert!(matches!(
            error,
            quotey_core::ExplanationError::VersionMismatch { expected: 2, actual: 1 }
        ));

        pool.close().await;
    }

    #[tokio::test]
    async fn get_snapshot_returns_quote_not_found_when_quote_missing() {
        let pool = setup_pool().await;
        let quote_id = QuoteId("Q-PS-MISSING-001".to_string());
        let repo = SqlPricingSnapshotRepository::new(pool.clone());

        let error = repo.get_snapshot(&quote_id, 1).await.expect_err("quote missing");
        assert!(matches!(
            error,
            quotey_core::ExplanationError::QuoteNotFound { quote_id: QuoteId(id) } if id == "Q-PS-MISSING-001"
        ));

        pool.close().await;
    }

    #[tokio::test]
    async fn get_snapshot_detects_ledger_mismatch_on_cached_snapshot() {
        let pool = setup_pool().await;
        let quote_id = QuoteId("Q-PS-MISMATCH-001".to_string());
        insert_quote(&pool, &quote_id, "USD").await;
        insert_ledger_entry(&pool, &quote_id, 1, "led-match-1", "hash-match-1", None).await;

        let snapshot = sample_snapshot(&quote_id, 1, "USD", "2026-02-24T00:00:00Z");
        insert_snapshot(&pool, &snapshot, "led-match-1", "hash-other").await;

        let repo = SqlPricingSnapshotRepository::new(pool.clone());
        let error = repo.get_snapshot(&quote_id, 1).await.expect_err("ledger mismatch");
        assert!(matches!(
            error,
            quotey_core::ExplanationError::EvidenceGatheringFailed { reason } if reason.contains("ledger mismatch")
        ));

        pool.close().await;
    }

    fn sample_snapshot(
        quote_id: &QuoteId,
        version: i32,
        currency: &str,
        created_at: &str,
    ) -> PricingSnapshot {
        PricingSnapshot {
            quote_id: quote_id.clone(),
            version,
            subtotal: Decimal::new(9000, 2),
            discount_total: Decimal::ZERO,
            tax_total: Decimal::ZERO,
            total: Decimal::new(9000, 2),
            currency: currency.to_string(),
            line_items: vec![PricingLineSnapshot {
                line_id: "line-1".to_string(),
                product_id: "plan-pro".to_string(),
                product_name: "plan-pro".to_string(),
                quantity: 1,
                unit_price: Decimal::new(9000, 2),
                discount_percent: Decimal::ZERO,
                discount_amount: Decimal::ZERO,
                line_subtotal: Decimal::new(9000, 2),
            }],
            calculation_steps: vec![],
            created_at: created_at.to_string(),
        }
    }

    async fn setup_pool() -> DbPool {
        let pool = connect_with_settings("sqlite::memory:?cache=shared", 1, 30)
            .await
            .expect("connect test pool");
        migrations::run_pending(&pool).await.expect("run migrations");
        pool
    }

    async fn insert_quote(pool: &DbPool, quote_id: &QuoteId, currency: &str) {
        let timestamp = "2026-02-24T00:00:00Z";
        sqlx::query(
            "INSERT INTO quote (id, status, currency, created_by, created_at, updated_at)
             VALUES (?, 'draft', ?, 'U-PS', ?, ?)",
        )
        .bind(&quote_id.0)
        .bind(currency)
        .bind(timestamp)
        .bind(timestamp)
        .execute(pool)
        .await
        .expect("insert quote");
    }

    async fn insert_quote_line(
        pool: &DbPool,
        quote_id: &QuoteId,
        line_id: &str,
        product_id: &str,
        quantity: i32,
        unit_price: &str,
        subtotal: Option<&str>,
    ) {
        let timestamp = "2026-02-24T00:00:00Z";
        sqlx::query(
            r#"
            INSERT INTO quote_line (
                id, quote_id, product_id, quantity, unit_price, subtotal, attributes_json, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, '{}', ?, ?)
            "#,
        )
        .bind(line_id)
        .bind(&quote_id.0)
        .bind(product_id)
        .bind(quantity)
        .bind(unit_price)
        .bind(subtotal)
        .bind(timestamp)
        .bind(timestamp)
        .execute(pool)
        .await
        .expect("insert quote line");
    }

    async fn insert_ledger_entry(
        pool: &DbPool,
        quote_id: &QuoteId,
        version_number: i32,
        entry_id: &str,
        content_hash: &str,
        prev_hash: Option<&str>,
    ) {
        let timestamp = "2026-02-24T00:00:00Z";
        sqlx::query(
            r#"
            INSERT INTO quote_ledger (
                entry_id, quote_id, version_number, content_hash, prev_hash, actor_id, action_type, timestamp, signature, metadata_json
            ) VALUES (?, ?, ?, ?, ?, 'U-PS', 'update', ?, 'sig', '{}')
            "#,
        )
        .bind(entry_id)
        .bind(&quote_id.0)
        .bind(version_number)
        .bind(content_hash)
        .bind(prev_hash)
        .bind(timestamp)
        .execute(pool)
        .await
        .expect("insert ledger");
    }

    async fn insert_snapshot(
        pool: &DbPool,
        snapshot: &PricingSnapshot,
        ledger_entry_id: &str,
        ledger_content_hash: &str,
    ) {
        let payload = PersistedPricingTrace::from_snapshot(snapshot);
        let payload_json = serde_json::to_string(&payload).expect("serialize payload");
        sqlx::query(
            r#"
            INSERT INTO quote_pricing_snapshot (
                id, quote_id, version, ledger_entry_id, ledger_content_hash,
                subtotal, discount_total, tax_total, total, currency,
                price_book_id, pricing_trace_json, policy_evaluation_json, priced_at, priced_by
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, NULL, ?, 'system')
            "#,
        )
        .bind(format!("psnap-test-{}", snapshot.quote_id.0))
        .bind(&snapshot.quote_id.0)
        .bind(snapshot.version)
        .bind(ledger_entry_id)
        .bind(ledger_content_hash)
        .bind(snapshot.subtotal.to_string())
        .bind(snapshot.discount_total.to_string())
        .bind(snapshot.tax_total.to_string())
        .bind(snapshot.total.to_string())
        .bind(&snapshot.currency)
        .bind(payload_json)
        .bind(&snapshot.created_at)
        .execute(pool)
        .await
        .expect("insert snapshot");
    }
}
