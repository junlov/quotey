# Explainable Policy Engine (FEAT-05) - Deep Technical Research

**Feature:** Policy Violation Explanations  
**Researcher:** ResearchAgent (kimi-k2)  
**Date:** 2026-02-23  
**Priority:** P1

---

## 1. Technical Overview

Provides instant plain-English explanations for every rejected discount or configuration conflict, with cited rules and actionable resolution paths. Transforms opaque policy violations into actionable guidance.

---

## 2. Explanation Architecture

### 2.1 Core Components

```rust
pub struct ExplainablePolicyEngine {
    rule_registry: Arc<dyn PolicyRuleRegistry>,
    explanation_generator: Arc<dyn ExplanationGenerator>,
    resolution_finder: Arc<dyn ResolutionFinder>,
    citation_formatter: Arc<dyn CitationFormatter>,
}

pub struct PolicyExplanation {
    pub summary: String,
    pub citations: Vec<PolicyCitation>,
    pub technical_details: TechnicalDetails,
    pub resolution_paths: Vec<ResolutionPath>,
    pub documentation_links: Vec<DocLink>,
}

pub struct PolicyCitation {
    pub rule_id: String,
    pub rule_name: String,
    pub rule_version: String,
    pub policy_section: String,
    pub direct_quote: String,
    pub effective_date: DateTime<Utc>,
}
```

### 2.2 Explanation Generation Flow

```rust
impl ExplainablePolicyEngine {
    pub async fn explain_violation(
        &self,
        violation: &PolicyViolation,
        context: &ExplanationContext,
    ) -> Result<PolicyExplanation, ExplanationError> {
        // 1. Load rule details
        let rule = self.rule_registry
            .get_rule(&violation.rule_id)
            .await?
            .ok_or(ExplanationError::RuleNotFound)?;
        
        // 2. Generate explanation
        let explanation = self.explanation_generator
            .generate(&rule, violation, context)
            .await?;
        
        // 3. Find resolution paths
        let resolutions = self.resolution_finder
            .find_paths(violation, context)
            .await?;
        
        // 4. Format citations
        let citations = self.citation_formatter
            .format(&rule, &context.user_role)
            .await?;
        
        Ok(PolicyExplanation {
            summary: explanation.summary,
            citations,
            technical_details: explanation.technical_details,
            resolution_paths: resolutions,
            documentation_links: rule.documentation_links,
        })
    }
}
```

---

## 3. Policy Rule Schema

### 3.1 Rule Definition

```sql
CREATE TABLE policy_rules (
    id TEXT PRIMARY KEY,
    rule_code TEXT NOT NULL UNIQUE,  -- e.g., "DISCOUNT_MGR_MAX"
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    
    -- Classification
    category TEXT NOT NULL,  -- 'pricing', 'configuration', 'approval'
    severity TEXT NOT NULL,  -- 'info', 'warning', 'blocking'
    
    -- Condition (stored as JSON for flexibility)
    condition_json TEXT NOT NULL,
    -- Example: {"type": "discount_threshold", "operator": ">", "value": 20}
    
    -- Explanations (role-based)
    default_explanation TEXT NOT NULL,
    explanation_templates_json TEXT,  -- {"rep": "...", "manager": "...", "vp": "..."}
    
    -- Resolution hints
    resolution_hints_json TEXT,  -- ["Reduce discount", "Request escalation"]
    auto_resolution_available BOOLEAN DEFAULT FALSE,
    auto_resolution_action TEXT,
    
    -- Documentation
    documentation_url TEXT,
    policy_version TEXT NOT NULL,
    effective_from TEXT NOT NULL,
    effective_until TEXT,  -- NULL = currently effective
    
    -- Metadata
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    active BOOLEAN DEFAULT TRUE
);

CREATE INDEX idx_policy_rules_category ON policy_rules(category, active);
CREATE INDEX idx_policy_rules_severity ON policy_rules(severity, active);
CREATE INDEX idx_policy_rules_code ON policy_rules(rule_code, active);

-- Rule parameter definitions (for dynamic values)
CREATE TABLE policy_rule_parameters (
    id TEXT PRIMARY KEY,
    rule_id TEXT NOT NULL REFERENCES policy_rules(id),
    param_name TEXT NOT NULL,
    param_type TEXT NOT NULL,  -- 'string', 'number', 'percentage', 'currency'
    default_value TEXT,
    description TEXT,
    
    UNIQUE(rule_id, param_name)
);
```

