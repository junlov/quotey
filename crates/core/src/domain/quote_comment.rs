use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::errors::DomainError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorType {
    Rep,
    Manager,
    System,
    Ai,
    Integration,
}

impl AuthorType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rep => "rep",
            Self::Manager => "manager",
            Self::System => "system",
            Self::Ai => "ai",
            Self::Integration => "integration",
        }
    }
}

impl FromStr for AuthorType {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "rep" => Ok(Self::Rep),
            "manager" => Ok(Self::Manager),
            "system" => Ok(Self::System),
            "ai" => Ok(Self::Ai),
            "integration" => Ok(Self::Integration),
            _ => Err(DomainError::InvalidEnumValue {
                enum_name: "AuthorType".to_string(),
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuoteComment {
    pub id: String,
    pub quote_id: String,
    pub author_type: AuthorType,
    pub author_id: String,
    pub body: String,
    pub metadata_json: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::AuthorType;

    #[test]
    fn parses_author_type_from_database_values() {
        assert_eq!(AuthorType::from_str("rep").expect("should parse"), AuthorType::Rep);
        assert_eq!(AuthorType::from_str("manager").expect("should parse"), AuthorType::Manager);
        assert_eq!(AuthorType::from_str("system").expect("should parse"), AuthorType::System);
        assert_eq!(AuthorType::from_str("ai").expect("should parse"), AuthorType::Ai);
        assert_eq!(
            AuthorType::from_str("integration").expect("should parse"),
            AuthorType::Integration
        );
    }

    #[test]
    fn parses_author_type_case_insensitive() {
        assert_eq!(AuthorType::from_str("REP").expect("should parse"), AuthorType::Rep);
        assert_eq!(AuthorType::from_str("Manager").expect("should parse"), AuthorType::Manager);
    }

    #[test]
    fn rejects_unknown_author_type_value() {
        let result = AuthorType::from_str("admin");
        assert!(result.is_err());
    }

    #[test]
    fn author_type_round_trips_through_as_str() {
        for variant in [
            AuthorType::Rep,
            AuthorType::Manager,
            AuthorType::System,
            AuthorType::Ai,
            AuthorType::Integration,
        ] {
            let s = variant.as_str();
            let parsed = AuthorType::from_str(s).expect("round-trip should work");
            assert_eq!(parsed, variant);
        }
    }
}
