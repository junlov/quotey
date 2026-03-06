use chrono::{DateTime, Duration, Utc};
use sqlx::Row;

use quotey_core::domain::quote_lock::{LockConflict, LockInfo};

use super::{QuoteLockRepository, RepositoryError};
use crate::DbPool;

pub struct SqlQuoteLockRepository {
    pool: DbPool,
}

impl SqlQuoteLockRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl QuoteLockRepository for SqlQuoteLockRepository {
    async fn lock_quote(
        &self,
        quote_id: &str,
        actor_id: &str,
        duration_minutes: u32,
    ) -> Result<(), LockConflict> {
        let now = Utc::now();
        let expires_at = now + Duration::minutes(i64::from(duration_minutes));
        let now_str = now.to_rfc3339();
        let expires_str = expires_at.to_rfc3339();

        // Attempt to acquire: succeed if currently unlocked OR lock has expired.
        let result = sqlx::query(
            "UPDATE quote
             SET locked_by = ?, locked_at = ?, lock_expires_at = ?
             WHERE id = ?
               AND (locked_by IS NULL OR lock_expires_at < ?)",
        )
        .bind(actor_id)
        .bind(&now_str)
        .bind(&expires_str)
        .bind(quote_id)
        .bind(&now_str)
        .execute(&self.pool)
        .await
        .map_err(|e| LockConflict {
            current_owner: format!("db_error: {e}"),
            locked_since: now,
            expires_at,
        })?;

        if result.rows_affected() > 0 {
            return Ok(());
        }

        // No rows affected — quote is locked by someone else. Fetch the conflict.
        let row = sqlx::query(
            "SELECT locked_by, locked_at, lock_expires_at
             FROM quote
             WHERE id = ?
               AND locked_by IS NOT NULL
               AND lock_expires_at >= ?",
        )
        .bind(quote_id)
        .bind(&now_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| LockConflict {
            current_owner: format!("db_error: {e}"),
            locked_since: now,
            expires_at,
        })?;

        match row {
            Some(r) => {
                let owner: String = r.get("locked_by");
                let since_raw: String = r.get("locked_at");
                let exp_raw: String = r.get("lock_expires_at");

                let locked_since = DateTime::parse_from_rfc3339(&since_raw)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or(now);
                let exp = DateTime::parse_from_rfc3339(&exp_raw)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or(expires_at);

                Err(LockConflict { current_owner: owner, locked_since, expires_at: exp })
            }
            None => {
                // Edge case: lock expired between our UPDATE and SELECT, or quote doesn't exist.
                // Treat as success — retry the acquire.
                let retry = sqlx::query(
                    "UPDATE quote
                     SET locked_by = ?, locked_at = ?, lock_expires_at = ?
                     WHERE id = ?
                       AND (locked_by IS NULL OR lock_expires_at < ?)",
                )
                .bind(actor_id)
                .bind(&now_str)
                .bind(&expires_str)
                .bind(quote_id)
                .bind(&now_str)
                .execute(&self.pool)
                .await
                .map_err(|e| LockConflict {
                    current_owner: format!("db_error: {e}"),
                    locked_since: now,
                    expires_at,
                })?;

                if retry.rows_affected() > 0 {
                    Ok(())
                } else {
                    Err(LockConflict {
                        current_owner: "unknown".to_string(),
                        locked_since: now,
                        expires_at,
                    })
                }
            }
        }
    }

