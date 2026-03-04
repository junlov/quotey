use serde::{Deserialize, Serialize};

use crate::{ExtractedRequirements, Product};

const MIN_MATCH_CONFIDENCE: f64 = 0.30;
const AMBIGUITY_DELTA: f64 = 0.12;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProductMatch {
    pub requirement_name: String,
    pub product_id: String,
    pub product_name: String,
    pub confidence: f64,
    pub reasoning: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchAmbiguity {
    pub requirement_name: String,
    pub candidates: Vec<ProductMatch>,
    pub question: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct ProductMatchResult {
    pub matches: Vec<ProductMatch>,
    pub ambiguities: Vec<MatchAmbiguity>,
    pub unmatched_requirements: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProductMatcher;

impl ProductMatcher {
    pub fn match_requirements(
        &self,
        extracted: &ExtractedRequirements,
        catalog: &[Product],
    ) -> ProductMatchResult {
        let mut result = ProductMatchResult::default();

        for requirement in &extracted.requirements {
            let candidate_matches = self.rank_requirement(&requirement.name, catalog);

            if candidate_matches.is_empty() {
                result.unmatched_requirements.push(requirement.name.clone());
                continue;
            }

            let top = &candidate_matches[0];
            if top.confidence < MIN_MATCH_CONFIDENCE {
                result.unmatched_requirements.push(requirement.name.clone());
                continue;
            }

            if candidate_matches.len() > 1 {
                let second = &candidate_matches[1];
                if (top.confidence - second.confidence).abs() <= AMBIGUITY_DELTA {
                    result.ambiguities.push(MatchAmbiguity {
                        requirement_name: requirement.name.clone(),
                        candidates: candidate_matches.into_iter().take(3).collect(),
                        question: format!("Which product should map to '{}'?", requirement.name),
                    });
                    continue;
                }
            }

            result.matches.push(top.clone());
        }

        result
    }

    fn rank_requirement(&self, requirement_name: &str, catalog: &[Product]) -> Vec<ProductMatch> {
        let query = normalize(requirement_name);
        if query.is_empty() {
            return Vec::new();
        }
        let query_tokens = tokenize(&query);

        let mut scored = Vec::new();
        for product in catalog.iter().filter(|product| product.active) {
            let product_name = normalize(&product.name);
            let sku = normalize(&product.sku);
            let description = normalize(product.description.as_deref().unwrap_or(""));

            let mut score = 0.0;
            let mut reasons = Vec::new();

            if product_name == query {
                score += 0.70;
                reasons.push("exact product name match");
            } else if product_name.contains(&query) || query.contains(&product_name) {
                score += 0.50;
                reasons.push("partial product name overlap");
            }

            if !sku.is_empty() && (sku == query || query.contains(&sku)) {
                score += 0.45;
                reasons.push("sku match");
            }

            if !description.is_empty() && description.contains(&query) {
                score += 0.30;
                reasons.push("description phrase match");
            }

            let token_overlap = token_overlap_ratio(&query_tokens, &tokenize(&product_name));
            if token_overlap > 0.0 {
                score += 0.35 * token_overlap;
                reasons.push("token overlap with product name");
            }

            let synonym_score = synonym_score(&query, &product_name, &description);
            if synonym_score > 0.0 {
                score += synonym_score;
                reasons.push("domain synonym match");
            }

            let confidence = score.clamp(0.0, 0.99);
            if confidence > 0.0 {
                scored.push(ProductMatch {
                    requirement_name: requirement_name.to_string(),
                    product_id: product.id.0.clone(),
                    product_name: product.name.clone(),
                    confidence,
                    reasoning: reasons.join("; "),
                });
            }
        }

        scored.sort_by(|left, right| {
            right.confidence.partial_cmp(&left.confidence).unwrap_or(std::cmp::Ordering::Equal)
        });
        scored
    }
}

fn normalize(input: &str) -> String {
    input
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c.is_ascii_whitespace() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn tokenize(input: &str) -> Vec<String> {
    input
        .split_whitespace()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn token_overlap_ratio(left: &[String], right: &[String]) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }

    let right_set: std::collections::HashSet<&str> =
        right.iter().map(|token| token.as_str()).collect();
    let overlap = left.iter().filter(|token| right_set.contains(token.as_str())).count();
    overlap as f64 / left.len().max(right.len()) as f64
}

fn synonym_score(query: &str, name: &str, description: &str) -> f64 {
    let synonym_map = [
        ("sso", ["single sign on", "sso", "identity"]),
        ("onboarding", ["onboarding", "implementation", "launch"]),
        ("support", ["support", "24 7", "priority support"]),
        ("compliance", ["soc2", "compliance", "security"]),
    ];

    for (_, aliases) in synonym_map {
        let query_hit = aliases.iter().any(|alias| query.contains(alias));
        if !query_hit {
            continue;
        }
        if aliases.iter().any(|alias| name.contains(alias) || description.contains(alias)) {
            return 0.22;
        }
    }

    0.0
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;

    use super::ProductMatcher;
    use crate::domain::product::ProductType;
    use crate::{
        ExtractedRequirement, ExtractedRequirements, Product, ProductId, RequirementSourceType,
        REQUIREMENT_EXTRACTION_SCHEMA_VERSION,
    };

    fn catalog() -> Vec<Product> {
        let now = Utc::now();
        vec![
            Product {
                id: ProductId("prod-sso".to_string()),
                sku: "ADDON-SSO".to_string(),
                name: "SSO Add-on".to_string(),
                description: Some("Single sign on identity integration".to_string()),
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
                description: Some("24/7 support coverage".to_string()),
                product_type: ProductType::Simple,
                family_id: None,
                base_price: Some(Decimal::new(12900, 2)),
                currency: "USD".to_string(),
                attributes: vec![],
                active: true,
                created_at: now,
                updated_at: now,
            },
            Product {
                id: ProductId("prod-enterprise".to_string()),
                sku: "PLAN-ENT".to_string(),
                name: "Enterprise Plan".to_string(),
                description: Some("Enterprise licensing tier".to_string()),
                product_type: ProductType::Simple,
                family_id: None,
                base_price: Some(Decimal::new(49900, 2)),
                currency: "USD".to_string(),
                attributes: vec![],
                active: true,
                created_at: now,
                updated_at: now,
            },
        ]
    }

    fn extracted(names: &[&str]) -> ExtractedRequirements {
        ExtractedRequirements {
            schema_version: REQUIREMENT_EXTRACTION_SCHEMA_VERSION.to_string(),
            source_type: RequirementSourceType::Email,
            sender_hint: None,
            context_hint: None,
            requirements: names
                .iter()
                .map(|name| ExtractedRequirement {
                    requirement_type: "product".to_string(),
                    name: (*name).to_string(),
                    quantity: None,
                    confidence: 0.9,
                    raw_excerpt: None,
                })
                .collect(),
            ambiguities: vec![],
            missing_info: vec![],
        }
    }

    #[test]
    fn matches_known_requirement_to_product() {
        let matcher = ProductMatcher;
        let result = matcher.match_requirements(&extracted(&["sso integration"]), &catalog());

        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].product_id, "prod-sso");
        assert!(result.ambiguities.is_empty());
    }

    #[test]
    fn marks_unmatched_requirement_when_confidence_too_low() {
        let matcher = ProductMatcher;
        let result =
            matcher.match_requirements(&extracted(&["quantum blockchain module"]), &catalog());

        assert!(result.matches.is_empty());
        assert_eq!(result.unmatched_requirements, vec!["quantum blockchain module"]);
    }

    #[test]
    fn emits_ambiguity_when_candidates_are_close() {
        let matcher = ProductMatcher;
        let result = matcher.match_requirements(&extracted(&["support add-on"]), &catalog());

        assert_eq!(result.ambiguities.len(), 1);
        assert!(result.matches.is_empty());
        assert!(result.ambiguities[0].candidates.len() >= 2);
    }
}
