//! Ambiguity Detection and Clarification System
//!
//! This module provides functionality to detect ambiguous or missing information
//! in quote creation and present clarification cards to users.

use serde::{Deserialize, Serialize};

/// Types of ambiguities that can be detected
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmbiguityType {
    /// Multiple products match the description
    ProductDisambiguation,
    /// Customer/account not found or unclear
    CustomerResolution,
    /// Quantity not specified or unclear
    MissingQuantity,
    /// Quantity could apply to multiple products
    QuantityAssociation,
    /// Start date not provided
    MissingStartDate,
    /// Start date format unclear
    DateFormatAmbiguity,
    /// Term not specified
    MissingTerm,
    /// Currency not specified (using default)
    DefaultCurrency,
    /// Billing country not specified
    MissingBillingCountry,
    /// Product configuration missing required attributes
    MissingProductConfiguration,
    /// Discount percentage unclear
    AmbiguousDiscount,
    /// Multiple dates mentioned, which is start date?
    DateDisambiguation,
}

impl AmbiguityType {
    /// Get a human-readable label for this ambiguity type
    pub fn label(&self) -> &'static str {
        match self {
            AmbiguityType::ProductDisambiguation => "Product Selection",
            AmbiguityType::CustomerResolution => "Customer",
            AmbiguityType::MissingQuantity => "Quantity",
            AmbiguityType::QuantityAssociation => "Quantity Assignment",
            AmbiguityType::MissingStartDate => "Start Date",
            AmbiguityType::DateFormatAmbiguity => "Date Format",
            AmbiguityType::MissingTerm => "Contract Term",
            AmbiguityType::DefaultCurrency => "Currency",
            AmbiguityType::MissingBillingCountry => "Billing Country",
            AmbiguityType::MissingProductConfiguration => "Product Configuration",
            AmbiguityType::AmbiguousDiscount => "Discount",
            AmbiguityType::DateDisambiguation => "Date Selection",
        }
    }

    /// Get the severity level of this ambiguity
    pub fn severity(&self) -> AmbiguitySeverity {
        match self {
            AmbiguityType::ProductDisambiguation => AmbiguitySeverity::High,
            AmbiguityType::CustomerResolution => AmbiguitySeverity::Critical,
            AmbiguityType::MissingQuantity => AmbiguitySeverity::High,
            AmbiguityType::QuantityAssociation => AmbiguitySeverity::High,
            AmbiguityType::MissingStartDate => AmbiguitySeverity::Medium,
            AmbiguityType::DateFormatAmbiguity => AmbiguitySeverity::Medium,
            AmbiguityType::MissingTerm => AmbiguitySeverity::Medium,
            AmbiguityType::DefaultCurrency => AmbiguitySeverity::Low,
            AmbiguityType::MissingBillingCountry => AmbiguitySeverity::Medium,
            AmbiguityType::MissingProductConfiguration => AmbiguitySeverity::High,
            AmbiguityType::AmbiguousDiscount => AmbiguitySeverity::Low,
            AmbiguityType::DateDisambiguation => AmbiguitySeverity::Medium,
        }
    }
}

/// Severity levels for ambiguities
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmbiguitySeverity {
    /// Blocks quote creation/progression
    Critical,
    /// Should be resolved for accurate pricing
    High,
    /// Should be clarified for completeness
    Medium,
    /// Nice to have clarified
    Low,
}

impl AmbiguitySeverity {
    /// Get emoji for this severity
    pub fn emoji(&self) -> &'static str {
        match self {
            AmbiguitySeverity::Critical => "🚫",
            AmbiguitySeverity::High => "⚠️",
            AmbiguitySeverity::Medium => "ℹ️",
            AmbiguitySeverity::Low => "💡",
        }
    }

    /// Get color code for this severity
    pub fn color(&self) -> &'static str {
        match self {
            AmbiguitySeverity::Critical => "#ef4444",
            AmbiguitySeverity::High => "#f59e0b",
            AmbiguitySeverity::Medium => "#3b82f6",
            AmbiguitySeverity::Low => "#6b7280",
        }
    }
}

/// A single detected ambiguity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ambiguity {
    /// Type of ambiguity
    pub ambiguity_type: AmbiguityType,
    /// Human-readable description of the issue
    pub description: String,
    /// Suggested resolution or clarification question
    pub clarification_prompt: String,
    /// Possible options for resolution (if applicable)
    pub options: Vec<AmbiguityOption>,
    /// Field or context this ambiguity relates to
    pub field: String,
    /// Original value that caused ambiguity (if any)
    pub original_value: Option<String>,
}

