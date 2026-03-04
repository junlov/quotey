use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const REQUIREMENT_EXTRACTION_SCHEMA_VERSION: &str = "requirement_extraction.v1";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExtractedRequirements {
    pub schema_version: String,
    pub source_type: RequirementSourceType,
    #[serde(default)]
    pub sender_hint: Option<String>,
    #[serde(default)]
    pub context_hint: Option<String>,
    #[serde(default)]
    pub requirements: Vec<ExtractedRequirement>,
    #[serde(default)]
    pub ambiguities: Vec<RequirementAmbiguity>,
    #[serde(default)]
    pub missing_info: Vec<String>,
}

impl ExtractedRequirements {
    pub fn validate(&self) -> Result<(), RequirementExtractionValidationError> {
        if self.schema_version != REQUIREMENT_EXTRACTION_SCHEMA_VERSION {
            return Err(RequirementExtractionValidationError::UnsupportedSchemaVersion {
                expected: REQUIREMENT_EXTRACTION_SCHEMA_VERSION.to_string(),
                actual: self.schema_version.clone(),
            });
        }

        if self.sender_hint.as_ref().is_some_and(|value| value.trim().is_empty()) {
            return Err(RequirementExtractionValidationError::EmptySenderHint);
        }

        if self.context_hint.as_ref().is_some_and(|value| value.trim().is_empty()) {
            return Err(RequirementExtractionValidationError::EmptyContextHint);
        }

        for requirement in &self.requirements {
            if requirement.requirement_type.trim().is_empty() {
                return Err(RequirementExtractionValidationError::MissingRequirementType);
            }
            if requirement.name.trim().is_empty() {
                return Err(RequirementExtractionValidationError::MissingRequirementName);
            }
            if let Some(quantity) = requirement.quantity {
                if quantity == 0 {
                    return Err(RequirementExtractionValidationError::InvalidQuantity { quantity });
                }
            }
            if !(0.0..=1.0).contains(&requirement.confidence) {
                return Err(RequirementExtractionValidationError::ConfidenceOutOfRange {
                    value: requirement.confidence,
                });
            }
        }

        for ambiguity in &self.ambiguities {
            if ambiguity.text.trim().is_empty() {
                return Err(RequirementExtractionValidationError::MissingAmbiguityText);
            }
            if ambiguity.question.trim().is_empty() {
                return Err(RequirementExtractionValidationError::MissingAmbiguityQuestion);
            }
            if ambiguity.options.len() < 2 {
                return Err(RequirementExtractionValidationError::AmbiguityNeedsMultipleOptions);
            }
            if !(0.0..=1.0).contains(&ambiguity.confidence) {
                return Err(RequirementExtractionValidationError::ConfidenceOutOfRange {
                    value: ambiguity.confidence,
                });
            }
        }

        if self.missing_info.iter().any(|value| value.trim().is_empty()) {
            return Err(RequirementExtractionValidationError::EmptyMissingInfoEntry);
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementSourceType {
    Email,
    Rfp,
    SlackThread,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExtractedRequirement {
    pub requirement_type: String,
    pub name: String,
    pub quantity: Option<u32>,
    pub confidence: f64,
    pub raw_excerpt: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RequirementAmbiguity {
    pub text: String,
    pub question: String,
    pub options: Vec<String>,
    pub confidence: f64,
}

#[derive(Clone, Debug, PartialEq, Error)]
pub enum RequirementExtractionValidationError {
    #[error("unsupported schema version: expected `{expected}`, got `{actual}`")]
    UnsupportedSchemaVersion { expected: String, actual: String },
    #[error("sender_hint cannot be empty when provided")]
    EmptySenderHint,
    #[error("context_hint cannot be empty when provided")]
    EmptyContextHint,
    #[error("requirement_type cannot be empty")]
    MissingRequirementType,
    #[error("requirement name cannot be empty")]
    MissingRequirementName,
    #[error("requirement quantity must be positive, got {quantity}")]
    InvalidQuantity { quantity: u32 },
    #[error("confidence must be between 0.0 and 1.0, got {value}")]
    ConfidenceOutOfRange { value: f64 },
    #[error("ambiguity text cannot be empty")]
    MissingAmbiguityText,
    #[error("ambiguity question cannot be empty")]
    MissingAmbiguityQuestion,
    #[error("ambiguity must include at least two options")]
    AmbiguityNeedsMultipleOptions,
    #[error("missing_info contains an empty entry")]
    EmptyMissingInfoEntry,
}

#[cfg(test)]
mod tests {
    use super::{
        ExtractedRequirement, ExtractedRequirements, RequirementAmbiguity,
        RequirementExtractionValidationError, RequirementSourceType,
        REQUIREMENT_EXTRACTION_SCHEMA_VERSION,
    };

    fn valid_payload() -> ExtractedRequirements {
        ExtractedRequirements {
            schema_version: REQUIREMENT_EXTRACTION_SCHEMA_VERSION.to_string(),
            source_type: RequirementSourceType::Rfp,
            sender_hint: Some("Acme procurement".to_string()),
            context_hint: Some("RFP renewal for security/compliance stack".to_string()),
            requirements: vec![ExtractedRequirement {
                requirement_type: "product".to_string(),
                name: "Enterprise Plan".to_string(),
                quantity: Some(150),
                confidence: 0.95,
                raw_excerpt: Some("Need enterprise plan for 150 users".to_string()),
            }],
            ambiguities: vec![RequirementAmbiguity {
                text: "enterprise tier".to_string(),
                question: "Did they mean Pro or Enterprise?".to_string(),
                options: vec!["Pro Plan".to_string(), "Enterprise Plan".to_string()],
                confidence: 0.62,
            }],
            missing_info: vec!["start date".to_string()],
        }
    }

    #[test]
    fn validate_accepts_well_formed_payload() {
        let payload = valid_payload();
        assert_eq!(payload.validate(), Ok(()));
    }

    #[test]
    fn validate_rejects_unknown_schema_version() {
        let mut payload = valid_payload();
        payload.schema_version = "requirement_extraction.v0".to_string();

        assert_eq!(
            payload.validate(),
            Err(RequirementExtractionValidationError::UnsupportedSchemaVersion {
                expected: REQUIREMENT_EXTRACTION_SCHEMA_VERSION.to_string(),
                actual: "requirement_extraction.v0".to_string(),
            })
        );
    }

    #[test]
    fn validate_rejects_out_of_range_confidence() {
        let mut payload = valid_payload();
        payload.requirements[0].confidence = 1.2;

        assert_eq!(
            payload.validate(),
            Err(RequirementExtractionValidationError::ConfidenceOutOfRange { value: 1.2 })
        );
    }

    #[test]
    fn validate_rejects_ambiguity_with_single_option() {
        let mut payload = valid_payload();
        payload.ambiguities[0].options = vec!["Enterprise Plan".to_string()];

        assert_eq!(
            payload.validate(),
            Err(RequirementExtractionValidationError::AmbiguityNeedsMultipleOptions)
        );
    }

    #[test]
    fn validate_rejects_empty_sender_hint() {
        let mut payload = valid_payload();
        payload.sender_hint = Some("   ".to_string());

        assert_eq!(payload.validate(), Err(RequirementExtractionValidationError::EmptySenderHint));
    }

    #[test]
    fn validate_rejects_empty_context_hint() {
        let mut payload = valid_payload();
        payload.context_hint = Some("".to_string());

        assert_eq!(payload.validate(), Err(RequirementExtractionValidationError::EmptyContextHint));
    }
}
