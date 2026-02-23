use std::collections::BTreeSet;

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
    use super::IntentExtractor;

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
}
