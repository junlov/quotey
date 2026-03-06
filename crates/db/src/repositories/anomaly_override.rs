use sqlx::{Row, SqlitePool};

use quotey_core::cpq::anomaly::{AnomalyOverride, AnomalyRuleKind, AnomalySeverity};

use super::RepositoryError;

pub struct SqlAnomalyOverrideRepository;

impl SqlAnomalyOverrideRepository {
    /// Persist a new anomaly override with justification.
    pub async fn save(pool: &SqlitePool, ovr: &AnomalyOverride) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO anomaly_override (id, quote_id, rule_kind, severity, justification, overridden_by, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&ovr.id)
        .bind(&ovr.quote_id)
        .bind(ovr.rule_kind.as_str())
        .bind(ovr.severity.as_str())
        .bind(&ovr.justification)
        .bind(&ovr.overridden_by)
        .bind(&ovr.created_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// List all overrides for a given quote.
    pub async fn find_by_quote_id(
        pool: &SqlitePool,
        quote_id: &str,
    ) -> Result<Vec<AnomalyOverride>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT id, quote_id, rule_kind, severity, justification, overridden_by, created_at
             FROM anomaly_override
             WHERE quote_id = ?
             ORDER BY created_at DESC",
        )
        .bind(quote_id)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|row| {
                Some(AnomalyOverride {
                    id: row.try_get("id").ok()?,
                    quote_id: row.try_get("quote_id").ok()?,
                    rule_kind: AnomalyRuleKind::parse_label(
                        &row.try_get::<String, _>("rule_kind").ok()?,
                    )?,
                    severity: AnomalySeverity::parse_label(
                        &row.try_get::<String, _>("severity").ok()?,
                    )?,
                    justification: row.try_get("justification").ok()?,
                    overridden_by: row.try_get("overridden_by").ok()?,
                    created_at: row.try_get("created_at").ok()?,
                })
            })
            .collect())
    }

    /// Count overrides by a specific rep (for tracking override rate).
    pub async fn count_by_rep(pool: &SqlitePool, rep: &str) -> Result<i64, RepositoryError> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM anomaly_override WHERE overridden_by = ?")
                .bind(rep)
                .fetch_one(pool)
                .await?;
        Ok(count)
    }

    /// Count all recorded anomaly overrides.
    pub async fn count_all(pool: &SqlitePool) -> Result<i64, RepositoryError> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM anomaly_override").fetch_one(pool).await?;
        Ok(count)
    }
}
