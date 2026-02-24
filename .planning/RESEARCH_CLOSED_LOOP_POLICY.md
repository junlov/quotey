# Closed-Loop Policy Optimizer (CLO) - Technical Research

**Feature:** Auto-Generated Policy Suggestions  
**Researcher:** ResearchAgent (kimi-k2)  
**Date:** 2026-02-23  
**Priority:** P2

---

## 1. Technical Overview

Learning loop for policy refinement. System suggests policy improvements based on actual outcomes and feedback patterns.

---

## 2. Learning Pipeline

### 2.1 Outcome Collection

```rust
pub struct OutcomeCollector {
    event_store: Arc<dyn EventStore>,
}

pub struct ApprovalOutcome {
    pub approval_id: String,
    pub quote_id: String,
    pub policy_version: String,
    pub request: ApprovalRequest,
    pub decision: ApprovalDecision,
    pub reviewer_id: String,
    pub reviewed_at: DateTime<Utc>,
    pub feedback: Option<ReviewerFeedback>,
}

pub struct ReviewerFeedback {
    pub difficulty_score: u8,  // 1-5
    pub was_obvious: bool,
    pub would_auto_approve: bool,
    pub notes: Option<String>,
}

pub struct QuoteOutcome {
    pub quote_id: String,
    pub final_status: QuoteStatus,
    pub final_margin: Decimal,
    pub days_to_close: u32,
    pub discount_concessions: Vec<DiscountConcession>,
    
}

pub struct DiscountConcession {
    pub requested_at: DateTime<Utc>,
    pub initial_discount_pct: Decimal,
    pub final_discount_pct: Decimal,
    pub rounds: u32,
    pub approved_by_override: bool,
}

impl OutcomeCollector {
    pub fn collect_approval_outcomes(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<ApprovalOutcome> {
        // Query event store for approval events
        let events = self.event_store.query(
            EventQuery {
                event_types: vec!["approval.requested", "approval.decided"],
                from: start,
                to: end,
            }
        );
        
        // Correlate and build outcomes
        events.chunks(2)
            .filter_map(|chunk| self.correlate_approval_pair(chunk))
            .collect()
    }
}
```

### 2.2 Pattern Detection

```rust
pub struct PatternDetector;

pub enum PolicyPattern {
    FalsePositive { count: u32, examples: Vec<String> },
    FalseNegative { count: u32, examples: Vec<String> },
    Inconsistent { count: u32, variance: f64 },
    Bottleneck { avg_delay_hours: f64, count: u32 },
    AutoApprovable { confidence: f32, count: u32 },
}

impl PatternDetector {
    pub fn detect_patterns(&self, outcomes: &[ApprovalOutcome]) -> Vec<PolicyPattern> {
        let mut patterns = Vec::new();
        
        // False positives: Approved but later had issues
        let false_positives: Vec<_> = outcomes.iter()
            .filter(|o| {
                o.decision == ApprovalDecision::Approved &&
                o.feedback.as_ref().map(|f| !f.would_auto_approve).unwrap_or(false)
            })
            .collect();
        
        if false_positives.len() >= 3 {
            patterns.push(PolicyPattern::FalsePositive {
                count: false_positives.len() as u32,
                examples: false_positives.iter().map(|o| o.approval_id.clone()).collect(),
            });
        }
        
        // False negatives: Rejected but should have been approved
        let false_negatives: Vec<_> = outcomes.iter()
            .filter(|o| {
                o.decision == ApprovalDecision::Rejected &&
                o.feedback.as_ref().map(|f| f.would_auto_approve).unwrap_or(false)
            })
            .collect();
        
        if false_negatives.len() >= 3 {
            patterns.push(PolicyPattern::FalseNegative {
                count: false_negatives.len() as u32,
                examples: false_negatives.iter().map(|o| o.approval_id.clone()).collect(),
            });
        }
        
        // Bottlenecks: Approvals taking too long
        let delays: Vec<_> = outcomes.iter()
            .filter(|o| {
                let review_duration = o.reviewed_at - o.request.requested_at;
                review_duration.num_hours() > 24
            })
            .collect();
        
        if !delays.is_empty() {
            let avg_delay = delays.iter()
                .map(|o| (o.reviewed_at - o.request.requested_at).num_hours() as f64)
                .sum::<f64>() / delays.len() as f64;
            
            patterns.push(PolicyPattern::Bottleneck {
                avg_delay_hours: avg_delay,
                count: delays.len() as u32,
            });
        }
        
        // Auto-approvable: High confidence patterns that could be automated
        let auto_approvable = self.detect_auto_approvable_patterns(outcomes);
        if auto_approvable.confidence > 0.9 {
            patterns.push(PolicyPattern::AutoApprovable {
                confidence: auto_approvable.confidence,
                count: auto_approvable.count,
            });
        }
        
        patterns
    }
    
    fn detect_auto_approvable_patterns(&self, outcomes: &[ApprovalOutcome]) -> AutoApprovablePattern {
        // Cluster approved outcomes by characteristics
        let approved = outcomes.iter()
            .filter(|o| o.decision == ApprovalDecision::Approved)
            .collect::<Vec<_>>();
        
        // Simple clustering: same discount tier, same segment, no issues
        let clusters = self.cluster_by_characteristics(&approved);
        
        // Find clusters with 100% approval rate
        let high_confidence_clusters: Vec<_> = clusters.iter()
            .filter(|c| c.approval_rate > 0.95 && c.count >= 5)
            .collect();
        
        if let Some(best) = high_confidence_clusters.first() {
            AutoApprovablePattern {
                confidence: best.approval_rate,
                count: best.count,
                characteristics: best.characteristics.clone(),
            }
        } else {
            AutoApprovablePattern {
                confidence: 0.0,
                count: 0,
                characteristics: vec![],
            }
        }
    }
}

pub struct AutoApprovablePattern {
    pub confidence: f32,
    pub count: u32,
    pub characteristics: Vec<String>,
}
```

