# Conversational Constraint Solver (FEAT-02) - Deep Technical Research

**Feature:** Natural Language Configuration  
**Researcher:** ResearchAgent (kimi-k2)  
**Date:** 2026-02-23  
**Priority:** P1

---

## 1. Technical Overview

Transforms quote configuration from form-filling into natural language negotiation. Users express needs conversationally, and the system reasons through constraints to find valid configurations or suggest alternatives.

---

## 2. Natural Language Understanding (NLU) Research

### 2.1 Intent Classification Architecture

```rust
pub enum ConfigurationIntent {
    // Product selection
    AddProduct { product_hint: String, quantity: Option<u32> },
    RemoveProduct { product_hint: String },
    ChangeQuantity { product_hint: String, new_quantity: u32 },
    
    // Constraint exploration
    CheckCompatibility { product_a: String, product_b: String },
    ExploreAlternatives { constraint: String },
    
    // Budget/price negotiation
    SetBudget { amount: Decimal, currency: String },
    RequestDiscount { percentage: Decimal, justification: String },
    
    // Information seeking
    ExplainConstraint { product_hint: String },
    CompareProducts { products: Vec<String> },
    GetRecommendation { requirements: Vec<String> },
    
    // Dialogue control
    Clarify { question: String },
    Confirm { action: String },
    Cancel,
}
```

### 2.2 Entity Extraction

```rust
pub struct ExtractedEntities {
    pub products: Vec<ProductMention>,
    pub quantities: Vec<QuantityMention>,
    pub amounts: Vec<AmountMention>,
    pub timeframes: Vec<TimeframeMention>,
    pub constraints: Vec<ConstraintMention>,
    pub qualifiers: Vec<Qualifier>,
}

pub struct ProductMention {
    pub text: String,
    pub confidence: f64,
    pub resolved_product: Option<ProductId>,
    pub mention_type: MentionType,  // Exact, Partial, Synonym
}

pub struct QuantityMention {
    pub text: String,
    pub value: u32,
    pub unit: QuantityUnit,  // Seats, Licenses, Users
    pub scope: QuantityScope,  // Per product, total, per month
}

pub struct AmountMention {
    pub text: String,
    pub value: Decimal,
    pub currency: String,
    pub amount_type: AmountType,  // Budget, Price, Limit
}
```

### 2.3 Pattern Matching Rules

```rust
pub struct IntentPattern {
    pub pattern: Regex,
    pub intent_type: ConfigurationIntentType,
    pub confidence: f64,
    pub extractors: Vec<Box<dyn EntityExtractor>>,
}

pub const INTENT_PATTERNS: &[IntentPattern] = &[
    // Budget patterns
    IntentPattern {
        pattern: regex!(r"(?i)my budget is (?<amount>[$€£]?[\d,]+(?:k|K)?)"),
        intent_type: ConfigurationIntentType::SetBudget,
        confidence: 0.95,
        extractors: vec![Box::new(AmountExtractor)],
    },
    
    // Product addition patterns
    IntentPattern {
        pattern: regex!(r"(?i)(?:add|include|need|want)(?:\s+a)?\s+(?<product>.+?)(?:\s+(?:for|to))?\s*(?<qty>\d+)?"),
        intent_type: ConfigurationIntentType::AddProduct,
        confidence: 0.90,
        extractors: vec![Box::new(ProductExtractor), Box::new(QuantityExtractor)],
    },
    
    // Compatibility check patterns
    IntentPattern {
        pattern: regex!(r"(?i)(?:can|does)\s+(?<product_a>.+?)\s+work\s+with\s+(?<product_b>.+?)"),
        intent_type: ConfigurationIntentType::CheckCompatibility,
        confidence: 0.88,
        extractors: vec![Box::new(ProductExtractor)],
    },
    
    // Constraint question patterns
    IntentPattern {
        pattern: regex!(r"(?i)(?:why|what).*(?:can't|cannot|won't|unable).*(?:add|use|select)\s+(?<product>.+)"),
        intent_type: ConfigurationIntentType::ExplainConstraint,
        confidence: 0.85,
        extractors: vec![Box::new(ProductExtractor)],
    },
];
```

---

## 3. Dialogue State Management

### 3.1 State Machine Design

