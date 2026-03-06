use async_trait::async_trait;
use chrono::Utc;
use sqlx::Row;

use quotey_core::domain::integration::{
    AdapterStatus, AdapterType, IntegrationConfig, IntegrationType,
};

use crate::DbPool;

use super::RepositoryError;

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait IntegrationConfigRepository: Send + Sync {
    async fn find_by_id(&self, id: &str) -> Result<Option<IntegrationConfig>, RepositoryError>;
    async fn save(&self, config: IntegrationConfig) -> Result<(), RepositoryError>;
    async fn list_by_type(
        &self,
        integration_type: &str,
        active_only: bool,
    ) -> Result<Vec<IntegrationConfig>, RepositoryError>;
    async fn list_all(&self, active_only: bool) -> Result<Vec<IntegrationConfig>, RepositoryError>;
    async fn delete(&self, id: &str) -> Result<bool, RepositoryError>;
    async fn update_status(
        &self,
        id: &str,
        status: &str,
        status_message: Option<&str>,
    ) -> Result<bool, RepositoryError>;
}

// ---------------------------------------------------------------------------
// SQL implementation
// ---------------------------------------------------------------------------

pub struct SqlIntegrationConfigRepository {
    pool: DbPool,
}

impl SqlIntegrationConfigRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

const SELECT_COLUMNS: &str =
    "id, integration_type, adapter_type, name, adapter_config, status, status_message, created_at, updated_at";

fn row_to_config(row: sqlx::sqlite::SqliteRow) -> Result<IntegrationConfig, RepositoryError> {
    let integration_type_str: String = row.get("integration_type");
    let adapter_type_str: String = row.get("adapter_type");
    let status_str: String = row.get("status");

    let integration_type =
        IntegrationType::parse_label(&integration_type_str).ok_or_else(|| {
            RepositoryError::Decode(format!("unknown integration_type: {integration_type_str}"))
        })?;
    let adapter_type = AdapterType::parse_label(&adapter_type_str).ok_or_else(|| {
        RepositoryError::Decode(format!("unknown adapter_type: {adapter_type_str}"))
    })?;
    let status = AdapterStatus::parse_label(&status_str)
        .ok_or_else(|| RepositoryError::Decode(format!("unknown status: {status_str}")))?;

    let created_at_str: String = row.get("created_at");
    let updated_at_str: String = row.get("updated_at");

    Ok(IntegrationConfig {
        id: row.get("id"),
        integration_type,
        adapter_type,
        name: row.get("name"),
        adapter_config: row.get("adapter_config"),
        status,
        status_message: row.get("status_message"),
        created_at: created_at_str
            .parse()
            .map_err(|e| RepositoryError::Decode(format!("created_at: {e}")))?,
        updated_at: updated_at_str
            .parse()
            .map_err(|e| RepositoryError::Decode(format!("updated_at: {e}")))?,
    })
}