### 3.2 Rule Condition DSL

```rust
pub enum PolicyCondition {
    // Comparison conditions
    GreaterThan { field: String, value: Decimal },
    LessThan { field: String, value: Decimal },
    Equals { field: String, value: String },
    InRange { field: String, min: Decimal, max: Decimal },
    
    // Set conditions
    Contains { field: String, values: Vec<String> },
    Excludes { field: String, values: Vec<String> },
    
    // Logical combinators
    And(Vec<PolicyCondition>),
    Or(Vec<PolicyCondition>),
    Not(Box<PolicyCondition>),
    
    // Context conditions
    UserRole(Vec<String>),
    AccountTier(Vec<String>),
    TimeWindow { start: Time, end: Time },
    
    // Custom (plugin)
    Custom { plugin_id: String, config: Value },
}

impl PolicyCondition {
    pub fn evaluate(&self, context: &EvaluationContext) -> bool {
        match self {
            PolicyCondition::GreaterThan { field, value } => {
                context.get_decimal(field).map_or(false, |v| v > *value)
            }
            PolicyCondition::And(conditions) => {
                conditions.iter().all(|c| c.evaluate(context))
            }
            PolicyCondition::Or(conditions) => {
                conditions.iter().any(|c| c.evaluate(context))
            }
            // ... other variants
        }
    }
}
```

---

## 4. Explanation Templates

### 4.1 Template System

```rust
pub struct ExplanationTemplate {
    pub template_id: String,
    pub template_text: String,
    pub variables: Vec<TemplateVariable>,
    pub role_overrides: HashMap<String, String>,
}

pub struct TemplateVariable {
    pub name: String,
    pub source: VariableSource,
    pub formatter: Box<dyn ValueFormatter>,
}

pub enum VariableSource {
    ViolationField(String),
    RuleParameter(String),
    ContextField(String),
    ComputedValue(String),
}

// Example templates
const DEFAULT_TEMPLATES: &[ExplanationTemplate] = &[
    ExplanationTemplate {
        template_id: "discount_threshold_exceeded".to_string(),
        template_text: 
            "Your requested discount of {{requested_discount}}% exceeds the {{role}} limit of {{max_discount}}%. "
            .to_string(),
        variables: vec![
            TemplateVariable {
                name: "requested_discount".to_string(),
                source: VariableSource::ViolationField("actual_value".to_string()),
                formatter: Box::new(PercentageFormatter),
            },
            TemplateVariable {
                name: "max_discount".to_string(),
                source: VariableSource::RuleParameter("max_discount_pct".to_string()),
                formatter: Box::new(PercentageFormatter),
            },
            TemplateVariable {
                name: "role".to_string(),
                source: VariableSource::ContextField("user_role".to_string()),
                formatter: Box::new(RoleFormatter),
            },
        ],
        role_overrides: hashmap! {
            "vp".to_string() => "Your discount exceeds standard policy but is within your VP authority.".to_string(),
        },
    },
];
```

### 4.2 Template Rendering

