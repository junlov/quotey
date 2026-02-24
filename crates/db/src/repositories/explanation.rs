//! Repository layer for Explain Any Number feature
//!
//! Provides data access for explanation requests, evidence, and audit trails.

use async_trait::async_trait;
use quotey_core::chrono::{DateTime, Utc};
use quotey_core::domain::explanation::{
    CreateExplanationRequest, EvidenceType, ExplanationAuditEvent, ExplanationEventType,
    ExplanationEvidence, ExplanationEvidenceId, ExplanationRequest, ExplanationRequestId,
    ExplanationRequestType, ExplanationStats, ExplanationStatus,
};
use quotey_core::domain::quote::{QuoteId, QuoteLineId};
use sqlx::{sqlite::SqliteRow, Row};

use super::RepositoryError;
use crate::DbPool;

/// Repository for explanation requests and responses
#[async_trait]
pub trait ExplanationRepository: Send + Sync {
    /// Create a new explanation request
    async fn create_request(
        &self,
        request: CreateExplanationRequest,
    ) -> Result<ExplanationRequest, RepositoryError>;

    /// Get explanation request by ID
    async fn get_request(
        &self,
        id: &ExplanationRequestId,
    ) -> Result<Option<ExplanationRequest>, RepositoryError>;

    /// Get all explanation requests for a quote
    async fn list_requests_for_quote(
        &self,
        quote_id: &QuoteId,
        limit: i32,
    ) -> Result<Vec<ExplanationRequest>, RepositoryError>;

    /// Update request status and completion info
    async fn update_request_status(
        &self,
        id: &ExplanationRequestId,
        status: ExplanationStatus,
        error_code: Option<String>,
        error_message: Option<String>,
        latency_ms: Option<i32>,
    ) -> Result<(), RepositoryError>;

    /// Add evidence to an explanation
    async fn add_evidence(
        &self,
        request_id: &ExplanationRequestId,
        evidence_type: EvidenceType,
        evidence_key: String,
        evidence_payload_json: String,
        source_reference: String,
        display_order: i32,
    ) -> Result<ExplanationEvidence, RepositoryError>;

    /// Get evidence for an explanation request
    async fn get_evidence_for_request(
        &self,
        request_id: &ExplanationRequestId,
    ) -> Result<Vec<ExplanationEvidence>, RepositoryError>;

    /// Append audit event
    async fn append_audit_event(
        &self,
        request_id: &ExplanationRequestId,
        event_type: ExplanationEventType,
        event_payload_json: String,
        actor_type: String,
        actor_id: String,
        correlation_id: String,
    ) -> Result<(), RepositoryError>;

    /// Get audit trail for a request
    async fn get_audit_trail(
        &self,
        request_id: &ExplanationRequestId,
    ) -> Result<Vec<ExplanationAuditEvent>, RepositoryError>;

    /// Get explanation statistics
    async fn get_stats(&self) -> Result<ExplanationStats, RepositoryError>;
}

/// SQLite implementation of ExplanationRepository
pub struct SqlExplanationRepository {
    pool: DbPool,
}

impl SqlExplanationRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ExplanationRepository for SqlExplanationRepository {
    async fn create_request(
        &self,
        request: CreateExplanationRequest,
    ) -> Result<ExplanationRequest, RepositoryError> {
        let id = ExplanationRequestId(format!("exp-req-{}", sqlx::types::Uuid::new_v4()));
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO explanation_requests (
                id, quote_id, line_id, request_type, thread_id, actor_id,
                correlation_id, quote_version, status, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id.0)
        .bind(&request.quote_id.0)
        .bind(request.line_id.as_ref().map(|l| &l.0))
        .bind(request.request_type.as_str())
        .bind(&request.thread_id)
        .bind(&request.actor_id)
        .bind(&request.correlation_id)
        .bind(request.quote_version)
        .bind(ExplanationStatus::Pending.as_str())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(ExplanationRequest {
            id,
            quote_id: request.quote_id,
            line_id: request.line_id,
            request_type: request.request_type,
            thread_id: request.thread_id,
            actor_id: request.actor_id,
            correlation_id: request.correlation_id,
            quote_version: request.quote_version,
            pricing_snapshot_id: None,
            status: ExplanationStatus::Pending,
            error_code: None,
            error_message: None,
            latency_ms: None,
            created_at: now,
            completed_at: None,
        })
    }