    async fn unlock_quote(&self, quote_id: &str, actor_id: &str) -> Result<(), RepositoryError> {
        sqlx::query(
            "UPDATE quote
             SET locked_by = NULL, locked_at = NULL, lock_expires_at = NULL
             WHERE id = ? AND locked_by = ?",
        )
        .bind(quote_id)
        .bind(actor_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn force_unlock(&self, quote_id: &str) -> Result<(), RepositoryError> {
        sqlx::query(
            "UPDATE quote
             SET locked_by = NULL, locked_at = NULL, lock_expires_at = NULL
             WHERE id = ?",
        )
        .bind(quote_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn check_lock(&self, quote_id: &str) -> Result<Option<LockInfo>, RepositoryError> {
        let now_str = Utc::now().to_rfc3339();

        let row = sqlx::query(
            "SELECT locked_by, locked_at, lock_expires_at
             FROM quote
             WHERE id = ?
               AND locked_by IS NOT NULL
               AND lock_expires_at >= ?",
        )
        .bind(quote_id)
        .bind(&now_str)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let locked_by: String =
                    r.try_get("locked_by").map_err(RepositoryError::Database)?;
                let locked_at_raw: String =
                    r.try_get("locked_at").map_err(RepositoryError::Database)?;
                let expires_raw: String =
                    r.try_get("lock_expires_at").map_err(RepositoryError::Database)?;

                let locked_at = DateTime::parse_from_rfc3339(&locked_at_raw)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| {
                        RepositoryError::Decode(format!("invalid locked_at: {locked_at_raw}: {e}"))
                    })?;
                let lock_expires_at = DateTime::parse_from_rfc3339(&expires_raw)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| {
                        RepositoryError::Decode(format!(
                            "invalid lock_expires_at: {expires_raw}: {e}"
                        ))
                    })?;

                Ok(Some(LockInfo {
                    quote_id: quote_id.to_string(),
                    locked_by,
                    locked_at,
                    lock_expires_at,
                }))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations::run_pending;

    async fn setup() -> (DbPool, SqlQuoteLockRepository) {
        let pool = crate::connect_with_settings("sqlite::memory:", 1, 30).await.expect("connect");
        run_pending(&pool).await.expect("migrations");
        let repo = SqlQuoteLockRepository::new(pool.clone());
        (pool, repo)
    }

    async fn insert_test_quote(pool: &sqlx::SqlitePool, id: &str) {
        sqlx::query(
            "INSERT INTO quote (id, status, currency, created_by, created_at, updated_at, version)
             VALUES (?, 'draft', 'USD', 'test-actor', datetime('now'), datetime('now'), 1)",
        )
        .bind(id)
        .execute(pool)
        .await
        .expect("insert test quote");
    }

    #[tokio::test]
    async fn acquire_lock_on_unlocked_quote() {
        let (pool, repo) = setup().await;
        insert_test_quote(&pool, "Q-LOCK-001").await;

        let result = repo.lock_quote("Q-LOCK-001", "alice", 30).await;
        assert!(result.is_ok(), "should acquire lock on unlocked quote");

        let info = repo.check_lock("Q-LOCK-001").await.expect("check_lock");
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.locked_by, "alice");
        assert_eq!(info.quote_id, "Q-LOCK-001");
    }

    #[tokio::test]
    async fn lock_conflict_when_already_locked() {
        let (pool, repo) = setup().await;
        insert_test_quote(&pool, "Q-LOCK-002").await;

        repo.lock_quote("Q-LOCK-002", "alice", 30).await.expect("first lock");

        let result = repo.lock_quote("Q-LOCK-002", "bob", 30).await;
        assert!(result.is_err(), "should conflict when already locked");

        let conflict = result.unwrap_err();
        assert_eq!(conflict.current_owner, "alice");
    }

    #[tokio::test]
    async fn auto_expire_releases_lock() {
        let (pool, repo) = setup().await;
        insert_test_quote(&pool, "Q-LOCK-003").await;

        // Directly insert a lock with expired timestamp.
        let past = (Utc::now() - Duration::hours(1)).to_rfc3339();
        let long_past = (Utc::now() - Duration::hours(2)).to_rfc3339();
        sqlx::query(
            "UPDATE quote SET locked_by = 'alice', locked_at = ?, lock_expires_at = ? WHERE id = ?",
        )
        .bind(&long_past)
        .bind(&past)
        .bind("Q-LOCK-003")
        .execute(&pool)
        .await
        .expect("set expired lock");

        // Now bob should be able to acquire the lock.
        let result = repo.lock_quote("Q-LOCK-003", "bob", 30).await;
        assert!(result.is_ok(), "should succeed because existing lock expired");

        let info = repo.check_lock("Q-LOCK-003").await.expect("check").unwrap();
        assert_eq!(info.locked_by, "bob");
    }

    #[tokio::test]
    async fn unlock_by_owner_succeeds() {
        let (pool, repo) = setup().await;
        insert_test_quote(&pool, "Q-LOCK-004").await;

        repo.lock_quote("Q-LOCK-004", "alice", 30).await.expect("lock");
        repo.unlock_quote("Q-LOCK-004", "alice").await.expect("unlock");

        let info = repo.check_lock("Q-LOCK-004").await.expect("check");
        assert!(info.is_none(), "lock should be released after owner unlock");
    }

    #[tokio::test]
    async fn unlock_by_non_owner_fails() {
        let (pool, repo) = setup().await;
        insert_test_quote(&pool, "Q-LOCK-005").await;

        repo.lock_quote("Q-LOCK-005", "alice", 30).await.expect("lock");

        // Bob tries to unlock — should silently fail (no rows affected).
        repo.unlock_quote("Q-LOCK-005", "bob").await.expect("unlock call itself should not error");

        // Lock should still be held by alice.
        let info = repo.check_lock("Q-LOCK-005").await.expect("check").unwrap();
        assert_eq!(info.locked_by, "alice");
    }

    #[tokio::test]
    async fn force_unlock_ignores_owner() {
        let (pool, repo) = setup().await;
        insert_test_quote(&pool, "Q-LOCK-006").await;

        repo.lock_quote("Q-LOCK-006", "alice", 30).await.expect("lock");
        repo.force_unlock("Q-LOCK-006").await.expect("force_unlock");

        let info = repo.check_lock("Q-LOCK-006").await.expect("check");
        assert!(info.is_none(), "force_unlock should release regardless of owner");
    }

    #[tokio::test]
    async fn check_lock_returns_none_for_unlocked() {
        let (pool, repo) = setup().await;
        insert_test_quote(&pool, "Q-LOCK-007").await;

        let info = repo.check_lock("Q-LOCK-007").await.expect("check");
        assert!(info.is_none(), "unlocked quote should return None");
    }

    #[tokio::test]
    async fn check_lock_returns_none_for_expired() {
        let (pool, repo) = setup().await;
        insert_test_quote(&pool, "Q-LOCK-008").await;

        // Insert an expired lock directly.
        let past = (Utc::now() - Duration::hours(1)).to_rfc3339();
        let long_past = (Utc::now() - Duration::hours(2)).to_rfc3339();
        sqlx::query(
            "UPDATE quote SET locked_by = 'alice', locked_at = ?, lock_expires_at = ? WHERE id = ?",
        )
        .bind(&long_past)
        .bind(&past)
        .bind("Q-LOCK-008")
        .execute(&pool)
        .await
        .expect("set expired lock");

        let info = repo.check_lock("Q-LOCK-008").await.expect("check");
        assert!(info.is_none(), "expired lock should return None from check_lock");
    }
}
