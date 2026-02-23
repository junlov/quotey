use std::time::Duration;

use sqlx::sqlite::SqlitePoolOptions;

pub type DbPool = sqlx::SqlitePool;

pub async fn connect(database_url: &str) -> Result<DbPool, sqlx::Error> {
    connect_with_settings(database_url, 5, 30).await
}

pub async fn connect_with_settings(
    database_url: &str,
    max_connections: u32,
    timeout_secs: u64,
) -> Result<DbPool, sqlx::Error> {
    SqlitePoolOptions::new()
        .max_connections(max_connections.max(1))
        .acquire_timeout(Duration::from_secs(timeout_secs.max(1)))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await?;
                sqlx::query("PRAGMA journal_mode = WAL").execute(&mut *conn).await?;
                sqlx::query("PRAGMA busy_timeout = 5000").execute(&mut *conn).await?;
                Ok(())
            })
        })
        .connect(database_url)
        .await
}
