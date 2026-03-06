# Top Paperclip-Inspired Ideas for Quotey

**Quick Reference from Deep Research**

---

## 🎯 Top 10 Ideas (Ranked by Impact vs Effort)

### 1. Visual Quote Timeline/Trace ⭐⭐⭐ HIGH PRIORITY
**Paperclip Feature:** Every ticket shows a full trace of actions  
**Quotey Version:** Visual quote lifecycle timeline

```
Q-2026-0042: Acme Corp Enterprise Deal ($150K)
├── [Mar 6, 09:00] Jane (Sales) created quote
│   └── Products: Enterprise Plan × 100 seats
├── [Mar 6, 09:05] System applied pricing
│   └── Base: $120K, Discount: 20% (-$24K)
│   └── ⚠️ Policy violation: Discount > 15%
├── [Mar 6, 09:30] Manager approval requested
├── [Mar 6, 10:15] Mike (Manager) approved exception
│   └── Reason: "Strategic account, competitive deal"
├── [Mar 6, 11:00] PDF generated and sent
├── [Mar 6, 14:30] Customer viewed quote (portal)
└── [Mar 6, 16:45] Customer approved ✓
    └── Digital signature captured
```

**Implementation:**
- New table: `quote_traces` (audit events with rich context)
- UI component: Timeline view in quote detail
- API: `GET /quotes/{id}/timeline`

**Effort:** Medium  
**Impact:** High (transparency, auditability, debugging)

---

### 2. Discount Budget System ⭐⭐⭐ HIGH PRIORITY
**Paperclip Feature:** Monthly budgets per agent with hard stops  
**Quotey Version:** Per-rep and team discount budgets

```rust
// crates/core/src/budget.rs
pub struct DiscountBudget {
    pub owner_id: String,          // rep or team
    pub period: BudgetPeriod,      // Monthly, Quarterly
    pub limit: Decimal,            // Max discount dollars
    pub used: Decimal,             // Used this period
    pub reserved: Decimal,         // Pending approvals
    
    // Thresholds
    pub warning_at: f64,           // 0.8 = 80%
    pub hard_stop: bool,           // Block at 100%?
}

// Before applying discount:
async fn check_budget(
    rep_id: &str, 
    requested_discount: Decimal
) -> Result<BudgetStatus> {
    let budget = get_budget(rep_id).await?;
    let projected = budget.used + budget.reserved + requested_discount;
    
    if projected > budget.limit {
        return Ok(BudgetStatus::Exceeded);
    }
    if projected > budget.limit * budget.warning_at {
        return Ok(BudgetStatus::Warning { 
            remaining: budget.limit - budget.used 
        });
    }
    Ok(BudgetStatus::Ok)
}
```

**UI Dashboard:**
```
Your Discount Budget (March 2026)
├─ Limit: $50,000
├─ Used: $32,000 (64%) ████████████░░░░░░░░
├─ Reserved: $8,000 (pending approvals)
└─ Available: $10,000

⚠️ Warning: This quote ($15K discount) will exceed budget!
   Options: [Request Override] [Reduce Discount] [Split Deal]
```

**Effort:** Medium  
**Impact:** High (cost control, rep empowerment)

---

### 3. Goal-Aligned Deal Context ⭐⭐⭐ HIGH PRIORITY
**Paperclip Feature:** Every task carries "goal ancestry"  
**Quotey Version:** Deal context showing strategic importance

```
┌─────────────────────────────────────────────────────────┐
│  Deal Context for Q-2026-0042                           │
├─────────────────────────────────────────────────────────┤
│  Company Goal: "Hit $2M ARR in Q2"                      │
│  └── Sales Target: "Close 50 enterprise deals"          │
│      └── This Deal: "Acme Corp - $150K ARR"             │
│          ├── Stage: Awaiting approval                   │
│          ├── Confidence: 85%                            │
│          └── Impact: 7.5% of quarterly target ✨        │
│                                                         │
│  Why this matters:                                      │
│  • Acme is a logo account (reference customer)          │
│  • Unlocks Healthcare vertical expansion                │
│  • Competitive win against CompetitorX                  │
└─────────────────────────────────────────────────────────┘
```