```rust
pub enum DialogueState {
    // Initial states
    Idle,
    AwaitingClarification(ClarificationNeeded),
    
    // Product selection flow
    SelectingProducts { candidates: Vec<ProductCandidate> },
    ConfirmingProduct { product: Product, quantity: u32 },
    
    // Constraint resolution flow
    ResolvingConflict { conflict: ConstraintConflict },
    ExploringAlternatives { alternatives: Vec<ConfigurationOption> },
    
    // Budget negotiation flow
    NegotiatingBudget { budget: Decimal, current_quote: Quote },
    SuggestingTradeoffs { options: Vec<TradeoffOption> },
    
    // Completion flow
    ReviewingConfiguration { quote: Quote },
    AwaitingConfirmation { action: String },
}

pub struct DialogueContext {
    pub session_id: SessionId,
    pub current_quote: Option<Quote>,
    pub state: DialogueState,
    pub history: Vec<DialogueTurn>,
    pub extracted_intents: Vec<ConfigurationIntent>,
    pub pending_clarifications: Vec<ClarificationNeeded>,
    pub user_preferences: UserPreferences,
}

pub struct DialogueTurn {
    pub timestamp: DateTime<Utc>,
    pub speaker: Speaker,  // User or System
    pub message: String,
    pub intent: Option<ConfigurationIntent>,
    pub entities: Option<ExtractedEntities>,
}
```

### 3.2 Context Persistence

```sql
-- Dialogue session storage
CREATE TABLE dialogue_sessions (
    id TEXT PRIMARY KEY,
    quote_id TEXT REFERENCES quote(id),
    slack_thread_id TEXT NOT NULL,
    current_state TEXT NOT NULL,
    state_data_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    expires_at TEXT NOT NULL  -- TTL for cleanup
);

CREATE INDEX idx_dialogue_quote ON dialogue_sessions(quote_id);
CREATE INDEX idx_dialogue_thread ON dialogue_sessions(slack_thread_id);

-- Dialogue history for context window
CREATE TABLE dialogue_turns (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES dialogue_sessions(id),
    turn_number INTEGER NOT NULL,
    speaker TEXT NOT NULL,  -- 'user' or 'system'
    message TEXT NOT NULL,
    intent_json TEXT,
    entities_json TEXT,
    timestamp TEXT NOT NULL
);

CREATE INDEX idx_turns_session ON dialogue_turns(session_id, turn_number);
```

---

## 4. Constraint Solver Integration

### 4.1 Intent-to-Constraint Mapping

```rust
pub struct ConstraintMapper {
    catalog: Arc<dyn Catalog>,
    constraint_engine: Arc<dyn ConstraintEngine>,
}

impl ConstraintMapper {
    pub async fn map_intent_to_constraints(
        &self,
        intent: &ConfigurationIntent,
        context: &DialogueContext,
    ) -> Result<ConstraintSet, MappingError> {
        match intent {
            ConfigurationIntent::AddProduct { product_hint, quantity } => {
                // 1. Resolve product mention to actual product
                let product = self.resolve_product(product_hint).await?;
                
                // 2. Build constraint input
                let mut lines = context.current_quote
                    .as_ref()
                    .map(|q| q.lines.clone())
                    .unwrap_or_default();
                
                lines.push(QuoteLine {
                    product_id: product.id.clone(),
                    quantity: quantity.unwrap_or(1),
                    ..Default::default()
                });
                
                // 3. Run constraint validation
                let constraint_input = ConstraintInput {
                    lines: &lines,
                    customer_segment: context.user_preferences.segment.as_deref(),
                };
                
                let result = self.constraint_engine.validate(&constraint_input).await?;
                
                Ok(ConstraintSet {
                    target_configuration: lines,
                    validation_result: result,
                    alternatives: self.generate_alternatives(&result).await?,
                })
            }
            
            ConfigurationIntent::SetBudget { amount, .. } => {
                // 1. Get current quote total
                let current_total = context.current_quote
                    .as_ref()
                    .map(|q| q.calculate_subtotal())
                    .unwrap_or(Decimal::ZERO);
                
                // 2. Check if budget is feasible
                if *amount < current_total {
                    // 3. Generate budget-compliant alternatives
                    let alternatives = self.find_budget_alternatives(
                        context.current_quote.as_ref().unwrap(),
                        *amount,
                    ).await?;
                    
                    Ok(ConstraintSet {
                        target_configuration: vec![],  // Not valid
                        validation_result: ConstraintResult::invalid(
                            "Budget insufficient for current configuration"
                        ),
                        alternatives,
                    })
                } else {
                    Ok(ConstraintSet::valid())
                }
            }
            
            _ => Err(MappingError::UnsupportedIntent),
        }
    }
}
```