/// An option for resolving an ambiguity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmbiguityOption {
    /// Unique identifier for this option
    pub id: String,
    /// Display label
    pub label: String,
    /// Description of what this option means
    pub description: Option<String>,
    /// Value to use if this option is selected
    pub value: serde_json::Value,
    /// Confidence score (0-1) for this option
    pub confidence: f64,
}

/// Collection of ambiguities for a quote
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AmbiguitySet {
    /// List of detected ambiguities
    pub ambiguities: Vec<Ambiguity>,
    /// Whether any critical ambiguities exist
    pub has_critical: bool,
    /// Whether any high severity ambiguities exist
    pub has_high: bool,
}

impl AmbiguitySet {
    /// Create a new empty ambiguity set
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an ambiguity to the set
    pub fn add(&mut self, ambiguity: Ambiguity) {
        if ambiguity.ambiguity_type.severity() == AmbiguitySeverity::Critical {
            self.has_critical = true;
        }
        if ambiguity.ambiguity_type.severity() == AmbiguitySeverity::High {
            self.has_high = true;
        }
        self.ambiguities.push(ambiguity);
    }

    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.ambiguities.is_empty()
    }

    /// Get count of ambiguities
    pub fn len(&self) -> usize {
        self.ambiguities.len()
    }

    /// Get ambiguities by severity
    pub fn by_severity(&self, severity: AmbiguitySeverity) -> Vec<&Ambiguity> {
        self.ambiguities
            .iter()
            .filter(|a| a.ambiguity_type.severity() == severity)
            .collect()
    }

    /// Get critical ambiguities
    pub fn critical(&self) -> Vec<&Ambiguity> {
        self.by_severity(AmbiguitySeverity::Critical)
    }

    /// Get high severity ambiguities
    pub fn high(&self) -> Vec<&Ambiguity> {
        self.by_severity(AmbiguitySeverity::High)
    }
}

/// Input for ambiguity detection
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AmbiguityDetectionInput {
    /// Raw user input text (if from natural language)
    pub raw_input: Option<String>,
    /// Parsed account/customer identifier
    pub account_id: Option<String>,
    /// Parsed product mentions
    pub product_mentions: Vec<ProductMention>,
    /// Parsed quantities
    pub quantities: Vec<QuantityMention>,
    /// Parsed dates
    pub dates: Vec<DateMention>,
    /// Parsed term
    pub term_months: Option<u32>,
    /// Parsed currency
    pub currency: Option<String>,
    /// Parsed billing country
    pub billing_country: Option<String>,
    /// Parsed discount
    pub discount_pct: Option<f64>,
}

/// A product mention from parsing
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProductMention {
    /// Original text that matched
    pub original_text: String,
    /// Matched product IDs (multiple = ambiguous)
    pub matched_products: Vec<String>,
    /// Confidence in match (0-1)
    pub confidence: f64,
}

/// A quantity mention from parsing
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuantityMention {
    /// The quantity value
    pub value: u32,
    /// Original text
    pub original_text: String,
    /// Product it might apply to (if specified)
    pub associated_product: Option<String>,
}

/// A date mention from parsing
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DateMention {
    /// Original text
    pub original_text: String,
    /// Parsed date (if successful)
    pub parsed_date: Option<String>,
    /// Context (start, end, etc.)
    pub context: Option<String>,
}

/// Engine for detecting ambiguities
pub struct AmbiguityDetectionEngine {
    /// Minimum confidence threshold for product matches
    pub product_confidence_threshold: f64,
}

impl Default for AmbiguityDetectionEngine {
    fn default() -> Self {
        Self {
            product_confidence_threshold: 0.7,
        }
    }
}

impl AmbiguityDetectionEngine {
    /// Create a new ambiguity detection engine
    pub fn new() -> Self {
        Self::default()
    }

    /// Detect ambiguities in the input
    pub fn detect(&self, input: &AmbiguityDetectionInput) -> AmbiguitySet {
        let mut set = AmbiguitySet::new();

        // Check for customer resolution issues
        self.detect_customer_ambiguity(input, &mut set);

        // Check for product disambiguation needs
        self.detect_product_ambiguities(input, &mut set);

        // Check for quantity issues
        self.detect_quantity_ambiguities(input, &mut set);

        // Check for date issues
        self.detect_date_ambiguities(input, &mut set);

        // Check for missing term
        self.detect_missing_term(input, &mut set);

        // Check for default currency
        self.detect_default_currency(input, &mut set);

        // Check for missing billing country
        self.detect_missing_billing_country(input, &mut set);

        set
    }