**Database:**
```sql
ALTER TABLE quotes ADD COLUMN strategic_value_score INT;
ALTER TABLE quotes ADD COLUMN company_goal_id UUID REFERENCES goals(id);
ALTER TABLE quotes ADD COLUMN impact_notes TEXT;
```

**Effort:** Low  
**Impact:** High (rep motivation, priority triage)

---

### 4. CPQ Skills System (Runtime Knowledge) ⭐⭐ MEDIUM PRIORITY
**Paperclip Feature:** `SKILL.md` files inject context at runtime  
**Quotey Version:** Pricing expertise as modular skills

**File Structure:**
```
.skills/
├── enterprise-pricing/
│   ├── SKILL.md
│   └── examples/
│       └── fortune500_deal.md
├── healthcare-vertical/
│   ├── SKILL.md
│   └── references/
│       └── hipaa_requirements.md
└── competitive-positioning/
    └── SKILL.md
```

**Example SKILL.md:**
```markdown
---
name: enterprise-pricing
description: Enterprise deal pricing strategies and floor prices
author: VP Sales
tags: [pricing, enterprise, negotiation]
---

# Enterprise Pricing Skill

## Floor Prices (Non-Negotiable Minimums)
- Enterprise Plan: $12K/year per seat minimum
- Professional Services: $250/hr minimum
- Implementation: $15K minimum

## Discount Authority Matrix
| Rep Level | Max Discount | Deal Size |
|-----------|--------------|-----------|
| SDR | 0% | < $5K |
| AE | 15% | < $50K |
| Senior AE | 25% | < $150K |
| VP Sales | 35% | Any |

## Competitive Positioning
vs CompetitorX:
- We win on: Security, Support, API
- They win on: Price (20-30% cheaper)
- Strategy: ROI calculator, reference customers

## Tools Available
- floor_price_calculator
- competitive_battlecard
- roi_calculator
```

**Runtime Injection:**
```rust
pub async fn create_quote_with_skills(
    input: QuoteInput,
    applicable_skills: Vec<Skill>
) -> Result<Quote> {
    // Build context with skills
    let mut context = QuoteContext::new(input);
    for skill in applicable_skills {
        context.inject_skill(skill.load_content().await?);
    }
    
    // Agent/LLM uses enriched context
    let pricing = pricing_engine.calculate(&context).await?;
    Ok(Quote::create(context, pricing))
}
```

**Effort:** High  
**Impact:** Very High (scalable expertise, onboarding acceleration)

---

### 5. Heartbeat Scheduling for Quotes ⭐⭐ MEDIUM PRIORITY
**Paperclip Feature:** Agents wake on schedule to check work  
**Quotey Version:** Automated quote follow-ups and renewals

```rust
// crates/core/src/scheduler/heartbeat.rs

#[derive(Clone)]
pub struct QuoteHeartbeat {
    pub quote_id: Uuid,
    pub cron_schedule: String,      // "0 9 * * MON" = Mondays 9am
    pub action: HeartbeatAction,
    pub max_retries: u32,
}

pub enum HeartbeatAction {
    FollowUpEmail { template: String },
    EscalateToManager,
    CheckCompetitorPricing,
    GenerateRenewalQuote,
}

// Scheduler service
pub async fn run_heartbeat(&self, quote_id: Uuid) -> Result<()> {
    let quote = self.db.get_quote(quote_id).await?;
    
    match quote.status {
        QuoteStatus::PendingApproval => {
            // Remind approver
            self.send_reminder(quote).await?;
        }
        QuoteStatus::Sent => {
            // Follow up with customer
            if quote.days_since_sent() > 7 {
                self.send_follow_up(quote).await?;
            }
        }
        QuoteStatus::Accepted => {
            // Schedule renewal quote 90 days before expiry
            self.schedule_renewal(quote).await?;
        }
        _ => {}
    }
    
    // Schedule next heartbeat
    self.schedule_next(quote_id).await?;
    Ok(())
}
```

