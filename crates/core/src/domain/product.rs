use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProductId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProductFamilyId(pub String);

// ---------------------------------------------------------------------------
// Product type — determines which CPQ rules apply
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProductType {
    /// Fixed-price product with no configurable dimensions.
    Simple,
    /// Product with configurable attributes (seats, tier, add-ons).
    Configurable,
    /// Contains other products — pricing is derived from components.
    Bundle,
}

impl ProductType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Configurable => "configurable",
            Self::Bundle => "bundle",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "simple" => Some(Self::Simple),
            "configurable" => Some(Self::Configurable),
            "bundle" => Some(Self::Bundle),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Product family — groups products for rule application
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductFamily {
    pub id: ProductFamilyId,
    pub name: String,
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// Product attribute — configurable dimension on a product
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AttributeValueType {
    Integer { min: Option<i64>, max: Option<i64> },
    Decimal { min: Option<Decimal>, max: Option<Decimal> },
    Enum { allowed_values: Vec<String> },
    Boolean,
    Text { max_length: Option<u32> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProductAttribute {
    pub key: String,
    pub display_name: String,
    pub value_type: AttributeValueType,
    pub required: bool,
    pub default_value: Option<String>,
}

// ---------------------------------------------------------------------------
// Product (enriched)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Product {
    pub id: ProductId,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub product_type: ProductType,
    pub family_id: Option<ProductFamilyId>,
    pub base_price: Option<Decimal>,
    pub currency: String,
    pub attributes: Vec<ProductAttribute>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Product {
    /// Quick constructor for tests and seed data (simple product, no attributes).
    pub fn simple(id: impl Into<String>, sku: impl Into<String>, name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: ProductId(id.into()),
            sku: sku.into(),
            name: name.into(),
            description: None,
            product_type: ProductType::Simple,
            family_id: None,
            base_price: None,
            currency: "USD".to_string(),
            attributes: Vec::new(),
            active: true,
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_type_roundtrip() {
        for pt in [ProductType::Simple, ProductType::Configurable, ProductType::Bundle] {
            assert_eq!(ProductType::from_str(pt.as_str()), Some(pt));
        }
        assert_eq!(ProductType::from_str("unknown"), None);
    }

    #[test]
    fn simple_product_defaults() {
        let p = Product::simple("prod-1", "SKU-001", "Widget");
        assert_eq!(p.product_type, ProductType::Simple);
        assert!(p.active);
        assert!(p.attributes.is_empty());
        assert_eq!(p.currency, "USD");
    }
}
