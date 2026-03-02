use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::Row;

use quotey_core::domain::dialogue::{
    DialogueSession, DialogueSessionId, DialogueSessionStatus, SlackQuoteState,
};
use quotey_core::domain::quote::QuoteId;

use super::RepositoryError;
use crate::DbPool;

#[async_trait]
pub trait DialogueSessionRepository: Send + Sync {
    async fn find_by_thread_id(
        &self,
        thread_id: &str,
    ) -> Result<Option<DialogueSession>, RepositoryError>;

    async fn find_by_id(
        &self,
        id: &DialogueSessionId,
    ) -> Result<Option<DialogueSession>, RepositoryError>;

    async fn save(&self, session: &DialogueSession) -> Result<(), RepositoryError>;

    async fn update_status(
        &self,
        id: &DialogueSessionId,
        status: DialogueSessionStatus,
    ) -> Result<(), RepositoryError>;

    async fn update_state(
        &self,
        id: &DialogueSessionId,
        state: SlackQuoteState,
    ) -> Result<(), RepositoryError>;

    async fn delete(&self, id: &DialogueSessionId) -> Result<(), RepositoryError>;
}

pub struct SqlDialogueSessionRepository {
    pool: DbPool,
}

impl SqlDialogueSessionRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DialogueSessionRepository for SqlDialogueSessionRepository {
    async fn find_by_thread_id(
        &self,
        thread_id: &str,
    ) -> Result<Option<DialogueSession>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                slack_thread_id,
                user_id,
                created_at,
                expires_at,
                current_intent_json,
                pending_clarifications_json,
                quote_draft_id,
                status
            FROM dialogue_sessions
            WHERE slack_thread_id = ?
            "#,
        )
        .bind(thread_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(parse_session_row(row)?)),
            None => Ok(None),
        }
    }

    async fn find_by_id(
        &self,
        id: &DialogueSessionId,
    ) -> Result<Option<DialogueSession>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                slack_thread_id,
                user_id,
                created_at,
                expires_at,
                current_intent_json,
                pending_clarifications_json,
                quote_draft_id,
                status
            FROM dialogue_sessions
            WHERE id = ?
            "#,
        )
        .bind(id.as_str())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(parse_session_row(row)?)),
            None => Ok(None),
        }
    }

    async fn save(&self, session: &DialogueSession) -> Result<(), RepositoryError> {
        let created_at = session.created_at.to_rfc3339();
        let expires_at = session.expires_at.to_rfc3339();
        let current_intent_json = session.context_json.clone();
        let pending_clarifications_json = session.pending_clarifications_json.clone();
        let quote_draft_id = session.quote_draft_id.as_ref().map(|id| id.0.clone());
        let status = session.status.as_str();

        sqlx::query(
            r#"
            INSERT INTO dialogue_sessions (
                id,
                slack_thread_id,
                user_id,
                created_at,
                expires_at,
                current_intent_json,
                pending_clarifications_json,
                quote_draft_id,
                status
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                slack_thread_id = excluded.slack_thread_id,
                user_id = excluded.user_id,
                expires_at = excluded.expires_at,
                current_intent_json = excluded.current_intent_json,
                pending_clarifications_json = excluded.pending_clarifications_json,
                quote_draft_id = excluded.quote_draft_id,
                status = excluded.status
            "#,
        )
        .bind(session.id.as_str())
        .bind(&session.slack_thread_id)
        .bind(&session.user_id)
        .bind(&created_at)
        .bind(&expires_at)
        .bind(&current_intent_json)
        .bind(&pending_clarifications_json)
        .bind(&quote_draft_id)
        .bind(status)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_status(
        &self,
        id: &DialogueSessionId,
        status: DialogueSessionStatus,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            UPDATE dialogue_sessions
            SET status = ?
            WHERE id = ?
            "#,
        )
        .bind(status.as_str())
        .bind(id.as_str())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_state(
        &self,
        id: &DialogueSessionId,
        state: SlackQuoteState,
    ) -> Result<(), RepositoryError> {
        let state_json = serde_json::to_string(&state)
            .map_err(|e| RepositoryError::Decode(format!("failed to serialize state: {e}")))?;

        sqlx::query(
            r#"
            UPDATE dialogue_sessions
            SET current_intent_json = ?
            WHERE id = ?
            "#,
        )
        .bind(&state_json)
        .bind(id.as_str())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn delete(&self, id: &DialogueSessionId) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            DELETE FROM dialogue_sessions
            WHERE id = ?
            "#,
        )
        .bind(id.as_str())
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

fn parse_session_row(row: sqlx::sqlite::SqliteRow) -> Result<DialogueSession, RepositoryError> {
    let id: String = row.try_get("id").map_err(RepositoryError::Database)?;
    let slack_thread_id: String =
        row.try_get("slack_thread_id").map_err(RepositoryError::Database)?;
    let user_id: String = row.try_get("user_id").map_err(RepositoryError::Database)?;
    let created_at_raw: String =
        row.try_get("created_at").map_err(RepositoryError::Database)?;
    let expires_at_raw: String =
        row.try_get("expires_at").map_err(RepositoryError::Database)?;
    let context_json: Option<String> =
        row.try_get("current_intent_json").map_err(RepositoryError::Database)?;
    let pending_clarifications_json: Option<String> =
        row.try_get("pending_clarifications_json").map_err(RepositoryError::Database)?;
    let quote_draft_id: Option<String> =
        row.try_get("quote_draft_id").map_err(RepositoryError::Database)?;
    let status_raw: String = row.try_get("status").map_err(RepositoryError::Database)?;

    let created_at = parse_datetime("created_at", &created_at_raw)?;
    let expires_at = parse_datetime("expires_at", &expires_at_raw)?;
    let status = DialogueSessionStatus::from_str(&status_raw).map_err(|e| {
        RepositoryError::Decode(format!("invalid dialogue session status: {e}"))
    })?;

    let current_state = match &context_json {
        Some(json) => serde_json::from_str::<SlackQuoteState>(json)
            .unwrap_or(SlackQuoteState::IntentCapture),
        None => SlackQuoteState::IntentCapture,
    };

    Ok(DialogueSession {
        id: DialogueSessionId(id),
        slack_thread_id,
        user_id,
        created_at,
        expires_at,
        current_state,
        context_json,
        pending_clarifications_json,
        quote_draft_id: quote_draft_id.map(QuoteId),
        status,
    })
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

    async fn in_memory_pool() -> Result<DbPool, RepositoryError> {
        crate::connect_with_settings("sqlite::memory:", 1, 30)
            .await
            .map_err(RepositoryError::Database)
    }

    fn test_session(thread_id: &str) -> DialogueSession {
        DialogueSession {
            id: DialogueSessionId(format!("session-{}", thread_id)),
            slack_thread_id: thread_id.to_string(),
            user_id: "U12345".to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(24),
            current_state: SlackQuoteState::IntentCapture,
            context_json: None,
            pending_clarifications_json: None,
            quote_draft_id: None,
            status: DialogueSessionStatus::Active,
        }
    }

    #[tokio::test]
    async fn save_and_find_by_thread_id() -> Result<(), String> {
        let pool = in_memory_pool().await.map_err(|e| e.to_string())?;
        run_pending(&pool).await.map_err(|e| e.to_string())?;

        let repo = SqlDialogueSessionRepository::new(pool);
        let session = test_session("thread-123");

        repo.save(&session).await.map_err(|e| e.to_string())?;
        let loaded = repo
            .find_by_thread_id("thread-123")
            .await
            .map_err(|e| e.to_string())?;

        let loaded = loaded.expect("session should be found");
        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.slack_thread_id, "thread-123");
        assert_eq!(loaded.user_id, "U12345");
        assert_eq!(loaded.status, DialogueSessionStatus::Active);

        Ok(())
    }

    #[tokio::test]
    async fn update_status() -> Result<(), String> {
        let pool = in_memory_pool().await.map_err(|e| e.to_string())?;
        run_pending(&pool).await.map_err(|e| e.to_string())?;

        let repo = SqlDialogueSessionRepository::new(pool);
        let session = test_session("thread-456");

        repo.save(&session).await.map_err(|e| e.to_string())?;
        repo.update_status(&session.id, DialogueSessionStatus::Completed)
            .await
            .map_err(|e| e.to_string())?;

        let loaded = repo
            .find_by_id(&session.id)
            .await
            .map_err(|e| e.to_string())?;
        let loaded = loaded.expect("session should exist");
        assert_eq!(loaded.status, DialogueSessionStatus::Completed);

        Ok(())
    }

    #[tokio::test]
    async fn find_nonexistent_returns_none() -> Result<(), String> {
        let pool = in_memory_pool().await.map_err(|e| e.to_string())?;
        run_pending(&pool).await.map_err(|e| e.to_string())?;

        let repo = SqlDialogueSessionRepository::new(pool);
        let result = repo
            .find_by_thread_id("nonexistent")
            .await
            .map_err(|e| e.to_string())?;

        assert!(result.is_none());

        Ok(())
    }
}
