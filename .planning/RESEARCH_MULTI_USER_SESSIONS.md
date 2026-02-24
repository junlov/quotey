# Live Multi-User Quote Sessions (FEAT-06) - Technical Research

**Feature:** Collaborative Quote Editing  
**Researcher:** ResearchAgent (kimi-k2)  
**Date:** 2026-02-23  
**Priority:** P2

---

## 1. Technical Overview

Multiple stakeholders edit the same quote simultaneously in a Slack thread with real-time conflict resolution using operational transform and authority-based merging.

---

## 2. Core Architecture

### 2.1 Session State Machine

```rust
pub enum SessionState {
    Solo,           // Single user editing
    Collaborative,  // Multiple users active
    Conflicting,    // Edit conflict detected
    Resolving,      // Conflict resolution in progress
    Resolved,       // Conflict resolved
}

pub struct QuoteSession {
    pub session_id: SessionId,
    pub quote_id: QuoteId,
    pub state: SessionState,
    pub participants: Vec<SessionParticipant>,
    pub active_edits: Vec<ActiveEdit>,
    pub operation_log: Vec<QuoteOperation>,
    pub last_sync: DateTime<Utc>,
}

pub struct SessionParticipant {
    pub user_id: String,
    pub slack_user_id: String,
    pub role: UserRole,
    pub status: ParticipantStatus,
    pub cursor_position: Option<CursorPosition>,
    pub last_activity: DateTime<Utc>,
}

pub enum ParticipantStatus {
    Viewing,
    Editing { field: String },
    Idle { since: DateTime<Utc> },
}
```

### 2.2 Operational Transform

```rust
pub struct QuoteOperation {
    pub operation_id: String,
    pub user_id: String,
    pub timestamp: DateTime<Utc>,
    pub operation_type: OperationType,
    pub target: OperationTarget,
    pub payload: OperationPayload,
}

pub enum OperationType {
    Insert,
    Update,
    Delete,
}

pub enum OperationTarget {
    LineItem { line_id: String },
    QuoteField { field: String },
    Discount,
    Note,
}

pub struct OperationalTransform;

impl OperationalTransform {
    /// Transform operation B against operation A
    pub fn transform(a: &QuoteOperation, b: &QuoteOperation) -> (QuoteOperation, QuoteOperation) {
        match (&a.target, &b.target) {
            // Same target: authority wins
            (ta, tb) if ta == tb => {
                if a.user_role() > b.user_role() {
                    (a.clone(), QuoteOperation::noop())
                } else {
                    (QuoteOperation::noop(), b.clone())
                }
            }
            // Different targets: both apply
            _ => (a.clone(), b.clone()),
        }
    }
    
    /// Merge concurrent operations
    pub fn merge(operations: &[QuoteOperation]) -> Vec<QuoteOperation> {
        // Sort by authority, then timestamp
        let mut sorted = operations.to_vec();
        sorted.sort_by(|a, b| {
            a.user_role()
                .cmp(&b.user_role())
                .then_with(|| a.timestamp.cmp(&b.timestamp))
        });
        
        // Remove conflicts (lower authority ops on same target)
        let mut result = Vec::new();
        let mut seen_targets = HashSet::new();
        
        for op in sorted {
            if seen_targets.insert(op.target.clone()) {
                result.push(op);
            }
        }
        
        result
    }
}
```

---

## 3. Authority-Based Conflict Resolution

```rust
pub enum UserRole {
    SalesRep = 1,
    SalesManager = 2,
    VP_Sales = 3,
    DealDesk = 4,
    Admin = 5,
}

pub struct ConflictResolver;

impl ConflictResolver {
    pub fn resolve(operations: &[QuoteOperation]) -> Resolution {
        // Group by target
        let mut by_target: HashMap<OperationTarget, Vec<&QuoteOperation>> = HashMap::new();
        for op in operations {
            by_target.entry(op.target.clone()).or_default().push(op);
        }
        
        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        
        for (target, ops) in by_target {
            if ops.len() == 1 {
                accepted.push(ops[0].clone());
            } else {
                // Conflict: highest authority wins
                let winner = ops.iter()
                    .max_by_key(|op| op.user_role())
                    .unwrap();
                
                accepted.push((*winner).clone());
                
                for op in ops {
                    if op.operation_id != winner.operation_id {
                        rejected.push(RejectedOperation {
                            operation: (*op).clone(),
                            reason: RejectionReason::AuthorityOverridden {
                                by_user: winner.user_id.clone(),
                            },
                        });
                    }
                }
            }
        }
        
        Resolution { accepted, rejected }
    }
}
```

---

## 4. Real-Time Sync

### 4.1 Session Manager