### 4.2 Alternative Generation

```rust
pub struct AlternativeGenerator {
    catalog: Arc<dyn Catalog>,
    pricing_engine: Arc<dyn PricingEngine>,
}

impl AlternativeGenerator {
    pub async fn generate_alternatives(
        &self,
        constraint_result: &ConstraintResult,
        current_config: &Quote,
        budget: Option<Decimal>,
    ) -> Result<Vec<ConfigurationOption>, GenerationError> {
        let mut alternatives = Vec::new();
        
        for violation in &constraint_result.violations {
            match violation.violation_type {
                ConstraintViolationType::RequiresProduct { product_id } => {
                    // Suggest adding required product
                    let product = self.catalog.get_product(&product_id).await?;
                    let mut new_config = current_config.clone();
                    new_config.add_line(product_id, 1);
                    
                    alternatives.push(ConfigurationOption {
                        description: format!("Add {} (required)", product.name),
                        quote: new_config,
                        tradeoffs: vec![Tradeoff::AdditionalCost(product.base_price)],
                    });
                }
                
                ConstraintViolationType::ProductConflict { product_a, product_b } => {
                    // Suggest removing one of conflicting products
                    alternatives.push(ConfigurationOption {
                        description: format!("Remove {} (incompatible)", product_a),
                        quote: current_config.without_product(&product_a),
                        tradeoffs: vec![Tradeoff::RemovesFeature(product_a.to_string())],
                    });
                    
                    alternatives.push(ConfigurationOption {
                        description: format!("Remove {} (incompatible)", product_b),
                        quote: current_config.without_product(&product_b),
                        tradeoffs: vec![Tradeoff::RemovesFeature(product_b.to_string())],
                    });
                }
                
                ConstraintViolationType::BudgetExceeded { current, limit } => {
                    // Generate budget-reducing options
                    let savings_needed = current - limit;
                    
                    // Option 1: Reduce quantities
                    if let Some(reduced) = self.reduce_quantities(current_config, savings_needed).await? {
                        alternatives.push(ConfigurationOption {
                            description: "Reduce quantities".to_string(),
                            quote: reduced,
                            tradeoffs: vec![Tradeoff::ReducedCapacity],
                        });
                    }
                    
                    // Option 2: Downgrade products
                    if let Some(downgraded) = self.downgrade_products(current_config, savings_needed).await? {
                        alternatives.push(ConfigurationOption {
                            description: "Downgrade to lower tier".to_string(),
                            quote: downgraded,
                            tradeoffs: vec![Tradeoff::ReducedFeatures],
                        });
                    }
                    
                    // Option 3: Remove optional add-ons
                    if let Some(trimmed) = self.remove_addons(current_config, savings_needed).await? {
                        alternatives.push(ConfigurationOption {
                            description: "Remove optional add-ons".to_string(),
                            quote: trimmed,
                            tradeoffs: vec![Tradeoff::RemovesAddons],
                        });
                    }
                }
                
                _ => {}
            }
        }
        
        // Rank by desirability
        self.rank_alternatives(&mut alternatives).await?;
        
        Ok(alternatives)
    }
}
```

---

## 5. Response Generation

### 5.1 Response Templates

