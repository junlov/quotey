use async_trait::async_trait;
use quotey_core::chrono::{DateTime, Utc};
use quotey_core::domain::quote::QuoteId;
use quotey_core::domain::simulation::{
    CreateScenarioRunRequest, ScenarioAuditEvent, ScenarioAuditEventId, ScenarioAuditEventType,
    ScenarioDelta, ScenarioDeltaId, ScenarioDeltaType, ScenarioRun, ScenarioRunId,
    ScenarioRunStatus, ScenarioVariant, ScenarioVariantId,
};
use sqlx::{sqlite::SqliteRow, Row};

use super::RepositoryError;
use crate::DbPool;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioRunRecord {
    pub id: String,
    pub quote_id: String,
    pub thread_id: String,
    pub actor_id: String,
    pub correlation_id: String,
    pub base_quote_version: i32,
    pub request_params_json: String,
    pub variant_count: i32,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

impl TryFrom<ScenarioRunRecord> for ScenarioRun {
    type Error = RepositoryError;

    fn try_from(value: ScenarioRunRecord) -> Result<Self, Self::Error> {
        let status = ScenarioRunStatus::parse(&value.status).ok_or_else(|| {
            RepositoryError::Decode(format!("invalid scenario run status: {}", value.status))
        })?;

        Ok(Self {
            id: ScenarioRunId(value.id),
            quote_id: QuoteId(value.quote_id),
            thread_id: value.thread_id,
            actor_id: value.actor_id,
            correlation_id: value.correlation_id,
            base_quote_version: value.base_quote_version,
            request_params_json: value.request_params_json,
            variant_count: value.variant_count,
            status,
            error_code: value.error_code,
            error_message: value.error_message,
            created_at: parse_rfc3339("scenario run created_at", &value.created_at)?,
            completed_at: value
                .completed_at
                .as_deref()
                .map(|ts| parse_rfc3339("scenario run completed_at", ts))
                .transpose()?,
        })
    }
}

impl From<ScenarioRun> for ScenarioRunRecord {
    fn from(value: ScenarioRun) -> Self {
        Self {
            id: value.id.0,
            quote_id: value.quote_id.0,
            thread_id: value.thread_id,
            actor_id: value.actor_id,
            correlation_id: value.correlation_id,
            base_quote_version: value.base_quote_version,
            request_params_json: value.request_params_json,
            variant_count: value.variant_count,
            status: value.status.as_str().to_string(),
            error_code: value.error_code,
            error_message: value.error_message,
            created_at: value.created_at.to_rfc3339(),
            completed_at: value.completed_at.map(|ts| ts.to_rfc3339()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioVariantRecord {
    pub id: String,
    pub scenario_run_id: String,
    pub variant_key: String,
    pub variant_order: i32,
    pub params_json: String,
    pub pricing_result_json: String,
    pub policy_result_json: String,
    pub approval_route_json: String,
    pub configuration_result_json: String,
    pub rank_score: f64,
    pub rank_order: i32,
    pub selected_for_promotion: i64,
    pub created_at: String,
}

impl TryFrom<ScenarioVariantRecord> for ScenarioVariant {
    type Error = RepositoryError;

    fn try_from(value: ScenarioVariantRecord) -> Result<Self, Self::Error> {
        let selected_for_promotion = match value.selected_for_promotion {
            0 => false,
            1 => true,
            raw => {
                return Err(RepositoryError::Decode(format!(
                    "invalid selected_for_promotion flag: {}",
                    raw
                )))
            }
        };

        Ok(Self {
            id: ScenarioVariantId(value.id),
            scenario_run_id: ScenarioRunId(value.scenario_run_id),
            variant_key: value.variant_key,
            variant_order: value.variant_order,
            params_json: value.params_json,
            pricing_result_json: value.pricing_result_json,
            policy_result_json: value.policy_result_json,
            approval_route_json: value.approval_route_json,
            configuration_result_json: value.configuration_result_json,
            rank_score: value.rank_score,
            rank_order: value.rank_order,
            selected_for_promotion,
            created_at: parse_rfc3339("scenario variant created_at", &value.created_at)?,
        })
    }
}

impl From<ScenarioVariant> for ScenarioVariantRecord {
    fn from(value: ScenarioVariant) -> Self {
        Self {
            id: value.id.0,
            scenario_run_id: value.scenario_run_id.0,
            variant_key: value.variant_key,
            variant_order: value.variant_order,
            params_json: value.params_json,
            pricing_result_json: value.pricing_result_json,
            policy_result_json: value.policy_result_json,
            approval_route_json: value.approval_route_json,
            configuration_result_json: value.configuration_result_json,
            rank_score: value.rank_score,
            rank_order: value.rank_order,
            selected_for_promotion: if value.selected_for_promotion { 1 } else { 0 },
            created_at: value.created_at.to_rfc3339(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioDeltaRecord {
    pub id: String,
    pub scenario_variant_id: String,
    pub delta_type: String,
    pub delta_payload_json: String,
    pub created_at: String,
}

impl TryFrom<ScenarioDeltaRecord> for ScenarioDelta {
    type Error = RepositoryError;

    fn try_from(value: ScenarioDeltaRecord) -> Result<Self, Self::Error> {
        let delta_type = ScenarioDeltaType::parse(&value.delta_type).ok_or_else(|| {
            RepositoryError::Decode(format!("invalid scenario delta type: {}", value.delta_type))
        })?;

        Ok(Self {
            id: ScenarioDeltaId(value.id),
            scenario_variant_id: ScenarioVariantId(value.scenario_variant_id),
            delta_type,
            delta_payload_json: value.delta_payload_json,
            created_at: parse_rfc3339("scenario delta created_at", &value.created_at)?,
        })
    }
}

impl From<ScenarioDelta> for ScenarioDeltaRecord {
    fn from(value: ScenarioDelta) -> Self {
        Self {
            id: value.id.0,
            scenario_variant_id: value.scenario_variant_id.0,
            delta_type: value.delta_type.as_str().to_string(),
            delta_payload_json: value.delta_payload_json,
            created_at: value.created_at.to_rfc3339(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioAuditEventRecord {
    pub id: String,
    pub scenario_run_id: String,
    pub scenario_variant_id: Option<String>,
    pub event_type: String,
    pub event_payload_json: String,
    pub actor_type: String,
    pub actor_id: String,
    pub correlation_id: String,
    pub occurred_at: String,
}

impl TryFrom<ScenarioAuditEventRecord> for ScenarioAuditEvent {
    type Error = RepositoryError;

    fn try_from(value: ScenarioAuditEventRecord) -> Result<Self, Self::Error> {
        let event_type = ScenarioAuditEventType::parse(&value.event_type).ok_or_else(|| {
            RepositoryError::Decode(format!(
                "invalid scenario audit event type: {}",
                value.event_type
            ))
        })?;

        Ok(Self {
            id: ScenarioAuditEventId(value.id),
            scenario_run_id: ScenarioRunId(value.scenario_run_id),
            scenario_variant_id: value.scenario_variant_id.map(ScenarioVariantId),
            event_type,
            event_payload_json: value.event_payload_json,
            actor_type: value.actor_type,
            actor_id: value.actor_id,
            correlation_id: value.correlation_id,
            occurred_at: parse_rfc3339("scenario audit occurred_at", &value.occurred_at)?,
        })
    }
}

impl From<ScenarioAuditEvent> for ScenarioAuditEventRecord {
    fn from(value: ScenarioAuditEvent) -> Self {
        Self {
            id: value.id.0,
            scenario_run_id: value.scenario_run_id.0,
            scenario_variant_id: value.scenario_variant_id.map(|id| id.0),
            event_type: value.event_type.as_str().to_string(),
            event_payload_json: value.event_payload_json,
            actor_type: value.actor_type,
            actor_id: value.actor_id,
            correlation_id: value.correlation_id,
            occurred_at: value.occurred_at.to_rfc3339(),
        }
    }
}

#[async_trait]
pub trait ScenarioRepository: Send + Sync {
    async fn create_run(
        &self,
        request: CreateScenarioRunRequest,
    ) -> Result<ScenarioRun, RepositoryError>;

    async fn get_run(&self, run_id: &ScenarioRunId)
        -> Result<Option<ScenarioRun>, RepositoryError>;

    async fn list_runs_for_quote(
        &self,
        quote_id: &QuoteId,
        limit: i32,
    ) -> Result<Vec<ScenarioRun>, RepositoryError>;

    async fn update_run_status(
        &self,
        run_id: &ScenarioRunId,
        status: ScenarioRunStatus,
        error_code: Option<String>,
        error_message: Option<String>,
    ) -> Result<(), RepositoryError>;

    #[allow(clippy::too_many_arguments)]
    async fn add_variant(
        &self,
        run_id: &ScenarioRunId,
        variant_key: String,
        variant_order: i32,
        params_json: String,
        pricing_result_json: String,
        policy_result_json: String,
        approval_route_json: String,
        configuration_result_json: String,
        rank_score: f64,
        rank_order: i32,
    ) -> Result<ScenarioVariant, RepositoryError>;

    async fn list_variants_for_run(
        &self,
        run_id: &ScenarioRunId,
    ) -> Result<Vec<ScenarioVariant>, RepositoryError>;

    async fn add_delta(
        &self,
        variant_id: &ScenarioVariantId,
        delta_type: ScenarioDeltaType,
        delta_payload_json: String,
    ) -> Result<ScenarioDelta, RepositoryError>;

    async fn list_deltas_for_variant(
        &self,
        variant_id: &ScenarioVariantId,
    ) -> Result<Vec<ScenarioDelta>, RepositoryError>;

    #[allow(clippy::too_many_arguments)]
    async fn append_audit_event(
        &self,
        run_id: &ScenarioRunId,
        variant_id: Option<ScenarioVariantId>,
        event_type: ScenarioAuditEventType,
        event_payload_json: String,
        actor_type: String,
        actor_id: String,
        correlation_id: String,
    ) -> Result<ScenarioAuditEvent, RepositoryError>;

    async fn list_audit_for_run(
        &self,
        run_id: &ScenarioRunId,
    ) -> Result<Vec<ScenarioAuditEvent>, RepositoryError>;

    async fn promote_variant(
        &self,
        run_id: &ScenarioRunId,
        variant_id: &ScenarioVariantId,
    ) -> Result<(), RepositoryError>;
}

pub struct SqlScenarioRepository {
    pool: DbPool,
}

impl SqlScenarioRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ScenarioRepository for SqlScenarioRepository {
    async fn create_run(
        &self,
        request: CreateScenarioRunRequest,
    ) -> Result<ScenarioRun, RepositoryError> {
        let id = ScenarioRunId(format!("sim-run-{}", sqlx::types::Uuid::new_v4()));
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO deal_flight_scenario_run (
                id, quote_id, thread_id, actor_id, correlation_id,
                base_quote_version, request_params_json, variant_count, status, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id.0)
        .bind(&request.quote_id.0)
        .bind(&request.thread_id)
        .bind(&request.actor_id)
        .bind(&request.correlation_id)
        .bind(request.base_quote_version)
        .bind(&request.request_params_json)
        .bind(request.variant_count)
        .bind(ScenarioRunStatus::Pending.as_str())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(ScenarioRun {
            id,
            quote_id: request.quote_id,
            thread_id: request.thread_id,
            actor_id: request.actor_id,
            correlation_id: request.correlation_id,
            base_quote_version: request.base_quote_version,
            request_params_json: request.request_params_json,
            variant_count: request.variant_count,
            status: ScenarioRunStatus::Pending,
            error_code: None,
            error_message: None,
            created_at: now,
            completed_at: None,
        })
    }

    async fn get_run(
        &self,
        run_id: &ScenarioRunId,
    ) -> Result<Option<ScenarioRun>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id, quote_id, thread_id, actor_id, correlation_id,
                base_quote_version, request_params_json, variant_count,
                status, error_code, error_message, created_at, completed_at
            FROM deal_flight_scenario_run
            WHERE id = ?
            "#,
        )
        .bind(&run_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| scenario_run_from_row(&r)).transpose()
    }

    async fn list_runs_for_quote(
        &self,
        quote_id: &QuoteId,
        limit: i32,
    ) -> Result<Vec<ScenarioRun>, RepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, quote_id, thread_id, actor_id, correlation_id,
                base_quote_version, request_params_json, variant_count,
                status, error_code, error_message, created_at, completed_at
            FROM deal_flight_scenario_run
            WHERE quote_id = ?
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(&quote_id.0)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(scenario_run_from_row).collect()
    }

    async fn update_run_status(
        &self,
        run_id: &ScenarioRunId,
        status: ScenarioRunStatus,
        error_code: Option<String>,
        error_message: Option<String>,
    ) -> Result<(), RepositoryError> {
        let completed_at = if matches!(
            status,
            ScenarioRunStatus::Success
                | ScenarioRunStatus::Failed
                | ScenarioRunStatus::Promoted
                | ScenarioRunStatus::Cancelled
        ) {
            Some(Utc::now().to_rfc3339())
        } else {
            None
        };

        sqlx::query(
            r#"
            UPDATE deal_flight_scenario_run
            SET status = ?, error_code = ?, error_message = ?, completed_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status.as_str())
        .bind(error_code)
        .bind(error_message)
        .bind(completed_at)
        .bind(&run_id.0)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn add_variant(
        &self,
        run_id: &ScenarioRunId,
        variant_key: String,
        variant_order: i32,
        params_json: String,
        pricing_result_json: String,
        policy_result_json: String,
        approval_route_json: String,
        configuration_result_json: String,
        rank_score: f64,
        rank_order: i32,
    ) -> Result<ScenarioVariant, RepositoryError> {
        let id = ScenarioVariantId(format!("sim-var-{}", sqlx::types::Uuid::new_v4()));
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO deal_flight_scenario_variant (
                id, scenario_run_id, variant_key, variant_order, params_json,
                pricing_result_json, policy_result_json, approval_route_json,
                configuration_result_json, rank_score, rank_order,
                selected_for_promotion, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?)
            "#,
        )
        .bind(&id.0)
        .bind(&run_id.0)
        .bind(&variant_key)
        .bind(variant_order)
        .bind(&params_json)
        .bind(&pricing_result_json)
        .bind(&policy_result_json)
        .bind(&approval_route_json)
        .bind(&configuration_result_json)
        .bind(rank_score)
        .bind(rank_order)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(ScenarioVariant {
            id,
            scenario_run_id: run_id.clone(),
            variant_key,
            variant_order,
            params_json,
            pricing_result_json,
            policy_result_json,
            approval_route_json,
            configuration_result_json,
            rank_score,
            rank_order,
            selected_for_promotion: false,
            created_at: now,
        })
    }

    async fn list_variants_for_run(
        &self,
        run_id: &ScenarioRunId,
    ) -> Result<Vec<ScenarioVariant>, RepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, scenario_run_id, variant_key, variant_order, params_json,
                pricing_result_json, policy_result_json, approval_route_json,
                configuration_result_json, rank_score, rank_order,
                selected_for_promotion, created_at
            FROM deal_flight_scenario_variant
            WHERE scenario_run_id = ?
            ORDER BY variant_order ASC, created_at ASC
            "#,
        )
        .bind(&run_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(scenario_variant_from_row).collect()
    }

    async fn add_delta(
        &self,
        variant_id: &ScenarioVariantId,
        delta_type: ScenarioDeltaType,
        delta_payload_json: String,
    ) -> Result<ScenarioDelta, RepositoryError> {
        let id = ScenarioDeltaId(format!("sim-delta-{}", sqlx::types::Uuid::new_v4()));
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO deal_flight_scenario_delta (
                id, scenario_variant_id, delta_type, delta_payload_json, created_at
            ) VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id.0)
        .bind(&variant_id.0)
        .bind(delta_type.as_str())
        .bind(&delta_payload_json)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(ScenarioDelta {
            id,
            scenario_variant_id: variant_id.clone(),
            delta_type,
            delta_payload_json,
            created_at: now,
        })
    }

    async fn list_deltas_for_variant(
        &self,
        variant_id: &ScenarioVariantId,
    ) -> Result<Vec<ScenarioDelta>, RepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT id, scenario_variant_id, delta_type, delta_payload_json, created_at
            FROM deal_flight_scenario_delta
            WHERE scenario_variant_id = ?
            ORDER BY created_at ASC
            "#,
        )
        .bind(&variant_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(scenario_delta_from_row).collect()
    }

    #[allow(clippy::too_many_arguments)]
    async fn append_audit_event(
        &self,
        run_id: &ScenarioRunId,
        variant_id: Option<ScenarioVariantId>,
        event_type: ScenarioAuditEventType,
        event_payload_json: String,
        actor_type: String,
        actor_id: String,
        correlation_id: String,
    ) -> Result<ScenarioAuditEvent, RepositoryError> {
        let id = ScenarioAuditEventId(format!("sim-audit-{}", sqlx::types::Uuid::new_v4()));
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO deal_flight_scenario_audit (
                id, scenario_run_id, scenario_variant_id, event_type, event_payload_json,
                actor_type, actor_id, correlation_id, occurred_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id.0)
        .bind(&run_id.0)
        .bind(variant_id.as_ref().map(|v| &v.0))
        .bind(event_type.as_str())
        .bind(&event_payload_json)
        .bind(&actor_type)
        .bind(&actor_id)
        .bind(&correlation_id)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(ScenarioAuditEvent {
            id,
            scenario_run_id: run_id.clone(),
            scenario_variant_id: variant_id,
            event_type,
            event_payload_json,
            actor_type,
            actor_id,
            correlation_id,
            occurred_at: now,
        })
    }

    async fn list_audit_for_run(
        &self,
        run_id: &ScenarioRunId,
    ) -> Result<Vec<ScenarioAuditEvent>, RepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, scenario_run_id, scenario_variant_id, event_type,
                event_payload_json, actor_type, actor_id, correlation_id, occurred_at
            FROM deal_flight_scenario_audit
            WHERE scenario_run_id = ?
            ORDER BY occurred_at ASC
            "#,
        )
        .bind(&run_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(scenario_audit_event_from_row).collect()
    }

    async fn promote_variant(
        &self,
        run_id: &ScenarioRunId,
        variant_id: &ScenarioVariantId,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "UPDATE deal_flight_scenario_variant SET selected_for_promotion = 0 WHERE scenario_run_id = ?",
        )
        .bind(&run_id.0)
        .execute(&mut *tx)
        .await?;

        let selected = sqlx::query(
            "UPDATE deal_flight_scenario_variant
             SET selected_for_promotion = 1
             WHERE id = ? AND scenario_run_id = ?",
        )
        .bind(&variant_id.0)
        .bind(&run_id.0)
        .execute(&mut *tx)
        .await?;

        if selected.rows_affected() == 0 {
            return Err(RepositoryError::Decode(format!(
                "scenario variant {} not found for run {}",
                variant_id.0, run_id.0
            )));
        }

        sqlx::query(
            "UPDATE deal_flight_scenario_run
             SET status = ?, completed_at = ?, error_code = NULL, error_message = NULL
             WHERE id = ?",
        )
        .bind(ScenarioRunStatus::Promoted.as_str())
        .bind(Utc::now().to_rfc3339())
        .bind(&run_id.0)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
}

fn scenario_run_record_from_row(row: &SqliteRow) -> Result<ScenarioRunRecord, RepositoryError> {
    Ok(ScenarioRunRecord {
        id: row.try_get("id")?,
        quote_id: row.try_get("quote_id")?,
        thread_id: row.try_get("thread_id")?,
        actor_id: row.try_get("actor_id")?,
        correlation_id: row.try_get("correlation_id")?,
        base_quote_version: row.try_get("base_quote_version")?,
        request_params_json: row.try_get("request_params_json")?,
        variant_count: row.try_get("variant_count")?,
        status: row.try_get("status")?,
        error_code: row.try_get("error_code")?,
        error_message: row.try_get("error_message")?,
        created_at: row.try_get("created_at")?,
        completed_at: row.try_get("completed_at")?,
    })
}

fn scenario_variant_record_from_row(
    row: &SqliteRow,
) -> Result<ScenarioVariantRecord, RepositoryError> {
    Ok(ScenarioVariantRecord {
        id: row.try_get("id")?,
        scenario_run_id: row.try_get("scenario_run_id")?,
        variant_key: row.try_get("variant_key")?,
        variant_order: row.try_get("variant_order")?,
        params_json: row.try_get("params_json")?,
        pricing_result_json: row.try_get("pricing_result_json")?,
        policy_result_json: row.try_get("policy_result_json")?,
        approval_route_json: row.try_get("approval_route_json")?,
        configuration_result_json: row.try_get("configuration_result_json")?,
        rank_score: row.try_get("rank_score")?,
        rank_order: row.try_get("rank_order")?,
        selected_for_promotion: row.try_get("selected_for_promotion")?,
        created_at: row.try_get("created_at")?,
    })
}

fn scenario_delta_record_from_row(row: &SqliteRow) -> Result<ScenarioDeltaRecord, RepositoryError> {
    Ok(ScenarioDeltaRecord {
        id: row.try_get("id")?,
        scenario_variant_id: row.try_get("scenario_variant_id")?,
        delta_type: row.try_get("delta_type")?,
        delta_payload_json: row.try_get("delta_payload_json")?,
        created_at: row.try_get("created_at")?,
    })
}

fn scenario_audit_record_from_row(
    row: &SqliteRow,
) -> Result<ScenarioAuditEventRecord, RepositoryError> {
    Ok(ScenarioAuditEventRecord {
        id: row.try_get("id")?,
        scenario_run_id: row.try_get("scenario_run_id")?,
        scenario_variant_id: row.try_get("scenario_variant_id")?,
        event_type: row.try_get("event_type")?,
        event_payload_json: row.try_get("event_payload_json")?,
        actor_type: row.try_get("actor_type")?,
        actor_id: row.try_get("actor_id")?,
        correlation_id: row.try_get("correlation_id")?,
        occurred_at: row.try_get("occurred_at")?,
    })
}

fn scenario_run_from_row(row: &SqliteRow) -> Result<ScenarioRun, RepositoryError> {
    ScenarioRun::try_from(scenario_run_record_from_row(row)?)
}

fn scenario_variant_from_row(row: &SqliteRow) -> Result<ScenarioVariant, RepositoryError> {
    ScenarioVariant::try_from(scenario_variant_record_from_row(row)?)
}

fn scenario_delta_from_row(row: &SqliteRow) -> Result<ScenarioDelta, RepositoryError> {
    ScenarioDelta::try_from(scenario_delta_record_from_row(row)?)
}

fn scenario_audit_event_from_row(row: &SqliteRow) -> Result<ScenarioAuditEvent, RepositoryError> {
    ScenarioAuditEvent::try_from(scenario_audit_record_from_row(row)?)
}

fn parse_rfc3339(field: &str, value: &str) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(value).map(|ts| ts.with_timezone(&Utc)).map_err(|err| {
        RepositoryError::Decode(format!("invalid {} timestamp '{}': {}", field, value, err))
    })
}

#[cfg(test)]
mod tests {
    use quotey_core::chrono::Utc;
    use quotey_core::domain::quote::QuoteId;
    use quotey_core::domain::simulation::{
        CreateScenarioRunRequest, ScenarioAuditEvent, ScenarioAuditEventId, ScenarioAuditEventType,
        ScenarioDelta, ScenarioDeltaId, ScenarioDeltaType, ScenarioRun, ScenarioRunId,
        ScenarioRunStatus, ScenarioVariant, ScenarioVariantId,
    };

    use super::{
        RepositoryError, ScenarioAuditEventRecord, ScenarioDeltaRecord, ScenarioRepository,
        ScenarioRunRecord, ScenarioVariantRecord, SqlScenarioRepository,
    };
    use crate::{connect_with_settings, migrations, DbPool};

    type TestResult<T> = Result<T, String>;

    #[test]
    fn scenario_run_record_round_trip() -> TestResult<()> {
        let run = ScenarioRun {
            id: ScenarioRunId("sim-run-1".to_string()),
            quote_id: QuoteId("Q-100".to_string()),
            thread_id: "thread-1".to_string(),
            actor_id: "U123".to_string(),
            correlation_id: "corr-1".to_string(),
            base_quote_version: 3,
            request_params_json: "{\"discount\":10}".to_string(),
            variant_count: 3,
            status: ScenarioRunStatus::Pending,
            error_code: None,
            error_message: None,
            created_at: Utc::now(),
            completed_at: None,
        };

        let round_trip = ScenarioRun::try_from(ScenarioRunRecord::from(run.clone()))
            .map_err(|error| format!("decode run: {error}"))?;
        if round_trip.id != run.id {
            return Err(format!("run id mismatch: {:?} != {:?}", round_trip.id, run.id));
        }
        if round_trip.quote_id != run.quote_id {
            return Err(format!(
                "run quote_id mismatch: {:?} != {:?}",
                round_trip.quote_id, run.quote_id
            ));
        }
        if round_trip.status != run.status {
            return Err(format!(
                "run status mismatch: {:?} != {:?}",
                round_trip.status, run.status
            ));
        }
        if round_trip.variant_count != run.variant_count {
            return Err(format!(
                "run variant_count mismatch: {:?} != {:?}",
                round_trip.variant_count, run.variant_count
            ));
        }

        Ok(())
    }

    #[test]
    fn scenario_variant_record_round_trip() -> TestResult<()> {
        let variant = ScenarioVariant {
            id: ScenarioVariantId("sim-var-1".to_string()),
            scenario_run_id: ScenarioRunId("sim-run-1".to_string()),
            variant_key: "v1".to_string(),
            variant_order: 1,
            params_json: "{}".to_string(),
            pricing_result_json: "{}".to_string(),
            policy_result_json: "{}".to_string(),
            approval_route_json: "{}".to_string(),
            configuration_result_json: "{}".to_string(),
            rank_score: 1.5,
            rank_order: 0,
            selected_for_promotion: true,
            created_at: Utc::now(),
        };

        let round_trip = ScenarioVariant::try_from(ScenarioVariantRecord::from(variant.clone()))
            .map_err(|error| format!("decode variant: {error}"))?;
        if round_trip.id != variant.id {
            return Err(format!("variant id mismatch: {:?} != {:?}", round_trip.id, variant.id));
        }
        if round_trip.scenario_run_id != variant.scenario_run_id {
            return Err(format!(
                "variant scenario_run_id mismatch: {:?} != {:?}",
                round_trip.scenario_run_id, variant.scenario_run_id
            ));
        }
        if !round_trip.selected_for_promotion {
            return Err("selected_for_promotion should remain true".to_string());
        }

        Ok(())
    }

    #[test]
    fn scenario_delta_record_round_trip() -> TestResult<()> {
        let delta = ScenarioDelta {
            id: ScenarioDeltaId("sim-delta-1".to_string()),
            scenario_variant_id: ScenarioVariantId("sim-var-1".to_string()),
            delta_type: ScenarioDeltaType::Policy,
            delta_payload_json: "{\"new_failures\":1}".to_string(),
            created_at: Utc::now(),
        };

        let round_trip = ScenarioDelta::try_from(ScenarioDeltaRecord::from(delta.clone()))
            .map_err(|error| format!("decode delta: {error}"))?;
        if round_trip.id != delta.id {
            return Err(format!("delta id mismatch: {:?} != {:?}", round_trip.id, delta.id));
        }
        if round_trip.delta_type != ScenarioDeltaType::Policy {
            return Err(format!(
                "delta type mismatch: {:?} != {:?}",
                round_trip.delta_type,
                ScenarioDeltaType::Policy
            ));
        }

        Ok(())
    }

    #[test]
    fn scenario_audit_record_round_trip() -> TestResult<()> {
        let event = ScenarioAuditEvent {
            id: ScenarioAuditEventId("sim-audit-1".to_string()),
            scenario_run_id: ScenarioRunId("sim-run-1".to_string()),
            scenario_variant_id: Some(ScenarioVariantId("sim-var-1".to_string())),
            event_type: ScenarioAuditEventType::VariantGenerated,
            event_payload_json: "{\"variant\":\"v1\"}".to_string(),
            actor_type: "agent".to_string(),
            actor_id: "sim-engine".to_string(),
            correlation_id: "corr-1".to_string(),
            occurred_at: Utc::now(),
        };

        let round_trip =
            ScenarioAuditEvent::try_from(ScenarioAuditEventRecord::from(event.clone()))
                .map_err(|error| format!("decode audit: {error}"))?;
        if round_trip.id != event.id {
            return Err(format!("audit event id mismatch: {:?} != {:?}", round_trip.id, event.id));
        }
        if round_trip.event_type != ScenarioAuditEventType::VariantGenerated {
            return Err(format!(
                "audit event type mismatch: {:?} != {:?}",
                round_trip.event_type,
                ScenarioAuditEventType::VariantGenerated
            ));
        }
        if round_trip.scenario_variant_id != event.scenario_variant_id {
            return Err(format!(
                "audit scenario_variant_id mismatch: {:?} != {:?}",
                round_trip.scenario_variant_id, event.scenario_variant_id
            ));
        }

        Ok(())
    }

    #[tokio::test]
    async fn sql_scenario_repo_round_trip_for_run_lifecycle() -> TestResult<()> {
        let pool = setup_pool().await?;
        let quote_id = QuoteId("Q-SIM-001".to_string());
        insert_quote(&pool, &quote_id).await?;
        let repo = SqlScenarioRepository::new(pool.clone());

        let run = repo
            .create_run(CreateScenarioRunRequest {
                quote_id: quote_id.clone(),
                thread_id: "T-SIM-1".to_string(),
                actor_id: "U-SIM-1".to_string(),
                correlation_id: "corr-sim-1".to_string(),
                base_quote_version: 2,
                request_params_json: "{\"count\":2}".to_string(),
                variant_count: 2,
            })
            .await
            .map_err(|error| format!("create run: {error}"))?;

        let fetched = repo.get_run(&run.id).await.map_err(|error| format!("get run: {error}"))?;
        let fetched =
            fetched.ok_or_else(|| "run should be present after create_run".to_string())?;
        if fetched.id != run.id {
            return Err(format!("fetched run id mismatch: {:?} != {:?}", fetched.id, run.id));
        }
        if fetched.status != ScenarioRunStatus::Pending {
            return Err(format!(
                "fetched run status mismatch: {:?} != {:?}",
                fetched.status,
                ScenarioRunStatus::Pending
            ));
        }

        repo.update_run_status(
            &run.id,
            ScenarioRunStatus::Success,
            None,
            Some("all variants generated".to_string()),
        )
        .await
        .map_err(|error| format!("update run status: {error}"))?;

        let updated =
            repo.get_run(&run.id).await.map_err(|error| format!("re-fetch run: {error}"))?;
        let updated =
            updated.ok_or_else(|| "run should still exist after status update".to_string())?;
        if updated.status != ScenarioRunStatus::Success {
            return Err(format!(
                "run status after update mismatch: {:?} != {:?}",
                updated.status,
                ScenarioRunStatus::Success
            ));
        }
        if updated.completed_at.is_none() {
            return Err("run should have completion timestamp after success".to_string());
        }

        let listed = repo
            .list_runs_for_quote(&quote_id, 10)
            .await
            .map_err(|error| format!("list runs: {error}"))?;
        if listed.len() != 1 {
            return Err(format!("expected 1 run, got {}", listed.len()));
        }
        if listed[0].id != run.id {
            return Err(format!("listed run id mismatch: {:?} != {:?}", listed[0].id, run.id));
        }

        pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn sql_scenario_repo_round_trip_for_variant_delta_audit_and_promotion() -> TestResult<()>
    {
        let pool = setup_pool().await?;
        let quote_id = QuoteId("Q-SIM-002".to_string());
        insert_quote(&pool, &quote_id).await?;
        let repo = SqlScenarioRepository::new(pool.clone());

        let run = repo
            .create_run(CreateScenarioRunRequest {
                quote_id: quote_id.clone(),
                thread_id: "T-SIM-2".to_string(),
                actor_id: "U-SIM-2".to_string(),
                correlation_id: "corr-sim-2".to_string(),
                base_quote_version: 1,
                request_params_json: "{\"discounts\":[0,10]}".to_string(),
                variant_count: 2,
            })
            .await
            .map_err(|error| format!("create run: {error}"))?;

        let baseline = repo
            .add_variant(
                &run.id,
                "baseline".to_string(),
                0,
                "{}".to_string(),
                "{\"total\":\"1000.00\"}".to_string(),
                "{\"status\":\"approved\"}".to_string(),
                "{\"route\":[]}".to_string(),
                "{\"constraints\":\"ok\"}".to_string(),
                0.0,
                0,
            )
            .await
            .map_err(|error| format!("add baseline variant: {error}"))?;

        let discounted = repo
            .add_variant(
                &run.id,
                "discounted_10".to_string(),
                1,
                "{\"discount_pct\":10}".to_string(),
                "{\"total\":\"900.00\"}".to_string(),
                "{\"status\":\"approval_required\"}".to_string(),
                "{\"route\":[\"sales_manager\"]}".to_string(),
                "{\"constraints\":\"ok\"}".to_string(),
                1.0,
                1,
            )
            .await
            .map_err(|error| format!("add discounted variant: {error}"))?;

        repo.add_delta(
            &discounted.id,
            ScenarioDeltaType::Price,
            "{\"total_delta\":\"-100.00\"}".to_string(),
        )
        .await
        .map_err(|error| format!("add price delta: {error}"))?;

        repo.append_audit_event(
            &run.id,
            Some(discounted.id.clone()),
            ScenarioAuditEventType::VariantGenerated,
            "{\"variant_key\":\"discounted_10\"}".to_string(),
            "agent".to_string(),
            "sim-engine".to_string(),
            "corr-sim-2".to_string(),
        )
        .await
        .map_err(|error| format!("append audit event: {error}"))?;

        repo.promote_variant(&run.id, &discounted.id)
            .await
            .map_err(|error| format!("promote discounted variant: {error}"))?;

        let variants = repo
            .list_variants_for_run(&run.id)
            .await
            .map_err(|error| format!("list variants: {error}"))?;
        if variants.len() != 2 {
            return Err(format!("expected 2 variants, got {}", variants.len()));
        }
        let discounted_variant = variants
            .iter()
            .find(|variant| variant.id == discounted.id)
            .ok_or_else(|| "discounted variant exists".to_string())?;
        if !discounted_variant.selected_for_promotion {
            return Err("discounted variant should be selected".to_string());
        }
        let baseline_variant = variants
            .iter()
            .find(|variant| variant.id == baseline.id)
            .ok_or_else(|| "baseline variant exists".to_string())?;
        if baseline_variant.selected_for_promotion {
            return Err("baseline variant should not be selected".to_string());
        }

        let deltas = repo
            .list_deltas_for_variant(&discounted.id)
            .await
            .map_err(|error| format!("list deltas: {error}"))?;
        if deltas.len() != 1 {
            return Err(format!("expected 1 delta, got {}", deltas.len()));
        }
        if deltas[0].delta_type != ScenarioDeltaType::Price {
            return Err(format!(
                "delta type mismatch: {:?} != {:?}",
                deltas[0].delta_type,
                ScenarioDeltaType::Price
            ));
        }

        let audit = repo
            .list_audit_for_run(&run.id)
            .await
            .map_err(|error| format!("list audit: {error}"))?;
        if audit.len() != 1 {
            return Err(format!("expected 1 audit row, got {}", audit.len()));
        }
        if audit[0].event_type != ScenarioAuditEventType::VariantGenerated {
            return Err(format!(
                "audit event type mismatch: {:?} != {:?}",
                audit[0].event_type,
                ScenarioAuditEventType::VariantGenerated
            ));
        }

        let promoted_run =
            repo.get_run(&run.id).await.map_err(|error| format!("get promoted run: {error}"))?;
        let promoted_run =
            promoted_run.ok_or_else(|| "run should exist after promotion".to_string())?;
        if promoted_run.status != ScenarioRunStatus::Promoted {
            return Err(format!(
                "run status mismatch: {:?} != {:?}",
                promoted_run.status,
                ScenarioRunStatus::Promoted
            ));
        }
        if promoted_run.completed_at.is_none() {
            return Err("promoted run should have completion timestamp".to_string());
        }

        pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn sql_scenario_repo_promote_missing_variant_returns_decode_error() -> TestResult<()> {
        let pool = setup_pool().await?;
        let quote_id = QuoteId("Q-SIM-003".to_string());
        insert_quote(&pool, &quote_id).await?;
        let repo = SqlScenarioRepository::new(pool.clone());

        let run = repo
            .create_run(CreateScenarioRunRequest {
                quote_id,
                thread_id: "T-SIM-3".to_string(),
                actor_id: "U-SIM-3".to_string(),
                correlation_id: "corr-sim-3".to_string(),
                base_quote_version: 1,
                request_params_json: "{}".to_string(),
                variant_count: 1,
            })
            .await
            .map_err(|error| format!("create run: {error}"))?;

        let promote_result =
            repo.promote_variant(&run.id, &ScenarioVariantId("sim-var-missing".to_string())).await;
        let error = match promote_result {
            Ok(_) => return Err("promote missing variant should return an error".to_string()),
            Err(error) => error,
        };
        if !matches!(&error, RepositoryError::Decode(message) if message.contains("not found")) {
            return Err(format!("unexpected promote error: {error}"));
        }

        pool.close().await;
        Ok(())
    }

    async fn setup_pool() -> TestResult<DbPool> {
        let pool = connect_with_settings("sqlite::memory:?cache=shared", 1, 30)
            .await
            .map_err(|error| format!("connect test pool: {error}"))?;
        migrations::run_pending(&pool).await.map_err(|error| format!("run migrations: {error}"))?;
        Ok(pool)
    }

    async fn insert_quote(pool: &DbPool, quote_id: &QuoteId) -> TestResult<()> {
        let timestamp = "2026-02-24T01:00:00Z";
        sqlx::query(
            "INSERT INTO quote (id, status, currency, created_by, created_at, updated_at)
             VALUES (?, 'draft', 'USD', 'U-SIM', ?, ?)",
        )
        .bind(&quote_id.0)
        .bind(timestamp)
        .bind(timestamp)
        .execute(pool)
        .await
        .map_err(|error| format!("insert quote: {error}"))?;
        Ok(())
    }
}
