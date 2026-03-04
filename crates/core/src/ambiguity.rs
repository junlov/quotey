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

/// States for assumption/ambiguity cards
/// Standardizes the display of quote constraints and pricing inputs
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssumptionState {
    /// Value explicitly provided by the user
    Confirmed,
    /// Value defaulted by the system (user can override)
    Assumed,
    /// Value is missing and needs user input
    NeedsConfirmation,
}

impl AssumptionState {
    /// Get emoji for this state
    pub fn emoji(&self) -> &'static str {
        match self {
            AssumptionState::Confirmed => "✅",
            AssumptionState::Assumed => "🤔",
            AssumptionState::NeedsConfirmation => "❓",
        }
    }

    /// Get color code for this state
    pub fn color(&self) -> &'static str {
        match self {
            AssumptionState::Confirmed => "#22c55e",
            AssumptionState::Assumed => "#f59e0b",
            AssumptionState::NeedsConfirmation => "#ef4444",
        }
    }

    /// Get label for this state
    pub fn label(&self) -> &'static str {
        match self {
            AssumptionState::Confirmed => "Confirmed",
            AssumptionState::Assumed => "Assumed",
            AssumptionState::NeedsConfirmation => "Needs Confirmation",
        }
    }
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
        self.ambiguities.iter().filter(|a| a.ambiguity_type.severity() == severity).collect()
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

/// A card representing a quote assumption or ambiguity
/// Standardized display for Confirmed, Assumed, and NeedsConfirmation states
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssumptionCard {
    /// Unique identifier for this card
    pub id: String,
    /// Field or context this card relates to
    pub field: String,
    /// Human-readable label for the field
    pub field_label: String,
    /// Current state of this card
    pub state: AssumptionState,
    /// Current value (if any)
    pub value: Option<String>,
    /// Human-readable description of the value or issue
    pub description: String,
    /// Detailed explanation shown on expand
    pub details: Option<String>,
    /// Possible options for resolution (for NeedsConfirmation/Assumed states)
    pub options: Vec<AmbiguityOption>,
    /// Whether this card can be edited by the user
    pub editable: bool,
    /// Category for grouping cards
    pub category: AssumptionCategory,
}

/// Categories for grouping assumption cards
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssumptionCategory {
    /// Customer/account information
    Customer,
    /// Product configuration
    Product,
    /// Pricing and discounts
    Pricing,
    /// Contract terms
    Term,
    /// Billing information
    Billing,
    /// General/other
    General,
}

impl AssumptionCategory {
    /// Get label for this category
    pub fn label(&self) -> &'static str {
        match self {
            AssumptionCategory::Customer => "Customer",
            AssumptionCategory::Product => "Products",
            AssumptionCategory::Pricing => "Pricing",
            AssumptionCategory::Term => "Terms",
            AssumptionCategory::Billing => "Billing",
            AssumptionCategory::General => "Other",
        }
    }

    /// Get emoji for this category
    pub fn emoji(&self) -> &'static str {
        match self {
            AssumptionCategory::Customer => "👤",
            AssumptionCategory::Product => "📦",
            AssumptionCategory::Pricing => "💰",
            AssumptionCategory::Term => "📅",
            AssumptionCategory::Billing => "🧾",
            AssumptionCategory::General => "📋",
        }
    }
}

impl AssumptionCard {
    /// Create a new confirmed card
    pub fn confirmed(
        id: impl Into<String>,
        field: impl Into<String>,
        field_label: impl Into<String>,
        value: impl Into<String>,
        description: impl Into<String>,
        category: AssumptionCategory,
    ) -> Self {
        Self {
            id: id.into(),
            field: field.into(),
            field_label: field_label.into(),
            state: AssumptionState::Confirmed,
            value: Some(value.into()),
            description: description.into(),
            details: None,
            options: vec![],
            editable: true,
            category,
        }
    }

