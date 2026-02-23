use sqlx::migrate::{MigrateError, Migrator};

use crate::DbPool;

pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

pub async fn run_pending(pool: &DbPool) -> Result<(), MigrateError> {
    MIGRATOR.run(pool).await
}

#[cfg(test)]
mod tests {
    use sqlx::Row;

    use super::run_pending;
    use crate::{connect_with_settings, migrations::MIGRATOR};

    #[tokio::test]
    async fn migrations_create_baseline_tables() {
        let pool = connect_with_settings("sqlite::memory:", 1, 30).await.expect("connect");
        run_pending(&pool).await.expect("run migrations");

        let quote_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'quote'",
        )
        .fetch_one(&pool)
        .await
        .expect("check quote table")
        .get::<i64, _>("count");

        let flow_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'flow_state'",
        )
        .fetch_one(&pool)
        .await
        .expect("check flow_state table")
        .get::<i64, _>("count");

        let audit_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'audit_event'",
        )
        .fetch_one(&pool)
        .await
        .expect("check audit_event table")
        .get::<i64, _>("count");

        assert_eq!(quote_count, 1);
        assert_eq!(flow_count, 1);
        assert_eq!(audit_count, 1);
    }

    #[tokio::test]
    async fn migrations_are_reversible() {
        let pool = connect_with_settings("sqlite::memory:", 1, 30).await.expect("connect");
        run_pending(&pool).await.expect("run migrations");

        MIGRATOR.undo(&pool, 0).await.expect("undo migrations");

        let quote_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'quote'",
        )
        .fetch_one(&pool)
        .await
        .expect("check quote table removed")
        .get::<i64, _>("count");

        assert_eq!(quote_count, 0);
    }
}
