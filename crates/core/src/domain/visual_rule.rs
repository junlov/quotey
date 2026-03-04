use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const VISUAL_RULE_SCHEMA_VERSION: &str = "visual_rule.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualRuleDefinition {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub rule_type: VisualRuleType,
    pub enabled: bool,
    pub priority: i32,
    pub conditions: Vec<VisualRuleCondition>,
    pub actions: Vec<VisualRuleAction>,
    pub metadata: VisualRuleMetadata,
}

impl VisualRuleDefinition {
    pub fn validate(&self) -> Result<(), VisualRuleValidationError> {
        if self.schema_version != VISUAL_RULE_SCHEMA_VERSION {
            return Err(VisualRuleValidationError::UnsupportedSchemaVersion {
                expected: VISUAL_RULE_SCHEMA_VERSION.to_string(),
                actual: self.schema_version.clone(),
            });
        }

        if self.id.trim().is_empty() {
            return Err(VisualRuleValidationError::MissingId);
        }

        if self.name.trim().is_empty() {
            return Err(VisualRuleValidationError::MissingName);
        }

        if self.conditions.is_empty() {
            return Err(VisualRuleValidationError::MissingConditions);
        }

        if self.actions.is_empty() {
            return Err(VisualRuleValidationError::MissingActions);
        }

        if self.conditions.first().and_then(|condition| condition.connector).is_some() {
            return Err(VisualRuleValidationError::UnexpectedFirstConnector);
        }

        Ok(())
    }

    pub fn canonical_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualRuleType {
    Pricing,
    Constraint,
    DiscountPolicy,
    ApprovalThreshold,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualRuleCondition {
    pub field_key: String,
    pub operator: VisualOperator,
    pub value: Value,
    pub connector: Option<LogicalConnector>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogicalConnector {
    And,
    Or,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualOperator {
    Equals,
    NotEquals,
    GreaterThan,
    GreaterOrEqual,
    LessThan,
    LessOrEqual,
    In,
    NotIn,
    Contains,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualRuleAction {
    pub action_type: VisualActionType,
    pub parameters: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualActionType {
    SetUnitPrice,
    ApplyDiscountCap,
    RequireProduct,
    ExcludeProduct,
    RouteApprovalRole,
    SetApprovalThreshold,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualRuleMetadata {
    pub created_by: String,
    pub updated_by: String,
    pub tags: Vec<String>,
    pub rationale: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum VisualRuleValidationError {
    #[error("unsupported schema version: expected `{expected}`, got `{actual}`")]
    UnsupportedSchemaVersion { expected: String, actual: String },
    #[error("visual rule id cannot be empty")]
    MissingId,
    #[error("visual rule name cannot be empty")]
    MissingName,
    #[error("visual rule requires at least one condition")]
    MissingConditions,
    #[error("visual rule requires at least one action")]
    MissingActions,
    #[error("first condition cannot include a logical connector")]
    UnexpectedFirstConnector,
}

#[cfg(test)]
mod tests {
    use super::{
        LogicalConnector, VisualActionType, VisualOperator, VisualRuleAction, VisualRuleCondition,
        VisualRuleDefinition, VisualRuleMetadata, VisualRuleType, VisualRuleValidationError,
        VISUAL_RULE_SCHEMA_VERSION,
    };
    use serde_json::{json, Value};
    use std::collections::BTreeMap;

    fn base_rule() -> VisualRuleDefinition {
        let mut parameters = BTreeMap::new();
        parameters.insert("max_discount_pct".to_string(), Value::from(12.5));

        VisualRuleDefinition {
            schema_version: VISUAL_RULE_SCHEMA_VERSION.to_string(),
            id: "rule-discount-smb".to_string(),
            name: "SMB discount guardrail".to_string(),
            description: Some("Cap SMB discount at 12.5%".to_string()),
            rule_type: VisualRuleType::DiscountPolicy,
            enabled: true,
            priority: 100,
            conditions: vec![VisualRuleCondition {
                field_key: "account_tier".to_string(),
                operator: VisualOperator::Equals,
                value: json!("smb"),
                connector: None,
            }],
            actions: vec![VisualRuleAction {
                action_type: VisualActionType::ApplyDiscountCap,
                parameters,
            }],
            metadata: VisualRuleMetadata {
                created_by: "salesops:1".to_string(),
                updated_by: "salesops:1".to_string(),
                tags: vec!["discount".to_string(), "segment:smb".to_string()],
                rationale: Some("Protect margin for low ARR accounts".to_string()),
            },
        }
    }

    #[test]
    fn validate_accepts_contract_complete_rule() {
        let rule = base_rule();
        assert_eq!(rule.validate(), Ok(()));
    }

    #[test]
    fn validate_rejects_unsupported_schema_version() {
        let mut rule = base_rule();
        rule.schema_version = "visual_rule.v0".to_string();

        assert_eq!(
            rule.validate(),
            Err(VisualRuleValidationError::UnsupportedSchemaVersion {
                expected: VISUAL_RULE_SCHEMA_VERSION.to_string(),
                actual: "visual_rule.v0".to_string(),
            })
        );
    }

    #[test]
    fn validate_rejects_first_condition_connector() {
        let mut rule = base_rule();
        rule.conditions[0].connector = Some(LogicalConnector::And);

        assert_eq!(rule.validate(), Err(VisualRuleValidationError::UnexpectedFirstConnector));
    }

    #[test]
    fn canonical_json_is_deterministic() {
        let mut first = base_rule();
        let mut second = base_rule();

        first.actions[0].parameters.clear();
        first.actions[0].parameters.insert("z".to_string(), json!("end"));
        first.actions[0].parameters.insert("a".to_string(), json!("start"));

        second.actions[0].parameters.clear();
        second.actions[0].parameters.insert("a".to_string(), json!("start"));
        second.actions[0].parameters.insert("z".to_string(), json!("end"));

        let first_json = first.canonical_json().expect("canonical json");
        let second_json = second.canonical_json().expect("canonical json");
        assert_eq!(first_json, second_json);
    }
}