```rust
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<SessionId, QuoteSession>>>,
    event_bus: Arc<dyn EventBus>,
}

impl SessionManager {
    /// Start collaborative session
    pub async fn start_session(&self, quote_id: &QuoteId, user_id: &str) -> Result<SessionId, SessionError> {
        let session = QuoteSession {
            session_id: generate_session_id(),
            quote_id: quote_id.clone(),
            state: SessionState::Solo,
            participants: vec![self.create_participant(user_id).await?],
            active_edits: Vec::new(),
            operation_log: Vec::new(),
            last_sync: Utc::now(),
        };
        
        let session_id = session.session_id.clone();
        self.sessions.write().await.insert(session_id.clone(), session);
        
        Ok(session_id)
    }
    
    /// Join existing session
    pub async fn join_session(&self, session_id: &SessionId, user_id: &str) -> Result<(), SessionError> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or(SessionError::SessionNotFound)?;
        
        // Add participant
        let participant = self.create_participant(user_id).await?;
        session.participants.push(participant);
        
        // Transition to collaborative if multiple participants
        if session.participants.len() > 1 {
            session.state = SessionState::Collaborative;
        }
        
        // Notify others
        self.event_bus.publish(SessionEvent::UserJoined {
            session_id: session_id.clone(),
            user_id: user_id.to_string(),
        }).await?;
        
        Ok(())
    }
    
    /// Apply operation from user
    pub async fn apply_operation(
        &self,
        session_id: &SessionId,
        operation: QuoteOperation,
    ) -> Result<ApplyResult, SessionError> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or(SessionError::SessionNotFound)?;
        
        // Check for concurrent operations
        let concurrent: Vec<_> = session.active_edits.iter()
            .filter(|e| e.user_id != operation.user_id)
            .map(|e| e.operation.clone())
            .collect();
        
        if concurrent.is_empty() {
            // No conflict
            session.operation_log.push(operation.clone());
            
            self.event_bus.publish(SessionEvent::OperationApplied {
                session_id: session_id.clone(),
                operation,
            }).await?;
            
            Ok(ApplyResult::Applied)
        } else {
            // Conflict detected
            session.state = SessionState::Conflicting;
            
            // Resolve
            let mut all_ops = concurrent;
            all_ops.push(operation);
            
            let resolution = ConflictResolver::resolve(&all_ops);
            
            session.state = SessionState::Resolved;
            
            // Apply accepted operations
            for op in &resolution.accepted {
                session.operation_log.push(op.clone());
            }
            
            self.event_bus.publish(SessionEvent::ConflictResolved {
                session_id: session_id.clone(),
                resolution: resolution.clone(),
            }).await?;
            
            Ok(ApplyResult::ConflictResolved(resolution))
        }
    }
}
```

---

## 5. Slack Integration

```rust
pub struct CollaborativeSessionRenderer;

impl CollaborativeSessionRenderer {
    pub fn render_session_card(session: &QuoteSession) -> MessageTemplate {
        let mut builder = MessageBuilder::new("Collaborative Session");
        
        // Header with participant count
        let active_count = session.participants.iter()
            .filter(|p| matches!(p.status, ParticipantStatus::Editing { .. }))
            .count();
        
        builder = builder.section("header", |s| {
            s.mrkdwn(format!(
                "🟢 *Live Session Active*\n{} participants | {} editing",
                session.participants.len(),
                active_count
            ))
        });
        
        // Participant list with status
        for participant in &session.participants {
            let status_emoji = match &participant.status {
                ParticipantStatus::Viewing => "👁️",
                ParticipantStatus::Editing { field } => "✏️",
                ParticipantStatus::Idle { .. } => "💤",
            };
            
            builder = builder.context(&participant.user_id, |c| {
                c.mrkdwn(format!(
                    "{} <@{}> ({})",
                    status_emoji,
                    participant.slack_user_id,
                    participant.role
                ))
            });
        }
        
        // Recent changes
        if let Some(last_op) = session.operation_log.last() {
            builder = builder.section("recent", |s| {
                s.mrkdwn(format!(
                    "*Last change:* {} (by <@{}>, {}s ago)",
                    last_op.operation_type,
                    last_op.user_id,
                    (Utc::now() - last_op.timestamp).num_seconds()
                ))
            });
        }
        
        // Actions
        builder = builder.actions("actions", |a| {
            a.button(ButtonElement::new("done", "✓ Done Editing").style(ButtonStyle::Primary))
             .button(ButtonElement::new("refresh", "🔄 Refresh"))
             .button(ButtonElement::new("leave", "Leave Session"))
        });
        
        builder.build()
    }
    
    pub fn render_conflict_notification(resolution: &Resolution) -> MessageTemplate {
        let rejected_count = resolution.rejected.len();
        
        MessageBuilder::new("Edit Conflict Resolved")
            .section("header", |s| {
                s.mrkdwn(format!(
                    "⚠️ *Edit Conflict Resolved*\n{} conflicting edit{} overridden by authority",
                    rejected_count,
                    if rejected_count == 1 { "" } else { "s" }
                ))
            })
            .section("details", |s| {
                let details: Vec<String> = resolution.rejected.iter()
                    .map(|r| format!(
                        "• Your change to {} was overridden by {}",
                        r.operation.target,
                        r.reason
                    ))
                    .collect();
                s.mrkdwn(details.join("\n"))
            })
            .build()
    }
}
```

---

## 6. Database Schema

```sql
-- Collaborative sessions
CREATE TABLE quote_sessions (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL REFERENCES quote(id),
    state TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    created_by TEXT NOT NULL
);

-- Session participants
CREATE TABLE session_participants (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES quote_sessions(id),
    user_id TEXT NOT NULL,
    role TEXT NOT NULL,
    joined_at TEXT NOT NULL,
    left_at TEXT,
    last_activity_at TEXT NOT NULL
);

-- Operation log (CRDT-style)
CREATE TABLE session_operations (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES quote_sessions(id),
    user_id TEXT NOT NULL,
    operation_type TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id TEXT,
    payload_json TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    applied BOOLEAN DEFAULT TRUE,
    rejected_reason TEXT
);

-- Active edits (ephemeral)
CREATE TABLE active_edits (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES quote_sessions(id),
    user_id TEXT NOT NULL,
    target_field TEXT NOT NULL,
    started_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);
```

---

## 7. WebSocket/Real-Time Considerations

Since Slack doesn't provide WebSocket for bots directly, use:

1. **Slack Events API**: Receive edit commands
2. **Chat.update**: Push updates to thread
3. **Polling fallback**: For users not actively editing

---

*Research compiled by ResearchAgent for the quotey project.*
