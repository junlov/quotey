# Emoji-Based Micro-Approvals (FEAT-03) - Deep Technical Research

**Feature:** Slack Emoji Reaction Approvals  
**Researcher:** ResearchAgent (kimi-k2)  
**Date:** 2026-02-23  
**Priority:** P1

---

## 1. Technical Overview

Removes friction from approval workflows by allowing authorized approvers to grant exceptions via simple emoji reactions (👍/👎/💬) on Slack threads. Captures reactions as valid approvals with cryptographic audit trails.

---

## 2. Slack Events API Research

### 2.1 Reaction Event Structure

```rust
// Slack reaction_added event payload
pub struct ReactionAddedEvent {
    pub user_id: String,           // Approver's Slack user ID
    pub reaction: String,          // Emoji name: "+1", "-1", "thumbsup", etc.
    pub item: ReactedItem,         // Message that was reacted to
    pub item_user_id: String,      // User who posted original message
    pub event_ts: String,          // Timestamp for ordering
    pub channel_id: String,        // Channel where reaction occurred
}

pub struct ReactedItem {
    pub item_type: String,         // "message"
    pub channel_id: String,
    pub ts: String,                // Message timestamp (unique ID)
}

// Supported approval emojis
pub const APPROVAL_EMOJIS: &[&str] = &["+1", "thumbsup", "white_check_mark"];
pub const REJECTION_EMOJIS: &[&str] = &["-1", "thumbsdown"];
pub const DISCUSSION_EMOJIS: &[&str] = &["speech_balloon", "thought_balloon", "question"];
```

### 2.2 Event Processing Flow

```rust
pub struct EmojiApprovalProcessor {
    approval_service: Arc<dyn ApprovalService>,
    authority_resolver: Arc<dyn AuthorityResolver>,
    audit_service: Arc<dyn AuditService>,
    quote_service: Arc<dyn QuoteService>,
}

impl EmojiApprovalProcessor {
    pub async fn process_reaction(
        &self,
        event: &ReactionAddedEvent,
    ) -> Result<ApprovalResult, ProcessingError> {
        // 1. Validate this is an approval-related message
        let approval_context = self.extract_approval_context(&event.item).await?;
        
        // 2. Verify reactor has approval authority
        let authority = self.authority_resolver
            .resolve(&event.user_id, &approval_context)
            .await?;
        
        if !authority.can_approve(&approval_context.approval_type) {
            return Ok(ApprovalResult::Unauthorized);
        }
        
        // 3. Parse emoji meaning
        let decision = self.parse_emoji_decision(&event.reaction)?;
        
        // 4. Create approval record
        let approval = ApprovalRecord {
            id: ApprovalId::generate(),
            quote_id: approval_context.quote_id,
            approver_id: event.user_id.clone(),
            approver_role: authority.role,
            decision: decision.clone(),
            context: approval_context,
            slack_metadata: SlackMetadata {
                channel_id: event.channel_id.clone(),
                message_ts: event.item.ts.clone(),
                reaction_ts: event.event_ts.clone(),
            },
            created_at: Utc::now(),
        };
        
        // 5. Record with audit trail
        self.approval_service.record_approval(approval.clone()).await?;
        self.audit_service.log_approval(&approval).await?;
        
        // 6. Update quote state if approved
        if let Decision::Approve = decision {
            self.quote_service
                .process_approval(approval_context.quote_id, approval.id)
                .await?;
        }
        
        Ok(ApprovalResult::Processed(approval))
    }
}
```

---

## 3. Authority Verification System

### 3.1 Authority Resolution

```rust
pub struct AuthorityResolver {
    org_hierarchy: Arc<dyn OrgHierarchy>,
    approval_matrix: Arc<dyn ApprovalMatrix>,
}

impl AuthorityResolver {
    pub async fn resolve(
        &self,
        slack_user_id: &str,
        context: &ApprovalContext,
    ) -> Result<Authority, AuthorityError> {
        // 1. Get user from Slack ID
        let user = self.org_hierarchy
            .find_by_slack_id(slack_user_id)
            .await?
            .ok_or(AuthorityError::UnknownUser)?;
        
        // 2. Check explicit approval authority
        let matrix_entry = self.approval_matrix
            .get_authority(&user.role, &context.approval_type)
            .await?;
        
        // 3. Validate limits
        if let Some(limit) = matrix_entry.max_amount {
            if context.amount > limit {
                return Ok(Authority {
                    role: user.role,
                    can_approve: false,
                    reason: Some(format!(
                        "Amount ${} exceeds ${} limit for {}",
                        context.amount, limit, user.role
                    )),
                });
            }
        }
        
        // 4. Check account tier authorization
        if let Some(ref tiers) = matrix_entry.allowed_account_tiers {
            if !tiers.contains(&context.account_tier) {
                return Ok(Authority {
                    role: user.role,
                    can_approve: false,
                    reason: Some(format!(
                        "{} tier accounts require higher authority",
                        context.account_tier
                    )),
                });
            }
        }
        
        Ok(Authority {
            role: user.role,
            can_approve: true,
            limits: matrix_entry.limits,
        })
    }
}

pub struct Authority {
    pub role: String,
    pub can_approve: bool,
    pub limits: Option<ApprovalLimits>,
    pub reason: Option<String>,
}
```

