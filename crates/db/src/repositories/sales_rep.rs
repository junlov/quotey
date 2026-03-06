use std::str::FromStr;

use chrono::{DateTime, Utc};
use sqlx::Row;

use quotey_core::domain::sales_rep::{SalesRep, SalesRepId, SalesRepRole, SalesRepStatus};

use super::{RepositoryError, SalesRepRepository};
use crate::DbPool;

pub struct SqlSalesRepRepository {
    pool: DbPool,
}

const SALES_REP_SELECT_COLUMNS: &str = "
    id,
    external_user_ref,
    name,
    email,
    role,
    title,
    team_id,
    reports_to,
    status,
    max_discount_pct,
    auto_approve_threshold_cents,
    discount_budget_monthly_cents,
    spent_discount_cents,
    capabilities_json,
    config_json,
    created_at,
    updated_at
";

impl SqlSalesRepRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl SalesRepRepository for SqlSalesRepRepository {
    async fn find_by_id(&self, id: &SalesRepId) -> Result<Option<SalesRep>, RepositoryError> {
        let sql = format!("SELECT {SALES_REP_SELECT_COLUMNS} FROM sales_rep WHERE id = ?");
        let row = sqlx::query(&sql).bind(&id.0).fetch_optional(&self.pool).await?;
        row.map(row_to_sales_rep).transpose()
    }

