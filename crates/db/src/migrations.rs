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

    const MANAGED_SCHEMA_OBJECTS: &[&str] = &[
        "quote",
        "quote_line",
        "flow_state",
        "audit_event",
        "idx_quote_status",
        "idx_quote_created_at",
        "idx_quote_line_quote_id",
        "idx_flow_state_quote_id",
        "idx_audit_event_quote_id",
        "idx_audit_event_timestamp",
        "idx_audit_event_type",
    ];

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

    #[tokio::test]
    async fn migrations_up_down_up_preserves_schema_signature() {
        let pool = connect_with_settings("sqlite::memory:", 1, 30).await.expect("connect");
        run_pending(&pool).await.expect("run migrations");

        let initial_signature = managed_schema_signature(&pool).await;
        assert_eq!(
            initial_signature.len(),
            MANAGED_SCHEMA_OBJECTS.len(),
            "initial migration pass should create all managed schema objects",
        );

        MIGRATOR.undo(&pool, 0).await.expect("undo migrations");

        let after_down_signature = managed_schema_signature(&pool).await;
        assert!(
            after_down_signature.is_empty(),
            "managed schema objects should be removed after full undo",
        );

        run_pending(&pool).await.expect("re-run migrations");

        let after_second_up_signature = managed_schema_signature(&pool).await;
        assert_eq!(
            after_second_up_signature, initial_signature,
            "up/down/up should preserve migration-managed schema signature",
        );
    }

    async fn managed_schema_signature(pool: &sqlx::SqlitePool) -> Vec<(String, String, String)> {
        let mut signature: Vec<(String, String, String)> = sqlx::query(
            "SELECT type, name, IFNULL(sql, '') AS sql
             FROM sqlite_master
             WHERE type IN ('table', 'index')",
        )
        .fetch_all(pool)
        .await
        .expect("load schema objects")
        .into_iter()
        .filter_map(|row| {
            let name = row.get::<String, _>("name");
            if MANAGED_SCHEMA_OBJECTS.contains(&name.as_str()) {
                Some((row.get::<String, _>("type"), name, row.get::<String, _>("sql")))
            } else {
                None
            }
        })
        .collect();
        signature.sort();
        signature
    }
}
