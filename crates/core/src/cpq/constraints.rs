use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::domain::quote::QuoteLine;
use rust_decimal::Decimal;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintViolation {
    pub code: String,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConstraintInput {
    pub quote_lines: Vec<QuoteLine>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintResult {
    pub valid: bool,
    pub violations: Vec<ConstraintViolation>,
}

impl Default for ConstraintResult {
    fn default() -> Self {
        Self { valid: true, violations: Vec::new() }
    }
}

pub trait ConstraintEngine: Send + Sync {
    fn validate(&self, input: &ConstraintInput) -> ConstraintResult;
}

#[derive(Default)]
pub struct DeterministicConstraintEngine;

impl ConstraintEngine for DeterministicConstraintEngine {
    fn validate(&self, input: &ConstraintInput) -> ConstraintResult {
        validate_configuration_input(input)
    }
}

pub fn validate_configuration() -> ConstraintResult {
    ConstraintResult::default()
}

pub fn validate_configuration_input(input: &ConstraintInput) -> ConstraintResult {
    if input.quote_lines.is_empty() {
        return ConstraintResult {
            valid: false,
            violations: vec![ConstraintViolation {
                code: "EMPTY_QUOTE".to_string(),
                message: "Quote must contain at least one line item".to_string(),
                suggestion: Some("Add at least one product line to continue".to_string()),
            }],
        };
    }

    let mut result = ConstraintResult::default();
    let mut seen_product_ids: HashSet<String> = HashSet::new();

    for line in &input.quote_lines {
        let trimmed_product_id = line.product_id.0.trim().to_owned();
        if trimmed_product_id.is_empty() {
            result.violations.push(ConstraintViolation {
                code: "MISSING_PRODUCT_ID".to_string(),
                message: "Quote line is missing product id".to_string(),
                suggestion: Some("Choose a valid product id".to_string()),
            });
            continue;
        }

        if !seen_product_ids.insert(trimmed_product_id.clone()) {
            result.violations.push(ConstraintViolation {
                code: "DUPLICATE_PRODUCT_ID".to_string(),
                message: format!("Duplicate product id in quote: {trimmed_product_id}"),
                suggestion: Some(
                    "Consolidate duplicate lines or split by option family".to_string(),
                ),
            });
        }

        if line.quantity == 0 {
            result.violations.push(ConstraintViolation {
                code: "ZERO_QUANTITY".to_string(),
                message: format!("Product {trimmed_product_id} has zero quantity"),
                suggestion: Some("Use a positive integer quantity".to_string()),
            });
        }

        if line.unit_price <= Decimal::ZERO {
            result.violations.push(ConstraintViolation {
                code: "NON_POSITIVE_UNIT_PRICE".to_string(),
                message: format!("Product {trimmed_product_id} has non-positive unit price"),
                suggestion: Some("Use a positive unit price with fixed decimals".to_string()),
            });
        }
    }

    if !result.violations.is_empty() {
        result.valid = false;
    }

    result
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::{validate_configuration_input, ConstraintInput};
    use crate::domain::{product::ProductId, quote::QuoteLine};

    #[test]
    fn detects_duplicate_zero_quantity_and_bad_price_violations() {
        let input = ConstraintInput {
            quote_lines: vec![
                QuoteLine {
                    product_id: ProductId("plan-pro".to_owned()),
                    quantity: 0,
                    unit_price: Decimal::ZERO,
                },
                QuoteLine {
                    product_id: ProductId("plan-pro".to_owned()),
                    quantity: 1,
                    unit_price: Decimal::NEGATIVE_ONE,
                },
                QuoteLine {
                    product_id: ProductId(" ".to_owned()),
                    quantity: 1,
                    unit_price: Decimal::new(1000, 2),
                },
            ],
        };

        let result = validate_configuration_input(&input);
        assert!(!result.valid);
        assert_eq!(result.violations.len(), 5);
        assert!(result.violations.iter().any(|v| v.code == "ZERO_QUANTITY"));
        assert!(result.violations.iter().any(|v| v.code == "NON_POSITIVE_UNIT_PRICE"));
        assert!(result.violations.iter().any(|v| v.code == "DUPLICATE_PRODUCT_ID"));
        assert!(result.violations.iter().any(|v| v.code == "MISSING_PRODUCT_ID"));
    }
}