```rust
pub struct ResponseTemplate {
    pub template_type: ResponseType,
    pub content_blocks: Vec<ContentBlock>,
    pub suggested_actions: Vec<SuggestedAction>,
}

pub enum ResponseType {
    Confirmation,
    ClarificationRequest,
    ConstraintViolation,
    AlternativeSuggestion,
    BudgetFeedback,
    Completion,
}

pub struct ResponseGenerator {
    templates: HashMap<ResponseType, Vec<ResponseTemplate>>,
}

impl ResponseGenerator {
    pub fn generate_response(
        &self,
        context: &DialogueContext,
        constraint_set: &ConstraintSet,
    ) -> ResponseTemplate {
        if !constraint_set.validation_result.is_valid() {
            self.generate_violation_response(context, constraint_set)
        } else if !constraint_set.alternatives.is_empty() {
            self.generate_alternative_response(context, constraint_set)
        } else {
            self.generate_confirmation_response(context)
        }
    }
    
    fn generate_violation_response(
        &self,
        context: &DialogueContext,
        constraint_set: &ConstraintSet,
    ) -> ResponseTemplate {
        let violations = &constraint_set.validation_result.violations;
        
        let content = if violations.len() == 1 {
            format!(
                "⚠️ **Configuration Issue**\n\n{}\n\n{}",
                violations[0].message,
                violations[0].suggestion.as_deref().unwrap_or("")
            )
        } else {
            let violation_list = violations.iter()
                .map(|v| format!("• {}", v.message))
                .collect::<Vec<_>>()
                .join("\n");
            
            format!(
                "⚠️ **{} Configuration Issues**\n\n{}\n\nI can suggest some alternatives.",
                violations.len(),
                violation_list
            )
        };
        
        ResponseTemplate {
            template_type: ResponseType::ConstraintViolation,
            content_blocks: vec![ContentBlock::Text(content)],
            suggested_actions: constraint_set.alternatives.iter().take(3)
                .map(|alt| SuggestedAction {
                    label: alt.description.clone(),
                    action: Action::ApplyConfiguration(alt.quote.clone()),
                })
                .collect(),
        }
    }
}
```

### 5.2 Natural Language Generation (NLG)

```rust
pub struct NaturalLanguageGenerator;

impl NaturalLanguageGenerator {
    pub fn explain_constraint(
        &self,
        violation: &ConstraintViolation,
        context: &DialogueContext,
    ) -> String {
        match &violation.violation_type {
            ConstraintViolationType::RequiresProduct { product_id } => {
                let product_name = self.resolve_product_name(product_id);
                let requiring_product = self.find_requiring_product(context, product_id);
                
                format!(
                    "You can't add {} without also including {}. \
                     {} requires {} as a dependency.",
                    requiring_product, product_name, 
                    requiring_product, product_name
                )
            }
            
            ConstraintViolationType::ProductConflict { product_a, product_b } => {
                format!(
                    "{} and {} can't be used together. \
                     These products have incompatible features.",
                    product_a, product_b
                )
            }
            
            ConstraintViolationType::MinimumQuantity { product_id, minimum } => {
                format!(
                    "{} requires a minimum quantity of {} units. \
                     This product has volume-based pricing that kicks in at {} units.",
                    self.resolve_product_name(product_id),
                    minimum, minimum
                )
            }
            
            _ => violation.message.clone(),
        }
    }
    
    pub fn format_alternatives(&self, alternatives: &[ConfigurationOption]) -> String {
        let mut response = "Here are some options:\n\n".to_string();
        
        for (i, alt) in alternatives.iter().enumerate() {
            let total = alt.quote.calculate_total();
            let tradeoffs = alt.tradeoffs.iter()
                .map(|t| format!("• {}", self.format_tradeoff(t)))
                .collect::<Vec<_>>()
                .join("\n  ");
            
            response.push_str(&format!(
                "{}. **{}** - ${}\n  {}\n\n",
                i + 1,
                alt.description,
                total,
                tradeoffs
            ));
        }
        
        response.push_str("Which option would you prefer?");
        response
    }
}
```

---

## 6. Slack Integration

### 6.1 Message Rendering

```rust
pub fn render_conversational_response(
    response: &ResponseTemplate,
    context: &DialogueContext,
) -> MessageTemplate {
    let mut builder = MessageBuilder::new("Configuration Assistant");
    
    // Main content
    for block in &response.content_blocks {
        match block {
            ContentBlock::Text(text) => {
                builder = builder.section("main", |s| s.mrkdwn(text.clone()));
            }
            ContentBlock::QuotePreview(quote) => {
                builder = builder.section("quote", |s| {
                    s.mrkdwn(format_quote_preview(quote))
                });
            }
            ContentBlock::Alternatives(alts) => {
                builder = render_alternatives(builder, alts);
            }
        }
    }
    
    // Suggested actions as buttons
    if !response.suggested_actions.is_empty() {
        builder = builder.actions("actions", |a| {
            for (i, action) in response.suggested_actions.iter().enumerate() {
                a.button(ButtonElement::new(
                    &format!("action_{}", i),
                    &action.label
                ).style(ButtonStyle::Primary));
            }
            a
        });
    }
    
    // Context footer
    builder = builder.context("footer", |c| {
        c.mrkdwn(format!(
            "Quote: {} | Turn: {}",
            context.current_quote.as_ref().map(|q| q.id.0.clone()).unwrap_or_default(),
            context.history.len()
        ))
    });
    
    builder.build()
}

fn render_alternatives(
    builder: MessageBuilder,
    alternatives: &[ConfigurationOption],
) -> MessageBuilder {
    let mut builder = builder;
    
    for (i, alt) in alternatives.iter().take(3).enumerate() {
        let total = alt.quote.calculate_total();
        builder = builder.section(&format!("alt_{}", i), |s| {
            s.mrkdwn(format!(
                "*Option {}: {}*\n• Total: ${}\n• {}",
                i + 1,
                alt.description,
                total,
                alt.tradeoffs.iter()
                    .map(|t| format!("{}", t))
                    .collect::<Vec<_>>()
                    .join("\n• ")
            ))
        });
    }
    
    builder
}
```

