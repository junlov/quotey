use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::errors::DomainError;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SalesRepId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SalesRepRole {
    Ae,
    Se,
    Manager,
    Vp,
    Cro,
    Ops,
}

impl SalesRepRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ae => "ae",
            Self::Se => "se",
            Self::Manager => "manager",
            Self::Vp => "vp",
            Self::Cro => "cro",
            Self::Ops => "ops",
        }
    }
}

impl FromStr for SalesRepRole {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ae" => Ok(Self::Ae),
            "se" => Ok(Self::Se),
            "manager" => Ok(Self::Manager),
            "vp" => Ok(Self::Vp),
            "cro" => Ok(Self::Cro),
            "ops" => Ok(Self::Ops),
            _ => Err(DomainError::InvalidEnumValue {
                enum_name: "SalesRepRole".to_string(),
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SalesRepStatus {
    Active,
    Inactive,
    Disabled,
}

impl SalesRepStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Disabled => "disabled",
        }
    }
}

impl FromStr for SalesRepStatus {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "inactive" => Ok(Self::Inactive),
            "disabled" => Ok(Self::Disabled),
            _ => Err(DomainError::InvalidEnumValue {
                enum_name: "SalesRepStatus".to_string(),
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SalesRep {
    pub id: SalesRepId,
    pub external_user_ref: Option<String>,
    pub name: String,
    pub email: Option<String>,
    pub role: SalesRepRole,
    pub title: Option<String>,
    pub team_id: Option<String>,
    pub reports_to: Option<SalesRepId>,
    pub status: SalesRepStatus,
    pub max_discount_pct: Option<f64>,
    pub auto_approve_threshold_cents: Option<i64>,
    pub capabilities_json: String,
    pub config_json: String,
    pub discount_budget_monthly_cents: i64,
    pub spent_discount_cents: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::domain::sales_rep::{SalesRepRole, SalesRepStatus};

    #[test]
    fn parses_sales_rep_role_from_database_values() {
        assert_eq!(
            SalesRepRole::from_str("manager").expect("role should parse"),
            SalesRepRole::Manager
        );
    }

    #[test]
    fn parses_sales_rep_status_from_database_values() {
        assert_eq!(
            SalesRepStatus::from_str("active").expect("status should parse"),
            SalesRepStatus::Active
        );
    }

    #[test]
    fn rejects_unknown_sales_rep_role_value() {
        let result = SalesRepRole::from_str("director");
        assert!(result.is_err());
    }
}
