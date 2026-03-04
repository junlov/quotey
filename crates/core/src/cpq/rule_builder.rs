use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{
    LogicalConnector, VisualActionType, VisualOperator, VisualRuleCondition, VisualRuleDefinition,
    VisualRuleType,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricingRuleDraft {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    pub conditions: Vec<PricingRuleCondition>,
    pub action: PricingRuleAction,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricingRuleCondition {
    pub field_key: String,
    pub operator: PricingRuleOperator,
    pub value: String,
    pub connector: Option<LogicalConnector>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PricingRuleOperator {
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
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PricingRuleAction {
    SetUnitPrice { amount: Decimal, currency: String },
    ApplyDiscountCap { max_discount_pct: Decimal },
}

pub fn build_pricing_rule(
    visual_rule: &VisualRuleDefinition,
) -> Result<PricingRuleDraft, PricingRuleBuilderError> {
    visual_rule.validate().map_err(PricingRuleBuilderError::InvalidVisualRule)?;

    if visual_rule.rule_type != VisualRuleType::Pricing {
        return Err(PricingRuleBuilderError::WrongRuleType { actual: visual_rule.rule_type });
    }

    let first_action = visual_rule.actions.first().ok_or(PricingRuleBuilderError::MissingAction)?;
    let action = build_action(first_action.action_type, &first_action.parameters)?;

    let mut conditions = Vec::with_capacity(visual_rule.conditions.len());
    for condition in &visual_rule.conditions {
        conditions.push(build_condition(condition)?);
    }

    Ok(PricingRuleDraft {
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
) -> Result<PricingRuleCondition, PricingRuleBuilderError> {
    let operator = match condition.operator {
        VisualOperator::Equals => PricingRuleOperator::Equals,
        VisualOperator::NotEquals => PricingRuleOperator::NotEquals,
        VisualOperator::GreaterThan => PricingRuleOperator::GreaterThan,
        VisualOperator::GreaterOrEqual => PricingRuleOperator::GreaterOrEqual,
        VisualOperator::LessThan => PricingRuleOperator::LessThan,
        VisualOperator::LessOrEqual => PricingRuleOperator::LessOrEqual,
        VisualOperator::In => PricingRuleOperator::In,
        VisualOperator::NotIn => PricingRuleOperator::NotIn,
        VisualOperator::Contains => PricingRuleOperator::Contains,
    };

    Ok(PricingRuleCondition {
        field_key: condition.field_key.clone(),
        operator,
        value: canonical_value(&condition.value),
        connector: condition.connector,
    })
}

fn build_action(
    action_type: VisualActionType,
    parameters: &std::collections::BTreeMap<String, Value>,
) -> Result<PricingRuleAction, PricingRuleBuilderError> {
    match action_type {
        VisualActionType::SetUnitPrice => {
            let amount = decimal_from_parameter(parameters, "amount")?;
            let currency = parameters
                .get("currency")
                .and_then(Value::as_str)
                .unwrap_or("USD")
                .trim()
                .to_ascii_uppercase();

            if currency.is_empty() {
                return Err(PricingRuleBuilderError::InvalidCurrency);
            }

            Ok(PricingRuleAction::SetUnitPrice { amount, currency })
        }
        VisualActionType::ApplyDiscountCap => {
            let max_discount_pct = decimal_from_parameter(parameters, "max_discount_pct")?;
            Ok(PricingRuleAction::ApplyDiscountCap { max_discount_pct })
        }
        other => Err(PricingRuleBuilderError::UnsupportedAction { action: other }),
    }
}

fn decimal_from_parameter(
    parameters: &std::collections::BTreeMap<String, Value>,
    key: &str,
) -> Result<Decimal, PricingRuleBuilderError> {
    let value = parameters
        .get(key)
        .ok_or_else(|| PricingRuleBuilderError::MissingParameter { key: key.to_string() })?;

    let number = match value {
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        _ => {
            return Err(PricingRuleBuilderError::InvalidParameterType { key: key.to_string() });
        }
    };

    Decimal::from_str_exact(number.trim()).map_err(|_| PricingRuleBuilderError::InvalidDecimal {
        key: key.to_string(),
        value: number,
    })
}

fn canonical_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum PricingRuleBuilderError {
    #[error("visual rule validation failed: {0}")]
    InvalidVisualRule(#[source] crate::VisualRuleValidationError),
    #[error("expected pricing rule type, got `{actual:?}`")]
    WrongRuleType { actual: VisualRuleType },
    #[error("pricing rule must include at least one action")]
    MissingAction,
    #[error("unsupported action for pricing builder: `{action:?}`")]
    UnsupportedAction { action: VisualActionType },
    #[error("missing action parameter `{key}`")]
    MissingParameter { key: String },
    #[error("invalid parameter type for `{key}`")]
    InvalidParameterType { key: String },
    #[error("invalid decimal for `{key}`: `{value}`")]
    InvalidDecimal { key: String, value: String },
    #[error("currency cannot be empty")]
    InvalidCurrency,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::{build_pricing_rule, PricingRuleAction, PricingRuleBuilderError};
    use crate::{
        VisualActionType, VisualOperator, VisualRuleAction, VisualRuleCondition,
        VisualRuleDefinition, VisualRuleMetadata, VisualRuleType, VISUAL_RULE_SCHEMA_VERSION,
    };

    fn pricing_rule_fixture(action: VisualActionType) -> VisualRuleDefinition {
        let mut parameters = BTreeMap::new();
        parameters.insert("amount".to_string(), json!("99.95"));
        parameters.insert("currency".to_string(), json!("usd"));

        VisualRuleDefinition {
            schema_version: VISUAL_RULE_SCHEMA_VERSION.to_string(),
            id: "price-us-enterprise".to_string(),
            name: "US Enterprise base price".to_string(),
            description: Some("Set enterprise plan USD list price".to_string()),
            rule_type: VisualRuleType::Pricing,
            enabled: true,
            priority: 10,
            conditions: vec![VisualRuleCondition {
                field_key: "customer_segment".to_string(),
                operator: VisualOperator::Equals,
                value: json!("enterprise"),
                connector: None,
            }],
            actions: vec![VisualRuleAction { action_type: action, parameters }],
            metadata: VisualRuleMetadata {
                created_by: "salesops:1".to_string(),
                updated_by: "salesops:1".to_string(),
                tags: vec!["pricing".to_string()],
                rationale: None,
            },
        }
    }

    #[test]
    fn builds_set_unit_price_rule_draft() {
        let visual_rule = pricing_rule_fixture(VisualActionType::SetUnitPrice);
        let draft = build_pricing_rule(&visual_rule).expect("rule should translate");

        assert_eq!(draft.id, "price-us-enterprise");
        assert_eq!(draft.conditions.len(), 1);
        assert_eq!(
            draft.action,
            PricingRuleAction::SetUnitPrice {
                amount: "99.95".parse().expect("decimal"),
                currency: "USD".to_string(),
            }
        );
    }

    #[test]
    fn builds_discount_cap_rule_draft() {
        let mut visual_rule = pricing_rule_fixture(VisualActionType::ApplyDiscountCap);
        visual_rule.actions[0].parameters.clear();
        visual_rule.actions[0].parameters.insert("max_discount_pct".to_string(), json!(15.5));

        let draft = build_pricing_rule(&visual_rule).expect("rule should translate");

        assert_eq!(
            draft.action,
            PricingRuleAction::ApplyDiscountCap {
                max_discount_pct: "15.5".parse().expect("decimal"),
            }
        );
    }

    #[test]
    fn rejects_non_pricing_rule_type() {
        let mut visual_rule = pricing_rule_fixture(VisualActionType::SetUnitPrice);
        visual_rule.rule_type = VisualRuleType::DiscountPolicy;

        assert_eq!(
            build_pricing_rule(&visual_rule),
            Err(PricingRuleBuilderError::WrongRuleType { actual: VisualRuleType::DiscountPolicy })
        );
    }
}
