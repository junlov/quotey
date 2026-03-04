use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    LogicalConnector, VisualActionType, VisualOperator, VisualRuleCondition, VisualRuleDefinition,
    VisualRuleType,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintRuleDraft {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    pub conditions: Vec<ConstraintRuleCondition>,
    pub action: ConstraintRuleAction,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintRuleCondition {
    pub field_key: String,
    pub operator: ConstraintRuleOperator,
    pub value: String,
    pub connector: Option<LogicalConnector>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintRuleOperator {
    Equals,
    NotEquals,
    Contains,
    In,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConstraintRuleAction {
    RequireProduct { required_product_id: String },
    ExcludeProduct { excluded_product_id: String },
}

pub fn build_constraint_rule(
    visual_rule: &VisualRuleDefinition,
) -> Result<ConstraintRuleDraft, ConstraintRuleBuilderError> {
    visual_rule.validate().map_err(ConstraintRuleBuilderError::InvalidVisualRule)?;

    if visual_rule.rule_type != VisualRuleType::Constraint {
        return Err(ConstraintRuleBuilderError::WrongRuleType { actual: visual_rule.rule_type });
    }

    let first_action =
        visual_rule.actions.first().ok_or(ConstraintRuleBuilderError::MissingAction)?;
    let action = build_action(first_action.action_type, &first_action.parameters)?;

    let mut conditions = Vec::with_capacity(visual_rule.conditions.len());
    for condition in &visual_rule.conditions {
        conditions.push(build_condition(condition)?);
    }

    Ok(ConstraintRuleDraft {
        id: visual_rule.id.clone(),
        name: visual_rule.name.clone(),
        enabled: visual_rule.enabled,
        priority: visual_rule.priority,
        conditions,
        action,
    })
}

fn build_condition(
    condition: &VisualRuleCondition,
) -> Result<ConstraintRuleCondition, ConstraintRuleBuilderError> {
    let operator = match condition.operator {
        VisualOperator::Equals => ConstraintRuleOperator::Equals,
        VisualOperator::NotEquals => ConstraintRuleOperator::NotEquals,
        VisualOperator::Contains => ConstraintRuleOperator::Contains,
        VisualOperator::In => ConstraintRuleOperator::In,
        unsupported => {
            return Err(ConstraintRuleBuilderError::UnsupportedConditionOperator {
                operator: unsupported,
            });
        }
    };

    Ok(ConstraintRuleCondition {
        field_key: condition.field_key.clone(),
        operator,
        value: canonical_value(&condition.value),
        connector: condition.connector,
    })
}

fn build_action(
    action_type: VisualActionType,
    parameters: &std::collections::BTreeMap<String, serde_json::Value>,
) -> Result<ConstraintRuleAction, ConstraintRuleBuilderError> {
    match action_type {
        VisualActionType::RequireProduct => {
            let required_product_id = required_string_parameter(parameters, "required_product_id")?;
            Ok(ConstraintRuleAction::RequireProduct { required_product_id })
        }
        VisualActionType::ExcludeProduct => {
            let excluded_product_id = required_string_parameter(parameters, "excluded_product_id")?;
            Ok(ConstraintRuleAction::ExcludeProduct { excluded_product_id })
        }
        other => Err(ConstraintRuleBuilderError::UnsupportedAction { action: other }),
    }
}

fn required_string_parameter(
    parameters: &std::collections::BTreeMap<String, serde_json::Value>,
    key: &str,
) -> Result<String, ConstraintRuleBuilderError> {
    let value = parameters
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| ConstraintRuleBuilderError::MissingParameter { key: key.to_string() })?;

    Ok(value.to_string())
}

fn canonical_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum ConstraintRuleBuilderError {
    #[error("visual rule validation failed: {0}")]
    InvalidVisualRule(#[source] crate::VisualRuleValidationError),
    #[error("expected constraint rule type, got `{actual:?}`")]
    WrongRuleType { actual: VisualRuleType },
    #[error("constraint rule must include at least one action")]
    MissingAction,
    #[error("unsupported action for constraint builder: `{action:?}`")]
    UnsupportedAction { action: VisualActionType },
    #[error("unsupported condition operator for constraint builder: `{operator:?}`")]
    UnsupportedConditionOperator { operator: VisualOperator },
    #[error("missing action parameter `{key}`")]
    MissingParameter { key: String },
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::{build_constraint_rule, ConstraintRuleAction, ConstraintRuleBuilderError};
    use crate::{
        VisualActionType, VisualOperator, VisualRuleAction, VisualRuleCondition,
        VisualRuleDefinition, VisualRuleMetadata, VisualRuleType, VISUAL_RULE_SCHEMA_VERSION,
    };

    fn rule_fixture(action_type: VisualActionType) -> VisualRuleDefinition {
        let mut parameters = BTreeMap::new();
        parameters.insert("required_product_id".to_string(), json!("plan-enterprise"));

        VisualRuleDefinition {
            schema_version: VISUAL_RULE_SCHEMA_VERSION.to_string(),
            id: "constraint-sso-requires-enterprise".to_string(),
            name: "SSO requires enterprise".to_string(),
            description: Some("Prevent invalid SSO combinations".to_string()),
            rule_type: VisualRuleType::Constraint,
            enabled: true,
            priority: 30,
            conditions: vec![VisualRuleCondition {
                field_key: "product_id".to_string(),
                operator: VisualOperator::Equals,
                value: json!("addon-sso"),
                connector: None,
            }],
            actions: vec![VisualRuleAction { action_type, parameters }],
            metadata: VisualRuleMetadata {
                created_by: "salesops:1".to_string(),
                updated_by: "salesops:1".to_string(),
                tags: vec!["constraints".to_string()],
                rationale: None,
            },
        }
    }

    #[test]
    fn builds_require_product_constraint() {
        let rule = rule_fixture(VisualActionType::RequireProduct);
        let draft = build_constraint_rule(&rule).expect("should build");

        assert_eq!(
            draft.action,
            ConstraintRuleAction::RequireProduct {
                required_product_id: "plan-enterprise".to_string(),
            }
        );
    }

    #[test]
    fn builds_exclude_product_constraint() {
        let mut rule = rule_fixture(VisualActionType::ExcludeProduct);
        rule.actions[0].parameters.clear();
        rule.actions[0].parameters.insert("excluded_product_id".to_string(), json!("addon-legacy"));

        let draft = build_constraint_rule(&rule).expect("should build");
        assert_eq!(
            draft.action,
            ConstraintRuleAction::ExcludeProduct {
                excluded_product_id: "addon-legacy".to_string(),
            }
        );
    }

    #[test]
    fn rejects_unsupported_condition_operator() {
        let mut rule = rule_fixture(VisualActionType::RequireProduct);
        rule.conditions[0].operator = VisualOperator::GreaterThan;

        assert_eq!(
            build_constraint_rule(&rule),
            Err(ConstraintRuleBuilderError::UnsupportedConditionOperator {
                operator: VisualOperator::GreaterThan,
            })
        );
    }
}