**Use Cases:**
- Auto-follow up on unresponded quotes after 7 days
- Escalate stalled approvals to VP after 48 hours
- Generate renewal quotes 90 days before contract expiry
- Check competitive pricing weekly during evaluation

**Effort:** Medium  
**Impact:** High (automation, deal velocity)

---

### 6. Multi-Company Architecture ⭐⭐ MEDIUM PRIORITY
**Paperclip Feature:** Single deployment, unlimited companies  
**Quotey Version:** True SaaS multi-tenancy

```rust
// Row-level security approach (PostgreSQL)

// Every table gets company_id
ALTER TABLE quotes ADD COLUMN company_id UUID NOT NULL;
ALTER TABLE products ADD COLUMN company_id UUID NOT NULL;

// RLS policies
CREATE POLICY company_isolation ON quotes
    FOR ALL
    USING (company_id = current_setting('app.current_company_id')::UUID);

// Application sets context per request
pub async fn with_company_context<T>(
    company_id: Uuid,
    f: impl FnOnce() -> T
) -> T {
    sqlx::query("SET app.current_company_id = $1")
        .bind(company_id)
        .execute(&pool)
        .await;
    
    let result = f().await;
    
    sqlx::query("RESET app.current_company_id")
        .execute(&pool)
        .await;
    
    result
}
```

**Benefits:**
- Single deployment serves multiple orgs
- Complete data isolation (database-enforced)
- Shared infrastructure, separate data
- Easier to manage than multiple instances

**Effort:** High (schema changes, auth updates)  
**Impact:** Very High (SaaS scalability)

---

### 7. Atomic Task Checkout ⭐ LOW PRIORITY
**Paperclip Feature:** Prevents double-work via atomic checkout  
**Quotey Version:** Prevent duplicate quote edits

```rust
pub async fn checkout_quote_for_edit(
    &self,
    quote_id: Uuid,
    user_id: &str
) -> Result<QuoteEditSession> {
    // Atomic UPDATE ... WHERE checked_out_by IS NULL
    let result = sqlx::query(
        r#"
        UPDATE quotes 
        SET checked_out_by = $1, 
            checked_out_at = NOW()
        WHERE id = $2 
          AND checked_out_by IS NULL
        RETURNING *
        "#
    )
    .bind(user_id)
    .bind(quote_id)
    .fetch_optional(&self.pool)
    .await?;
    
    match result {
        Some(quote) => Ok(QuoteEditSession::new(quote)),
        None => Err(Error::AlreadyCheckedOut {
            by: get_current_editor(quote_id).await?,
        }),
    }
}
```

**UI Indication:**
```
⚠️ Quote is being edited by Jane (since 10:30 AM)
   [Request Access] [View Read-Only]
```

**Effort:** Low  
**Impact:** Medium (prevents conflicts)

---

### 8. Ticket-Based Communication ⭐⭐ MEDIUM PRIORITY
**Paperclip Feature:** All communication happens via structured tickets  
**Quotey Version:** Quote threads with structured messages

```rust
pub struct QuoteThread {
    pub id: Uuid,
    pub quote_id: Uuid,
    pub messages: Vec<ThreadMessage>,
}

pub struct ThreadMessage {
    pub id: Uuid,
    pub author: Author,           // User or Agent
    pub message_type: MessageType,
    pub content: String,
    pub attachments: Vec<Attachment>,
    pub timestamp: DateTime<Utc>,
}

pub enum MessageType {
    Comment,
    StatusChange { from: Status, to: Status },
    ApprovalRequest { level: u8 },
    ApprovalResponse { approved: bool, reason: String },
    SystemEvent { event: String },
    CustomerView,                  // Customer viewed quote
    CustomerAction { action: String },
}
```

**Thread View:**
```
Q-2026-0042 Discussion
├─ [Jane] Created quote for Acme Corp
│  "Working on the enterprise deal we discussed"
│
├─ [System] Auto-generated pricing
│  Applied 20% volume discount
│
├─ [Mike] Approval requested
│  "Need approval for 20% discount (above 15% threshold)"
│
├─ [Sarah] Approved
│  "Approved - strategic account"
│ 👍 3 reactions
│
├─ [Customer] Viewed quote
│  Customer opened quote 3 times
│
└─ [Jane] ✅ Deal closed!
   Customer signed via portal
```