### 6.2 Thread Context Management

```rust
pub struct SlackConversationAdapter {
    dialogue_service: Arc<DialogueService>,
    quote_service: Arc<QuoteService>,
}

impl SlackConversationAdapter {
    pub async fn handle_message(
        &self,
        event: &ThreadMessageEvent,
    ) -> Result<MessageTemplate, AdapterError> {
        // 1. Get or create dialogue session
        let session = self.dialogue_service
            .get_or_create_session(&event.thread_ts)
            .await?;
        
        // 2. Get current quote context
        let quote = if let Some(ref quote_id) = session.quote_id {
            self.quote_service.get_quote(quote_id).await?
        } else {
            None
        };
        
        // 3. Process message through dialogue system
        let response = self.dialogue_service
            .process_turn(session.id, &event.text, quote)
            .await?;
        
        // 4. Render response
        Ok(render_conversational_response(&response.template, &response.context))
    }
}
```

---

## 7. Safety and Guardrails

### 7.1 Critical Safety Rules

```rust
pub struct ConversationalSafetyGuardrails;

impl ConversationalSafetyGuardrails {
    /// NEVER allow LLM to set prices directly
    pub fn validate_price_related(&self, intent: &ConfigurationIntent) -> SafetyResult {
        match intent {
            ConfigurationIntent::RequestDiscount { percentage, .. } => {
                if *percentage > Decimal::from(50) {
                    return SafetyResult::Reject(
                        "Discounts over 50% require manual approval".to_string()
                    );
                }
                SafetyResult::AllowWithEngineCheck
            }
            
            ConfigurationIntent::SetBudget { .. } => {
                // Budget is a constraint, not a price - safe
                SafetyResult::Allow
            }
            
            _ => SafetyResult::Allow,
        }
    }
    
    /// ALWAYS validate through constraint engine
    pub fn requires_validation(&self, intent: &ConfigurationIntent) -> bool {
        matches!(intent,
            ConfigurationIntent::AddProduct { .. } |
            ConfigurationIntent::RemoveProduct { .. } |
            ConfigurationIntent::ChangeQuantity { .. }
        )
    }
    
    /// FALLBACK to forms when confidence is low
    pub fn should_use_fallback(&self, extraction: &ExtractionResult) -> bool {
        extraction.confidence < 0.7 || 
        extraction.ambiguous_entities.len() > 2
    }
}
```

### 7.2 Confidence Thresholds

```rust
pub struct ConfidenceConfig {
    /// Minimum confidence for intent classification
    pub intent_threshold: f64,  // 0.85
    
    /// Minimum confidence for entity resolution
    pub entity_threshold: f64,  // 0.80
    
    /// Minimum confidence for product matching
    pub product_match_threshold: f64,  // 0.90
    
    /// Threshold for triggering clarification
    pub clarification_threshold: f64,  // 0.70
}
```

---

## 8. Testing Strategy

### 8.1 Intent Recognition Tests

```rust
#[test]
fn recognizes_add_product_intent() {
    let test_cases = vec![
        ("I need the enterprise plan", "enterprise plan", None),
        ("Add 5 seats of pro", "pro", Some(5)),
        ("Can I get premium support?", "premium support", None),
        ("Include API access for 100 users", "API access", Some(100)),
    ];
    
    for (input, expected_product, expected_qty) in test_cases {
        let result = IntentExtractor::extract(input);
        assert!(matches!(result.intent, ConfigurationIntent::AddProduct { .. }));
    }
}

#[test]
fn recognizes_budget_constraint() {
    let test_cases = vec![
        ("My budget is $50k", 50000),
        ("I can spend up to 100,000", 100000),
        ("Max budget: 75k", 75000),
    ];
    
    for (input, expected_amount) in test_cases {
        let result = IntentExtractor::extract(input);
        match result.intent {
            ConfigurationIntent::SetBudget { amount, .. } => {
                assert_eq!(amount, Decimal::from(expected_amount));
            }
            _ => panic!("Expected SetBudget intent"),
        }
    }
}
```