    /// Create a new assumed card
    pub fn assumed(
        id: impl Into<String>,
        field: impl Into<String>,
        field_label: impl Into<String>,
        value: impl Into<String>,
        description: impl Into<String>,
        category: AssumptionCategory,
    ) -> Self {
        Self {
            id: id.into(),
            field: field.into(),
            field_label: field_label.into(),
            state: AssumptionState::Assumed,
            value: Some(value.into()),
            description: description.into(),
            details: None,
            options: vec![],
            editable: true,
            category,
        }
    }

    /// Create a new needs confirmation card
    pub fn needs_confirmation(
        id: impl Into<String>,
        field: impl Into<String>,
        field_label: impl Into<String>,
        description: impl Into<String>,
        options: Vec<AmbiguityOption>,
        category: AssumptionCategory,
    ) -> Self {
        Self {
            id: id.into(),
            field: field.into(),
            field_label: field_label.into(),
            state: AssumptionState::NeedsConfirmation,
            value: None,
            description: description.into(),
            details: None,
            options,
            editable: true,
            category,
        }
    }

    /// Set details for this card
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Set editable flag
    pub fn with_editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }

    /// Convert an ambiguity to a needs confirmation card
    pub fn from_ambiguity(ambiguity: &Ambiguity, category: AssumptionCategory) -> Self {
        Self {
            id: format!("{}_card", ambiguity.field),
            field: ambiguity.field.clone(),
            field_label: ambiguity.ambiguity_type.label().to_string(),
            state: AssumptionState::NeedsConfirmation,
            value: ambiguity.original_value.clone(),
            description: ambiguity.clarification_prompt.clone(),
            details: Some(ambiguity.description.clone()),
            options: ambiguity.options.clone(),
            editable: true,
            category,
        }
    }
}

/// Collection of assumption cards for a quote
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AssumptionCardSet {
    /// List of cards
    pub cards: Vec<AssumptionCard>,
    /// Whether all required cards are confirmed
    pub all_confirmed: bool,
    /// Count of cards by state
    pub confirmed_count: usize,
    pub assumed_count: usize,
    pub needs_confirmation_count: usize,
}

impl AssumptionCardSet {
    /// Create a new empty card set
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a card to the set
    pub fn add(&mut self, card: AssumptionCard) {
        match card.state {
            AssumptionState::Confirmed => self.confirmed_count += 1,
            AssumptionState::Assumed => self.assumed_count += 1,
            AssumptionState::NeedsConfirmation => self.needs_confirmation_count += 1,
        }
        self.cards.push(card);
        self.update_all_confirmed();
    }

    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Get count of cards
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// Get cards by state
    pub fn by_state(&self, state: AssumptionState) -> Vec<&AssumptionCard> {
        self.cards.iter().filter(|c| c.state == state).collect()
    }

    /// Get cards by category
    pub fn by_category(&self, category: AssumptionCategory) -> Vec<&AssumptionCard> {
        self.cards.iter().filter(|c| c.category == category).collect()
    }

    /// Update the all_confirmed flag
    fn update_all_confirmed(&mut self) {
        self.all_confirmed = self.needs_confirmation_count == 0;
    }