---

## 3. Policy Suggestion Generator

```rust
pub struct PolicySuggestionGenerator {
    detector: PatternDetector,
}

pub struct PolicySuggestion {
    pub suggestion_id: String,
    pub suggestion_type: SuggestionType,
    pub confidence: f32,
    pub description: String,
    pub current_policy: PolicyRule,
    pub proposed_change: PolicyChange,
    pub expected_impact: ExpectedImpact,
    pub evidence: Vec<EvidenceItem>,
}

pub enum SuggestionType {
    RelaxConstraint,
    TightenConstraint,
    AddException,
    Automate,
    RequireAdditionalReview,
}

pub struct PolicyChange {
    pub field: String,
    pub old_value: String,
    pub new_value: String,
    pub diff_description: String,
}

pub struct ExpectedImpact {
    pub false_positive_reduction: Option<f32>,
    pub false_negative_reduction: Option<f32>,
    pub throughput_improvement: Option<f32>,
    pub risk_change: RiskAssessment,
}

pub enum RiskAssessment {
    Decreased,
    Neutral,
    Increased { magnitude: RiskMagnitude },
}

pub enum RiskMagnitude {
    Low,
    Medium,
    High,
}

impl PolicySuggestionGenerator {
    pub fn generate_suggestions(
        &self,
        outcomes: &[ApprovalOutcome],
        current_policy: &PolicySet,
    ) -> Vec<PolicySuggestion> {
        let patterns = self.detector.detect_patterns(outcomes);
        let mut suggestions = Vec::new();
        
        for pattern in patterns {
            match pattern {
                PolicyPattern::FalsePositive { count, examples } => {
                    suggestions.push(self.suggest_constraint_tightening(
                        current_policy,
                        &examples,
                        count,
                    ));
                }
                PolicyPattern::FalseNegative { count, examples } => {
                    suggestions.push(self.suggest_constraint_relaxation(
                        current_policy,
                        &examples,
                        count,
                    ));
                }
                PolicyPattern::Bottleneck { avg_delay_hours, count } => {
                    suggestions.push(self.suggest_throughput_improvement(
                        current_policy,
                        avg_delay_hours,
                        count,
                    ));
                }
                PolicyPattern::AutoApprovable { confidence, count } => {
                    suggestions.push(self.suggest_automation(
                        current_policy,
                        confidence,
                        count,
                    ));
                }
                _ => {}
            }
        }
        
        // Sort by confidence
        suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        suggestions
    }
    
    fn suggest_constraint_relaxation(
        &self,
        policy: &PolicySet,
        examples: &[String],
        count: u32,
    ) -> PolicySuggestion {
        // Analyze common characteristics of false negatives
        let common_discount = self.find_common_discount(examples);
        
        PolicySuggestion {
            suggestion_id: format!("RELAX-{}", Uuid::new_v4()),
            suggestion_type: SuggestionType::RelaxConstraint,
            confidence: (count as f32 / 10.0).min(0.95),
            description: format!(
                "Increase discount threshold based on {} rejected-but-valid requests",
                count
            ),
            current_policy: policy.rules["max_discount"].clone(),
            proposed_change: PolicyChange {
                field: "max_discount".to_string(),
                old_value: "20%".to_string(),
                new_value: "25%".to_string(),
                diff_description: "Increase from 20% to 25%".to_string(),
            },
            expected_impact: ExpectedImpact {
                false_negative_reduction: Some(0.8),
                false_positive_reduction: None,
                throughput_improvement: Some(0.3),
                risk_change: RiskAssessment::Neutral,
            },
            evidence: examples.iter().take(3).map(|e| EvidenceItem {
                quote_id: e.clone(),
                observation: "Should have been auto-approved".to_string(),
            }).collect(),
        }
    }
    
    fn suggest_automation(
        &self,
        policy: &PolicySet,
        confidence: f32,
        count: u32,
    ) -> PolicySuggestion {
        PolicySuggestion {
            suggestion_id: format!("AUTO-{}", Uuid::new_v4()),
            suggestion_type: SuggestionType::Automate,
            confidence,
            description: format!(
                "Enable auto-approval for {} requests with 100% historical approval rate",
                count
            ),
            current_policy: policy.rules["approval_workflow"].clone(),
            proposed_change: PolicyChange {
                field: "auto_approve_rules".to_string(),
                old_value: "None".to_string(),
                new_value: "High confidence patterns".to_string(),
                diff_description: "Add auto-approval rules for known good patterns".to_string(),
            },
            expected_impact: ExpectedImpact {
                false_negative_reduction: None,
                false_positive_reduction: None,
                throughput_improvement: Some(0.5),
                risk_change: RiskAssessment::Neutral,
            },
            evidence: vec![],
        }
    }
}
```

