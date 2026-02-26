use chrono::{DateTime, Utc};
use quotey_core::domain::product::{
    AttributeValueType, Product, ProductAttribute, ProductFamilyId, ProductId, ProductType,
};
use rust_decimal::Decimal;
use std::str::FromStr;

use super::{ProductRepository, RepositoryError};
use crate::DbPool;

pub struct SqlProductRepository {
    pool: DbPool,
}

impl SqlProductRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Hydrate attributes for a product from the product_attribute table.
    async fn load_attributes(
        &self,
        product_id: &str,
    ) -> Result<Vec<ProductAttribute>, RepositoryError> {
        let rows = sqlx::query_as::<_, AttributeRow>(
            "SELECT key, display_name, value_type, required, default_value \
             FROM product_attribute WHERE product_id = ? ORDER BY sort_order",
        )
        .bind(product_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|r| r.try_into()).collect()
    }

    /// Persist attributes for a product (replaces all existing).
    async fn save_attributes(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        product_id: &str,
        attrs: &[ProductAttribute],
    ) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM product_attribute WHERE product_id = ?")
            .bind(product_id)
            .execute(&mut **tx)
            .await?;

        for (i, attr) in attrs.iter().enumerate() {
            let vt_json = serde_json::to_string(&attr.value_type)
                .map_err(|e| RepositoryError::Decode(format!("serialize attribute type: {e}")))?;
            sqlx::query(
                "INSERT INTO product_attribute (product_id, key, display_name, value_type, required, default_value, sort_order) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(product_id)
            .bind(&attr.key)
            .bind(&attr.display_name)
            .bind(&vt_json)
            .bind(attr.required)
            .bind(&attr.default_value)
            .bind(i as i32)
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl ProductRepository for SqlProductRepository {
    async fn find_by_id(&self, id: &ProductId) -> Result<Option<Product>, RepositoryError> {
        let row = sqlx::query_as::<_, ProductRow>(
            "SELECT id, sku, name, description, product_type, family_id, \
             base_price, currency, active, created_at, updated_at \
             FROM product WHERE id = ?",
        )
        .bind(&id.0)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            None => Ok(None),
            Some(r) => {
                let attrs = self.load_attributes(&r.id).await?;
                Ok(Some(r.into_product(attrs)?))
            }
        }
    }

    async fn save(&self, product: Product) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await?;

        let base_price_str = product.base_price.map(|d| d.to_string());

        sqlx::query(
            "INSERT INTO product (id, sku, name, description, product_type, family_id, \
             base_price, currency, active, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET \
             sku=excluded.sku, name=excluded.name, description=excluded.description, \
             product_type=excluded.product_type, family_id=excluded.family_id, \
             base_price=excluded.base_price, currency=excluded.currency, \
             active=excluded.active, updated_at=excluded.updated_at",
        )
        .bind(&product.id.0)
        .bind(&product.sku)
        .bind(&product.name)
        .bind(&product.description)
        .bind(product.product_type.as_str())
        .bind(product.family_id.as_ref().map(|f| &f.0))
        .bind(&base_price_str)
        .bind(&product.currency)
        .bind(product.active)
        .bind(product.created_at.to_rfc3339())
        .bind(product.updated_at.to_rfc3339())
        .execute(&mut *tx)
        .await?;

        self.save_attributes(&mut tx, &product.id.0, &product.attributes).await?;

        tx.commit().await?;
        Ok(())
    }

    async fn search(
        &self,
        query: &str,
        active_only: bool,
        limit: u32,
    ) -> Result<Vec<Product>, RepositoryError> {
        let rows: Vec<ProductRow> = if query.is_empty() {
            let sql = if active_only {
                "SELECT id, sku, name, description, product_type, family_id, \
                 base_price, currency, active, created_at, updated_at \
                 FROM product WHERE active = 1 ORDER BY name LIMIT ?"
            } else {
                "SELECT id, sku, name, description, product_type, family_id, \
                 base_price, currency, active, created_at, updated_at \
                 FROM product ORDER BY name LIMIT ?"
            };
            sqlx::query_as::<_, ProductRow>(sql).bind(limit as i64).fetch_all(&self.pool).await?
        } else {
            // FTS search — append * for prefix matching.
            let fts_query = format!("{}*", query.replace('"', ""));
            let sql = if active_only {
                "SELECT p.id, p.sku, p.name, p.description, p.product_type, p.family_id, \
                 p.base_price, p.currency, p.active, p.created_at, p.updated_at \
                 FROM product p \
                 INNER JOIN product_fts f ON f.product_id = p.id \
                 WHERE product_fts MATCH ? AND p.active = 1 \
                 ORDER BY rank LIMIT ?"
            } else {
                "SELECT p.id, p.sku, p.name, p.description, p.product_type, p.family_id, \
                 p.base_price, p.currency, p.active, p.created_at, p.updated_at \
                 FROM product p \
                 INNER JOIN product_fts f ON f.product_id = p.id \
                 WHERE product_fts MATCH ? \
                 ORDER BY rank LIMIT ?"
            };
            sqlx::query_as::<_, ProductRow>(sql)
                .bind(&fts_query)
                .bind(limit as i64)
                .fetch_all(&self.pool)
                .await?
        };

        let mut products = Vec::with_capacity(rows.len());
        for r in rows {
            let attrs = self.load_attributes(&r.id).await?;
            products.push(r.into_product(attrs)?);
        }
        Ok(products)
    }

    async fn list_by_family(&self, family_id: &str) -> Result<Vec<Product>, RepositoryError> {
        let rows = sqlx::query_as::<_, ProductRow>(
            "SELECT id, sku, name, description, product_type, family_id, \
             base_price, currency, active, created_at, updated_at \
             FROM product WHERE family_id = ? ORDER BY name",
        )
        .bind(family_id)
        .fetch_all(&self.pool)
        .await?;

        let mut products = Vec::with_capacity(rows.len());
        for r in rows {
            let attrs = self.load_attributes(&r.id).await?;
            products.push(r.into_product(attrs)?);
        }
        Ok(products)
    }
}

