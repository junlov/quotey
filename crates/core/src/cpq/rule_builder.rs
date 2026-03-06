use std::collections::BTreeMap;

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricingRulePreviewInput {
    pub quote_id: String,
    pub context_fields: BTreeMap<String, String>,
    pub quantity: u32,
    pub unit_price: Decimal,
    /// Discount percentage in the [0, 100] range.
    pub discount_pct: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricingRulePreviewCase {
    pub quote_id: String,
    pub matched: bool,
    pub before_total: Decimal,
    pub after_total: Decimal,
    pub before_unit_price: Decimal,
    pub after_unit_price: Decimal,
    pub before_discount_pct: Decimal,
    pub after_discount_pct: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricingRulePreviewResult {
    pub rule_id: String,
    pub sql_preview: String,
    pub affected_quote_ids: Vec<String>,
    pub cases: Vec<PricingRulePreviewCase>,
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

/// Render a deterministic SQL preview statement for the pricing rule draft.
pub fn pricing_rule_sql_preview(rule: &PricingRuleDraft) -> String {
    let where_clause = render_where_clause(&rule.conditions);
    let action_sql = match &rule.action {
        PricingRuleAction::SetUnitPrice { amount, .. } => {
            format!("unit_price = {}", amount.normalize())
        }
        PricingRuleAction::ApplyDiscountCap { max_discount_pct } => {
            format!("discount_pct = MIN(discount_pct, {})", max_discount_pct.normalize())
        }
    };

    format!("UPDATE quote_line SET {} WHERE {};", action_sql, where_clause)
}

/// Preview pricing-rule impact against sample quote rows before persisting the rule.
pub fn preview_pricing_rule(
    rule: &PricingRuleDraft,
    samples: &[PricingRulePreviewInput],
) -> PricingRulePreviewResult {
    let sql_preview = pricing_rule_sql_preview(rule);
    let mut affected_quote_ids = Vec::new();
    let mut cases = Vec::with_capacity(samples.len());

    for sample in samples {
        let matched = sample_matches_rule(sample, &rule.conditions);
        let normalized_before_discount = clamp_discount_pct(sample.discount_pct);
        let (after_unit_price, after_discount_pct) = if matched {
            apply_action(&rule.action, sample)
        } else {
            (sample.unit_price, normalized_before_discount)
        };

        if matched {
            affected_quote_ids.push(sample.quote_id.clone());
        }

        cases.push(PricingRulePreviewCase {
            quote_id: sample.quote_id.clone(),
            matched,
            before_total: line_total(
                sample.unit_price,
                normalized_before_discount,
                sample.quantity,
            ),
            after_total: line_total(after_unit_price, after_discount_pct, sample.quantity),
            before_unit_price: sample.unit_price,
            after_unit_price,
            before_discount_pct: normalized_before_discount,
            after_discount_pct,
        });
    }

    PricingRulePreviewResult { rule_id: rule.id.clone(), sql_preview, affected_quote_ids, cases }
}

fn sample_matches_rule(
    sample: &PricingRulePreviewInput,
    conditions: &[PricingRuleCondition],
) -> bool {
    let mut iter = conditions.iter();
    let Some(first) = iter.next() else {
        return true;
    };

    let mut result = evaluate_condition(sample, first);
    for condition in iter {
        let current = evaluate_condition(sample, condition);
        match condition.connector.unwrap_or(LogicalConnector::And) {
            LogicalConnector::And => result = result && current,
            LogicalConnector::Or => result = result || current,
        }
    }

    result
}

fn evaluate_condition(sample: &PricingRulePreviewInput, condition: &PricingRuleCondition) -> bool {
    let candidate =
        sample.context_fields.get(&condition.field_key).map(String::as_str).unwrap_or_default();
    let expected = condition.value.trim();

    match condition.operator {
        PricingRuleOperator::Equals => candidate == expected,
        PricingRuleOperator::NotEquals => candidate != expected,
        PricingRuleOperator::Contains => {
            candidate.to_ascii_lowercase().contains(&expected.to_ascii_lowercase())
        }
        PricingRuleOperator::In => {
            split_csv_values(expected).iter().any(|entry| candidate.eq_ignore_ascii_case(entry))
        }
        PricingRuleOperator::NotIn => {
            split_csv_values(expected).iter().all(|entry| !candidate.eq_ignore_ascii_case(entry))
        }
        PricingRuleOperator::GreaterThan => {
            compare_decimal(candidate, expected, |lhs, rhs| lhs > rhs)
        }
        PricingRuleOperator::GreaterOrEqual => {
            compare_decimal(candidate, expected, |lhs, rhs| lhs >= rhs)
        }
        PricingRuleOperator::LessThan => compare_decimal(candidate, expected, |lhs, rhs| lhs < rhs),
        PricingRuleOperator::LessOrEqual => {
            compare_decimal(candidate, expected, |lhs, rhs| lhs <= rhs)
        }
    }
}

fn compare_decimal<F>(left: &str, right: &str, comparator: F) -> bool
where
    F: Fn(Decimal, Decimal) -> bool,
{
    let Ok(lhs) = Decimal::from_str_exact(left.trim()) else {
        return false;
    };
    let Ok(rhs) = Decimal::from_str_exact(right.trim()) else {
        return false;
    };
    comparator(lhs, rhs)
}

fn split_csv_values(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn apply_action(
    action: &PricingRuleAction,
    sample: &PricingRulePreviewInput,
) -> (Decimal, Decimal) {
    let current_discount = clamp_discount_pct(sample.discount_pct);
    match action {
        PricingRuleAction::SetUnitPrice { amount, .. } => (*amount, current_discount),
        PricingRuleAction::ApplyDiscountCap { max_discount_pct } => {
            (sample.unit_price, clamp_discount_pct(current_discount.min(*max_discount_pct)))
        }
    }
}

fn clamp_discount_pct(value: Decimal) -> Decimal {
    if value < Decimal::ZERO {
        Decimal::ZERO
    } else if value > Decimal::from(100u32) {
        Decimal::from(100u32)
    } else {
        value
    }
}

fn line_total(unit_price: Decimal, discount_pct: Decimal, quantity: u32) -> Decimal {
    let quantity_decimal = Decimal::from(u64::from(quantity));
    let hundred = Decimal::from(100u32);
    unit_price * quantity_decimal * (hundred - discount_pct) / hundred
}

fn render_where_clause(conditions: &[PricingRuleCondition]) -> String {
    let mut rendered = String::new();
    for (idx, condition) in conditions.iter().enumerate() {
        if idx > 0 {
            let connector = condition.connector.unwrap_or(LogicalConnector::And);
            let connector_sql = match connector {
                LogicalConnector::And => "AND",
                LogicalConnector::Or => "OR",
            };
            rendered.push(' ');
            rendered.push_str(connector_sql);
            rendered.push(' ');
        }
        rendered.push_str(&render_condition_sql(condition));
    }

    if rendered.is_empty() {
        "1=1".to_string()
    } else {
        rendered
    }
}

fn render_condition_sql(condition: &PricingRuleCondition) -> String {
    let field = condition.field_key.trim();
    let value = condition.value.trim();

    match condition.operator {
        PricingRuleOperator::Equals => {
            format!("{} = '{}'", field, escape_sql_literal(value))
        }
        PricingRuleOperator::NotEquals => {
            format!("{} != '{}'", field, escape_sql_literal(value))
        }
        PricingRuleOperator::GreaterThan => {
            format!("{} > {}", field, render_numeric_or_literal(value))
        }
        PricingRuleOperator::GreaterOrEqual => {
            format!("{} >= {}", field, render_numeric_or_literal(value))
        }
        PricingRuleOperator::LessThan => {
            format!("{} < {}", field, render_numeric_or_literal(value))
        }
        PricingRuleOperator::LessOrEqual => {
            format!("{} <= {}", field, render_numeric_or_literal(value))
        }
        PricingRuleOperator::Contains => {
            format!("{} LIKE '%{}%'", field, escape_sql_literal(value))
        }
        PricingRuleOperator::In | PricingRuleOperator::NotIn => {
            let entries = split_csv_values(value)
                .into_iter()
                .map(|entry| format!("'{}'", escape_sql_literal(&entry)))
                .collect::<Vec<_>>();
            let op = if condition.operator == PricingRuleOperator::In { "IN" } else { "NOT IN" };
            format!(
                "{} {} ({})",
                field,
                op,
                if entries.is_empty() { "''".to_string() } else { entries.join(", ") }
            )
        }
    }
}

fn render_numeric_or_literal(value: &str) -> String {
    if Decimal::from_str_exact(value).is_ok() {
        value.to_string()
    } else {
        format!("'{}'", escape_sql_literal(value))
    }
}

fn escape_sql_literal(value: &str) -> String {
    value.replace('\'', "''")
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

    use rust_decimal::Decimal;
    use serde_json::json;

    use super::{
        build_pricing_rule, preview_pricing_rule, pricing_rule_sql_preview, PricingRuleAction,
        PricingRuleBuilderError, PricingRulePreviewInput,
    };
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

    #[test]
    fn sql_preview_renders_update_statement() {
        let visual_rule = pricing_rule_fixture(VisualActionType::SetUnitPrice);
        let draft = build_pricing_rule(&visual_rule).expect("rule should translate");
        let sql = pricing_rule_sql_preview(&draft);

        assert!(sql.starts_with("UPDATE quote_line SET unit_price ="));
        assert!(sql.contains("customer_segment = 'enterprise'"));
        assert!(sql.ends_with(';'));
    }

    #[test]
    fn preview_highlights_affected_quotes_with_before_after_totals() {
        let visual_rule = pricing_rule_fixture(VisualActionType::SetUnitPrice);
        let draft = build_pricing_rule(&visual_rule).expect("rule should translate");

        let mut enterprise_fields = BTreeMap::new();
        enterprise_fields.insert("customer_segment".to_string(), "enterprise".to_string());

        let mut smb_fields = BTreeMap::new();
        smb_fields.insert("customer_segment".to_string(), "smb".to_string());

        let samples = vec![
            PricingRulePreviewInput {
                quote_id: "Q-enterprise".to_string(),
                context_fields: enterprise_fields,
                quantity: 2,
                unit_price: Decimal::new(10000, 2),
                discount_pct: Decimal::new(1000, 2),
            },
            PricingRulePreviewInput {
                quote_id: "Q-smb".to_string(),
                context_fields: smb_fields,
                quantity: 2,
                unit_price: Decimal::new(10000, 2),
                discount_pct: Decimal::new(1000, 2),
            },
        ];

        let result = preview_pricing_rule(&draft, &samples);
        assert_eq!(result.affected_quote_ids, vec!["Q-enterprise".to_string()]);

        let enterprise = result
            .cases
            .iter()
            .find(|case| case.quote_id == "Q-enterprise")
            .expect("enterprise case");
        assert!(enterprise.matched);
        assert_eq!(enterprise.before_total, Decimal::new(18000, 2));
        assert_eq!(enterprise.after_total, Decimal::new(17991, 2));

        let smb = result.cases.iter().find(|case| case.quote_id == "Q-smb").expect("smb case");
        assert!(!smb.matched);
        assert_eq!(smb.before_total, smb.after_total);
    }

    #[test]
    fn preview_applies_discount_cap_action() {
        let mut visual_rule = pricing_rule_fixture(VisualActionType::ApplyDiscountCap);
        visual_rule.actions[0].parameters.clear();
        visual_rule.actions[0].parameters.insert("max_discount_pct".to_string(), json!("15"));

        let draft = build_pricing_rule(&visual_rule).expect("rule should translate");
        let mut fields = BTreeMap::new();
        fields.insert("customer_segment".to_string(), "enterprise".to_string());

        let samples = vec![PricingRulePreviewInput {
            quote_id: "Q-1".to_string(),
            context_fields: fields,
            quantity: 1,
            unit_price: Decimal::new(10000, 2),
            discount_pct: Decimal::new(2500, 2),
        }];
        let result = preview_pricing_rule(&draft, &samples);
        let case = result.cases.first().expect("case");

        assert!(case.matched);
        assert_eq!(case.before_discount_pct, Decimal::new(2500, 2));
        assert_eq!(case.after_discount_pct, Decimal::new(1500, 2));
        assert_eq!(case.before_total, Decimal::new(7500, 2));
        assert_eq!(case.after_total, Decimal::new(8500, 2));
    }
}