    async fn get_request(
        &self,
        id: &ExplanationRequestId,
    ) -> Result<Option<ExplanationRequest>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id, quote_id, line_id, request_type, thread_id, actor_id,
                correlation_id, quote_version, pricing_snapshot_id, status,
                error_code, error_message, latency_ms, created_at, completed_at
            FROM explanation_requests
            WHERE id = ?
            "#,
        )
        .bind(&id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| explanation_request_from_row(&r)).transpose()
    }

    async fn list_requests_for_quote(
        &self,
        quote_id: &QuoteId,
        limit: i32,
    ) -> Result<Vec<ExplanationRequest>, RepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, quote_id, line_id, request_type, thread_id, actor_id,
                correlation_id, quote_version, pricing_snapshot_id, status,
                error_code, error_message, latency_ms, created_at, completed_at
            FROM explanation_requests
            WHERE quote_id = ?
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(&quote_id.0)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(explanation_request_from_row).collect()
    }

    async fn update_request_status(
        &self,
        id: &ExplanationRequestId,
        status: ExplanationStatus,
        error_code: Option<String>,
        error_message: Option<String>,
        latency_ms: Option<i32>,
    ) -> Result<(), RepositoryError> {
        let completed_at =
            if status != ExplanationStatus::Pending { Some(Utc::now().to_rfc3339()) } else { None };

        sqlx::query(
            r#"
            UPDATE explanation_requests
            SET status = ?, error_code = ?, error_message = ?, latency_ms = ?, completed_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status.as_str())
        .bind(error_code)
        .bind(error_message)
        .bind(latency_ms)
        .bind(completed_at)
        .bind(&id.0)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn add_evidence(
        &self,
        request_id: &ExplanationRequestId,
        evidence_type: EvidenceType,
        evidence_key: String,
        evidence_payload_json: String,
        source_reference: String,
        display_order: i32,
    ) -> Result<ExplanationEvidence, RepositoryError> {
        let id = ExplanationEvidenceId(format!("exp-ev-{}", sqlx::types::Uuid::new_v4()));
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO explanation_evidence (
                id, explanation_request_id, evidence_type, evidence_key,
                evidence_payload_json, source_reference, display_order, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id.0)
        .bind(&request_id.0)
        .bind(evidence_type.as_str())
        .bind(&evidence_key)
        .bind(&evidence_payload_json)
        .bind(&source_reference)
        .bind(display_order)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(ExplanationEvidence {
            id,
            explanation_request_id: request_id.clone(),
            evidence_type,
            evidence_key,
            evidence_payload_json,
            source_reference,
            display_order,
            created_at: now,
        })
    }

    async fn get_evidence_for_request(
        &self,
        request_id: &ExplanationRequestId,
    ) -> Result<Vec<ExplanationEvidence>, RepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, explanation_request_id, evidence_type, evidence_key,
                evidence_payload_json, source_reference, display_order, created_at
            FROM explanation_evidence
            WHERE explanation_request_id = ?
            ORDER BY display_order ASC, created_at ASC
            "#,
        )
        .bind(&request_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(explanation_evidence_from_row).collect()
    }

    async fn append_audit_event(
        &self,
        request_id: &ExplanationRequestId,
        event_type: ExplanationEventType,
        event_payload_json: String,
        actor_type: String,
        actor_id: String,
        correlation_id: String,
    ) -> Result<(), RepositoryError> {
        let id = format!("exp-audit-{}", sqlx::types::Uuid::new_v4());
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO explanation_audit (
                id, explanation_request_id, event_type, event_payload_json,
                actor_type, actor_id, correlation_id, occurred_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&request_id.0)
        .bind(event_type.as_str())
        .bind(event_payload_json)
        .bind(actor_type)
        .bind(actor_id)
        .bind(correlation_id)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_audit_trail(
        &self,
        request_id: &ExplanationRequestId,
    ) -> Result<Vec<ExplanationAuditEvent>, RepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, explanation_request_id, event_type, event_payload_json,
                actor_type, actor_id, correlation_id, occurred_at
            FROM explanation_audit
            WHERE explanation_request_id = ?
            ORDER BY occurred_at ASC
            "#,
        )
        .bind(&request_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(explanation_audit_event_from_row).collect()
    }

    async fn get_stats(&self) -> Result<ExplanationStats, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                total_requests, success_count, error_count, missing_evidence_count,
                avg_latency_ms, p95_latency_ms, last_updated_at
            FROM explanation_request_stats
            WHERE id = 1
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(explanation_stats_from_row(&row)?)
    }
}