### 3.2 Approval Matrix Schema

```sql
CREATE TABLE approval_authority_matrix (
    id TEXT PRIMARY KEY,
    role TEXT NOT NULL,
    approval_type TEXT NOT NULL,  -- 'discount', 'custom_terms', 'exception'
    max_amount REAL,              -- NULL = unlimited
    max_discount_pct REAL,        -- NULL = unlimited
    allowed_account_tiers TEXT,   -- JSON array ["enterprise", "strategic"]
    requires_secondary_approval BOOLEAN DEFAULT FALSE,
    secondary_approver_role TEXT,
    active BOOLEAN DEFAULT TRUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    
    UNIQUE(role, approval_type)
);

CREATE INDEX idx_approval_matrix_role ON approval_authority_matrix(role, active);
CREATE INDEX idx_approval_matrix_type ON approval_authority_matrix(approval_type, active);

-- User to role mapping with Slack integration
CREATE TABLE approver_profiles (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL UNIQUE,
    slack_user_id TEXT UNIQUE,
    email TEXT NOT NULL,
    role TEXT NOT NULL REFERENCES approval_authority_matrix(role),
    department TEXT,
    delegated_to TEXT,           -- User ID during delegation
    delegation_expires_at TEXT,
    active BOOLEAN DEFAULT TRUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_approver_slack ON approver_profiles(slack_user_id, active);
CREATE INDEX idx_approver_role ON approver_profiles(role, active);
```

---

## 4. Approval State Machine

### 4.1 States and Transitions

```rust
pub enum ApprovalState {
    Pending,           // Awaiting approver response
    Approved,          // Valid approval received
    Rejected,          // Explicit rejection
    Escalated,         // Moved to higher authority
    Expired,           // Timeout without response
    Cancelled,         // Request withdrawn
}

pub enum ApprovalEvent {
    EmojiReaction { decision: Decision, approver: String },
    Timeout,
    EscalationTriggered,
    RequestCancelled,
    UndoRequested,     // Within 5-minute window
}

impl StateMachine for ApprovalState {
    type Event = ApprovalEvent;
    type Context = ApprovalContext;
    
    fn transition(
        &self,
        event: &ApprovalEvent,
        context: &ApprovalContext,
    ) -> Result<TransitionResult, TransitionError> {
        match (self, event) {
            // Pending → Approved
            (ApprovalState::Pending, ApprovalEvent::EmojiReaction { decision: Decision::Approve, approver })
                if self.is_authorized(approver, context) => {
                Ok(TransitionResult::new(ApprovalState::Approved)
                    .with_action(ApprovalAction::NotifyApproval))
            }
            
            // Pending → Rejected
            (ApprovalState::Pending, ApprovalEvent::EmojiReaction { decision: Decision::Reject, approver })
                if self.is_authorized(approver, context) => {
                Ok(TransitionResult::new(ApprovalState::Rejected)
                    .with_action(ApprovalAction::NotifyRejection))
            }
            
            // Pending → Escalated
            (ApprovalState::Pending, ApprovalEvent::Timeout) => {
                Ok(TransitionResult::new(ApprovalState::Escalated)
                    .with_action(ApprovalAction::NotifyEscalation))
            }
            
            // Approved → Pending (undo within window)
            (ApprovalState::Approved, ApprovalEvent::UndoRequested)
                if self.within_undo_window(context) => {
                Ok(TransitionResult::new(ApprovalState::Pending)
                    .with_action(ApprovalAction::RevertApproval))
            }
            
            _ => Err(TransitionError::InvalidTransition),
        }
    }
}
```

### 4.2 Undo Window Implementation

```rust
pub struct UndoWindow {
    duration: Duration,  // 5 minutes
}

impl UndoWindow {
    pub fn is_within(&self, approval_time: DateTime<Utc>) -> bool {
        let elapsed = Utc::now() - approval_time;
        elapsed <= chrono::Duration::from_std(self.duration).unwrap()
    }
}

pub async fn handle_undo_request(
    &self,
    approval_id: &ApprovalId,
    requester: &str,
) -> Result<UndoResult, UndoError> {
    let approval = self.approval_service.get(approval_id).await?;
    
    // Verify requester is original approver
    if approval.approver_id != requester {
        return Err(UndoError::Unauthorized);
    }
    
    // Check undo window
    let window = UndoWindow::new(Duration::from_secs(300));
    if !window.is_within(approval.created_at) {
        return Err(UndoError::WindowExpired);
    }
    
    // Revert state
    self.quote_service.revert_approval(approval.quote_id).await?;
    self.approval_service.mark_undone(approval_id).await?;
    
    Ok(UndoResult::Success)
}
```