**Effort:** Medium  
**Impact:** Medium (collaboration, history)

---

### 9. Cost Tracking Per Deal ⭐ LOW PRIORITY
**Paperclip Feature:** Every agent action has a cost  
**Quotey Version:** Track cost to produce each quote

```rust
pub struct QuoteCost {
    pub quote_id: Uuid,
    pub components: Vec<CostComponent>,
    pub total_cost: Decimal,
}

pub struct CostComponent {
    pub category: CostCategory,
    pub description: String,
    pub amount: Decimal,
    pub timestamp: DateTime<Utc>,
}

pub enum CostCategory {
    RepTime { hours: f64, hourly_rate: Decimal },
    ManagerTime { hours: f64, hourly_rate: Decimal },
    LegalReview { flat_fee: Decimal },
    CustomWork { hours: f64 },
    ToolUsage { tool: String, cost: Decimal },
}
```

**ROI Calculation:**
```
Quote Q-2026-0042 Cost Analysis
├── Revenue: $150,000
├── Cost to produce: $2,350
│   ├── Rep time: 8 hrs × $150 = $1,200
│   ├── Manager approval: 0.5 hrs × $200 = $100
│   ├── Legal review: $1,000
│   └── System costs: $50
└── Margin: 98.4%

💡 Insight: High-margin deal, consider similar prospects
```

**Effort:** Low  
**Impact:** Low-Medium (margin visibility)

---

### 10. Company Templates ⭐⭐ MEDIUM PRIORITY
**Paperclip Feature:** Export/import entire company configurations  
**Quotey Version:** CPQ template marketplace

```yaml
# templates/enterprise-saas.yaml
name: "Enterprise SaaS Sales"
description: "Complete CPQ setup for enterprise SaaS"
author: "Quotey Team"
version: "1.0.0"

categories:
  - name: "Products"
    items:
      - name: "Enterprise Plan"
        base_price: 12000
        unit: "per seat/year"
        
      - name: "Implementation"
        base_price: 15000
        unit: "flat fee"
        
  - name: "Pricing Rules"
    rules:
      - condition: "volume > 100"
        discount: "15%"
        
      - condition: "annual_commitment"
        discount: "10%"
        
  - name: "Approval Matrix"
    matrix:
      - discount_max: 15
        approver: "manager"
        
      - discount_max: 25
        approver: "vp_sales"
        
  - name: "Skills"
    skills:
      - enterprise-pricing
      - security-compliance
```

**Usage:**
```bash
# Export current config as template
quotey template export --name "My Setup" --output ./my-template.yaml

# Import template to new org
quotey template import ./enterprise-saas.yaml
```

**Effort:** Medium  
**Impact:** High (accelerate new customer onboarding)

---

## Implementation Roadmap

### Phase 1: Foundation (Weeks 1-2)
- [ ] Quote timeline/trace table
- [ ] Basic timeline UI component
- [ ] Goal context fields on quotes

### Phase 2: Governance (Weeks 3-4)
- [ ] Discount budget system
- [ ] Budget dashboard UI
- [ ] Hard stop at budget limit (optional)

### Phase 3: Intelligence (Weeks 5-6)
- [ ] Skills system foundation
- [ ] First skill: enterprise-pricing
- [ ] Skill injection in quote creation

### Phase 4: Scale (Weeks 7-8)
- [ ] Multi-company schema changes
- [ ] RLS policies
- [ ] Company isolation

### Phase 5: Automation (Weeks 9-10)
- [ ] Heartbeat scheduler
- [ ] Follow-up automation
- [ ] Renewal reminders

---

## Quick Wins (This Week)

1. **Add quote timeline table** - 2 hours
2. **Display goal context on quote page** - 1 hour
3. **Add discount tracking fields** - 1 hour

Total: 4 hours for immediate value

---

*Generated from Paperclip Deep Research*