// Helper functions for row mapping

fn explanation_request_from_row(row: &SqliteRow) -> Result<ExplanationRequest, RepositoryError> {
    let id: String = row.try_get("id")?;
    let quote_id: String = row.try_get("quote_id")?;
    let line_id: Option<String> = row.try_get("line_id")?;
    let request_type: String = row.try_get("request_type")?;
    let status: String = row.try_get("status")?;
    let created_at: String = row.try_get("created_at")?;
    let completed_at: Option<String> = row.try_get("completed_at")?;

    Ok(ExplanationRequest {
        id: ExplanationRequestId(id),
        quote_id: QuoteId(quote_id),
        line_id: line_id.map(QuoteLineId),
        request_type: ExplanationRequestType::parse(&request_type).ok_or_else(|| {
            RepositoryError::Decode(format!("invalid request_type: {request_type}"))
        })?,
        thread_id: row.try_get("thread_id")?,
        actor_id: row.try_get("actor_id")?,
        correlation_id: row.try_get("correlation_id")?,
        quote_version: row.try_get("quote_version")?,
        pricing_snapshot_id: row.try_get("pricing_snapshot_id")?,
        status: ExplanationStatus::parse(&status)
            .ok_or_else(|| RepositoryError::Decode(format!("invalid status: {status}")))?,
        error_code: row.try_get("error_code")?,
        error_message: row.try_get("error_message")?,
        latency_ms: row.try_get("latency_ms")?,
        created_at: parse_timestamp("created_at", created_at)?,
        completed_at: completed_at.and_then(|ts| parse_timestamp("completed_at", ts).ok()),
    })
}

fn explanation_evidence_from_row(row: &SqliteRow) -> Result<ExplanationEvidence, RepositoryError> {
    let id: String = row.try_get("id")?;
    let request_id: String = row.try_get("explanation_request_id")?;
    let evidence_type: String = row.try_get("evidence_type")?;
    let created_at: String = row.try_get("created_at")?;

    Ok(ExplanationEvidence {
        id: ExplanationEvidenceId(id),
        explanation_request_id: ExplanationRequestId(request_id),
        evidence_type: EvidenceType::parse(&evidence_type).ok_or_else(|| {
            RepositoryError::Decode(format!("invalid evidence_type: {evidence_type}"))
        })?,
        evidence_key: row.try_get("evidence_key")?,
        evidence_payload_json: row.try_get("evidence_payload_json")?,
        source_reference: row.try_get("source_reference")?,
        display_order: row.try_get("display_order")?,
        created_at: parse_timestamp("created_at", created_at)?,
    })
}

fn explanation_audit_event_from_row(
    row: &SqliteRow,
) -> Result<ExplanationAuditEvent, RepositoryError> {
    let id: String = row.try_get("id")?;
    let request_id: String = row.try_get("explanation_request_id")?;
    let event_type: String = row.try_get("event_type")?;
    let occurred_at: String = row.try_get("occurred_at")?;

    Ok(ExplanationAuditEvent {
        id,
        explanation_request_id: ExplanationRequestId(request_id),
        event_type: ExplanationEventType::parse(&event_type)
            .ok_or_else(|| RepositoryError::Decode(format!("invalid event_type: {event_type}")))?,
        event_payload_json: row.try_get("event_payload_json")?,
        actor_type: row.try_get("actor_type")?,
        actor_id: row.try_get("actor_id")?,
        correlation_id: row.try_get("correlation_id")?,
        occurred_at: parse_timestamp("occurred_at", occurred_at)?,
    })
}