    fn detect_customer_ambiguity(&self, input: &AmbiguityDetectionInput, set: &mut AmbiguitySet) {
        if input.account_id.is_none() {
            set.add(Ambiguity {
                ambiguity_type: AmbiguityType::CustomerResolution,
                description: "No customer specified".to_string(),
                clarification_prompt: "Which customer is this quote for?".to_string(),
                options: vec![],
                field: "account_id".to_string(),
                original_value: None,
            });
        }
    }

    fn detect_product_ambiguities(&self, input: &AmbiguityDetectionInput, set: &mut AmbiguitySet) {
        for product in &input.product_mentions {
            if product.matched_products.len() > 1 {
                let options: Vec<AmbiguityOption> = product
                    .matched_products
                    .iter()
                    .map(|p| AmbiguityOption {
                        id: p.clone(),
                        label: p.clone(),
                        description: None,
                        value: serde_json::json!(p),
                        confidence: 1.0 / product.matched_products.len() as f64,
                    })
                    .collect();

                set.add(Ambiguity {
                    ambiguity_type: AmbiguityType::ProductDisambiguation,
                    description: format!(
                        "'{}' could match multiple products",
                        product.original_text
                    ),
                    clarification_prompt: format!(
                        "Which product did you mean by '{}' ?",
                        product.original_text
                    ),
                    options,
                    field: "product".to_string(),
                    original_value: Some(product.original_text.clone()),
                });
            } else if product.matched_products.is_empty() {
                set.add(Ambiguity {
                    ambiguity_type: AmbiguityType::ProductDisambiguation,
                    description: format!(
                        "'{}' did not match any known products",
                        product.original_text
                    ),
                    clarification_prompt: format!(
                        "I couldn't find a product matching '{}'. Please specify the exact product name or ID.",
                        product.original_text
                    ),
                    options: vec![],
                    field: "product".to_string(),
                    original_value: Some(product.original_text.clone()),
                });
            } else if product.confidence < self.product_confidence_threshold {
                set.add(Ambiguity {
                    ambiguity_type: AmbiguityType::ProductDisambiguation,
                    description: format!(
                        "Low confidence match for '{}'",
                        product.original_text
                    ),
                    clarification_prompt: format!(
                        "I matched '{}' to '{}'. Is this correct?",
                        product.original_text,
                        product.matched_products[0]
                    ),
                    options: vec![
                        AmbiguityOption {
                            id: "yes".to_string(),
                            label: "Yes, that's correct".to_string(),
                            description: None,
                            value: serde_json::json!(product.matched_products[0]),
                            confidence: 1.0,
                        },
                        AmbiguityOption {
                            id: "no".to_string(),
                            label: "No, let me specify".to_string(),
                            description: None,
                            value: serde_json::Value::Null,
                            confidence: 0.0,
                        },
                    ],
                    field: "product".to_string(),
                    original_value: Some(product.original_text.clone()),
                });
            }
        }
    }

    fn detect_quantity_ambiguities(&self, input: &AmbiguityDetectionInput, set: &mut AmbiguitySet) {
        // Check if we have products but no quantities
        if !input.product_mentions.is_empty() && input.quantities.is_empty() {
            set.add(Ambiguity {
                ambiguity_type: AmbiguityType::MissingQuantity,
                description: "No quantity specified for products".to_string(),
                clarification_prompt: "How many units of each product?".to_string(),
                options: vec![],
                field: "quantity".to_string(),
                original_value: None,
            });
            return;
        }

        // Check if quantity count doesn't match product count
        let product_count = input
            .product_mentions
            .iter()
            .filter(|p| !p.matched_products.is_empty())
            .count();

        if product_count > 1 && input.quantities.len() == 1 {
            set.add(Ambiguity {
                ambiguity_type: AmbiguityType::QuantityAssociation,
                description: format!(
                    "One quantity specified for {} products",
                    product_count
                ),
                clarification_prompt: format!(
                    "You specified {} units. Which product(s) should this apply to?",
                    input.quantities[0].value
                ),
                options: vec![],
                field: "quantity_association".to_string(),
                original_value: Some(input.quantities[0].value.to_string()),
            });
        }
    }