---

## 5. Cryptographic Audit Trail

### 5.1 Approval Record Structure

```rust
pub struct ApprovalRecord {
    pub id: ApprovalId,
    pub quote_id: QuoteId,
    pub request_id: RequestId,
    
    // Approver info
    pub approver_id: String,
    pub approver_slack_id: String,
    pub approver_role: String,
    
    // Decision
    pub decision: Decision,
    pub decision_emoji: String,
    
    // Context
    pub approval_type: ApprovalType,
    pub amount: Option<Decimal>,
    pub discount_pct: Option<Decimal>,
    pub justification: String,
    
    // Slack metadata for verification
    pub slack_metadata: SlackMetadata,
    
    // Cryptographic proof
    pub signature: String,  // HMAC of approval data
    
    // Timestamps
    pub created_at: DateTime<Utc>,
    pub undo_requested_at: Option<DateTime<Utc>>,
    pub undone_at: Option<DateTime<Utc>>,
}

pub struct SlackMetadata {
    pub workspace_id: String,
    pub channel_id: String,
    pub message_ts: String,
    pub reaction_ts: String,
    pub reaction_event_id: String,
}
```

### 5.2 Signature Generation

```rust
pub struct ApprovalSigner {
    signing_key: HmacKey,
}

impl ApprovalSigner {
    pub fn sign(&self, record: &ApprovalRecord) -> String {
        let data = format!(
            "{}|{}|{}|{}|{}|{}",
            record.id.0,
            record.quote_id.0,
            record.approver_id,
            record.decision,
            record.slack_metadata.reaction_ts,
            record.created_at.to_rfc3339()
        );
        
        let mut mac = HmacSha256::new_from_slice(&self.signing_key.0)
            .expect("HMAC can take key of any size");
        mac.update(data.as_bytes());
        
        hex::encode(mac.finalize().into_bytes())
    }
    
    pub fn verify(&self, record: &ApprovalRecord) -> bool {
        let expected = self.sign(record);
        record.signature == expected
    }
}
```

### 5.3 Audit Log Schema

```sql
CREATE TABLE emoji_approval_audit (
    id TEXT PRIMARY KEY,
    approval_id TEXT NOT NULL,
    event_type TEXT NOT NULL,  -- 'created', 'undone', 'verified'
    
    -- Complete snapshot
    approval_data_json TEXT NOT NULL,
    signature TEXT NOT NULL,
    
    -- Verification
    verified_at TEXT,
    verification_result BOOLEAN,
    
    -- Chain linking
    previous_audit_hash TEXT,
    this_audit_hash TEXT,  -- Hash of this record
    
    created_at TEXT NOT NULL
);

CREATE INDEX idx_emoji_audit_approval ON emoji_approval_audit(approval_id);
CREATE INDEX idx_emoji_audit_event ON emoji_approval_audit(event_type);
```

---

## 6. Slack Integration

### 6.1 Approval Request Message

```rust
pub fn render_approval_request(
    context: &ApprovalContext,
    approver: &str,
) -> MessageTemplate {
    MessageBuilder::new("Approval Required")
        .section("header", |s| {
            s.mrkdwn(format!(
                "@{} **Approval Required**\n\n*Quote:* {}\n*Customer:* {}\n*Value:* ${}",
                approver,
                context.quote_id.0,
                context.customer_name,
                context.amount
            ))
        })
        .section("details", |s| {
            s.mrkdwn(format!(
                "*Exception Type:* {}\n*Current Discount:* {}%\n*Threshold:* {}%",
                context.approval_type,
                context.current_discount_pct,
                context.threshold_pct
            ))
        })
        .context("justification", |c| {
            c.mrkdwn(format!(
                "*Rep Justification:* {}",
                context.rep_justification
            ))
        })
        .actions("decision", |a| {
            a.button(
                ButtonElement::new("approve", "👍 Approve")
                    .style(ButtonStyle::Primary)
                    .action_id("emoji_approve")
            )
            .button(
                ButtonElement::new("reject", "👎 Reject")
                    .style(ButtonStyle::Danger)
                    .action_id("emoji_reject")
            )
            .button(
                ButtonElement::new("discuss", "💬 Discuss")
                    .style(ButtonStyle::Secondary)
                    .action_id("emoji_discuss")
            )
        })
        .context("footer", |c| {
            c.mrkdwn("React with 👍 to approve, 👎 to reject, or 💬 to discuss".to_string())
        })
        .build()
}
```