// ---------------------------------------------------------------------------
// Row types for sqlx FromRow
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct ProductRow {
    id: String,
    sku: String,
    name: String,
    description: Option<String>,
    product_type: String,
    family_id: Option<String>,
    base_price: Option<String>,
    currency: String,
    active: bool,
    created_at: String,
    updated_at: String,
}

impl ProductRow {
    fn into_product(self, attributes: Vec<ProductAttribute>) -> Result<Product, RepositoryError> {
        let product_type =
            self.product_type.parse::<ProductType>().map_err(RepositoryError::Decode)?;

        let base_price = self
            .base_price
            .as_deref()
            .map(Decimal::from_str)
            .transpose()
            .map_err(|e| RepositoryError::Decode(format!("invalid base_price: {e}")))?;

        let family_id = self.family_id.map(ProductFamilyId);

        let created_at: DateTime<Utc> = self
            .created_at
            .parse()
            .map_err(|e| RepositoryError::Decode(format!("invalid created_at: {e}")))?;
        let updated_at: DateTime<Utc> = self
            .updated_at
            .parse()
            .map_err(|e| RepositoryError::Decode(format!("invalid updated_at: {e}")))?;

        Ok(Product {
            id: ProductId(self.id),
            sku: self.sku,
            name: self.name,
            description: self.description,
            product_type,
            family_id,
            base_price,
            currency: self.currency,
            attributes,
            active: self.active,
            created_at,
            updated_at,
        })
    }
}

#[derive(sqlx::FromRow)]
struct AttributeRow {
    key: String,
    display_name: String,
    value_type: String,
    required: bool,
    default_value: Option<String>,
}

impl TryFrom<AttributeRow> for ProductAttribute {
    type Error = RepositoryError;

    fn try_from(row: AttributeRow) -> Result<Self, RepositoryError> {
        let value_type: AttributeValueType =
            serde_json::from_str(&row.value_type).map_err(|e| {
                RepositoryError::Decode(format!("invalid attribute value_type JSON: {e}"))
            })?;

        Ok(ProductAttribute {
            key: row.key,
            display_name: row.display_name,
            value_type,
            required: row.required,
            default_value: row.default_value,
        })
    }
}
