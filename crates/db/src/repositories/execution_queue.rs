use quotey_core::chrono::{DateTime, Utc};
use sqlx::{sqlite::SqliteRow, Row};

use quotey_core::domain::execution::{
    ExecutionTask, ExecutionTaskId, ExecutionTaskState, ExecutionTransitionEvent,
    ExecutionTransitionId, IdempotencyRecord, IdempotencyRecordState, OperationKey,
};
use quotey_core::domain::quote::QuoteId;

use super::{ExecutionQueueRepository, IdempotencyRepository, RepositoryError};
use crate::DbPool;

pub struct SqlExecutionQueueRepository {
    pool: DbPool,
}

impl SqlExecutionQueueRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ExecutionQueueRepository for SqlExecutionQueueRepository {
    async fn find_task_by_id(
        &self,
        id: &ExecutionTaskId,
    ) -> Result<Option<ExecutionTask>, RepositoryError> {
        let row = sqlx::query(
            "SELECT
                id,
                quote_id,
                operation_kind,
                payload_json,
                idempotency_key,
                state,
                retry_count,
                max_retries,
                available_at,
                claimed_by,
                claimed_at,
                last_error,
                result_fingerprint,
                state_version,
                created_at,
                updated_at
             FROM execution_queue_task
             WHERE id = ?",
        )
        .bind(&id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.map(task_from_row).transpose()
    }

    async fn list_tasks_for_quote(
        &self,
        quote_id: &QuoteId,
        state: Option<ExecutionTaskState>,
    ) -> Result<Vec<ExecutionTask>, RepositoryError> {
        let rows = if let Some(state) = state {
            sqlx::query(
                "SELECT
                    id,
                    quote_id,
                    operation_kind,
                    payload_json,
                    idempotency_key,
                    state,
                    retry_count,
                    max_retries,
                    available_at,
                    claimed_by,
                    claimed_at,
                    last_error,
                    result_fingerprint,
                    state_version,
                    created_at,
                    updated_at
                 FROM execution_queue_task
                 WHERE quote_id = ? AND state = ?
                 ORDER BY available_at ASC, created_at ASC",
            )
            .bind(&quote_id.0)
            .bind(state.as_str())
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT
                    id,
                    quote_id,
                    operation_kind,
                    payload_json,
                    idempotency_key,
                    state,
                    retry_count,
                    max_retries,
                    available_at,
                    claimed_by,
                    claimed_at,
                    last_error,
                    result_fingerprint,
                    state_version,
                    created_at,
                    updated_at
                 FROM execution_queue_task
                 WHERE quote_id = ?
                 ORDER BY available_at ASC, created_at ASC",
            )
            .bind(&quote_id.0)
            .fetch_all(&self.pool)
            .await?
        };

        rows.into_iter().map(task_from_row).collect()
    }