```rust
pub struct TemplateRenderer;

impl TemplateRenderer {
    pub fn render(
        &self,
        template: &ExplanationTemplate,
        violation: &PolicyViolation,
        rule: &PolicyRule,
        context: &ExplanationContext,
    ) -> Result<String, RenderError> {
        // Check for role override
        let template_text = context.user_role
            .as_ref()
            .and_then(|role| template.role_overrides.get(role))
            .cloned()
            .unwrap_or_else(|| template.template_text.clone());
        
        // Build variable values
        let mut values = HashMap::new();
        for var in &template.variables {
            let value = self.resolve_variable(var, violation, rule, context)?;
            values.insert(var.name.clone(), value);
        }
        
        // Simple template substitution
        let mut result = template_text;
        for (key, value) in values {
            result = result.replace(&format!("{{{{{}}}}}", key), &value);
        }
        
        Ok(result)
    }
    
    fn resolve_variable(
        &self,
        var: &TemplateVariable,
        violation: &PolicyViolation,
        rule: &PolicyRule,
        context: &ExplanationContext,
    ) -> Result<String, ResolveError> {
        let raw_value = match &var.source {
            VariableSource::ViolationField(field) => {
                violation.metadata.get(field).cloned()
                    .ok_or(ResolveError::FieldNotFound(field.clone()))?
            }
            VariableSource::RuleParameter(param) => {
                rule.parameters.get(param).cloned()
                    .ok_or(ResolveError::ParameterNotFound(param.clone()))?
            }
            VariableSource::ContextField(field) => {
                context.get_field(field).ok_or(ResolveError::ContextNotFound(field.clone()))?
            }
            VariableSource::ComputedValue(computation) => {
                self.compute_value(computation, violation, rule, context)?
            }
        };
        
        Ok(var.formatter.format(&raw_value))
    }
}
```

---

## 5. Resolution Path Finding

### 5.1 Resolution Engine

```rust
pub struct ResolutionFinder {
    constraint_engine: Arc<dyn ConstraintEngine>,
    pricing_engine: Arc<dyn PricingEngine>,
    policy_engine: Arc<dyn PolicyEngine>,
}

impl ResolutionFinder {
    pub async fn find_paths(
        &self,
        violation: &PolicyViolation,
        context: &ExplanationContext,
    ) -> Result<Vec<ResolutionPath>, ResolutionError> {
        let mut paths = Vec::new();
        
        match &violation.violation_type {
            PolicyViolationType::DiscountExceeded { current, limit } => {
                // Path 1: Reduce to threshold
                paths.push(ResolutionPath {
                    description: format!("Reduce discount to {}%", limit),
                    action: ResolutionAction::AdjustDiscount { new_pct: *limit },
                    effort: EffortLevel::Low,
                    impact: ImpactLevel::Low,
                    auto_applicable: true,
                });
                
                // Path 2: Request escalation
                let next_approver = self.find_next_approver(*current).await?;
                paths.push(ResolutionPath {
                    description: format!("Request {} approval", next_approver),
                    action: ResolutionAction::RequestEscalation { approver_role: next_approver },
                    effort: EffortLevel::Medium,
                    impact: ImpactLevel::None,
                    auto_applicable: false,
                });
                
                // Path 3: Modify quote to qualify for higher discount
                if let Some(qualifying_changes) = self.find_qualifying_changes(context).await? {
                    paths.push(ResolutionPath {
                        description: "Modify quote terms for automatic approval".to_string(),
                        action: ResolutionAction::ApplyChanges { changes: qualifying_changes },
                        effort: EffortLevel::High,
                        impact: ImpactLevel::Medium,
                        auto_applicable: false,
                    });
                }
            }
            
            PolicyViolationType::MissingRequiredProduct { product_id } => {
                let product = self.catalog.get_product(product_id).await?;
                paths.push(ResolutionPath {
                    description: format!("Add {} (required)", product.name),
                    action: ResolutionAction::AddProduct { product_id: product_id.clone() },
                    effort: EffortLevel::Low,
                    impact: ImpactLevel::Medium,
                    auto_applicable: true,
                });
            }
            
            PolicyViolationType::ProductConflict { product_a, product_b } => {
                paths.push(ResolutionPath {
                    description: format!("Remove {}", product_a),
                    action: ResolutionAction::RemoveProduct { product_id: product_a.clone() },
                    effort: EffortLevel::Low,
                    impact: ImpactLevel::Low,
                    auto_applicable: true,
                });
                paths.push(ResolutionPath {
                    description: format!("Remove {}", product_b),
                    action: ResolutionAction::RemoveProduct { product_id: product_b.clone() },
                    effort: EffortLevel::Low,
                    impact: ImpactLevel::Low,
                    auto_applicable: true,
                });
            }
            
            _ => {}
        }
        
        // Rank by effort/impact ratio
        paths.sort_by(|a, b| {
            let score_a = Self::score_path(a);
            let score_b = Self::score_path(b);
            score_b.partial_cmp(&score_a).unwrap()
        });
        
        Ok(paths)
    }
    
    fn score_path(path: &ResolutionPath) -> f64 {
        let effort_score = match path.effort {
            EffortLevel::Low => 3.0,
            EffortLevel::Medium => 2.0,
            EffortLevel::High => 1.0,
        };
        let impact_score = match path.impact {
            ImpactLevel::None => 3.0,
            ImpactLevel::Low => 2.0,
            ImpactLevel::Medium => 1.0,
            ImpactLevel::High => 0.5,
        };
        effort_score * impact_score
    }
}
```