### 8.2 Dialogue Flow Tests

```rust
#[tokio::test]
async fn test_multi_turn_configuration() {
    let mut dialogue = DialogueSession::new();
    
    // Turn 1: User expresses intent
    let response1 = dialogue.process("I need enterprise tier").await.unwrap();
    assert!(response1.contains("How many seats"));
    
    // Turn 2: User provides quantity
    let response2 = dialogue.process("150 seats").await.unwrap();
    assert!(response2.contains("Enterprise"));
    assert!(response2.contains("150"));
    
    // Turn 3: User adds budget constraint
    let response3 = dialogue.process("My budget is $40k").await.unwrap();
    assert!(response3.contains("budget") || response3.contains("alternative"));
}
```

### 8.3 Constraint Validation Tests

```rust
#[tokio::test]
async fn test_conflict_detection_in_dialogue() {
    let mut dialogue = DialogueSession::new();
    
    // Add product A
    dialogue.process("Add product A").await.unwrap();
    
    // Try to add conflicting product B
    let response = dialogue.process("Also add product B").await.unwrap();
    
    // Should detect conflict and explain
    assert!(response.contains("can't") || response.contains("incompatible"));
    assert!(response.contains("alternative") || response.contains("option"));
}
```

---

## 9. Performance Considerations

### 9.1 Response Time Targets

| Operation | Target | Notes |
|-----------|--------|-------|
| Intent extraction | <50ms | Pattern matching |
| Entity resolution | <100ms | Product lookup |
| Constraint validation | <200ms | Engine call |
| Alternative generation | <300ms | Multiple options |
| Total response | <800ms | End-to-end |

### 9.2 Optimization Strategies

```rust
// Pre-compiled regex patterns
lazy_static! {
    static ref INTENT_PATTERNS: Vec<IntentPattern> = {
        compile_patterns()
    };
}

// Cached product aliases
pub struct ProductAliasCache {
    aliases: RwLock<HashMap<String, ProductId>>,
}

// Connection pooling for constraint engine
pub struct ConstraintEnginePool {
    pool: deadpool::managed::Pool<ConstraintEngineManager>,
}
```

---

## 10. Integration with Existing CPQ

### 10.1 Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    Slack Interface                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   Command    │  │   Thread     │  │  Interactive │      │
│  │   Handler    │  │   Handler    │  │  Components  │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
└─────────┼─────────────────┼─────────────────┼──────────────┘
          │                 │                 │
          └─────────────────┼─────────────────┘
                            │
┌───────────────────────────▼───────────────────────────────┐
│              Conversational Constraint Solver               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   Natural    │  │   Dialogue   │  │   Response   │     │
│  │  Language    │  │    State     │  │  Generator   │     │
│  │  Understanding│  │   Manager    │  │              │     │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘     │
└─────────┼─────────────────┼─────────────────┼─────────────┘
          │                 │                 │
          │    ┌────────────┘                 │
          │    │                              │
          ▼    ▼                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     CPQ Core                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  Constraint  │  │   Pricing    │  │    Policy    │      │
│  │    Engine    │  │    Engine    │  │    Engine    │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

### 10.2 Data Flow

1. User sends message in Slack thread
2. NLU extracts intent and entities
3. Dialogue manager updates state
4. Intent mapper converts to constraint operations
5. Constraint engine validates configuration
6. Alternative generator creates options if needed
7. Response generator formats output
8. Slack renderer creates Block Kit message

---

## 11. References

1. **Rasa NLU**: Open-source NLU framework - rasa.com
2. **Dialogue State Tracking**: Williams & Young (2007) "Partially Observable Markov Decision Processes"
3. **Intent Classification**: Liu (2019) "BERT for Joint Intent Classification"
4. **Constraint Satisfaction**: Rossi et al. (2006) "Handbook of Constraint Programming"
5. **NLG**: Reiter & Dale (2000) "Building Natural Language Generation Systems"

---

*Research compiled by ResearchAgent for the quotey project.*
