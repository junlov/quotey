# Smart Thread Routing (FEAT-10) - Deep Technical Research

**Feature:** Intelligent Approval Routing  
**Researcher:** ResearchAgent (kimi-k2)  
**Date:** 2026-02-23  
**Priority:** P1

---

## 1. Technical Overview

Smart Thread Routing automatically identifies the correct approver based on quote characteristics and routes approval requests via Slack @mentions with full context. Includes OOO detection, fallback routing, and auto-escalation.

---

## 2. Authority Resolution System

### 2.1 Core Structures

```rust
pub struct ApprovalRole {
    pub role_id: String,
    pub role_name: String,
    pub role_level: u32,
    pub max_deal_value: Option<Decimal>,
    pub max_discount_pct: Option<Decimal>,
    pub allowed_account_tiers: Vec<String>,
}

pub struct AuthorityUser {
    pub user_id: String,
    pub slack_user_id: String,
    pub email: String,
    pub role_id: String,
    pub is_active: bool,
}

pub struct AuthorityResolution {
    pub user: AuthorityUser,
    pub role: ApprovalRole,
    pub resolution_type: ResolutionType,
}
```

### 2.2 Authority Resolver

```rust
pub struct AuthorityResolver {
    registry: Arc<AuthorityRegistry>,
    availability_service: Arc<dyn AvailabilityService>,
}

impl AuthorityResolver {
    pub async fn resolve_primary(
        &self,
        request: &RoutingRequest,
    ) -> Result<AuthorityResolution, ResolutionError> {
        let eligible = self.find_eligible_roles(request).await?;
        
        for role in eligible {
            let users = self.registry.get_users_by_role(&role.role_id).await?;
            for user in users {
                if !user.is_active { continue; }
                
                let availability = self.availability_service
                    .check_availability(&user.slack_user_id)
                    .await?;
                
                if matches!(availability, Availability::Available) {
                    return Ok(AuthorityResolution {
                        user: user.clone(),
                        role: role.clone(),
                        resolution_type: ResolutionType::Primary,
                    });
                }
            }
        }
        
        Err(ResolutionError::NoAvailableApprovers)
    }
}
```

---

## 3. Availability Service

```rust
#[async_trait]
pub trait AvailabilityService: Send + Sync {
    async fn check_availability(&self, user_id: &str) -> Result<Availability, AvailabilityError>;
}

pub enum Availability {
    Available,
    OOO { returns_at: DateTime<Utc> },
    Delegated { to_user_id: String },
}
```

---

## 4. Routing Engine

```rust
pub struct RoutingDecision {
    pub primary: AuthorityResolution,
    pub fallback: Option<AuthorityResolution>,
    pub escalation: Option<AuthorityResolution>,
    pub sla_deadline: DateTime<Utc>,
}

pub struct RoutingEngine {
    authority_resolver: Arc<AuthorityResolver>,
    rules_engine: Arc<RoutingRulesEngine>,
}

impl RoutingEngine {
    pub async fn route(&self, request: &RoutingRequest) -> Result<RoutingDecision, RoutingError> {
        let primary = self.authority_resolver.resolve_primary(request).await?;
        let fallback = self.authority_resolver.resolve_fallback(&primary, request).await.ok();
        let escalation = self.authority_resolver.escalate(&primary, request).await.ok();
        
        Ok(RoutingDecision {
            primary,
            fallback,
            escalation,
            sla_deadline: Utc::now() + Duration::hours(4),
        })
    }
}
```

---

## 5. Slack Integration

### 5.1 Smart Mention

```rust
pub fn compose_mention(decision: &RoutingDecision, quote: &Quote) -> MessageTemplate {
    MessageBuilder::new("Approval Request")
        .section("header", |s| {
            s.mrkdwn(format!(
                "@{} **Approval Required**\nQuote: {} | Customer: {} | Value: ${} | Discount: {}%",
                decision.primary.user.slack_user_id,
                quote.id.0,
                quote.customer_name,
                quote.total,
                quote.discount_pct
            ))
        })
        .actions("decisions", |a| {
            a.button(ButtonElement::new("approve", "Approve").style(ButtonStyle::Primary))
             .button(ButtonElement::new("reject", "Reject").style(ButtonStyle::Danger))
             .button(ButtonElement::new("discuss", "Discuss"))
        })
        .build()
}
```

---

## 6. Database Schema

```sql
CREATE TABLE approval_roles (
    id TEXT PRIMARY KEY,
    role_name TEXT NOT NULL,
    role_level INTEGER NOT NULL,
    max_deal_value REAL,
    max_discount_pct REAL,
    allowed_account_tiers TEXT
);

CREATE TABLE authority_users (
    id TEXT PRIMARY KEY,
    slack_user_id TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL,
    role_id TEXT NOT NULL REFERENCES approval_roles(id),
    is_active BOOLEAN DEFAULT TRUE
);

CREATE TABLE org_hierarchy (
    user_id TEXT PRIMARY KEY REFERENCES authority_users(id),
    manager_id TEXT REFERENCES authority_users(id)
);

CREATE TABLE routing_rules (
    id TEXT PRIMARY KEY,
    condition_type TEXT NOT NULL,
    condition_params TEXT,
    action_type TEXT NOT NULL,
    priority INTEGER DEFAULT 0
);
```

---

*Research compiled by ResearchAgent for the quotey project.*
