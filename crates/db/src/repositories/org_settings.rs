use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::Row;

use quotey_core::domain::org_settings::OrgSetting;

use super::{OrgSettingsRepository, RepositoryError};
use crate::DbPool;

pub struct SqlOrgSettingsRepository {
    pool: DbPool,
}

impl SqlOrgSettingsRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl OrgSettingsRepository for SqlOrgSettingsRepository {
    async fn get(&self, key: &str) -> Result<Option<OrgSetting>, RepositoryError> {
        let row = sqlx::query(
            "SELECT key, value_json, description, updated_at, updated_by
             FROM org_settings
             WHERE key = ?",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        row.map(row_to_org_setting).transpose()
    }

    async fn set(
        &self,
        key: &str,
        value_json: &str,
        actor: Option<&str>,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO org_settings (key, value_json, updated_at, updated_by)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(key) DO UPDATE SET
                value_json = excluded.value_json,
                updated_at = excluded.updated_at,
                updated_by = excluded.updated_by",
        )
        .bind(key)
        .bind(value_json)
        .bind(&now)
        .bind(actor)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<OrgSetting>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT key, value_json, description, updated_at, updated_by
             FROM org_settings
             ORDER BY key",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_org_setting).collect()
    }
}

/// Parse a timestamp that may be either RFC 3339 (`2026-03-06T12:00:00+00:00`)
/// or SQLite's `datetime('now')` format (`2026-03-06 12:00:00`).
fn parse_timestamp(raw: &str, field: &str, key: &str) -> Result<DateTime<Utc>, RepositoryError> {
    // Try RFC 3339 first (written by our set method).
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Ok(dt.with_timezone(&Utc));
    }
    // Fall back to SQLite datetime('now') format: "YYYY-MM-DD HH:MM:SS"
    if let Ok(naive) = NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S") {
        return Ok(naive.and_utc());
    }
    Err(RepositoryError::Decode(format!("invalid org_settings.{field} `{raw}` for key `{key}`")))
}

fn row_to_org_setting(row: sqlx::sqlite::SqliteRow) -> Result<OrgSetting, RepositoryError> {
    let key: String = row.try_get("key").map_err(RepositoryError::Database)?;
    let updated_at_raw: String = row.try_get("updated_at").map_err(RepositoryError::Database)?;
    let updated_at = parse_timestamp(&updated_at_raw, "updated_at", &key)?;

    Ok(OrgSetting {
        key: key.clone(),
        value_json: row.try_get("value_json").map_err(RepositoryError::Database)?,
        description: row.try_get("description").map_err(RepositoryError::Database)?,
        updated_at,
        updated_by: row.try_get("updated_by").map_err(RepositoryError::Database)?,
    })
}

#[cfg(test)]
mod tests {
    use super::SqlOrgSettingsRepository;
    use crate::repositories::OrgSettingsRepository;
    use crate::{connect_with_settings, migrations};

    async fn setup() -> sqlx::SqlitePool {
        let pool = connect_with_settings("sqlite::memory:", 1, 30).await.expect("connect");
        migrations::run_pending(&pool).await.expect("migrations");
        pool
    }

    #[tokio::test]
    async fn get_seeded_setting() {
        let pool = setup().await;
        let repo = SqlOrgSettingsRepository::new(pool);

        let setting = repo
            .get("require_manager_approval_above_discount_pct")
            .await
            .expect("get")
            .expect("seeded setting should exist");

        assert_eq!(setting.value_json, "0.10");
        assert!(setting.description.is_some());
        assert!(setting.updated_by.is_none());
    }

    #[tokio::test]
    async fn get_returns_none_for_unknown_key() {
        let pool = setup().await;
        let repo = SqlOrgSettingsRepository::new(pool);

        let result = repo.get("nonexistent_key").await.expect("get should not error");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn set_creates_new_setting() {
        let pool = setup().await;
        let repo = SqlOrgSettingsRepository::new(pool);

        repo.set("custom_key", "42", Some("admin@example.com")).await.expect("set");

        let setting = repo.get("custom_key").await.expect("get").expect("setting should exist");
        assert_eq!(setting.value_json, "42");
        assert_eq!(setting.updated_by.as_deref(), Some("admin@example.com"));
    }

    #[tokio::test]
    async fn set_updates_existing_setting() {
        let pool = setup().await;
        let repo = SqlOrgSettingsRepository::new(pool);

        // Read original
        let original = repo
            .get("max_quote_age_days")
            .await
            .expect("get")
            .expect("seeded setting should exist");
        assert_eq!(original.value_json, "90");

        // Update
        repo.set("max_quote_age_days", "120", Some("ops-bot")).await.expect("set");

        let updated =
            repo.get("max_quote_age_days").await.expect("get").expect("setting should still exist");
        assert_eq!(updated.value_json, "120");
        assert_eq!(updated.updated_by.as_deref(), Some("ops-bot"));
        assert!(updated.updated_at >= original.updated_at);
    }

    #[tokio::test]
    async fn list_all_returns_seeded_defaults() {
        let pool = setup().await;
        let repo = SqlOrgSettingsRepository::new(pool);

        let all = repo.list_all().await.expect("list_all");
        // We seed exactly 8 settings.
        assert_eq!(all.len(), 8);

        // Verify alphabetical order by key.
        let keys: Vec<&str> = all.iter().map(|s| s.key.as_str()).collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted);
    }

    #[tokio::test]
    async fn typed_accessors() {
        use quotey_core::domain::org_settings::OrgSetting;

        let now = chrono::Utc::now();

        // f64
        let setting = OrgSetting {
            key: "discount".into(),
            value_json: "0.10".into(),
            description: None,
            updated_at: now,
            updated_by: None,
        };
        assert!((setting.value_as_f64().unwrap() - 0.10).abs() < f64::EPSILON);

        // i64
        let setting = OrgSetting {
            key: "cents".into(),
            value_json: "10000000".into(),
            description: None,
            updated_at: now,
            updated_by: None,
        };
        assert_eq!(setting.value_as_i64(), Some(10_000_000));

        // bool true
        let setting = OrgSetting {
            key: "flag_t".into(),
            value_json: "true".into(),
            description: None,
            updated_at: now,
            updated_by: None,
        };
        assert_eq!(setting.value_as_bool(), Some(true));

        // bool false
        let setting = OrgSetting {
            key: "flag_f".into(),
            value_json: "false".into(),
            description: None,
            updated_at: now,
            updated_by: None,
        };
        assert_eq!(setting.value_as_bool(), Some(false));

        // non-bool returns None
        let setting = OrgSetting {
            key: "num".into(),
            value_json: "42".into(),
            description: None,
            updated_at: now,
            updated_by: None,
        };
        assert_eq!(setting.value_as_bool(), None);

        // non-numeric returns None for f64
        let setting = OrgSetting {
            key: "text".into(),
            value_json: "hello".into(),
            description: None,
            updated_at: now,
            updated_by: None,
        };
        assert_eq!(setting.value_as_f64(), None);
        assert_eq!(setting.value_as_i64(), None);
    }
}
