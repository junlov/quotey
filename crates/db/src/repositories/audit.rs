use quotey_core::audit::{AuditCategory, AuditEvent, AuditOutcome};
use quotey_core::domain::quote::QuoteId;
use std::collections::BTreeMap;

use super::RepositoryError;
use crate::DbPool;

pub struct SqlAuditEventRepository {
    pool: DbPool,
}

impl SqlAuditEventRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Persist an audit event to the `audit_event` table.
    pub async fn save(&self, event: &AuditEvent) -> Result<(), RepositoryError> {
        let quote_id = event.quote_id.as_ref().map(|q| q.0.as_str());
        let category = format!("{:?}", event.category);

        // Build payload JSON with fields that don't have dedicated columns.
        let payload = serde_json::json!({
            "correlation_id": event.correlation_id,
            "thread_id": event.thread_id,
            "outcome": format!("{:?}", event.outcome),
        });

        let metadata_json =
            serde_json::to_string(&event.metadata).unwrap_or_else(|_| "{}".to_string());

        sqlx::query(
            "INSERT INTO audit_event (id, timestamp, actor, actor_type, quote_id, event_type, event_category, payload_json, metadata_json) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO NOTHING",
        )
        .bind(&event.event_id)
        .bind(event.occurred_at.to_rfc3339())
        .bind(&event.actor)
        .bind("agent")
        .bind(quote_id)
        .bind(&event.event_type)
        .bind(&category)
        .bind(payload.to_string())
        .bind(&metadata_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Find audit events for a specific quote.
    pub async fn find_by_quote_id(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Vec<AuditEvent>, RepositoryError> {
        let rows = sqlx::query_as::<_, AuditEventRow>(
            "SELECT id, timestamp, actor, quote_id, event_type, event_category, payload_json, metadata_json \
             FROM audit_event WHERE quote_id = ? ORDER BY timestamp",
        )
        .bind(&quote_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|r| r.try_into_event()).collect()
    }

    /// Find audit events by type.
    pub async fn find_by_type(&self, event_type: &str) -> Result<Vec<AuditEvent>, RepositoryError> {
        let rows = sqlx::query_as::<_, AuditEventRow>(
            "SELECT id, timestamp, actor, quote_id, event_type, event_category, payload_json, metadata_json \
             FROM audit_event WHERE event_type = ? ORDER BY timestamp",
        )
        .bind(event_type)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|r| r.try_into_event()).collect()
    }

    /// Count audit events for a quote.
    pub async fn count_by_quote_id(&self, quote_id: &QuoteId) -> Result<i64, RepositoryError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_event WHERE quote_id = ?")
            .bind(&quote_id.0)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }
}

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct AuditEventRow {
    id: String,
    timestamp: String,
    actor: String,
    quote_id: Option<String>,
    event_type: String,
    event_category: String,
    payload_json: String,
    metadata_json: Option<String>,
}

impl AuditEventRow {
    fn try_into_event(self) -> Result<AuditEvent, RepositoryError> {
        let occurred_at = self
            .timestamp
            .parse()
            .map_err(|e| RepositoryError::Decode(format!("invalid timestamp: {e}")))?;

        let category = match self.event_category.as_str() {
            "Ingress" => AuditCategory::Ingress,
            "Flow" => AuditCategory::Flow,
            "Pricing" => AuditCategory::Pricing,
            "Policy" => AuditCategory::Policy,
            "Persistence" => AuditCategory::Persistence,
            "System" => AuditCategory::System,
            other => {
                return Err(RepositoryError::Decode(format!("unknown audit category: {other}")))
            }
        };

        let payload: serde_json::Value = serde_json::from_str(&self.payload_json)
            .map_err(|e| RepositoryError::Decode(format!("invalid payload JSON: {e}")))?;

        let correlation_id =
            payload.get("correlation_id").and_then(|v| v.as_str()).unwrap_or("").to_string();

        let thread_id = payload.get("thread_id").and_then(|v| v.as_str()).map(|s| s.to_string());

        let outcome = match payload.get("outcome").and_then(|v| v.as_str()).unwrap_or("Success") {
            "Rejected" => AuditOutcome::Rejected,
            "Failed" => AuditOutcome::Failed,
            _ => AuditOutcome::Success,
        };

        let metadata: BTreeMap<String, String> = self
            .metadata_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        Ok(AuditEvent {
            event_id: self.id,
            quote_id: self.quote_id.map(QuoteId),
            thread_id,
            correlation_id,
            event_type: self.event_type,
            category,
            actor: self.actor,
            outcome,
            metadata,
            occurred_at,
        })
    }
}