fn explanation_stats_from_row(row: &SqliteRow) -> Result<ExplanationStats, RepositoryError> {
    let last_updated_at: String = row.try_get("last_updated_at")?;

    Ok(ExplanationStats {
        total_requests: row.try_get("total_requests")?,
        success_count: row.try_get("success_count")?,
        error_count: row.try_get("error_count")?,
        missing_evidence_count: row.try_get("missing_evidence_count")?,
        avg_latency_ms: row.try_get("avg_latency_ms")?,
        p95_latency_ms: row.try_get("p95_latency_ms")?,
        last_updated_at: parse_timestamp("last_updated_at", last_updated_at)?,
    })
}

fn parse_timestamp(column: &str, value: String) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(&value)
        .map(|ts| ts.with_timezone(&Utc))
        .map_err(|e| RepositoryError::Decode(format!("invalid timestamp in `{column}`: {e}")))
}

#[cfg(test)]
mod tests {
    use quotey_core::domain::explanation::{
        CreateExplanationRequest, EvidenceType, ExplanationEventType, ExplanationRequestType,
        ExplanationStatus,
    };
    use quotey_core::domain::quote::{QuoteId, QuoteLineId};

    use super::{ExplanationRepository, SqlExplanationRepository};
    use crate::{connect_with_settings, migrations, DbPool};

    #[tokio::test]
    async fn sql_explanation_repo_round_trip_for_request_lifecycle() {
        let pool = setup_pool().await;
        let quote_id = QuoteId("Q-EXP-REQ-001".to_string());
        insert_quote(&pool, &quote_id).await;
        let repo = SqlExplanationRepository::new(pool.clone());

        let created = repo
            .create_request(CreateExplanationRequest {
                quote_id: quote_id.clone(),
                line_id: Some(QuoteLineId("line-1".to_string())),
                request_type: ExplanationRequestType::Line,
                thread_id: "T-EXP-1".to_string(),
                actor_id: "U-123".to_string(),
                correlation_id: "corr-exp-req-1".to_string(),
                quote_version: 2,
            })
            .await
            .expect("create request");

        let fetched = repo.get_request(&created.id).await.expect("get request");
        assert_eq!(fetched, Some(created.clone()));

        let listed = repo.list_requests_for_quote(&quote_id, 10).await.expect("list requests");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);

        repo.update_request_status(&created.id, ExplanationStatus::Success, None, None, Some(120))
            .await
            .expect("update status");

        let updated = repo
            .get_request(&created.id)
            .await
            .expect("get updated request")
            .expect("request exists");
        assert_eq!(updated.status, ExplanationStatus::Success);
        assert_eq!(updated.latency_ms, Some(120));
        assert!(updated.completed_at.is_some());