---

## 4. Human-in-the-Loop Review

```rust
pub struct SuggestionReviewer;

pub struct ReviewSession {
    pub session_id: String,
    pub suggestions: Vec<PolicySuggestion>,
    pub reviewer_id: String,
    pub status: ReviewStatus,
    pub decisions: Vec<SuggestionDecision>,
}

pub struct SuggestionDecision {
    pub suggestion_id: String,
    pub decision: ReviewDecision,
    pub notes: Option<String>,
    pub decided_at: DateTime<Utc>,
}

pub enum ReviewDecision {
    Approve,
    Reject,
    Modify { modified_change: PolicyChange },
    Defer,
}

impl SuggestionReviewer {
    pub fn create_review_session(
        &self,
        suggestions: Vec<PolicySuggestion>,
        reviewer_id: String,
    ) -> ReviewSession {
        ReviewSession {
            session_id: Uuid::new_v4().to_string(),
            suggestions,
            reviewer_id,
            status: ReviewStatus::Pending,
            decisions: vec![],
        }
    }
    
    pub fn apply_decisions(&self, session: &ReviewSession) -> Vec<AppliedPolicyChange> {
        session.decisions.iter()
            .filter_map(|d| match d.decision {
                ReviewDecision::Approve => {
                    let suggestion = session.suggestions.iter()
                        .find(|s| s.suggestion_id == d.suggestion_id)?;
                    Some(AppliedPolicyChange {
                        policy_field: suggestion.proposed_change.field.clone(),
                        new_value: suggestion.proposed_change.new_value.clone(),
                        effective_at: Utc::now(),
                        applied_by: session.reviewer_id.clone(),
                    })
                }
                ReviewDecision::Modify { ref modified_change } => {
                    Some(AppliedPolicyChange {
                        policy_field: modified_change.field.clone(),
                        new_value: modified_change.new_value.clone(),
                        effective_at: Utc::now(),
                        applied_by: session.reviewer_id.clone(),
                    })
                }
                _ => None,
            })
            .collect()
    }
}

pub struct AppliedPolicyChange {
    pub policy_field: String,
    pub new_value: String,
    pub effective_at: DateTime<Utc>,
    pub applied_by: String,
}
```

---

## 5. Slack Integration

