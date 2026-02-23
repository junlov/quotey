use serde::{Deserialize, Serialize};

use crate::domain::quote::QuoteLine;

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

    validate_configuration()
}