    fn detect_date_ambiguities(&self, input: &AmbiguityDetectionInput, set: &mut AmbiguitySet) {
        if input.dates.is_empty() {
            set.add(Ambiguity {
                ambiguity_type: AmbiguityType::MissingStartDate,
                description: "No start date specified".to_string(),
                clarification_prompt: "When should this contract start?".to_string(),
                options: vec![],
                field: "start_date".to_string(),
                original_value: None,
            });
            return;
        }

        // Check for unparsable dates
        for date in &input.dates {
            if date.parsed_date.is_none() {
                set.add(Ambiguity {
                    ambiguity_type: AmbiguityType::DateFormatAmbiguity,
                    description: format!("Could not understand date '{}'", date.original_text),
                    clarification_prompt: format!(
                        "I didn't understand the date '{}'. Please use format YYYY-MM-DD (e.g., 2026-03-15).",
                        date.original_text
                    ),
                    options: vec![],
                    field: "start_date".to_string(),
                    original_value: Some(date.original_text.clone()),
                });
            }
        }

        // Check for multiple dates without clear context
        if input.dates.len() > 1 {
            let unparsed_count = input.dates.iter().filter(|d| d.parsed_date.is_none()).count();
            if unparsed_count < input.dates.len() {
                set.add(Ambiguity {
                    ambiguity_type: AmbiguityType::DateDisambiguation,
                    description: format!("{} dates mentioned", input.dates.len()),
                    clarification_prompt: "Which date should be the contract start date?".to_string(),
                    options: input
                        .dates
                        .iter()
                        .filter_map(|d| {
                            d.parsed_date.as_ref().map(|pd| AmbiguityOption {
                                id: pd.clone(),
                                label: format!("{} ({})", d.original_text, pd),
                                description: None,
                                value: serde_json::json!(pd),
                                confidence: 1.0,
                            })
                        })
                        .collect(),
                    field: "start_date".to_string(),
                    original_value: None,
                });
            }
        }
    }

    fn detect_missing_term(&self, input: &AmbiguityDetectionInput, set: &mut AmbiguitySet) {
        if input.term_months.is_none() {
            set.add(Ambiguity {
                ambiguity_type: AmbiguityType::MissingTerm,
                description: "Contract term not specified".to_string(),
                clarification_prompt: "What term length? (e.g., 12 months, 1 year)".to_string(),
                options: vec![
                    AmbiguityOption {
                        id: "12".to_string(),
                        label: "12 months".to_string(),
                        description: None,
                        value: serde_json::json!(12),
                        confidence: 1.0,
                    },
                    AmbiguityOption {
                        id: "24".to_string(),
                        label: "24 months".to_string(),
                        description: None,
                        value: serde_json::json!(24),
                        confidence: 1.0,
                    },
                    AmbiguityOption {
                        id: "36".to_string(),
                        label: "36 months".to_string(),
                        description: None,
                        value: serde_json::json!(36),
                        confidence: 1.0,
                    },
                ],
                field: "term_months".to_string(),
                original_value: None,
            });
        }
    }

    fn detect_default_currency(&self, input: &AmbiguityDetectionInput, set: &mut AmbiguitySet) {
        if input.currency.is_none() {
            set.add(Ambiguity {
                ambiguity_type: AmbiguityType::DefaultCurrency,
                description: "Using default currency (USD)".to_string(),
                clarification_prompt: "Is USD the correct currency?".to_string(),
                options: vec![
                    AmbiguityOption {
                        id: "usd".to_string(),
                        label: "Yes, USD".to_string(),
                        description: None,
                        value: serde_json::json!("USD"),
                        confidence: 1.0,
                    },
                    AmbiguityOption {
                        id: "other".to_string(),
                        label: "No, specify another".to_string(),
                        description: None,
                        value: serde_json::Value::Null,
                        confidence: 0.0,
                    },
                ],
                field: "currency".to_string(),
                original_value: None,
            });
        }
    }

    fn detect_missing_billing_country(&self, input: &AmbiguityDetectionInput, set: &mut AmbiguitySet) {
        if input.billing_country.is_none() {
            set.add(Ambiguity {
                ambiguity_type: AmbiguityType::MissingBillingCountry,
                description: "Billing country not specified".to_string(),
                clarification_prompt: "What country should be used for billing/tax purposes?".to_string(),
                options: vec![],
                field: "billing_country".to_string(),
                original_value: None,
            });
        }
    }
}