### 6.2 Approval Confirmation Message

```rust
pub fn render_approval_confirmation(
    approval: &ApprovalRecord,
) -> MessageTemplate {
    let emoji = match approval.decision {
        Decision::Approve => "✅",
        Decision::Reject => "❌",
        Decision::Discuss => "💬",
    };
    
    MessageBuilder::new("Approval Update")
        .section("result", |s| {
            s.mrkdwn(format!(
                "{} **{}** by <@{}> at {}\n\n*Quote:* {}\n*Decision:* {} via {}",
                emoji,
                approval.decision,
                approval.approver_slack_id,
                approval.created_at.format("%H:%M"),
                approval.quote_id.0,
                approval.decision,
                approval.decision_emoji
            ))
        })
        .context("undo", |c| {
            if approval.within_undo_window() {
                c.mrkdwn("Undo available for 5 minutes".to_string())
            } else {
                c.mrkdwn("Approval finalized".to_string())
            }
        })
        .build()
}
```

---

## 7. Error Handling

### 7.1 Error Types

```rust
pub enum ApprovalError {
    UnauthorizedUser {
        slack_user_id: String,
        required_role: String,
    },
    InvalidEmoji {
        emoji: String,
        valid_emojis: Vec<String>,
    },
    ApprovalWindowExpired {
        expired_at: DateTime<Utc>,
    },
    QuoteNotFound {
        quote_id: QuoteId,
    },
    AlreadyDecided {
        current_state: ApprovalState,
    },
    VerificationFailed {
        approval_id: ApprovalId,
        reason: String,
    },
}

impl Into<InterfaceError> for ApprovalError {
    fn into(self) -> InterfaceError {
        match self {
            ApprovalError::UnauthorizedUser { slack_user_id, required_role } => {
                InterfaceError::BadRequest {
                    correlation_id: generate_id(),
                    message: format!(
                        "<@{}> does not have {} approval authority. "
                        "Please contact your administrator for access.",
                        slack_user_id, required_role
                    ),
                }
            }
            // ... other variants
        }
    }
}
```

---

## 8. Testing Strategy

### 8.1 Unit Tests

```rust
#[test]
fn recognizes_approval_emoji() {
    let processor = EmojiApprovalProcessor::new();
    
    assert!(processor.is_approval_emoji("+1"));
    assert!(processor.is_approval_emoji("thumbsup"));
    assert!(!processor.is_approval_emoji("smile"));
}

#[test]
fn authority_verification() {
    let resolver = AuthorityResolver::new(test_matrix());
    
    let vp = test_user("sarah", "vp_sales");
    let context = test_context(ApprovalType::Discount, dec!(25000));
    
    let authority = resolver.resolve(&vp.slack_id, &context).unwrap();
    assert!(authority.can_approve);
}

#[test]
fn undo_window_calculation() {
    let window = UndoWindow::new(Duration::from_secs(300));
    
    let recent = Utc::now() - Duration::from_secs(60);
    assert!(window.is_within(recent));
    
    let old = Utc::now() - Duration::from_secs(600);
    assert!(!window.is_within(old));
}
```

### 8.2 Integration Tests

```rust
#[tokio::test]
async fn full_approval_flow() {
    let harness = TestHarness::new().await;
    
    // 1. Create approval request
    let quote = harness.create_test_quote().await;
    let approval = harness.request_approval(&quote, ApprovalType::Discount).await;
    
    // 2. Simulate emoji reaction
    let result = harness
        .process_reaction(&approval, "+1", "sarah_vp")
        .await;
    
    // 3. Verify approval recorded
    assert!(matches!(result, ApprovalResult::Processed(_)));
    
    // 4. Verify quote state updated
    let updated = harness.get_quote(&quote.id).await;
    assert_eq!(updated.status, QuoteStatus::Approved);
    
    // 5. Verify audit trail
    let audit = harness.get_audit_log(&approval.id).await;
    assert_eq!(audit.len(), 1);
    assert!(audit[0].verification_result);
}
```

---

## 9. Security Considerations

| Threat | Mitigation |
|--------|------------|
| Spoofed emoji reactions | Verify via Slack Events API signing secret |
| Unauthorized approvers | Authority matrix + org hierarchy lookup |
| Replay attacks | Timestamp validation + unique event IDs |
| Audit tampering | Cryptographic signatures + hash chain |
| Misclick approval | 5-minute undo window |
| Delegation abuse | Time-bounded delegation + audit logging |

---

## 10. Performance Targets

| Metric | Target |
|--------|--------|
| Event processing latency | <100ms |
| Authority resolution | <50ms |
| Audit log write | <20ms |
| Quote state update | <100ms |
| End-to-end emoji→confirmation | <500ms |

---

*Research compiled by ResearchAgent for the quotey project.*