```rust
pub fn render_suggestion_review(suggestion: &PolicySuggestion) -> MessageTemplate {
    let mut builder = MessageBuilder::new("Policy Optimization");
    
    // Header with confidence
    builder = builder.section("header", |s| {
        s.mrkdwn(format!(
            "🤖 *Policy Suggestion* ({:.0}% confidence)",
            suggestion.confidence * 100.0
        ))
    });
    
    // Description
    builder = builder.section("description", |s| {
        s.mrkdwn(suggestion.description.clone())
    });
    
    // Proposed change
    builder = builder.section("change", |s| {
        s.mrkdwn(format!(
            "*Proposed Change:*\n`{}`: {} → {}",
            suggestion.proposed_change.field,
            suggestion.proposed_change.old_value,
            suggestion.proposed_change.new_value
        ))
    });
    
    // Expected impact
    let impact_parts: Vec<String> = [
        suggestion.expected_impact.false_positive_reduction
            .map(|v| format!("False +: -{:.0}%", v * 100.0)),
        suggestion.expected_impact.false_negative_reduction
            .map(|v| format!("False -: -{:.0}%", v * 100.0)),
        suggestion.expected_impact.throughput_improvement
            .map(|v| format!("Throughput: +{:.0}%", v * 100.0)),
    ].into_iter().flatten().collect();
    
    builder = builder.section("impact", |s| {
        s.mrkdwn(format!(
            "*Expected Impact:*\n{} | Risk: {:?}",
            impact_parts.join(" | "),
            suggestion.expected_impact.risk_change
        ))
    });
    
    // Evidence
    if !suggestion.evidence.is_empty() {
        let evidence_text = suggestion.evidence.iter()
            .map(|e| format!("• {}: {}", e.quote_id, e.observation))
            .collect::<Vec<_>>()
            .join("\n");
        
        builder = builder.section("evidence", |s| {
            s.mrkdwn(format!("*Evidence:*\n{}", evidence_text))
        });
    }
    
    // Actions
    builder = builder.actions("actions", |a| {
        a.button(ButtonElement::new("approve", "✅ Approve").style(ButtonStyle::Primary))
         .button(ButtonElement::new("reject", "❌ Reject"))
         .button(ButtonElement::new("modify", "✏️ Modify"))
    });
    
    builder.build()
}
```

---

## 6. Data Model

```sql
-- Policy learning outcomes
CREATE TABLE policy_outcomes (
    id TEXT PRIMARY KEY,
    approval_id TEXT REFERENCES approvals(id),
    quote_id TEXT REFERENCES quotes(id),
    policy_version TEXT NOT NULL,
    reviewer_id TEXT NOT NULL,
    decision TEXT NOT NULL,  -- approved, rejected
    reviewer_feedback_json TEXT,
    reviewed_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- Detected patterns
CREATE TABLE detected_patterns (
    id TEXT PRIMARY KEY,
    pattern_type TEXT NOT NULL,
    confidence REAL NOT NULL,
    occurrence_count INTEGER NOT NULL,
    evidence_quote_ids_json TEXT NOT NULL,
    detected_at TEXT NOT NULL,
    resolved_at TEXT
);

-- Policy suggestions
CREATE TABLE policy_suggestions (
    id TEXT PRIMARY KEY,
    pattern_id TEXT REFERENCES detected_patterns(id),
    suggestion_type TEXT NOT NULL,
    confidence REAL NOT NULL,
    description TEXT NOT NULL,
    current_policy_json TEXT NOT NULL,
    proposed_change_json TEXT NOT NULL,
    expected_impact_json TEXT NOT NULL,
    status TEXT NOT NULL,  -- pending, approved, rejected, applied
    reviewed_by TEXT,
    review_notes TEXT,
    created_at TEXT NOT NULL,
    reviewed_at TEXT,
    applied_at TEXT
);

-- Policy change history
CREATE TABLE policy_changes (
    id TEXT PRIMARY KEY,
    suggestion_id TEXT REFERENCES policy_suggestions(id),
    policy_field TEXT NOT NULL,
    old_value TEXT,
    new_value TEXT NOT NULL,
    applied_by TEXT NOT NULL,
    applied_at TEXT NOT NULL
);
```

---

## 7. Learning Loop Workflow

```
┌─────────────────────────────────────────────────────────────────┐
│                    CLOSED-LOOP POLICY OPTIMIZER                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  1. COLLECT                                                    │
│     • Approval decisions                                       │
│     • Quote outcomes                                           │
│     • Reviewer feedback                                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  2. ANALYZE                                                    │
│     • Detect patterns in outcomes                              │
│     • Identify false positives/negatives                       │
│     • Find bottlenecks                                         │
│     • Discover auto-approvable patterns                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  3. GENERATE                                                   │
│     • Create policy suggestions                                │
│     • Calculate expected impact                                │
│     • Gather evidence                                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  4. REVIEW (Human-in-the-loop)                                 │
│     • Present suggestions to admin                             │
│     • Show evidence and impact                                 │
│     • Collect approval/rejection/modification                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  5. APPLY                                                      │
│     • Update policy rules                                      │
│     • Version the changes                                      │
│     • Audit trail                                              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  6. MONITOR                                                    │
│     • Track effectiveness of changes                           │
│     • Alert on unexpected outcomes                             │
│     • Feed back to step 1                                      │
└─────────────────────────────────────────────────────────────────┘
```

---

*Research compiled by ResearchAgent for the quotey project.*