#[async_trait]
impl IntegrationConfigRepository for SqlIntegrationConfigRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<IntegrationConfig>, RepositoryError> {
        let sql = format!("SELECT {SELECT_COLUMNS} FROM integration_config WHERE id = ?");
        let row = sqlx::query(&sql).bind(id).fetch_optional(&self.pool).await?;
        row.map(row_to_config).transpose()
    }

    async fn save(&self, config: IntegrationConfig) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO integration_config
                (id, integration_type, adapter_type, name, adapter_config, status, status_message, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                integration_type = excluded.integration_type,
                adapter_type = excluded.adapter_type,
                name = excluded.name,
                adapter_config = excluded.adapter_config,
                status = excluded.status,
                status_message = excluded.status_message,
                updated_at = excluded.updated_at",
        )
        .bind(&config.id)
        .bind(config.integration_type.as_str())
        .bind(config.adapter_type.as_str())
        .bind(&config.name)
        .bind(&config.adapter_config)
        .bind(config.status.as_str())
        .bind(&config.status_message)
        .bind(config.created_at.to_rfc3339())
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_by_type(
        &self,
        integration_type: &str,
        active_only: bool,
    ) -> Result<Vec<IntegrationConfig>, RepositoryError> {
        let sql = if active_only {
            format!("SELECT {SELECT_COLUMNS} FROM integration_config WHERE integration_type = ? AND status = 'active' ORDER BY name")
        } else {
            format!("SELECT {SELECT_COLUMNS} FROM integration_config WHERE integration_type = ? ORDER BY name")
        };
        let rows = sqlx::query(&sql).bind(integration_type).fetch_all(&self.pool).await?;
        rows.into_iter().map(row_to_config).collect()
    }

    async fn list_all(&self, active_only: bool) -> Result<Vec<IntegrationConfig>, RepositoryError> {
        let sql = if active_only {
            format!("SELECT {SELECT_COLUMNS} FROM integration_config WHERE status = 'active' ORDER BY integration_type, name")
        } else {
            format!(
                "SELECT {SELECT_COLUMNS} FROM integration_config ORDER BY integration_type, name"
            )
        };
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
        rows.into_iter().map(row_to_config).collect()
    }

    async fn delete(&self, id: &str) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM integration_config WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn update_status(
        &self,
        id: &str,
        status: &str,
        status_message: Option<&str>,
    ) -> Result<bool, RepositoryError> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE integration_config SET status = ?, status_message = ?, updated_at = ? WHERE id = ?",
        )
        .bind(status)
        .bind(status_message)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{connect_with_settings, migrations};

    async fn setup() -> DbPool {
        let pool = connect_with_settings("sqlite::memory:", 1, 30).await.unwrap();
        migrations::run_pending(&pool).await.unwrap();
        pool
    }

    fn make_config(
        id: &str,
        int_type: IntegrationType,
        adp_type: AdapterType,
        name: &str,
    ) -> IntegrationConfig {
        let now = Utc::now();
        IntegrationConfig {
            id: id.to_string(),
            integration_type: int_type,
            adapter_type: adp_type,
            name: name.to_string(),
            adapter_config: r#"{"key":"value"}"#.to_string(),
            status: AdapterStatus::Active,
            status_message: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn save_and_find_by_id() {
        let pool = setup().await;
        let repo = SqlIntegrationConfigRepository::new(pool);
        let cfg = make_config("INT-001", IntegrationType::Crm, AdapterType::Salesforce, "SF Prod");

        repo.save(cfg.clone()).await.unwrap();
        let found = repo.find_by_id("INT-001").await.unwrap().unwrap();

        assert_eq!(found.integration_type, IntegrationType::Crm);
        assert_eq!(found.adapter_type, AdapterType::Salesforce);
        assert_eq!(found.name, "SF Prod");
        assert_eq!(found.adapter_config, r#"{"key":"value"}"#);
    }

    #[tokio::test]
    async fn upsert_updates_existing() {
        let pool = setup().await;
        let repo = SqlIntegrationConfigRepository::new(pool);
        let mut cfg =
            make_config("INT-002", IntegrationType::Pdf, AdapterType::Builtin, "Default PDF");
        repo.save(cfg.clone()).await.unwrap();

        cfg.name = "Updated PDF".to_string();
        cfg.adapter_config = r#"{"template":"invoice"}"#.to_string();
        repo.save(cfg).await.unwrap();

        let found = repo.find_by_id("INT-002").await.unwrap().unwrap();
        assert_eq!(found.name, "Updated PDF");
        assert_eq!(found.adapter_config, r#"{"template":"invoice"}"#);
    }

    #[tokio::test]
    async fn list_by_type_filters() {
        let pool = setup().await;
        let repo = SqlIntegrationConfigRepository::new(pool);

        repo.save(make_config("INT-A", IntegrationType::Crm, AdapterType::Salesforce, "SF"))
            .await
            .unwrap();
        repo.save(make_config("INT-B", IntegrationType::Crm, AdapterType::Hubspot, "HS"))
            .await
            .unwrap();
        repo.save(make_config("INT-C", IntegrationType::Notification, AdapterType::Slack, "Slack"))
            .await
            .unwrap();

        let crm = repo.list_by_type("crm", false).await.unwrap();
        assert_eq!(crm.len(), 2);

        let notif = repo.list_by_type("notification", false).await.unwrap();
        assert_eq!(notif.len(), 1);
    }

    #[tokio::test]
    async fn list_all_active_only() {
        let pool = setup().await;
        let repo = SqlIntegrationConfigRepository::new(pool);

        repo.save(make_config("INT-X", IntegrationType::Erp, AdapterType::Netsuite, "NS"))
            .await
            .unwrap();

        let mut inactive = make_config("INT-Y", IntegrationType::Erp, AdapterType::Webhook, "WH");
        inactive.status = AdapterStatus::Inactive;
        repo.save(inactive).await.unwrap();

        let all = repo.list_all(false).await.unwrap();
        assert_eq!(all.len(), 2);

        let active = repo.list_all(true).await.unwrap();
        assert_eq!(active.len(), 1);
    }

    #[tokio::test]
    async fn delete_returns_true_for_existing() {
        let pool = setup().await;
        let repo = SqlIntegrationConfigRepository::new(pool);
        repo.save(make_config("INT-DEL", IntegrationType::Pdf, AdapterType::Builtin, "PDF"))
            .await
            .unwrap();

        assert!(repo.delete("INT-DEL").await.unwrap());
        assert!(!repo.delete("INT-DEL").await.unwrap());
        assert!(repo.find_by_id("INT-DEL").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn update_status_changes_status() {
        let pool = setup().await;
        let repo = SqlIntegrationConfigRepository::new(pool);
        repo.save(make_config(
            "INT-ST",
            IntegrationType::Notification,
            AdapterType::Email,
            "Email",
        ))
        .await
        .unwrap();

        let updated = repo.update_status("INT-ST", "error", Some("SMTP timeout")).await.unwrap();
        assert!(updated);

        let found = repo.find_by_id("INT-ST").await.unwrap().unwrap();
        assert_eq!(found.status, AdapterStatus::Error);
        assert_eq!(found.status_message.as_deref(), Some("SMTP timeout"));
    }

    #[tokio::test]
    async fn find_nonexistent_returns_none() {
        let pool = setup().await;
        let repo = SqlIntegrationConfigRepository::new(pool);
        assert!(repo.find_by_id("NOPE").await.unwrap().is_none());
    }
}