---

## 6. Slack Integration

### 6.1 Explanation Card Rendering

```rust
pub fn render_explanation_card(
    explanation: &PolicyExplanation,
    context: &ExplanationContext,
) -> MessageTemplate {
    let mut builder = MessageBuilder::new("Policy Explanation");
    
    // Header with violation icon
    builder = builder.section("header", |s| {
        s.mrkdwn(format!(
            "❌ **{}**\n\n{}",
            explanation.violation_type,
            explanation.summary
        ))
    });
    
    // Rule citation
    if !explanation.citations.is_empty() {
        let citation = &explanation.citations[0];
        builder = builder.section("citation", |s| {
            s.mrkdwn(format!(
                "*Policy Reference:* {} ({})\n> {}",
                citation.rule_name,
                citation.rule_code,
                citation.direct_quote
            ))
        });
    }
    
    // Technical details (collapsible)
    if let Some(ref details) = explanation.technical_details {
        builder = builder.section("details", |s| {
            s.mrkdwn(format!(
                "*Details:*\n• Current: {}\n• Threshold: {}\n• Difference: {}",
                details.current_value,
                details.threshold_value,
                details.difference
            ))
        });
    }
    
    // Resolution paths
    if !explanation.resolution_paths.is_empty() {
        builder = builder.divider();
        builder = builder.section("resolutions_header", |s| {
            s.mrkdwn("*Resolution Options:*".to_string())
        });
        
        for (i, path) in explanation.resolution_paths.iter().take(3).enumerate() {
            let effort_emoji = match path.effort {
                EffortLevel::Low => "🟢",
                EffortLevel::Medium => "🟡",
                EffortLevel::High => "🔴",
            };
            
            builder = builder.section(&format!("path_{}", i), |s| {
                s.mrkdwn(format!(
                    "{}. {} {}\n   Impact: {} | Auto: {}",
                    i + 1,
                    effort_emoji,
                    path.description,
                    path.impact,
                    if path.auto_applicable { "✓" } else { "✗" }
                ))
            });
        }
        
        // Action buttons
        builder = builder.actions("resolution_actions", |a| {
            for (i, path) in explanation.resolution_paths.iter().take(3).enumerate() {
                if path.auto_applicable {
                    a.button(
                        ButtonElement::new(&format!("apply_{}", i), &format!("Apply Option {}", i + 1))
                            .style(ButtonStyle::Primary)
                    );
                }
            }
            a
        });
    }
    
    // Documentation links
    if !explanation.documentation_links.is_empty() {
        let links = explanation.documentation_links.iter()
            .map(|l| format!("<{}|{}>", l.url, l.title))
            .collect::<Vec<_>>()
            .join(" | ");
        
        builder = builder.context("docs", |c| {
            c.mrkdwn(format!("📚 {}", links))
        });
    }
    
    builder.build()
}
```

