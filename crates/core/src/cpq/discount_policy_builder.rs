use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{VisualActionType, VisualRuleDefinition, VisualRuleType};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscountPolicyDraft {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
    pub customer_segment: Option<String>,
    pub product_category: Option<String>,
    pub min_deal_value: Option<Decimal>,
    pub max_discount_auto_approve_pct: Decimal,
    pub max_discount_with_approval_pct: Option<Decimal>,
    pub required_approver_role: Option<String>,
}

pub fn build_discount_policy(
    visual_rule: &VisualRuleDefinition,
) -> Result<DiscountPolicyDraft, DiscountPolicyBuilderError> {
    visual_rule.validate().map_err(DiscountPolicyBuilderError::InvalidVisualRule)?;

    if visual_rule.rule_type != VisualRuleType::DiscountPolicy {
        return Err(DiscountPolicyBuilderError::WrongRuleType { actual: visual_rule.rule_type });
    }

    let mut customer_segment = None;
    let mut product_category = None;
    let mut min_deal_value = None;

    for condition in &visual_rule.conditions {
        match condition.field_key.as_str() {
            "customer_segment" => customer_segment = Some(canonical_value(&condition.value)),
            "product_category" => product_category = Some(canonical_value(&condition.value)),
            "deal_value" => {
                min_deal_value = Some(decimal_from_json(&condition.value).map_err(|_| {
                    DiscountPolicyBuilderError::InvalidDecimal {
                        key: "deal_value".to_string(),
                        value: condition.value.to_string(),
                    }
                })?)
            }
            _ => {}
        }
    }

    let mut max_discount_auto_approve_pct = None;
    let mut max_discount_with_approval_pct = None;
    let mut required_approver_role = None;

    for action in &visual_rule.actions {
        match action.action_type {
            VisualActionType::ApplyDiscountCap => {
                max_discount_auto_approve_pct =
                    Some(decimal_from_parameter(&action.parameters, "max_discount_pct")?);
            }
            VisualActionType::RouteApprovalRole => {
                required_approver_role =
                    Some(required_string_parameter(&action.parameters, "approver_role")?);
                if let Some(threshold) = action.parameters.get("max_discount_with_approval_pct") {
                    max_discount_with_approval_pct =
                        Some(decimal_from_json(threshold).map_err(|_| {
                            DiscountPolicyBuilderError::InvalidDecimal {
                                key: "max_discount_with_approval_pct".to_string(),
                                value: threshold.to_string(),
                            }
                        })?);
                }
            }
            VisualActionType::SetApprovalThreshold => {
                max_discount_with_approval_pct =
                    Some(decimal_from_parameter(&action.parameters, "threshold_pct")?);
            }
            _ => {}
        }
    }

    let max_discount_auto_approve_pct =
        max_discount_auto_approve_pct.ok_or(DiscountPolicyBuilderError::MissingParameter {
            key: "max_discount_pct".to_string(),
        })?;

    Ok(DiscountPolicyDraft {
        id: visual_rule.id.clone(),
        name: visual_rule.name.clone(),
        enabled: visual_rule.enabled,
        priority: visual_rule.priority,
        customer_segment,
        product_category,
        min_deal_value,
        max_discount_auto_approve_pct,
        max_discount_with_approval_pct,
        required_approver_role,
    })
}

fn required_string_parameter(
    parameters: &std::collections::BTreeMap<String, Value>,
    key: &str,
) -> Result<String, DiscountPolicyBuilderError> {
    let value = parameters
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .ok_or_else(|| DiscountPolicyBuilderError::MissingParameter { key: key.to_string() })?;

    Ok(value.to_string())
}

fn decimal_from_parameter(
    parameters: &std::collections::BTreeMap<String, Value>,
    key: &str,
) -> Result<Decimal, DiscountPolicyBuilderError> {
    let value = parameters
        .get(key)
        .ok_or_else(|| DiscountPolicyBuilderError::MissingParameter { key: key.to_string() })?;
    decimal_from_json(value).map_err(|_| DiscountPolicyBuilderError::InvalidDecimal {
        key: key.to_string(),
        value: value.to_string(),
    })
}