    /// Get cards that need user attention (sorted by priority)
    pub fn attention_required(&self) -> Vec<&AssumptionCard> {
        let mut cards: Vec<_> = self
            .cards
            .iter()
            .filter(|c| {
                matches!(c.state, AssumptionState::NeedsConfirmation | AssumptionState::Assumed)
            })
            .collect();
        // Sort: NeedsConfirmation first, then Assumed
        cards.sort_by_key(|c| match c.state {
            AssumptionState::NeedsConfirmation => 0,
            AssumptionState::Assumed => 1,
            AssumptionState::Confirmed => 2,
        });
        cards
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
        Self { product_confidence_threshold: 0.7 }
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
                    description: format!("Low confidence match for '{}'", product.original_text),
                    clarification_prompt: format!(
                        "I matched '{}' to '{}'. Is this correct?",
                        product.original_text, product.matched_products[0]
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

        // Check if no quantities at all (even without products)
        if input.quantities.is_empty() {
            set.add(Ambiguity {
                ambiguity_type: AmbiguityType::MissingQuantity,
                description: "No quantity specified".to_string(),
                clarification_prompt: "How many units?".to_string(),
                options: vec![],
                field: "quantity".to_string(),
                original_value: None,
            });
        }

        // Check if quantity count doesn't match product count
        let product_count =
            input.product_mentions.iter().filter(|p| !p.matched_products.is_empty()).count();

        if product_count > 1 && input.quantities.len() == 1 {
            set.add(Ambiguity {
                ambiguity_type: AmbiguityType::QuantityAssociation,
                description: format!("One quantity specified for {} products", product_count),
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
                    clarification_prompt: "Which date should be the contract start date?"
                        .to_string(),
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

    fn detect_missing_billing_country(
        &self,
        input: &AmbiguityDetectionInput,
        set: &mut AmbiguitySet,
    ) {
        if input.billing_country.is_none() {
            set.add(Ambiguity {
                ambiguity_type: AmbiguityType::MissingBillingCountry,
                description: "Billing country not specified".to_string(),
                clarification_prompt: "What country should be used for billing/tax purposes?"
                    .to_string(),
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

        let mut text = format!(
            "*{} {}*\n{}",
            emoji,
            ambiguity.ambiguity_type.label(),
            ambiguity.clarification_prompt
        );

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

/// Render an assumption card as Slack blocks
/// Standardized card pattern for Confirmed, Assumed, and NeedsConfirmation states
pub fn render_assumption_card_slack_blocks(card: &AssumptionCard) -> Vec<serde_json::Value> {
    let mut blocks = vec![];
    let state_emoji = card.state.emoji();

    // Card header with state indicator
    let value_text = match (&card.value, &card.state) {
        (Some(val), AssumptionState::Confirmed) => format!("{} {}", state_emoji, val),
        (Some(val), AssumptionState::Assumed) => format!("{} {} (assumed)", state_emoji, val),
        (None, AssumptionState::NeedsConfirmation) => format!("{} _Required_", state_emoji),
        _ => state_emoji.to_string(),
    };

    blocks.push(serde_json::json!({
        "type": "section",
        "text": {
            "type": "mrkdwn",
            "text": format!("*{}*\n{}", card.field_label, value_text)
        }
    }));

    // Description text
    blocks.push(serde_json::json!({
        "type": "context",
        "elements": [
            {
                "type": "mrkdwn",
                "text": card.description.clone()
            }
        ]
    }));

    // Action buttons based on state
    if card.editable {
        match card.state {
            AssumptionState::Confirmed => {
                // Show "Change" button for confirmed values
                blocks.push(serde_json::json!({
                    "type": "actions",
                    "elements": [
                        {
                            "type": "button",
                            "text": {
                                "type": "plain_text",
                                "text": "Change",
                                "emoji": true
                            },
                            "value": format!("change:{}", card.field),
                            "action_id": format!("card_change_{}", card.field),
                            "style": "secondary"
                        }
                    ]
                }));
            }
            AssumptionState::Assumed => {
                // Show "Confirm" and "Change" buttons for assumed values
                // Change button is ALWAYS shown for assumed values since users
                // must be able to override system defaults
                let elements = vec![
                    serde_json::json!({
                        "type": "button",
                        "text": {
                            "type": "plain_text",
                            "text": "✓ Confirm",
                            "emoji": true
                        },
                        "value": format!("confirm:{}", card.field),
                        "action_id": format!("card_confirm_{}", card.field),
                        "style": "primary"
                    }),
                    serde_json::json!({
                        "type": "button",
                        "text": {
                            "type": "plain_text",
                            "text": "Change",
                            "emoji": true
                        },
                        "value": format!("change:{}", card.field),
                        "action_id": format!("card_change_{}", card.field),
                        "style": "secondary"
                    }),
                ];

                blocks.push(serde_json::json!({
                    "type": "actions",
                    "elements": elements
                }));
            }
            AssumptionState::NeedsConfirmation => {
                // Show resolution options for needs confirmation
                if !card.options.is_empty() {
                    let elements: Vec<serde_json::Value> = card
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
                                "value": format!("{}:{}", card.field, opt.id),
                                "action_id": format!("card_resolve_{}_{}", card.field, opt.id),
                                "style": "primary"
                            })
                        })
                        .collect();

                    blocks.push(serde_json::json!({
                        "type": "actions",
                        "elements": elements
                    }));
                } else {
                    // Show "Provide" button when no options available
                    blocks.push(serde_json::json!({
                        "type": "actions",
                        "elements": [
                            {
                                "type": "button",
                                "text": {
                                    "type": "plain_text",
                                    "text": "Provide",
                                    "emoji": true
                                },
                                "value": format!("provide:{}", card.field),
                                "action_id": format!("card_provide_{}", card.field),
                                "style": "primary"
                            }
                        ]
                    }));
                }
            }
        }
    }

    blocks.push(serde_json::json!({
        "type": "divider"
    }));

    blocks
}

/// Render an assumption card set as Slack blocks
/// Groups cards by category and shows summary header
pub fn render_assumption_card_set_slack_blocks(
    card_set: &AssumptionCardSet,
) -> Vec<serde_json::Value> {
    let mut blocks = vec![];

    if card_set.is_empty() {
        return blocks;
    }

    // Summary header
    let summary_emoji = if card_set.needs_confirmation_count > 0 {
        "❓"
    } else if card_set.assumed_count > 0 {
        "🤔"
    } else {
        "✅"
    };

    blocks.push(serde_json::json!({
        "type": "header",
        "text": {
            "type": "plain_text",
            "text": format!("{} Quote Information", summary_emoji),
            "emoji": true
        }
    }));

    // Status summary
    let status_text = format!(
        "{} confirmed · {} assumed · {} needs confirmation",
        card_set.confirmed_count, card_set.assumed_count, card_set.needs_confirmation_count
    );

    blocks.push(serde_json::json!({
        "type": "section",
        "text": {
            "type": "mrkdwn",
            "text": status_text
        }
    }));

    blocks.push(serde_json::json!({
        "type": "divider"
    }));

    // Group cards by category
    let categories = [
        AssumptionCategory::Customer,
        AssumptionCategory::Product,
        AssumptionCategory::Pricing,
        AssumptionCategory::Term,
        AssumptionCategory::Billing,
        AssumptionCategory::General,
    ];

    for category in &categories {
        let category_cards: Vec<_> =
            card_set.cards.iter().filter(|c| &c.category == category).collect();

        if !category_cards.is_empty() {
            // Category header
            blocks.push(serde_json::json!({
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": format!("{} *{}*", category.emoji(), category.label())
                }
            }));

            // Cards in this category
            for card in category_cards {
                let card_blocks = render_assumption_card_slack_blocks(card);
                blocks.extend(card_blocks);
            }
        }
    }

    blocks
}

/// Convert an ambiguity set to an assumption card set
/// Transforms detected ambiguities into the card pattern
pub fn ambiguity_set_to_card_set(set: &AmbiguitySet) -> AssumptionCardSet {
    let mut card_set = AssumptionCardSet::new();

    for ambiguity in &set.ambiguities {
        let category = match ambiguity.ambiguity_type {
            AmbiguityType::CustomerResolution => AssumptionCategory::Customer,
            AmbiguityType::ProductDisambiguation
            | AmbiguityType::MissingProductConfiguration
            | AmbiguityType::MissingQuantity
            | AmbiguityType::QuantityAssociation => AssumptionCategory::Product,
            AmbiguityType::AmbiguousDiscount => AssumptionCategory::Pricing,
            AmbiguityType::DefaultCurrency | AmbiguityType::MissingBillingCountry => {
                AssumptionCategory::Billing
            }
            AmbiguityType::MissingTerm
            | AmbiguityType::MissingStartDate
            | AmbiguityType::DateFormatAmbiguity
            | AmbiguityType::DateDisambiguation => AssumptionCategory::Term,
        };

        let card = AssumptionCard::from_ambiguity(ambiguity, category);
        card_set.add(card);
    }

    card_set
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
                matched_products: vec![
                    "plan-enterprise".to_string(),
                    "addon-enterprise".to_string(),
                ],
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
            options: vec![AmbiguityOption {
                id: "10".to_string(),
                label: "10".to_string(),
                description: None,
                value: serde_json::json!(10),
                confidence: 1.0,
            }],
            field: "quantity".to_string(),
            original_value: None,
        });

        let blocks = render_ambiguity_slack_blocks(&set);
        assert!(!blocks.is_empty());
        assert_eq!(blocks[0]["type"], "header");
    }

    #[test]
    fn assumption_state_has_correct_emojis() {
        assert_eq!(AssumptionState::Confirmed.emoji(), "✅");
        assert_eq!(AssumptionState::Assumed.emoji(), "🤔");
        assert_eq!(AssumptionState::NeedsConfirmation.emoji(), "❓");
    }

    #[test]
    fn assumption_state_has_correct_labels() {
        assert_eq!(AssumptionState::Confirmed.label(), "Confirmed");
        assert_eq!(AssumptionState::Assumed.label(), "Assumed");
        assert_eq!(AssumptionState::NeedsConfirmation.label(), "Needs Confirmation");
    }

    #[test]
    fn creates_confirmed_card() {
        let card = AssumptionCard::confirmed(
            "customer",
            "account_id",
            "Customer",
            "Acme Corp",
            "The customer for this quote",
            AssumptionCategory::Customer,
        );

        assert_eq!(card.state, AssumptionState::Confirmed);
        assert_eq!(card.value, Some("Acme Corp".to_string()));
        assert!(card.editable);
    }

    #[test]
    fn creates_assumed_card() {
        let card = AssumptionCard::assumed(
            "currency",
            "currency",
            "Currency",
            "USD",
            "Using default currency",
            AssumptionCategory::Billing,
        );

        assert_eq!(card.state, AssumptionState::Assumed);
        assert_eq!(card.value, Some("USD".to_string()));
    }

    #[test]
    fn creates_needs_confirmation_card() {
        let options = vec![
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
        ];

        let card = AssumptionCard::needs_confirmation(
            "term",
            "term_months",
            "Contract Term",
            "What term length do you need?",
            options,
            AssumptionCategory::Term,
        );

        assert_eq!(card.state, AssumptionState::NeedsConfirmation);
        assert_eq!(card.value, None);
        assert_eq!(card.options.len(), 2);
    }

    #[test]
    fn assumption_card_set_tracks_counts() {
        let mut set = AssumptionCardSet::new();

        set.add(AssumptionCard::confirmed(
            "c1",
            "f1",
            "Field 1",
            "Value 1",
            "Desc",
            AssumptionCategory::General,
        ));
        set.add(AssumptionCard::assumed(
            "c2",
            "f2",
            "Field 2",
            "Value 2",
            "Desc",
            AssumptionCategory::General,
        ));
        set.add(AssumptionCard::needs_confirmation(
            "c3",
            "f3",
            "Field 3",
            "Desc",
            vec![],
            AssumptionCategory::General,
        ));

        assert_eq!(set.confirmed_count, 1);
        assert_eq!(set.assumed_count, 1);
        assert_eq!(set.needs_confirmation_count, 1);
        assert!(!set.all_confirmed);
    }

    #[test]
    fn all_confirmed_true_when_no_needs_confirmation() {
        let mut set = AssumptionCardSet::new();

        set.add(AssumptionCard::confirmed(
            "c1",
            "f1",
            "Field 1",
            "Value 1",
            "Desc",
            AssumptionCategory::General,
        ));
        set.add(AssumptionCard::confirmed(
            "c2",
            "f2",
            "Field 2",
            "Value 2",
            "Desc",
            AssumptionCategory::General,
        ));

        assert!(set.all_confirmed);
    }

    #[test]
    fn attention_required_sorts_by_priority() {
        let mut set = AssumptionCardSet::new();

        set.add(AssumptionCard::confirmed(
            "c1",
            "f1",
            "Confirmed",
            "Value",
            "Desc",
            AssumptionCategory::General,
        ));
        set.add(AssumptionCard::assumed(
            "c2",
            "f2",
            "Assumed",
            "Value",
            "Desc",
            AssumptionCategory::General,
        ));
        set.add(AssumptionCard::needs_confirmation(
            "c3",
            "f3",
            "Needs Confirm",
            "Desc",
            vec![],
            AssumptionCategory::General,
        ));

        let attention = set.attention_required();
        assert_eq!(attention.len(), 2);
        assert_eq!(attention[0].state, AssumptionState::NeedsConfirmation);
        assert_eq!(attention[1].state, AssumptionState::Assumed);
    }

    #[test]
    fn renders_confirmed_card_slack_blocks() {
        let card = AssumptionCard::confirmed(
            "customer",
            "account_id",
            "Customer",
            "Acme Corp",
            "The customer for this quote",
            AssumptionCategory::Customer,
        );

        let blocks = render_assumption_card_slack_blocks(&card);
        assert!(!blocks.is_empty());
        // First block should be the section with field label and value
        assert_eq!(blocks[0]["type"], "section");
    }

    #[test]
    fn renders_card_set_slack_blocks() {
        let mut set = AssumptionCardSet::new();

        set.add(AssumptionCard::confirmed(
            "c1",
            "f1",
            "Field 1",
            "Value 1",
            "Desc",
            AssumptionCategory::General,
        ));

        let blocks = render_assumption_card_set_slack_blocks(&set);
        assert!(!blocks.is_empty());
        assert_eq!(blocks[0]["type"], "header");
    }

    #[test]
    fn ambiguity_set_to_card_set_conversion() {
        let mut ambiguity_set = AmbiguitySet::new();
        ambiguity_set.add(Ambiguity {
            ambiguity_type: AmbiguityType::CustomerResolution,
            description: "No customer".to_string(),
            clarification_prompt: "Which customer?".to_string(),
            options: vec![],
            field: "account_id".to_string(),
            original_value: None,
        });

        let card_set = ambiguity_set_to_card_set(&ambiguity_set);
        assert_eq!(card_set.len(), 1);
        assert_eq!(card_set.cards[0].category, AssumptionCategory::Customer);
    }

    #[test]
    fn converts_ambiguity_to_card_correctly() {
        let ambiguity = Ambiguity {
            ambiguity_type: AmbiguityType::MissingTerm,
            description: "Term not specified".to_string(),
            clarification_prompt: "What term?".to_string(),
            options: vec![AmbiguityOption {
                id: "12".to_string(),
                label: "12 months".to_string(),
                description: None,
                value: serde_json::json!(12),
                confidence: 1.0,
            }],
            field: "term_months".to_string(),
            original_value: None,
        };

        let card = AssumptionCard::from_ambiguity(&ambiguity, AssumptionCategory::Term);
        assert_eq!(card.state, AssumptionState::NeedsConfirmation);
        assert_eq!(card.field, "term_months");
        assert_eq!(card.category, AssumptionCategory::Term);
    }
}