---

## 7. Performance Optimization

### 7.1 Caching Strategy

```rust
pub struct ExplanationCache {
    rule_templates: LruCache<String, ExplanationTemplate>,
    rendered_explanations: LruCache<String, String>,  // hash → rendered
}

impl ExplanationCache {
    pub async fn get_or_render(
        &mut self,
        violation_hash: &str,
        render_fn: impl FnOnce() -> Future<Output = Result<String, RenderError>>,
    ) -> Result<String, RenderError> {
        if let Some(cached) = self.rendered_explanations.get(violation_hash) {
            return Ok(cached.clone());
        }
        
        let rendered = render_fn().await?;
        self.rendered_explanations.put(violation_hash.to_string(), rendered.clone());
        Ok(rendered)
    }
}
```

### 7.2 Pre-computation

```rust
pub struct PolicyPrecomputer {
    policy_engine: Arc<dyn PolicyEngine>,
}

impl PolicyPrecomputer {
    /// Pre-compute explanations for common violation patterns
    pub async fn precompute_common_explanations(&self) -> Result<(), PrecomputeError> {
        let common_scenarios = vec![
            ("discount_20_25", test_context_with_discount(dec!(22))),
            ("discount_25_30", test_context_with_discount(dec!(27))),
            ("missing_enterprise_api", test_context_without_product("enterprise_api")),
        ];
        
        for (name, context) in common_scenarios {
            let result = self.policy_engine.evaluate(&context).await?;
            if let Some(violation) = result.violations.first() {
                let explanation = self.explanation_generator
                    .generate(violation, &context)
                    .await?;
                self.cache.store(&format!("precomp:{}", name), explanation).await?;
            }
        }
        
        Ok(())
    }
}
```

---

## 8. Testing

### 8.1 Template Tests

```rust
#[test]
fn renders_discount_explanation() {
    let template = ExplanationTemplate {
        template_id: "test".to_string(),
        template_text: "Discount {{requested}}% exceeds {{limit}}%".to_string(),
        variables: vec![
            TemplateVariable {
                name: "requested".to_string(),
                source: VariableSource::ViolationField("actual".to_string()),
                formatter: Box::new(PercentageFormatter),
            },
            TemplateVariable {
                name: "limit".to_string(),
                source: VariableSource::RuleParameter("max".to_string()),
                formatter: Box::new(PercentageFormatter),
            },
        ],
        role_overrides: HashMap::new(),
    };
    
    let violation = PolicyViolation {
        rule_id: "TEST".to_string(),
        metadata: hashmap! { "actual".to_string() => "25".to_string() },
    };
    
    let rule = PolicyRule {
        parameters: hashmap! { "max".to_string() => "20".to_string() },
    };
    
    let renderer = TemplateRenderer;
    let result = renderer.render(&template, &violation, &rule, &context);
    
    assert_eq!(result.unwrap(), "Discount 25% exceeds 20%");
}
```

### 8.2 Resolution Tests

```rust
#[tokio::test]
async fn finds_discount_resolution_paths() {
    let finder = ResolutionFinder::new(test_engines());
    
    let violation = PolicyViolation {
        violation_type: PolicyViolationType::DiscountExceeded {
            current: dec!(25),
            limit: dec!(20),
        },
    };
    
    let paths = finder.find_paths(&violation, &test_context()).await.unwrap();
    
    assert_eq!(paths.len(), 2);
    assert!(paths.iter().any(|p| matches!(p.action, ResolutionAction::AdjustDiscount { .. })));
    assert!(paths.iter().any(|p| matches!(p.action, ResolutionAction::RequestEscalation { .. })));
}
```

---

## 9. Metrics

| Metric | Target |
|--------|--------|
| Explanation generation latency | <50ms |
| Cache hit rate | >80% |
| Resolution path acceptance | >60% |
| User comprehension score | >4.0/5 |
| Policy lookup latency | <10ms |

---

*Research compiled by ResearchAgent for the quotey project.*