/// Render an ambiguity set as Slack blocks
pub fn render_ambiguity_slack_blocks(set: &AmbiguitySet) -> Vec<serde_json::Value> {
    let mut blocks = vec![];

    if set.is_empty() {
        return blocks;
    }

    // Header
    let severity_emoji = if set.has_critical {
        "🚫"
    } else if set.has_high {
        "⚠️"
    } else {
        "ℹ️"
    };

    blocks.push(serde_json::json!({
        "type": "header",
        "text": {
            "type": "plain_text",
            "text": format!("{} Clarification Needed", severity_emoji),
            "emoji": true
        }
    }));

    blocks.push(serde_json::json!({
        "type": "section",
        "text": {
            "type": "mrkdwn",
            "text": format!("I need some clarification to build your quote accurately ({} items):", set.len())
        }
    }));

    blocks.push(serde_json::json!({
        "type": "divider"
    }));

    // Each ambiguity
    for ambiguity in &set.ambiguities {
        let emoji = ambiguity.ambiguity_type.severity().emoji();

        let mut text = format!("*{} {}*\n{}", emoji, ambiguity.ambiguity_type.label(), ambiguity.clarification_prompt);

        if !ambiguity.options.is_empty() {
            text.push_str("\n\n*Options:*");
            for option in &ambiguity.options {
                text.push_str(&format!("\n• {}", option.label));
            }
        }

        blocks.push(serde_json::json!({
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": text
            }
        }));

        // Add action buttons if options exist
        if !ambiguity.options.is_empty() {
            let elements: Vec<serde_json::Value> = ambiguity
                .options
                .iter()
                .map(|opt| {
                    serde_json::json!({
                        "type": "button",
                        "text": {
                            "type": "plain_text",
                            "text": opt.label.clone(),
                            "emoji": true
                        },
                        "value": format!("{}:{}", ambiguity.field, opt.id),
                        "action_id": format!("clarify_{}_{}", ambiguity.field, opt.id)
                    })
                })
                .collect();

            blocks.push(serde_json::json!({
                "type": "actions",
                "elements": elements
            }));
        }

        blocks.push(serde_json::json!({
            "type": "divider"
        }));
    }

    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_missing_customer() {
        let engine = AmbiguityDetectionEngine::new();
        let input = AmbiguityDetectionInput {
            raw_input: Some("quote for enterprise plan".to_string()),
            account_id: None,
            product_mentions: vec![],
            quantities: vec![],
            dates: vec![],
            term_months: None,
            currency: None,
            billing_country: None,
            discount_pct: None,
        };

        let set = engine.detect(&input);
        assert_eq!(set.len(), 6); // customer, quantity, dates, term, currency, billing_country
        assert!(set.has_critical);
    }

    #[test]
    fn detects_product_disambiguation() {
        let engine = AmbiguityDetectionEngine::new();
        let input = AmbiguityDetectionInput {
            raw_input: Some("quote for enterprise".to_string()),
            account_id: Some("ACME".to_string()),
            product_mentions: vec![ProductMention {
                original_text: "enterprise".to_string(),
                matched_products: vec!["plan-enterprise".to_string(), "addon-enterprise".to_string()],
                confidence: 0.5,
            }],
            quantities: vec![QuantityMention {
                value: 100,
                original_text: "100".to_string(),
                associated_product: None,
            }],
            dates: vec![],
            term_months: Some(12),
            currency: Some("USD".to_string()),
            billing_country: Some("US".to_string()),
            discount_pct: None,
        };

        let set = engine.detect(&input);
        let product_ambiguities: Vec<_> = set
            .ambiguities
            .iter()
            .filter(|a| a.ambiguity_type == AmbiguityType::ProductDisambiguation)
            .collect();
        assert_eq!(product_ambiguities.len(), 1);
        assert_eq!(product_ambiguities[0].options.len(), 2);
    }

    #[test]
    fn renders_slack_blocks() {
        let mut set = AmbiguitySet::new();
        set.add(Ambiguity {
            ambiguity_type: AmbiguityType::MissingQuantity,
            description: "No quantity".to_string(),
            clarification_prompt: "How many?".to_string(),
            options: vec![
                AmbiguityOption {
                    id: "10".to_string(),
                    label: "10".to_string(),
                    description: None,
                    value: serde_json::json!(10),
                    confidence: 1.0,
                },
            ],
            field: "quantity".to_string(),
            original_value: None,
        });

        let blocks = render_ambiguity_slack_blocks(&set);
        assert!(!blocks.is_empty());
        assert_eq!(blocks[0]["type"], "header");
    }
}