        pool.close().await;
    }

    #[tokio::test]
    async fn sql_explanation_repo_round_trip_for_evidence_and_audit() {
        let pool = setup_pool().await;
        let quote_id = QuoteId("Q-EXP-REQ-002".to_string());
        insert_quote(&pool, &quote_id).await;
        let repo = SqlExplanationRepository::new(pool.clone());

        let request = repo
            .create_request(CreateExplanationRequest {
                quote_id: quote_id.clone(),
                line_id: None,
                request_type: ExplanationRequestType::Total,
                thread_id: "T-EXP-2".to_string(),
                actor_id: "U-456".to_string(),
                correlation_id: "corr-exp-req-2".to_string(),
                quote_version: 1,
            })
            .await
            .expect("create request");

        repo.add_evidence(
            &request.id,
            EvidenceType::PricingTrace,
            "step:subtotal".to_string(),
            "{\"amount\":\"1000.00\"}".to_string(),
            "pricing_snapshot:abc123:step_1".to_string(),
            0,
        )
        .await
        .expect("add evidence");

        repo.add_evidence(
            &request.id,
            EvidenceType::PolicyEvaluation,
            "policy:discount-cap".to_string(),
            "{\"decision\":\"passed\"}".to_string(),
            "policy_eval:def456".to_string(),
            1,
        )
        .await
        .expect("add evidence");

        let evidence = repo.get_evidence_for_request(&request.id).await.expect("get evidence");
        assert_eq!(evidence.len(), 2);
        assert_eq!(evidence[0].display_order, 0);
        assert_eq!(evidence[1].display_order, 1);

        repo.append_audit_event(
            &request.id,
            ExplanationEventType::RequestReceived,
            "{}".to_string(),
            "user".to_string(),
            "U-456".to_string(),
            "corr-exp-req-2".to_string(),
        )
        .await
        .expect("append audit received");

        repo.append_audit_event(
            &request.id,
            ExplanationEventType::ExplanationDelivered,
            "{\"status\":\"ok\"}".to_string(),
            "system".to_string(),
            "quotey-agent".to_string(),
            "corr-exp-req-2".to_string(),
        )
        .await
        .expect("append audit delivered");

        let audit = repo.get_audit_trail(&request.id).await.expect("get audit");
        assert_eq!(audit.len(), 2);
        assert_eq!(audit[0].event_type, ExplanationEventType::RequestReceived);
        assert_eq!(audit[1].event_type, ExplanationEventType::ExplanationDelivered);

        pool.close().await;
    }

    #[tokio::test]
    async fn sql_explanation_repo_stats_track_status_transitions() {
        let pool = setup_pool().await;
        let quote_id = QuoteId("Q-EXP-REQ-003".to_string());
        insert_quote(&pool, &quote_id).await;
        let repo = SqlExplanationRepository::new(pool.clone());

        let request = repo
            .create_request(CreateExplanationRequest {
                quote_id: quote_id.clone(),
                line_id: None,
                request_type: ExplanationRequestType::Policy,
                thread_id: "T-EXP-3".to_string(),
                actor_id: "U-789".to_string(),
                correlation_id: "corr-exp-req-3".to_string(),
                quote_version: 1,
            })
            .await
            .expect("create request");

        let stats_after_create = repo.get_stats().await.expect("stats after create");
        assert_eq!(stats_after_create.total_requests, 1);
        assert_eq!(stats_after_create.success_count, 0);
        assert_eq!(stats_after_create.error_count, 0);
        assert_eq!(stats_after_create.missing_evidence_count, 0);

        repo.update_request_status(
            &request.id,
            ExplanationStatus::MissingEvidence,
            None,
            Some("no deterministic trace snapshot found".to_string()),
            Some(42),
        )
        .await
        .expect("update to missing evidence");

        let stats_after_update = repo.get_stats().await.expect("stats after update");
        assert_eq!(stats_after_update.total_requests, 1);
        assert_eq!(stats_after_update.success_count, 0);
        assert_eq!(stats_after_update.error_count, 0);
        assert_eq!(stats_after_update.missing_evidence_count, 1);

        pool.close().await;
    }

    async fn setup_pool() -> DbPool {
        let pool = connect_with_settings("sqlite::memory:?cache=shared", 1, 30)
            .await
            .expect("connect test pool");
        migrations::run_pending(&pool).await.expect("run migrations");
        pool
    }

    async fn insert_quote(pool: &DbPool, quote_id: &QuoteId) {
        let timestamp = "2026-02-24T00:00:00Z";
        sqlx::query(
            "INSERT INTO quote (id, status, currency, created_by, created_at, updated_at)
             VALUES (?, 'draft', 'USD', 'U-EXP', ?, ?)",
        )
        .bind(&quote_id.0)
        .bind(timestamp)
        .bind(timestamp)
        .execute(pool)
        .await
        .expect("insert quote");
    }
}