    async fn save_task(&self, task: ExecutionTask) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO execution_queue_task (
                id,
                quote_id,
                operation_kind,
                payload_json,
                idempotency_key,
                state,
                retry_count,
                max_retries,
                available_at,
                claimed_by,
                claimed_at,
                last_error,
                result_fingerprint,
                state_version,
                created_at,
                updated_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                quote_id = excluded.quote_id,
                operation_kind = excluded.operation_kind,
                payload_json = excluded.payload_json,
                idempotency_key = excluded.idempotency_key,
                state = excluded.state,
                retry_count = excluded.retry_count,
                max_retries = excluded.max_retries,
                available_at = excluded.available_at,
                claimed_by = excluded.claimed_by,
                claimed_at = excluded.claimed_at,
                last_error = excluded.last_error,
                result_fingerprint = excluded.result_fingerprint,
                state_version = excluded.state_version,
                updated_at = excluded.updated_at",
        )
        .bind(&task.id.0)
        .bind(&task.quote_id.0)
        .bind(&task.operation_kind)
        .bind(&task.payload_json)
        .bind(&task.idempotency_key.0)
        .bind(task.state.as_str())
        .bind(i64::from(task.retry_count))
        .bind(i64::from(task.max_retries))
        .bind(task.available_at.to_rfc3339())
        .bind(task.claimed_by.as_deref())
        .bind(task.claimed_at.map(|value| value.to_rfc3339()))
        .bind(task.last_error.as_deref())
        .bind(task.result_fingerprint.as_deref())
        .bind(i64::from(task.state_version))
        .bind(task.created_at.to_rfc3339())
        .bind(task.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn append_transition(
        &self,
        transition: ExecutionTransitionEvent,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO execution_queue_transition_audit (
                id,
                task_id,
                quote_id,
                from_state,
                to_state,
                transition_reason,
                error_class,
                decision_context_json,
                actor_type,
                actor_id,
                idempotency_key,
                correlation_id,
                state_version,
                occurred_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&transition.id.0)
        .bind(&transition.task_id.0)
        .bind(&transition.quote_id.0)
        .bind(transition.from_state.as_ref().map(ExecutionTaskState::as_str))
        .bind(transition.to_state.as_str())
        .bind(&transition.transition_reason)
        .bind(transition.error_class.as_deref())
        .bind(&transition.decision_context_json)
        .bind(&transition.actor_type)
        .bind(&transition.actor_id)
        .bind(transition.idempotency_key.as_ref().map(|key| key.0.as_str()))
        .bind(&transition.correlation_id)
        .bind(i64::from(transition.state_version))
        .bind(transition.occurred_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_transitions_for_task(
        &self,
        task_id: &ExecutionTaskId,
    ) -> Result<Vec<ExecutionTransitionEvent>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT
                id,
                task_id,
                quote_id,
                from_state,
                to_state,
                transition_reason,
                error_class,
                decision_context_json,
                actor_type,
                actor_id,
                idempotency_key,
                correlation_id,
                state_version,
                occurred_at
             FROM execution_queue_transition_audit
             WHERE task_id = ?
             ORDER BY occurred_at ASC",
        )
        .bind(&task_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(transition_from_row).collect()
    }
}

#[async_trait::async_trait]
impl IdempotencyRepository for SqlExecutionQueueRepository {
    async fn find_operation(
        &self,
        operation_key: &OperationKey,
    ) -> Result<Option<IdempotencyRecord>, RepositoryError> {
        let row = sqlx::query(
            "SELECT
                operation_key,
                quote_id,
                operation_kind,
                payload_hash,
                state,
                attempt_count,
                first_seen_at,
                last_seen_at,
                result_snapshot_json,
                error_snapshot_json,
                expires_at,
                correlation_id,
                created_by_component,
                updated_by_component
             FROM execution_idempotency_ledger
             WHERE operation_key = ?",
        )
        .bind(&operation_key.0)
        .fetch_optional(&self.pool)
        .await?;

        row.map(idempotency_from_row).transpose()
    }

    async fn save_operation(&self, record: IdempotencyRecord) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO execution_idempotency_ledger (
                operation_key,
                quote_id,
                operation_kind,
                payload_hash,
                state,
                attempt_count,
                first_seen_at,
                last_seen_at,
                result_snapshot_json,
                error_snapshot_json,
                expires_at,
                correlation_id,
                created_by_component,
                updated_by_component
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(operation_key) DO UPDATE SET
                quote_id = excluded.quote_id,
                operation_kind = excluded.operation_kind,
                payload_hash = excluded.payload_hash,
                state = excluded.state,
                attempt_count = excluded.attempt_count,
                last_seen_at = excluded.last_seen_at,
                result_snapshot_json = excluded.result_snapshot_json,
                error_snapshot_json = excluded.error_snapshot_json,
                expires_at = excluded.expires_at,
                correlation_id = excluded.correlation_id,
                updated_by_component = excluded.updated_by_component",
        )
        .bind(&record.operation_key.0)
        .bind(&record.quote_id.0)
        .bind(&record.operation_kind)
        .bind(&record.payload_hash)
        .bind(record.state.as_str())
        .bind(i64::from(record.attempt_count))
        .bind(record.first_seen_at.to_rfc3339())
        .bind(record.last_seen_at.to_rfc3339())
        .bind(record.result_snapshot_json.as_deref())
        .bind(record.error_snapshot_json.as_deref())
        .bind(record.expires_at.map(|value| value.to_rfc3339()))
        .bind(&record.correlation_id)
        .bind(&record.created_by_component)
        .bind(&record.updated_by_component)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

fn task_from_row(row: SqliteRow) -> Result<ExecutionTask, RepositoryError> {
    let state_raw = row.try_get::<String, _>("state")?;
    let state = ExecutionTaskState::parse(&state_raw).ok_or_else(|| {
        RepositoryError::Decode(format!("unknown execution task state `{state_raw}`"))
    })?;

    Ok(ExecutionTask {
        id: ExecutionTaskId(row.try_get("id")?),
        quote_id: QuoteId(row.try_get("quote_id")?),
        operation_kind: row.try_get("operation_kind")?,
        payload_json: row.try_get("payload_json")?,
        idempotency_key: OperationKey(row.try_get("idempotency_key")?),
        state,
        retry_count: parse_u32("retry_count", row.try_get("retry_count")?)?,
        max_retries: parse_u32("max_retries", row.try_get("max_retries")?)?,
        available_at: parse_timestamp("available_at", row.try_get("available_at")?)?,
        claimed_by: row.try_get("claimed_by")?,
        claimed_at: parse_optional_timestamp("claimed_at", row.try_get("claimed_at")?)?,
        last_error: row.try_get("last_error")?,
        result_fingerprint: row.try_get("result_fingerprint")?,
        state_version: parse_u32("state_version", row.try_get("state_version")?)?,
        created_at: parse_timestamp("created_at", row.try_get("created_at")?)?,
        updated_at: parse_timestamp("updated_at", row.try_get("updated_at")?)?,
    })
}

fn transition_from_row(row: SqliteRow) -> Result<ExecutionTransitionEvent, RepositoryError> {
    let from_state = row
        .try_get::<Option<String>, _>("from_state")?
        .map(|value| {
            ExecutionTaskState::parse(&value)
                .ok_or_else(|| RepositoryError::Decode(format!("unknown from_state `{value}`")))
        })
        .transpose()?;

    let to_state_raw = row.try_get::<String, _>("to_state")?;
    let to_state = ExecutionTaskState::parse(&to_state_raw)
        .ok_or_else(|| RepositoryError::Decode(format!("unknown to_state `{to_state_raw}`")))?;

    Ok(ExecutionTransitionEvent {
        id: ExecutionTransitionId(row.try_get("id")?),
        task_id: ExecutionTaskId(row.try_get("task_id")?),
        quote_id: QuoteId(row.try_get("quote_id")?),
        from_state,
        to_state,
        transition_reason: row.try_get("transition_reason")?,
        error_class: row.try_get("error_class")?,
        decision_context_json: row.try_get("decision_context_json")?,
        actor_type: row.try_get("actor_type")?,
        actor_id: row.try_get("actor_id")?,
        idempotency_key: row.try_get::<Option<String>, _>("idempotency_key")?.map(OperationKey),
        correlation_id: row.try_get("correlation_id")?,
        state_version: parse_u32("state_version", row.try_get("state_version")?)?,
        occurred_at: parse_timestamp("occurred_at", row.try_get("occurred_at")?)?,
    })
}

fn idempotency_from_row(row: SqliteRow) -> Result<IdempotencyRecord, RepositoryError> {
    let state_raw = row.try_get::<String, _>("state")?;
    let state = IdempotencyRecordState::parse(&state_raw).ok_or_else(|| {
        RepositoryError::Decode(format!("unknown idempotency state `{state_raw}`"))
    })?;

    Ok(IdempotencyRecord {
        operation_key: OperationKey(row.try_get("operation_key")?),
        quote_id: QuoteId(row.try_get("quote_id")?),
        operation_kind: row.try_get("operation_kind")?,
        payload_hash: row.try_get("payload_hash")?,
        state,
        attempt_count: parse_u32("attempt_count", row.try_get("attempt_count")?)?,
        first_seen_at: parse_timestamp("first_seen_at", row.try_get("first_seen_at")?)?,
        last_seen_at: parse_timestamp("last_seen_at", row.try_get("last_seen_at")?)?,
        result_snapshot_json: row.try_get("result_snapshot_json")?,
        error_snapshot_json: row.try_get("error_snapshot_json")?,
        expires_at: parse_optional_timestamp("expires_at", row.try_get("expires_at")?)?,
        correlation_id: row.try_get("correlation_id")?,
        created_by_component: row.try_get("created_by_component")?,
        updated_by_component: row.try_get("updated_by_component")?,
    })
}

fn parse_u32(column: &str, value: i64) -> Result<u32, RepositoryError> {
    u32::try_from(value).map_err(|_| {
        RepositoryError::Decode(format!(
            "invalid value for `{column}` (expected non-negative u32): {value}"
        ))
    })
}

fn parse_timestamp(column: &str, value: String) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(&value).map(|timestamp| timestamp.with_timezone(&Utc)).map_err(
        |error| {
            RepositoryError::Decode(format!("invalid timestamp in `{column}`: `{value}` ({error})"))
        },
    )
}

fn parse_optional_timestamp(
    column: &str,
    value: Option<String>,
) -> Result<Option<DateTime<Utc>>, RepositoryError> {
    value.map(|timestamp| parse_timestamp(column, timestamp)).transpose()
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use quotey_core::domain::execution::{
        ExecutionTask, ExecutionTaskId, ExecutionTaskState, ExecutionTransitionEvent,
        ExecutionTransitionId, IdempotencyRecord, IdempotencyRecordState, OperationKey,
    };
    use quotey_core::domain::quote::QuoteId;

    use super::SqlExecutionQueueRepository;
    use crate::migrations;
    use crate::repositories::{ExecutionQueueRepository, IdempotencyRepository};
    use crate::{connect_with_settings, DbPool};

    #[tokio::test]
    async fn sql_execution_queue_repo_round_trip_for_task_and_transition() {
        let pool = setup_pool().await;
        let quote_id = QuoteId("Q-REL-001".to_string());
        insert_quote(&pool, &quote_id).await;

        let repo = SqlExecutionQueueRepository::new(pool.clone());
        let task = sample_task(&quote_id);

        repo.save_task(task.clone()).await.expect("save task");

        let found = repo.find_task_by_id(&task.id).await.expect("find task");
        assert_eq!(found, Some(task.clone()));

        let queued_tasks = repo
            .list_tasks_for_quote(&quote_id, Some(ExecutionTaskState::Queued))
            .await
            .expect("list queued tasks");
        assert_eq!(queued_tasks, vec![task.clone()]);

        let transition = ExecutionTransitionEvent {
            id: ExecutionTransitionId("trans-1".to_string()),
            task_id: task.id.clone(),
            quote_id: quote_id.clone(),
            from_state: Some(ExecutionTaskState::Queued),
            to_state: ExecutionTaskState::Running,
            transition_reason: "worker-claim".to_string(),
            error_class: None,
            decision_context_json: "{\"worker\":\"worker-1\"}".to_string(),
            actor_type: "system".to_string(),
            actor_id: "worker-1".to_string(),
            idempotency_key: Some(task.idempotency_key.clone()),
            correlation_id: "corr-rel-001".to_string(),
            state_version: 2,
            occurred_at: parse_ts("2026-02-23T12:01:00Z"),
        };

        repo.append_transition(transition.clone()).await.expect("append transition");

        let transitions = repo.list_transitions_for_task(&task.id).await.expect("list transitions");
        assert_eq!(transitions, vec![transition]);

        pool.close().await;
    }

    #[tokio::test]
    async fn sql_execution_queue_repo_round_trip_for_idempotency_record() {
        let pool = setup_pool().await;
        let quote_id = QuoteId("Q-REL-002".to_string());
        insert_quote(&pool, &quote_id).await;

        let repo = SqlExecutionQueueRepository::new(pool.clone());
        let operation = IdempotencyRecord {
            operation_key: OperationKey("op-rel-001".to_string()),
            quote_id: quote_id.clone(),
            operation_kind: "slack.post_message".to_string(),
            payload_hash: "sha256:abcd".to_string(),
            state: IdempotencyRecordState::Reserved,
            attempt_count: 1,
            first_seen_at: parse_ts("2026-02-23T12:00:00Z"),
            last_seen_at: parse_ts("2026-02-23T12:00:00Z"),
            result_snapshot_json: None,
            error_snapshot_json: None,
            expires_at: Some(parse_ts("2026-03-01T12:00:00Z")),
            correlation_id: "corr-rel-002".to_string(),
            created_by_component: "slack-ingress".to_string(),
            updated_by_component: "slack-ingress".to_string(),
        };

        repo.save_operation(operation.clone()).await.expect("save operation");

        let found = repo.find_operation(&operation.operation_key).await.expect("find operation");
        assert_eq!(found, Some(operation.clone()));

        let mut updated_operation = operation.clone();
        updated_operation.state = IdempotencyRecordState::Completed;
        updated_operation.attempt_count = 2;
        updated_operation.last_seen_at = parse_ts("2026-02-23T12:05:00Z");
        updated_operation.result_snapshot_json =
            Some("{\"result\":\"already-delivered\"}".to_string());
        updated_operation.updated_by_component = "queue-worker".to_string();

        repo.save_operation(updated_operation.clone()).await.expect("update operation");

        let found_updated = repo
            .find_operation(&updated_operation.operation_key)
            .await
            .expect("find updated operation");
        assert_eq!(found_updated, Some(updated_operation));

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
        let timestamp = "2026-02-23T12:00:00Z";

        sqlx::query(
            "INSERT INTO quote (id, status, currency, created_by, created_at, updated_at)
             VALUES (?, 'draft', 'USD', 'U-REL', ?, ?)",
        )
        .bind(&quote_id.0)
        .bind(timestamp)
        .bind(timestamp)
        .execute(pool)
        .await
        .expect("insert quote");
    }

    fn sample_task(quote_id: &QuoteId) -> ExecutionTask {
        ExecutionTask {
            id: ExecutionTaskId("task-rel-001".to_string()),
            quote_id: quote_id.clone(),
            operation_kind: "crm.write_quote".to_string(),
            payload_json: "{\"deal_id\":\"D-1\"}".to_string(),
            idempotency_key: OperationKey("op-rel-task-001".to_string()),
            state: ExecutionTaskState::Queued,
            retry_count: 0,
            max_retries: 5,
            available_at: parse_ts("2026-02-23T12:00:00Z"),
            claimed_by: None,
            claimed_at: None,
            last_error: None,
            result_fingerprint: None,
            state_version: 1,
            created_at: parse_ts("2026-02-23T12:00:00Z"),
            updated_at: parse_ts("2026-02-23T12:00:00Z"),
        }
    }

    fn parse_ts(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value).expect("valid rfc3339").with_timezone(&Utc)
    }
}
