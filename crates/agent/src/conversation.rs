use std::collections::BTreeSet;

use quotey_core::cpq::constraints::{
    ConstraintEngine, ConstraintInput, ConstraintResult, DeterministicConstraintEngine,
};
use quotey_core::domain::product::ProductId;
use quotey_core::domain::quote::QuoteLine;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractedIntent {
    pub product_mentions: Vec<String>,
    pub quantity_mentions: Vec<u32>,
    pub budget_cents: Option<i64>,
    pub timeline_hint: Option<String>,
    pub requested_discount_pct: Option<u8>,
    pub constraints: Vec<String>,
    pub confidence_score: u8,
    pub clarification_prompt: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct IntentExtractor;

impl IntentExtractor {
    pub fn new() -> Self {
        Self
    }

    pub fn extract(&self, text: &str) -> ExtractedIntent {
        let normalized_text = normalize_text(text);
        let tokens = tokenize(&normalized_text);

        let product_mentions = extract_products(&normalized_text);
        let quantity_mentions = extract_quantities(&tokens);
        let budget_cents = extract_budget_cents(&tokens);
        let timeline_hint = extract_timeline_hint(&normalized_text);
        let requested_discount_pct = extract_discount_pct(&tokens);
        let constraints = extract_constraints(
            &normalized_text,
            budget_cents.is_some(),
            requested_discount_pct.is_some(),
        );

        let confidence_score = confidence_score(
            !product_mentions.is_empty(),
            !quantity_mentions.is_empty(),
            budget_cents.is_some(),
            timeline_hint.is_some(),
            !constraints.is_empty(),
            requested_discount_pct.is_some(),
        );

        let clarification_prompt = if product_mentions.is_empty()
            && quantity_mentions.is_empty()
            && budget_cents.is_none()
        {
            Some(
                "I need at least one of product, quantity, or budget details to proceed."
                    .to_string(),
            )
        } else if confidence_score < 40 {
            Some("Please add more specifics (product, quantity, budget, or timeline).".to_string())
        } else {
            None
        };

        ExtractedIntent {
            product_mentions,
            quantity_mentions,
            budget_cents,
            timeline_hint,
            requested_discount_pct,
            constraints,
            confidence_score,
            clarification_prompt,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogItem {
    pub product_id: ProductId,
    pub display_name: String,
    pub aliases: Vec<String>,
    pub unit_price_cents: i64,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConstraintSet {
    pub constraint_input: ConstraintInput,
    pub matched_product_ids: Vec<ProductId>,
    pub unresolved_product_mentions: Vec<String>,
    pub estimated_total_cents: i64,
    pub budget_cents: Option<i64>,
    pub requested_discount_pct: Option<u8>,
    pub constraints: Vec<String>,
    pub validation: ConstraintResult,
    pub clarification_prompt: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ConstraintMapper<E = DeterministicConstraintEngine> {
    catalog: Vec<CatalogItem>,
    constraint_engine: E,
}

impl ConstraintMapper<DeterministicConstraintEngine> {
    pub fn with_catalog(catalog: Vec<CatalogItem>) -> Self {
        Self::new(catalog, DeterministicConstraintEngine)
    }
}

impl<E> ConstraintMapper<E>
where
    E: ConstraintEngine,
{
    pub fn new(catalog: Vec<CatalogItem>, constraint_engine: E) -> Self {
        Self { catalog, constraint_engine }
    }

    pub fn map_intent(&self, intent: &ExtractedIntent) -> ConstraintSet {
        let requested_quantity = intent.quantity_mentions.first().copied().unwrap_or(1);
        let mut unresolved_product_mentions = Vec::new();
        let mut mapped_lines = Vec::new();
        let mut seen_product_ids = BTreeSet::new();

        for product_mention in &intent.product_mentions {
            if let Some(item) = self.match_catalog_item(product_mention) {
                if seen_product_ids.insert(item.product_id.0.clone()) {
                    mapped_lines.push(MappedLine {
                        catalog_item: item.clone(),
                        quantity: requested_quantity,
                    });
                }
            } else {
                unresolved_product_mentions.push(product_mention.clone());
            }
        }

        let mut clarification_prompt =
            self.clarification_for_unresolved_products(&unresolved_product_mentions);
        if mapped_lines.is_empty() && clarification_prompt.is_none() {
            clarification_prompt = Some(
                "Please specify at least one known product to continue (starter, premium, enterprise)."
                    .to_string(),
            );
        }

        let budget_fit = enforce_budget(&mut mapped_lines, intent.budget_cents);
        if !budget_fit {
            clarification_prompt = Some(
                "Budget cap cannot satisfy the requested configuration. Increase budget, reduce quantity, or choose a lower tier."
                    .to_string(),
            );
        }

        let quote_lines = mapped_lines.iter().map(MappedLine::as_quote_line).collect::<Vec<_>>();
        let constraint_input = ConstraintInput { quote_lines };
        let validation = self.constraint_engine.validate(&constraint_input);

        if !validation.valid && clarification_prompt.is_none() {
            clarification_prompt = validation.violations.first().and_then(|violation| {
                violation.suggestion.clone().or_else(|| Some(violation.message.clone()))
            });
        }

        let estimated_total_cents = mapped_lines.iter().map(MappedLine::line_total_cents).sum();
        let matched_product_ids = mapped_lines
            .iter()
            .map(|line| line.catalog_item.product_id.clone())
            .collect::<Vec<_>>();

        ConstraintSet {
            constraint_input,
            matched_product_ids,
            unresolved_product_mentions,
            estimated_total_cents,
            budget_cents: intent.budget_cents,
            requested_discount_pct: intent.requested_discount_pct,
            constraints: intent.constraints.clone(),
            validation,
            clarification_prompt,
        }
    }

    fn match_catalog_item(&self, mention: &str) -> Option<&CatalogItem> {
        let normalized_mention = normalize_text(mention);
        self.catalog.iter().find(|item| {
            normalize_text(&item.product_id.0) == normalized_mention
                || normalize_text(&item.display_name).contains(&normalized_mention)
                || item.aliases.iter().any(|alias| normalize_text(alias) == normalized_mention)
        })
    }

    fn clarification_for_unresolved_products(
        &self,
        unresolved_mentions: &[String],
    ) -> Option<String> {
        if unresolved_mentions.is_empty() {
            return None;
        }

        let available_products =
            self.catalog.iter().map(|item| item.display_name.clone()).collect::<Vec<_>>();

        Some(format!(
            "I couldn't map these product mentions: {}. Available products: {}.",
            unresolved_mentions.join(", "),
            available_products.join(", ")
        ))
    }
}

#[derive(Clone, Debug)]
struct MappedLine {
    catalog_item: CatalogItem,
    quantity: u32,
}

impl MappedLine {
    fn line_total_cents(&self) -> i64 {
        self.catalog_item.unit_price_cents.saturating_mul(i64::from(self.quantity))
    }

    fn as_quote_line(&self) -> QuoteLine {
        let unit_price =
            cents_to_decimal_string(self.catalog_item.unit_price_cents).parse().unwrap_or_default();

        QuoteLine {
            product_id: self.catalog_item.product_id.clone(),
            quantity: self.quantity,
            unit_price,
            discount_pct: 0.0,
            notes: None,
        }
    }
}

fn enforce_budget(mapped_lines: &mut Vec<MappedLine>, budget_cents: Option<i64>) -> bool {
    let Some(budget_limit_cents) = budget_cents else {
        return true;
    };
    if mapped_lines.is_empty() {
        return false;
    }

    if total_cents(mapped_lines) <= budget_limit_cents {
        return true;
    }

    let mut optional_indexes = mapped_lines
        .iter()
        .enumerate()
        .filter(|(_, line)| !line.catalog_item.required)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    optional_indexes.sort_by_key(|index| mapped_lines[*index].line_total_cents());
    optional_indexes.reverse();

    for index in optional_indexes {
        if total_cents(mapped_lines) <= budget_limit_cents {
            break;
        }
        mapped_lines.remove(index);
        if mapped_lines.is_empty() {
            return false;
        }
    }

    if total_cents(mapped_lines) <= budget_limit_cents {
        return true;
    }

    let target_line_index = mapped_lines
        .iter()
        .enumerate()
        .find(|(_, line)| line.catalog_item.required)
        .map(|(index, _)| index)
        .unwrap_or(0);

    let unit_price_cents = mapped_lines[target_line_index].catalog_item.unit_price_cents;
    if unit_price_cents <= 0 {
        return false;
    }

    let max_affordable_quantity = budget_limit_cents / unit_price_cents;
    if max_affordable_quantity <= 0 {
        return false;
    }

    mapped_lines[target_line_index].quantity =
        mapped_lines[target_line_index].quantity.min(max_affordable_quantity as u32);

    total_cents(mapped_lines) <= budget_limit_cents
}

fn total_cents(mapped_lines: &[MappedLine]) -> i64 {
    mapped_lines.iter().map(MappedLine::line_total_cents).sum()
}

fn cents_to_decimal_string(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let absolute = cents.unsigned_abs();
    format!("{sign}{}.{:02}", absolute / 100, absolute % 100)
}

fn normalize_text(text: &str) -> String {
    text.to_ascii_lowercase()
}

fn tokenize(text: &str) -> Vec<String> {
    let mut sanitized = String::with_capacity(text.len());
    for character in text.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '$' | '%' | '.' | 'k' | 'm') {
            sanitized.push(character);
        } else {
            sanitized.push(' ');
        }
    }
    sanitized.split_whitespace().map(|token| token.to_string()).collect()
}

fn extract_products(normalized_text: &str) -> Vec<String> {
    let mut products = BTreeSet::new();

    if normalized_text.contains("enterprise") {
        products.insert("enterprise".to_string());
    }
    if normalized_text.contains("premium") {
        products.insert("premium".to_string());
    }
    if normalized_text.contains("starter") || normalized_text.contains("basic") {
        products.insert("starter".to_string());
    }
    if normalized_text.contains("support") {
        products.insert("support".to_string());
    }
    if normalized_text.contains("api") {
        products.insert("api_access".to_string());
    }
    if normalized_text.contains("analytics") {
        products.insert("analytics".to_string());
    }
    if normalized_text.contains("security") || normalized_text.contains("compliance") {
        products.insert("security".to_string());
    }

    products.into_iter().collect()
}

fn extract_quantities(tokens: &[String]) -> Vec<u32> {
    let mut quantities = Vec::new();
    for window in tokens.windows(2) {
        if let [value, unit] = window {
            if !is_quantity_unit(unit) {
                continue;
            }
            if let Ok(quantity) = value.parse::<u32>() {
                quantities.push(quantity);
            }
        }
    }
    quantities
}

fn is_quantity_unit(token: &str) -> bool {
    matches!(
        token,
        "seat"
            | "seats"
            | "user"
            | "users"
            | "license"
            | "licenses"
            | "employee"
            | "employees"
            | "dept"
            | "departments"
            | "team"
            | "teams"
    )
}

fn extract_budget_cents(tokens: &[String]) -> Option<i64> {
    let budget_context = ["budget", "spend", "cap", "under", "below", "max"];
    for (index, token) in tokens.iter().enumerate() {
        let in_context = index > 0 && budget_context.contains(&tokens[index - 1].as_str());
        if token.starts_with('$') || in_context {
            if let Some(cents) = parse_money_token(token) {
                return Some(cents);
            }
        }
    }
    None
}

fn parse_money_token(token: &str) -> Option<i64> {
    let trimmed = token.trim_start_matches('$').trim_end_matches(',');
    if trimmed.is_empty() {
        return None;
    }

    let (number_part, multiplier) = if let Some(prefix) = trimmed.strip_suffix('k') {
        (prefix, 1_000.0)
    } else if let Some(prefix) = trimmed.strip_suffix('m') {
        (prefix, 1_000_000.0)
    } else {
        (trimmed, 1.0)
    };

    let amount = number_part.parse::<f64>().ok()?;
    let dollars = amount * multiplier;
    Some((dollars * 100.0).round() as i64)
}

fn extract_timeline_hint(normalized_text: &str) -> Option<String> {
    let timeline_patterns = [
        "this quarter",
        "next quarter",
        "q1",
        "q2",
        "q3",
        "q4",
        "this month",
        "next month",
        "this year",
        "next year",
        "by friday",
        "by monday",
        "eom",
    ];

    timeline_patterns
        .iter()
        .find(|pattern| normalized_text.contains(**pattern))
        .map(|pattern| (*pattern).to_string())
}

fn extract_discount_pct(tokens: &[String]) -> Option<u8> {
    for token in tokens {
        if let Some(raw) = token.strip_suffix('%') {
            if let Ok(percent) = raw.parse::<u8>() {
                return Some(percent);
            }
        }
    }
    None
}

fn extract_constraints(normalized_text: &str, has_budget: bool, has_discount: bool) -> Vec<String> {
    let mut constraints = BTreeSet::new();
    if has_budget {
        constraints.insert("budget_cap".to_string());
    }
    if has_discount {
        constraints.insert("discount_request".to_string());
    }
    if normalized_text.contains("must include")
        || normalized_text.contains("need")
        || normalized_text.contains("requires")
    {
        constraints.insert("required_feature".to_string());
    }
    if normalized_text.contains("without")
        || normalized_text.contains("exclude")
        || normalized_text.contains("no ")
    {
        constraints.insert("exclusion".to_string());
    }
    if normalized_text.contains("at most") || normalized_text.contains("no more than") {
        constraints.insert("upper_bound".to_string());
    }
    constraints.into_iter().collect()
}

fn confidence_score(
    has_product: bool,
    has_quantity: bool,
    has_budget: bool,
    has_timeline: bool,
    has_constraints: bool,
    has_discount: bool,
) -> u8 {
    let mut score = 10u8;
    if has_product {
        score += 30;
    }
    if has_quantity {
        score += 15;
    }
    if has_budget {
        score += 20;
    }
    if has_timeline {
        score += 10;
    }
    if has_constraints {
        score += 10;
    }
    if has_discount {
        score += 15;
    }
    score.min(100)
}

#[cfg(test)]
mod tests {
    use quotey_core::domain::product::ProductId;

    use super::{CatalogItem, ConstraintMapper, ExtractedIntent, IntentExtractor};

    #[test]
    fn extracts_core_fields_from_rich_request() {
        let extractor = IntentExtractor::new();
        let intent = extractor
            .extract("Need enterprise with premium support for 200 seats under $50k this quarter");

        assert!(intent.product_mentions.contains(&"enterprise".to_string()));
        assert!(intent.product_mentions.contains(&"support".to_string()));
        assert_eq!(intent.quantity_mentions, vec![200]);
        assert_eq!(intent.budget_cents, Some(5_000_000));
        assert_eq!(intent.timeline_hint.as_deref(), Some("this quarter"));
        assert!(intent.confidence_score >= 80);
    }

    #[test]
    fn extracts_discount_request() {
        let extractor = IntentExtractor::new();
        let intent = extractor.extract("Can we do 15% on enterprise renewal?");
        assert_eq!(intent.requested_discount_pct, Some(15));
        assert!(intent.constraints.contains(&"discount_request".to_string()));
    }

    #[test]
    fn ambiguous_text_requests_clarification() {
        let extractor = IntentExtractor::new();
        let intent = extractor.extract("Can you help?");
        assert!(intent.clarification_prompt.is_some());
        assert!(intent.product_mentions.is_empty());
        assert!(intent.budget_cents.is_none());
    }

    #[test]
    fn handles_twenty_plus_common_phrases() {
        struct Case {
            text: &'static str,
            expect_products: bool,
            expect_budget: bool,
        }

        let cases = vec![
            Case { text: "enterprise for 100 seats", expect_products: true, expect_budget: false },
            Case {
                text: "premium plan with api access",
                expect_products: true,
                expect_budget: false,
            },
            Case {
                text: "starter package for 10 users",
                expect_products: true,
                expect_budget: false,
            },
            Case { text: "need support only", expect_products: true, expect_budget: false },
            Case { text: "budget is $25k for q2", expect_products: false, expect_budget: true },
            Case {
                text: "under 40k with enterprise features",
                expect_products: true,
                expect_budget: true,
            },
            Case {
                text: "no more than $100000 annual",
                expect_products: false,
                expect_budget: true,
            },
            Case { text: "api and security add-ons", expect_products: true, expect_budget: false },
            Case { text: "basic tier by friday", expect_products: true, expect_budget: false },
            Case {
                text: "premium support and analytics for 60 seats",
                expect_products: true,
                expect_budget: false,
            },
            Case {
                text: "enterprise expansion next quarter",
                expect_products: true,
                expect_budget: false,
            },
            Case {
                text: "need 300 licenses for compliance rollout",
                expect_products: true,
                expect_budget: false,
            },
            Case { text: "budget cap 75k, no support", expect_products: true, expect_budget: true },
            Case { text: "starter no api", expect_products: true, expect_budget: false },
            Case {
                text: "q3 renewals at 20% discount",
                expect_products: false,
                expect_budget: false,
            },
            Case {
                text: "enterprise and premium by eom",
                expect_products: true,
                expect_budget: false,
            },
            Case {
                text: "need 45 users and $12k spend limit",
                expect_products: false,
                expect_budget: true,
            },
            Case {
                text: "requires security controls",
                expect_products: true,
                expect_budget: false,
            },
            Case {
                text: "exclude support and include api",
                expect_products: true,
                expect_budget: false,
            },
            Case {
                text: "this month we need 80 seats",
                expect_products: false,
                expect_budget: false,
            },
            Case { text: "max $15k for starter", expect_products: true, expect_budget: true },
            Case {
                text: "enterprise migration with 2 departments",
                expect_products: true,
                expect_budget: false,
            },
        ];

        let extractor = IntentExtractor::new();
        for (index, case) in cases.iter().enumerate() {
            let intent = extractor.extract(case.text);
            if case.expect_products {
                assert!(
                    !intent.product_mentions.is_empty(),
                    "case {index} expected products: {}",
                    case.text
                );
            }
            if case.expect_budget {
                assert!(
                    intent.budget_cents.is_some(),
                    "case {index} expected budget: {}",
                    case.text
                );
            }
            assert!(
                intent.confidence_score > 0,
                "case {index} should produce non-zero confidence: {}",
                case.text
            );
        }
    }

    #[test]
    fn maps_intent_to_constraint_set_for_solver() {
        let mapper = ConstraintMapper::with_catalog(catalog_fixture());
        let intent = ExtractedIntent {
            product_mentions: vec!["enterprise".to_string(), "support".to_string()],
            quantity_mentions: vec![5],
            budget_cents: None,
            timeline_hint: None,
            requested_discount_pct: Some(10),
            constraints: vec!["required_feature".to_string()],
            confidence_score: 95,
            clarification_prompt: None,
        };

        let constraint_set = mapper.map_intent(&intent);
        assert_eq!(constraint_set.constraint_input.quote_lines.len(), 2);
        assert!(constraint_set.validation.valid);
        assert_eq!(constraint_set.matched_product_ids.len(), 2);
        assert_eq!(constraint_set.estimated_total_cents, 8_500_000);
        assert_eq!(constraint_set.requested_discount_pct, Some(10));
        assert!(constraint_set.clarification_prompt.is_none());
    }

    #[test]
    fn budget_mapping_drops_optional_items_to_fit_cap() {
        let mapper = ConstraintMapper::with_catalog(catalog_fixture());
        let intent = ExtractedIntent {
            product_mentions: vec!["enterprise".to_string(), "support".to_string()],
            quantity_mentions: vec![1],
            budget_cents: Some(1_500_000),
            timeline_hint: None,
            requested_discount_pct: None,
            constraints: vec!["budget_cap".to_string()],
            confidence_score: 90,
            clarification_prompt: None,
        };

        let constraint_set = mapper.map_intent(&intent);
        assert!(constraint_set.validation.valid);
        assert_eq!(constraint_set.constraint_input.quote_lines.len(), 1);
        assert_eq!(
            constraint_set.constraint_input.quote_lines[0].product_id,
            ProductId("enterprise".to_string())
        );
        assert_eq!(constraint_set.estimated_total_cents, 1_500_000);
        assert!(constraint_set.clarification_prompt.is_none());
    }

    #[test]
    fn budget_mapping_reduces_quantity_when_single_line_exceeds_cap() {
        let mapper = ConstraintMapper::with_catalog(catalog_fixture());
        let intent = ExtractedIntent {
            product_mentions: vec!["premium".to_string()],
            quantity_mentions: vec![12],
            budget_cents: Some(1_800_000),
            timeline_hint: None,
            requested_discount_pct: None,
            constraints: vec!["budget_cap".to_string()],
            confidence_score: 88,
            clarification_prompt: None,
        };

        let constraint_set = mapper.map_intent(&intent);
        assert!(constraint_set.validation.valid);
        assert_eq!(constraint_set.constraint_input.quote_lines.len(), 1);
        assert_eq!(constraint_set.constraint_input.quote_lines[0].quantity, 6);
        assert_eq!(constraint_set.estimated_total_cents, 1_800_000);
    }

    #[test]
    fn unresolved_product_mentions_trigger_clarifying_prompt() {
        let mapper = ConstraintMapper::with_catalog(catalog_fixture());
        let intent = ExtractedIntent {
            product_mentions: vec!["platinum".to_string()],
            quantity_mentions: vec![2],
            budget_cents: None,
            timeline_hint: None,
            requested_discount_pct: None,
            constraints: Vec::new(),
            confidence_score: 40,
            clarification_prompt: None,
        };

        let constraint_set = mapper.map_intent(&intent);
        assert!(!constraint_set.validation.valid);
        assert_eq!(constraint_set.unresolved_product_mentions, vec!["platinum".to_string()]);
        assert!(constraint_set.clarification_prompt.is_some());
    }

    fn catalog_fixture() -> Vec<CatalogItem> {
        vec![
            CatalogItem {
                product_id: ProductId("enterprise".to_string()),
                display_name: "Enterprise".to_string(),
                aliases: vec!["enterprise tier".to_string(), "ent".to_string()],
                unit_price_cents: 1_500_000,
                required: true,
            },
            CatalogItem {
                product_id: ProductId("premium".to_string()),
                display_name: "Premium".to_string(),
                aliases: vec!["pro".to_string(), "premium tier".to_string()],
                unit_price_cents: 300_000,
                required: true,
            },
            CatalogItem {
                product_id: ProductId("support".to_string()),
                display_name: "Premium Support".to_string(),
                aliases: vec!["support".to_string(), "support add-on".to_string()],
                unit_price_cents: 200_000,
                required: false,
            },
        ]
    }
}