fn decimal_from_json(value: &Value) -> Result<Decimal, ()> {
    match value {
        Value::Number(number) => Decimal::from_str_exact(&number.to_string()).map_err(|_| ()),
        Value::String(text) => Decimal::from_str_exact(text.trim()).map_err(|_| ()),
        _ => Err(()),
    }
}

fn canonical_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum DiscountPolicyBuilderError {
    #[error("visual rule validation failed: {0}")]
    InvalidVisualRule(#[source] crate::VisualRuleValidationError),
    #[error("expected discount_policy rule type, got `{actual:?}`")]
    WrongRuleType { actual: VisualRuleType },
    #[error("missing parameter `{key}`")]
    MissingParameter { key: String },
    #[error("invalid decimal for `{key}`: `{value}`")]
    InvalidDecimal { key: String, value: String },
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::{build_discount_policy, DiscountPolicyBuilderError};
    use crate::{
        LogicalConnector, VisualActionType, VisualOperator, VisualRuleAction, VisualRuleCondition,
        VisualRuleDefinition, VisualRuleMetadata, VisualRuleType, VISUAL_RULE_SCHEMA_VERSION,
    };

    fn fixture() -> VisualRuleDefinition {
        let mut discount_parameters = BTreeMap::new();
        discount_parameters.insert("max_discount_pct".to_string(), json!(10.0));

        let mut approval_parameters = BTreeMap::new();
        approval_parameters.insert("approver_role".to_string(), json!("sales_manager"));
        approval_parameters.insert("max_discount_with_approval_pct".to_string(), json!(20.0));

        VisualRuleDefinition {
            schema_version: VISUAL_RULE_SCHEMA_VERSION.to_string(),
            id: "policy-smb-discount".to_string(),
            name: "SMB discount policy".to_string(),
            description: Some("SMB thresholds".to_string()),
            rule_type: VisualRuleType::DiscountPolicy,
            enabled: true,
            priority: 20,
            conditions: vec![
                VisualRuleCondition {
                    field_key: "customer_segment".to_string(),
                    operator: VisualOperator::Equals,
                    value: json!("smb"),
                    connector: None,
                },
                VisualRuleCondition {
                    field_key: "deal_value".to_string(),
                    operator: VisualOperator::GreaterOrEqual,
                    value: json!(5000),
                    connector: Some(LogicalConnector::And),
                },
            ],
            actions: vec![
                VisualRuleAction {
                    action_type: VisualActionType::ApplyDiscountCap,
                    parameters: discount_parameters,
                },
                VisualRuleAction {
                    action_type: VisualActionType::RouteApprovalRole,
                    parameters: approval_parameters,
                },
            ],
            metadata: VisualRuleMetadata {
                created_by: "salesops:1".to_string(),
                updated_by: "salesops:1".to_string(),
                tags: vec!["policy".to_string()],
                rationale: None,
            },
        }
    }

    #[test]
    fn builds_discount_policy_thresholds() {
        let rule = fixture();
        let policy = build_discount_policy(&rule).expect("should build");

        assert_eq!(policy.customer_segment.as_deref(), Some("smb"));
        assert_eq!(policy.min_deal_value, Some("5000".parse().expect("decimal from deal value")));
        assert_eq!(policy.max_discount_auto_approve_pct, "10.0".parse().expect("auto threshold"));
        assert_eq!(
            policy.max_discount_with_approval_pct,
            Some("20.0".parse().expect("approval threshold"))
        );
        assert_eq!(policy.required_approver_role.as_deref(), Some("sales_manager"));
    }

    #[test]
    fn requires_auto_approve_cap_parameter() {
        let mut rule = fixture();
        rule.actions.retain(|action| action.action_type != VisualActionType::ApplyDiscountCap);

        assert_eq!(
            build_discount_policy(&rule),
            Err(DiscountPolicyBuilderError::MissingParameter {
                key: "max_discount_pct".to_string(),
            })
        );
    }
}
