use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ExtractedRequirements, Product, ProductMatchResult, Quote, QuoteId, QuoteLine, QuoteStatus,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftQuoteBuildRequest {
    pub quote_id: String,
    pub created_by: String,
    pub account_id: Option<String>,
    pub deal_id: Option<String>,
    pub currency: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct DraftQuoteBuildResult {
    pub quote: Option<Quote>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DraftQuoteBuilder;

impl DraftQuoteBuilder {
    pub fn build_from_matches(
        &self,
        request: &DraftQuoteBuildRequest,
        extracted: &ExtractedRequirements,
        matches: &ProductMatchResult,
        catalog: &[Product],
    ) -> Result<DraftQuoteBuildResult, DraftQuoteBuildError> {
        if request.quote_id.trim().is_empty() {
            return Err(DraftQuoteBuildError::MissingQuoteId);
        }
        if request.created_by.trim().is_empty() {
            return Err(DraftQuoteBuildError::MissingActor);
        }

        let mut lines = Vec::new();
        let mut warnings = Vec::new();

        for matched in &matches.matches {
            let product = catalog
                .iter()
                .find(|product| product.id.0 == matched.product_id)
                .ok_or_else(|| DraftQuoteBuildError::MissingCatalogProduct {
                    product_id: matched.product_id.clone(),
                })?;

            let quantity = extracted
                .requirements
                .iter()
                .find(|requirement| requirement.name == matched.requirement_name)
                .and_then(|requirement| requirement.quantity)
                .unwrap_or(1);

            let unit_price = product.base_price.unwrap_or(Decimal::ZERO);
            if unit_price == Decimal::ZERO {
                warnings.push(format!(
                    "Product '{}' has no base price; draft line priced at 0 for review",
                    product.name
                ));
            }

            lines.push(QuoteLine {
                product_id: product.id.clone(),
                quantity,
                unit_price,
                discount_pct: 0.0,
                notes: Some(format!(
                    "Auto-matched from requirement '{}' ({:.2} confidence)",
                    matched.requirement_name, matched.confidence
                )),
            });
        }

        warnings.extend(
            matches
                .ambiguities
                .iter()
                .map(|entry| format!("Clarification required for '{}'", entry.requirement_name)),
        );
        warnings.extend(
            matches
                .unmatched_requirements
                .iter()
                .map(|entry| format!("Unmatched requirement '{}'", entry)),
        );

        if lines.is_empty() {
            warnings.push("No matched products available to build draft quote".to_string());
            return Ok(DraftQuoteBuildResult { quote: None, warnings });
        }

        let now = Utc::now();
        let quote = Quote {
            id: QuoteId(request.quote_id.clone()),
            version: 1,
            status: QuoteStatus::Draft,
            account_id: request.account_id.clone(),
            deal_id: request.deal_id.clone(),
            currency: request.currency.clone(),
            term_months: None,
            start_date: None,
            end_date: None,
            valid_until: None,
            notes: Some(
                "Auto-generated draft from extracted requirements; review required".to_string(),
            ),
            created_by: request.created_by.clone(),
            lines,
            created_at: now,
            updated_at: now,
        };

        Ok(DraftQuoteBuildResult { quote: Some(quote), warnings })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum DraftQuoteBuildError {
    #[error("quote_id is required")]
    MissingQuoteId,
    #[error("created_by is required")]
    MissingActor,
    #[error("matched product `{product_id}` not found in catalog")]
    MissingCatalogProduct { product_id: String },
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;

    use super::{DraftQuoteBuildRequest, DraftQuoteBuilder};
    use crate::domain::product::ProductType;
    use crate::{
        ExtractedRequirement, ExtractedRequirements, MatchAmbiguity, Product, ProductId,
        ProductMatch, ProductMatchResult, RequirementSourceType,
        REQUIREMENT_EXTRACTION_SCHEMA_VERSION,
    };

    fn request() -> DraftQuoteBuildRequest {
        DraftQuoteBuildRequest {
            quote_id: "Q-2026-AUTO-001".to_string(),
            created_by: "agent:autoquote".to_string(),
            account_id: Some("acct-1".to_string()),
            deal_id: Some("deal-1".to_string()),
            currency: "USD".to_string(),
        }
    }

    fn extracted() -> ExtractedRequirements {
        ExtractedRequirements {
            schema_version: REQUIREMENT_EXTRACTION_SCHEMA_VERSION.to_string(),
            source_type: RequirementSourceType::Email,
            sender_hint: Some("rep@customer.example".to_string()),
            context_hint: Some("Expansion quote for new identity requirements".to_string()),
            requirements: vec![
                ExtractedRequirement {
                    requirement_type: "product".to_string(),
                    name: "SSO integration".to_string(),
                    quantity: Some(150),
                    confidence: 0.9,
                    raw_excerpt: None,
                },
                ExtractedRequirement {
                    requirement_type: "product".to_string(),
                    name: "Priority support".to_string(),
                    quantity: None,
                    confidence: 0.8,
                    raw_excerpt: None,
                },
            ],
            ambiguities: vec![],
            missing_info: vec![],
        }
    }

    fn catalog() -> Vec<Product> {
        let now = Utc::now();
        vec![
            Product {
                id: ProductId("prod-sso".to_string()),
                sku: "ADDON-SSO".to_string(),
                name: "SSO Add-on".to_string(),
                description: None,
                product_type: ProductType::Simple,
                family_id: None,
                base_price: Some(Decimal::new(9900, 2)),
                currency: "USD".to_string(),
                attributes: vec![],
                active: true,
                created_at: now,
                updated_at: now,
            },
            Product {
                id: ProductId("prod-support".to_string()),
                sku: "ADDON-SUPPORT".to_string(),
                name: "Premium Support".to_string(),
                description: None,
                product_type: ProductType::Simple,
                family_id: None,
                base_price: Some(Decimal::new(12900, 2)),
                currency: "USD".to_string(),
                attributes: vec![],
                active: true,
                created_at: now,
                updated_at: now,
            },
        ]
    }

    #[test]
    fn builds_draft_quote_from_matches() {
        let builder = DraftQuoteBuilder;
        let matches = ProductMatchResult {
            matches: vec![
                ProductMatch {
                    requirement_name: "SSO integration".to_string(),
                    product_id: "prod-sso".to_string(),
                    product_name: "SSO Add-on".to_string(),
                    confidence: 0.95,
                    reasoning: "synonym".to_string(),
                },
                ProductMatch {
                    requirement_name: "Priority support".to_string(),
                    product_id: "prod-support".to_string(),
                    product_name: "Premium Support".to_string(),
                    confidence: 0.77,
                    reasoning: "token overlap".to_string(),
                },
            ],
            ambiguities: vec![],
            unmatched_requirements: vec![],
        };

        let result = builder
            .build_from_matches(&request(), &extracted(), &matches, &catalog())
            .expect("build should succeed");
        let quote = result.quote.expect("quote should exist");
        assert_eq!(quote.lines.len(), 2);
        assert_eq!(quote.lines[0].quantity, 150);
        assert_eq!(quote.lines[1].quantity, 1);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn returns_warnings_for_ambiguities_and_unmatched() {
        let builder = DraftQuoteBuilder;
        let matches = ProductMatchResult {
            matches: vec![],
            ambiguities: vec![MatchAmbiguity {
                requirement_name: "enterprise tier".to_string(),
                candidates: vec![],
                question: "Which tier?".to_string(),
            }],
            unmatched_requirements: vec!["custom legal clause".to_string()],
        };

        let result = builder
            .build_from_matches(&request(), &extracted(), &matches, &catalog())
            .expect("build should succeed");

        assert!(result.quote.is_none());
        assert!(result.warnings.iter().any(|warning| warning.contains("No matched products")));
        assert!(result.warnings.iter().any(|warning| warning.contains("Clarification required")));
        assert!(result.warnings.iter().any(|warning| warning.contains("Unmatched requirement")));
    }
}