    async fn find_by_external_user_ref(
        &self,
        external_user_ref: &str,
    ) -> Result<Option<SalesRep>, RepositoryError> {
        let sql =
            format!("SELECT {SALES_REP_SELECT_COLUMNS} FROM sales_rep WHERE external_user_ref = ?");
        let row = sqlx::query(&sql).bind(external_user_ref).fetch_optional(&self.pool).await?;
        row.map(row_to_sales_rep).transpose()
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<SalesRep>, RepositoryError> {
        let sql = format!("SELECT {SALES_REP_SELECT_COLUMNS} FROM sales_rep WHERE email = ?");
        let row = sqlx::query(&sql).bind(email).fetch_optional(&self.pool).await?;
        row.map(row_to_sales_rep).transpose()
    }

    async fn save(&self, sales_rep: SalesRep) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO sales_rep (
                id,
                external_user_ref,
                name,
                email,
                role,
                title,
                team_id,
                reports_to,
                status,
                max_discount_pct,
                auto_approve_threshold_cents,
                discount_budget_monthly_cents,
                spent_discount_cents,
                capabilities_json,
                config_json,
                created_at,
                updated_at
             )
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                external_user_ref = excluded.external_user_ref,
                name = excluded.name,
                email = excluded.email,
                role = excluded.role,
                title = excluded.title,
                team_id = excluded.team_id,
                reports_to = excluded.reports_to,
                status = excluded.status,
                max_discount_pct = excluded.max_discount_pct,
                auto_approve_threshold_cents = excluded.auto_approve_threshold_cents,
                discount_budget_monthly_cents = excluded.discount_budget_monthly_cents,
                spent_discount_cents = excluded.spent_discount_cents,
                capabilities_json = excluded.capabilities_json,
                config_json = excluded.config_json,
                updated_at = excluded.updated_at",
        )
        .bind(&sales_rep.id.0)
        .bind(&sales_rep.external_user_ref)
        .bind(&sales_rep.name)
        .bind(&sales_rep.email)
        .bind(sales_rep.role.as_str())
        .bind(&sales_rep.title)
        .bind(&sales_rep.team_id)
        .bind(sales_rep.reports_to.as_ref().map(|id| id.0.as_str()))
        .bind(sales_rep.status.as_str())
        .bind(sales_rep.max_discount_pct)
        .bind(sales_rep.auto_approve_threshold_cents)
        .bind(sales_rep.discount_budget_monthly_cents)
        .bind(sales_rep.spent_discount_cents)
        .bind(&sales_rep.capabilities_json)
        .bind(&sales_rep.config_json)
        .bind(sales_rep.created_at.to_rfc3339())
        .bind(sales_rep.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_by_role(
        &self,
        role: &str,
        active_only: bool,
    ) -> Result<Vec<SalesRep>, RepositoryError> {
        let mut sql = format!("SELECT {SALES_REP_SELECT_COLUMNS} FROM sales_rep WHERE role = ?");
        if active_only {
            sql.push_str(" AND status = 'active'");
        }
        sql.push_str(" ORDER BY updated_at DESC");

        let rows = sqlx::query(&sql).bind(role).fetch_all(&self.pool).await?;
        rows.into_iter().map(row_to_sales_rep).collect()
    }

    async fn list_by_team(
        &self,
        team_id: &str,
        active_only: bool,
    ) -> Result<Vec<SalesRep>, RepositoryError> {
        let mut sql = format!("SELECT {SALES_REP_SELECT_COLUMNS} FROM sales_rep WHERE team_id = ?");
        if active_only {
            sql.push_str(" AND status = 'active'");
        }
        sql.push_str(" ORDER BY updated_at DESC");

        let rows = sqlx::query(&sql).bind(team_id).fetch_all(&self.pool).await?;
        rows.into_iter().map(row_to_sales_rep).collect()
    }

    async fn list_active(&self, limit: u32) -> Result<Vec<SalesRep>, RepositoryError> {
        let sql = format!(
            "SELECT {SALES_REP_SELECT_COLUMNS}
             FROM sales_rep
             WHERE status = 'active'
             ORDER BY updated_at DESC
             LIMIT ?"
        );
        let rows = sqlx::query(&sql).bind(limit).fetch_all(&self.pool).await?;
        rows.into_iter().map(row_to_sales_rep).collect()
    }
}

fn row_to_sales_rep(row: sqlx::sqlite::SqliteRow) -> Result<SalesRep, RepositoryError> {
    let id: String = row.try_get("id").map_err(RepositoryError::Database)?;
    let role_raw: String = row.try_get("role").map_err(RepositoryError::Database)?;
    let status_raw: String = row.try_get("status").map_err(RepositoryError::Database)?;
    let created_at_raw: String = row.try_get("created_at").map_err(RepositoryError::Database)?;
    let updated_at_raw: String = row.try_get("updated_at").map_err(RepositoryError::Database)?;

    let role = SalesRepRole::from_str(&role_raw).map_err(|error| {
        RepositoryError::Decode(format!("invalid sales_rep.role `{role_raw}` for `{id}`: {error}"))
    })?;
    let status = SalesRepStatus::from_str(&status_raw).map_err(|error| {
        RepositoryError::Decode(format!(
            "invalid sales_rep.status `{status_raw}` for `{id}`: {error}"
        ))
    })?;
    let created_at = DateTime::parse_from_rfc3339(&created_at_raw)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|error| {
            RepositoryError::Decode(format!(
                "invalid sales_rep.created_at `{created_at_raw}` for `{id}`: {error}"
            ))
        })?;
    let updated_at = DateTime::parse_from_rfc3339(&updated_at_raw)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|error| {
            RepositoryError::Decode(format!(
                "invalid sales_rep.updated_at `{updated_at_raw}` for `{id}`: {error}"
            ))
        })?;

    Ok(SalesRep {
        id: SalesRepId(id),
        external_user_ref: row.try_get("external_user_ref").map_err(RepositoryError::Database)?,
        name: row.try_get("name").map_err(RepositoryError::Database)?,
        email: row.try_get("email").map_err(RepositoryError::Database)?,
        role,
        title: row.try_get("title").map_err(RepositoryError::Database)?,
        team_id: row.try_get("team_id").map_err(RepositoryError::Database)?,
        reports_to: row
            .try_get::<Option<String>, _>("reports_to")
            .map_err(RepositoryError::Database)?
            .map(SalesRepId),
        status,
        max_discount_pct: row.try_get("max_discount_pct").map_err(RepositoryError::Database)?,
        auto_approve_threshold_cents: row
            .try_get("auto_approve_threshold_cents")
            .map_err(RepositoryError::Database)?,
        discount_budget_monthly_cents: row
            .try_get("discount_budget_monthly_cents")
            .map_err(RepositoryError::Database)?,
        spent_discount_cents: row
            .try_get("spent_discount_cents")
            .map_err(RepositoryError::Database)?,
        capabilities_json: row.try_get("capabilities_json").map_err(RepositoryError::Database)?,
        config_json: row.try_get("config_json").map_err(RepositoryError::Database)?,
        created_at,
        updated_at,
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use quotey_core::domain::sales_rep::{SalesRep, SalesRepId, SalesRepRole, SalesRepStatus};

    use super::SqlSalesRepRepository;
    use crate::repositories::SalesRepRepository;
    use crate::{connect_with_settings, migrations};

    async fn setup() -> sqlx::SqlitePool {
        let pool = connect_with_settings("sqlite::memory:", 1, 30).await.expect("connect");
        migrations::run_pending(&pool).await.expect("migrations");
        pool
    }

    fn sample_sales_rep(id: &str, external_user_ref: &str) -> SalesRep {
        let now = Utc::now();
        SalesRep {
            id: SalesRepId(id.to_string()),
            external_user_ref: Some(external_user_ref.to_string()),
            name: "Alice Example".to_string(),
            email: Some("alice@example.com".to_string()),
            role: SalesRepRole::Ae,
            title: Some("Account Executive".to_string()),
            team_id: Some("team-enterprise".to_string()),
            reports_to: None,
            status: SalesRepStatus::Active,
            max_discount_pct: Some(15.0),
            auto_approve_threshold_cents: Some(10_000),
            discount_budget_monthly_cents: 25_000,
            spent_discount_cents: 2_500,
            capabilities_json: "[\"enterprise\",\"security\"]".to_string(),
            config_json: "{\"territory\":\"west\"}".to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    fn sample_rep(id: &str, name: &str, role: SalesRepRole) -> SalesRep {
        let now = Utc::now();
        SalesRep {
            id: SalesRepId(id.to_string()),
            external_user_ref: Some(format!("U-{}", id.to_uppercase())),
            name: name.to_string(),
            email: Some(format!("{}@example.com", id)),
            role,
            title: None,
            team_id: Some("team-enterprise".to_string()),
            reports_to: None,
            status: SalesRepStatus::Active,
            max_discount_pct: Some(15.0),
            auto_approve_threshold_cents: Some(10_000),
            discount_budget_monthly_cents: 500_000,
            spent_discount_cents: 0,
            capabilities_json: "[]".to_string(),
            config_json: "{}".to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn save_and_find_by_external_ref_round_trip() {
        let pool = setup().await;
        let repo = SqlSalesRepRepository::new(pool);

        let sales_rep = sample_sales_rep("rep-1", "U-ALICE");
        repo.save(sales_rep.clone()).await.expect("save");

        let found = repo
            .find_by_external_user_ref("U-ALICE")
            .await
            .expect("find by external_user_ref")
            .expect("rep should exist");
        assert_eq!(found.id, sales_rep.id);
        assert_eq!(found.role, SalesRepRole::Ae);
        assert_eq!(found.status, SalesRepStatus::Active);
        assert_eq!(found.discount_budget_monthly_cents, 25_000);
        assert_eq!(found.spent_discount_cents, 2_500);
    }

    #[tokio::test]
    async fn save_and_find_by_id() {
        let pool = setup().await;
        let repo = SqlSalesRepRepository::new(pool);

        let rep = sample_rep("rep-id-1", "Alice", SalesRepRole::Ae);
        repo.save(rep.clone()).await.expect("save");

        let found =
            repo.find_by_id(&SalesRepId("rep-id-1".into())).await.expect("find").expect("exists");
        assert_eq!(found.name, "Alice");
        assert_eq!(found.discount_budget_monthly_cents, 500_000);
    }

    #[tokio::test]
    async fn find_by_id_returns_none_for_nonexistent() {
        let pool = setup().await;
        let repo = SqlSalesRepRepository::new(pool);
        let found = repo.find_by_id(&SalesRepId("ghost".into())).await.expect("find");
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn find_by_email() {
        let pool = setup().await;
        let repo = SqlSalesRepRepository::new(pool);

        let rep = sample_rep("rep-email", "Carol", SalesRepRole::Manager);
        repo.save(rep).await.expect("save");

        let found =
            repo.find_by_email("rep-email@example.com").await.expect("find").expect("exists");
        assert_eq!(found.name, "Carol");
        assert_eq!(found.role, SalesRepRole::Manager);
    }

    #[tokio::test]
    async fn find_by_email_returns_none_for_unknown() {
        let pool = setup().await;
        let repo = SqlSalesRepRepository::new(pool);
        let found = repo.find_by_email("nobody@example.com").await.expect("find");
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn upsert_updates_existing() {
        let pool = setup().await;
        let repo = SqlSalesRepRepository::new(pool);

        let mut rep = sample_rep("rep-ups", "Dave", SalesRepRole::Ae);
        repo.save(rep.clone()).await.expect("save");

        rep.name = "David".to_string();
        rep.role = SalesRepRole::Se;
        rep.spent_discount_cents = 100_000;
        repo.save(rep).await.expect("upsert");

        let found =
            repo.find_by_id(&SalesRepId("rep-ups".into())).await.expect("find").expect("exists");
        assert_eq!(found.name, "David");
        assert_eq!(found.role, SalesRepRole::Se);
        assert_eq!(found.spent_discount_cents, 100_000);
    }

    #[tokio::test]
    async fn list_by_role_filters_correctly() {
        let pool = setup().await;
        let repo = SqlSalesRepRepository::new(pool);

        repo.save(sample_rep("ae-1", "Alice", SalesRepRole::Ae)).await.expect("save");
        repo.save(sample_rep("ae-2", "Amy", SalesRepRole::Ae)).await.expect("save");
        repo.save(sample_rep("mgr-1", "Mike", SalesRepRole::Manager)).await.expect("save");

        let aes = repo.list_by_role("ae", true).await.expect("list");
        assert_eq!(aes.len(), 2);

        let managers = repo.list_by_role("manager", true).await.expect("list");
        assert_eq!(managers.len(), 1);
    }

    #[tokio::test]
    async fn list_by_role_respects_active_only() {
        let pool = setup().await;
        let repo = SqlSalesRepRepository::new(pool);

        repo.save(sample_rep("ae-a", "Active AE", SalesRepRole::Ae)).await.expect("save");
        let mut inactive = sample_rep("ae-i", "Inactive AE", SalesRepRole::Ae);
        inactive.status = SalesRepStatus::Inactive;
        repo.save(inactive).await.expect("save");

        let active_only = repo.list_by_role("ae", true).await.expect("list");
        assert_eq!(active_only.len(), 1);

        let all = repo.list_by_role("ae", false).await.expect("list");
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn list_by_team() {
        let pool = setup().await;
        let repo = SqlSalesRepRepository::new(pool);

        let mut rep1 = sample_rep("tw-1", "Alpha", SalesRepRole::Ae);
        rep1.team_id = Some("team-west".to_string());
        repo.save(rep1).await.expect("save");

        let mut rep2 = sample_rep("tw-2", "Bravo", SalesRepRole::Se);
        rep2.team_id = Some("team-west".to_string());
        repo.save(rep2).await.expect("save");

        let mut rep3 = sample_rep("te-1", "Charlie", SalesRepRole::Ae);
        rep3.team_id = Some("team-east".to_string());
        repo.save(rep3).await.expect("save");

        let west = repo.list_by_team("team-west", true).await.expect("list");
        assert_eq!(west.len(), 2);

        let east = repo.list_by_team("team-east", true).await.expect("list");
        assert_eq!(east.len(), 1);
    }

    #[tokio::test]
    async fn list_active_respects_limit() {
        let pool = setup().await;
        let repo = SqlSalesRepRepository::new(pool);

        for i in 0..5 {
            repo.save(sample_rep(&format!("la-{i}"), &format!("Rep {i}"), SalesRepRole::Ae))
                .await
                .expect("save");
        }

        let limited = repo.list_active(3).await.expect("list");
        assert_eq!(limited.len(), 3);

        let all = repo.list_active(100).await.expect("list");
        assert_eq!(all.len(), 5);
    }

    #[tokio::test]
    async fn self_referential_reports_to() {
        let pool = setup().await;
        let repo = SqlSalesRepRepository::new(pool);

        let manager = sample_rep("mgr-self", "Manager", SalesRepRole::Manager);
        repo.save(manager).await.expect("save manager");

        let mut ae = sample_rep("ae-self", "AE", SalesRepRole::Ae);
        ae.reports_to = Some(SalesRepId("mgr-self".to_string()));
        repo.save(ae).await.expect("save ae");

        let found =
            repo.find_by_id(&SalesRepId("ae-self".into())).await.expect("find").expect("exists");
        assert_eq!(found.reports_to, Some(SalesRepId("mgr-self".to_string())));
    }

    #[tokio::test]
    async fn budget_fields_persist() {
        let pool = setup().await;
        let repo = SqlSalesRepRepository::new(pool);

        let mut rep = sample_rep("rep-bud", "Budget Rep", SalesRepRole::Ae);
        rep.discount_budget_monthly_cents = 1_500_000;
        rep.spent_discount_cents = 750_000;
        repo.save(rep).await.expect("save");

        let found =
            repo.find_by_id(&SalesRepId("rep-bud".into())).await.expect("find").expect("exists");
        assert_eq!(found.discount_budget_monthly_cents, 1_500_000);
        assert_eq!(found.spent_discount_cents, 750_000);
    }
}
