# VNX-06: Reliability Outbox + Guaranteed Delivery Replay - Design Document

**Bead:** quotey-1req.1.1  
**Status:** In Progress  
**Author:** Kimi Code CLI  
**Date:** March 6, 2026

---

## 1. Executive Summary

This design extends Quotey's existing execution queue infrastructure to provide **guaranteed delivery** for all side effects (Slack messages, CRM sync, PDF generation, emails). The outbox pattern ensures:

- **At-least-once delivery** with idempotency
- **Automatic retry** with exponential backoff
- **Manual replay** for failed operations
- **Complete audit trail** for compliance

---

## 2. Current State Analysis

### 2.1 Existing Infrastructure (Already Built!)

| Component | Location | Status |
|-----------|----------|--------|
| `execution_queue_task` table | `migrations/0012_execution_queue_rel.sql` | ✅ Exists |
| `execution_idempotency_ledger` table | Same migration | ✅ Exists |
| `execution_queue_transition_audit` table | Same migration | ✅ Exists |
| `ExecutionTask` domain model | `crates/core/src/domain/execution.rs` | ✅ Exists |
| `ExecutionQueueRepository` | `crates/db/src/repositories/execution_queue.rs` | ✅ Exists |
| CRM sync with retries | `crates/server/src/crm.rs` | ✅ Partial |

### 2.2 Gap Analysis

**Current Side Effects (Scattered):**
```
crates/slack/src/          -> Direct Slack API calls (no retry)
crates/server/src/pdf.rs   -> Direct PDF generation (no persistence)
crates/server/src/crm.rs   -> Has retry logic (good pattern)
crates/mcp/src/server.rs   -> Tool invocations (needs audit)
```

**Missing:**
- Unified outbox for all side effects
- Replay capability for failed operations
- Dead-letter queue for manual intervention
- Observability dashboard

---

## 3. Proposed Architecture

### 3.1 Design Principles

1. **Reuse existing infrastructure** - Extend `execution_queue`, don't replace
2. **Opt-in migration** - Side effects gradually move to outbox
3. **Deterministic idempotency** - Same input = same idempotency key
4. **Observable state** - Clear visibility into pending/failed operations

### 3.2 High-Level Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         OUTBOX PATTERN ARCHITECTURE                          │
└─────────────────────────────────────────────────────────────────────────────┘

1. SIDE EFFECT REQUEST
   ┌──────────────┐     ┌──────────────┐     ┌──────────────────────────────┐
   │ Quote Created │────▶│ Outbox Entry │────▶│ SQLite (execution_queue_task)│
   └──────────────┘     └──────────────┘     └──────────────────────────────┘
                              │
                              ▼
                        ┌──────────────┐
                        │ Idempotency  │────▶ Prevent duplicates
                        │ Key Check    │
                        └──────────────┘

2. ASYNC PROCESSING (Worker)
   ┌─────────────────────────────────────────────────────────────────────────┐
   │                         Outbox Worker (Tokio)                            │
   │  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌───────────┐ │
   │  │ Poll Queue  │───▶│ Claim Task  │───▶│ Execute     │───▶│ Record    │ │
   │  │ (pending)   │    │ (atomic)    │    │ Side Effect │    │ Result    │ │
   │  └─────────────┘    └─────────────┘    └─────────────┘    └───────────┘ │
   │         │                                                    │          │
   │         │                                                    ▼          │
   │         │                                           ┌───────────────┐   │
   │         │                                           │ State: Done   │   │
   │         │                                           │ or Failed     │   │
   │         │                                           └───────────────┘   │
   │         │                                                                │
   │         └───────────────────────────────────────────────────────────────▶│
   │                    Retry with exponential backoff                        │
   └─────────────────────────────────────────────────────────────────────────┘

3. OBSERVABILITY & REPLAY
   ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
   │ Admin API    │────▶│ List Failed  │────▶│ Manual Replay│
   │ /admin/outbox│     │ Operations   │     │ /replay      │
   └──────────────┘     └──────────────┘     └──────────────┘
```

---

## 4. Data Model

### 4.1 Extend Existing `execution_queue_task` Table

The table already exists. We add **operation_kind** handlers:

```rust
// crates/core/src/domain/outbox.rs

/// Side effect operation types supported by the outbox
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutboxOperation {
    // Slack operations
    SlackPostMessage { channel: String, text: String },
    SlackUpdateBlocks { channel: String, ts: String, blocks: String },
    SlackUploadFile { channel: String, filename: String, content: Vec<u8> },
    
    // CRM operations
    CrmSyncQuote { provider: String, quote_id: QuoteId },
    CrmCreateDeal { provider: String, payload: CrmDealPayload },
    
    // Document operations
    PdfGenerate { quote_id: QuoteId, template: String },
    EmailSend { to: String, subject: String, body: String, attachment: Option<String> },
    
    // Custom operations
    WebhookCall { url: String, method: String, headers: HashMap<String, String>, body: String },
}

impl OutboxOperation {
    /// Deterministic idempotency key based on operation content
    pub fn idempotency_key(&self, quote_id: &QuoteId) -> OperationKey {
        let content = format!("{:?}", self);
        let hash = blake3::hash(content.as_bytes());
        OperationKey(format!("{}-{}", quote_id.0, hash.to_hex()))
    }
    
    /// Operation kind for metrics/alerting
    pub fn kind(&self) -> &'static str {
        match self {
            Self::SlackPostMessage { .. } => "slack.post_message",
            Self::SlackUpdateBlocks { .. } => "slack.update_blocks",
            Self::SlackUploadFile { .. } => "slack.upload_file",
            Self::CrmSyncQuote { provider, .. } => &format!("crm.{}.sync_quote", provider),
            Self::CrmCreateDeal { provider, .. } => &format!("crm.{}.create_deal", provider),
            Self::PdfGenerate { .. } => "pdf.generate",
            Self::EmailSend { .. } => "email.send",
            Self::WebhookCall { .. } => "webhook.call",
        }
    }
}
```

### 4.2 New Table: `outbox_dead_letter` (Manual Intervention)

```sql
-- Migration: add dead letter queue for failed operations
CREATE TABLE outbox_dead_letter (
    id TEXT PRIMARY KEY,                          -- Original task ID
    quote_id TEXT NOT NULL,
    operation_kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    
    -- Failure context
    failed_at TEXT NOT NULL,
    failure_reason TEXT NOT NULL,
    error_class TEXT,
    stack_trace TEXT,
    retry_count INTEGER NOT NULL,
    max_retries INTEGER NOT NULL,
    
    -- Manual intervention
    resolution_status TEXT NOT NULL DEFAULT 'pending', -- pending, replayed, abandoned
    resolved_by TEXT,
    resolved_at TEXT,
    resolution_notes TEXT,
    
    -- Audit
    created_at TEXT NOT NULL,
    original_created_at TEXT NOT NULL,
    
    FOREIGN KEY (quote_id) REFERENCES quote(id)
);

CREATE INDEX idx_outbox_dl_resolution ON outbox_dead_letter(resolution_status, failed_at);
CREATE INDEX idx_outbox_dl_quote ON outbox_dead_letter(quote_id);
```

### 4.3 Task State Machine

```
                    ┌─────────────────────────────────────┐
                    │              QUEUED                 │
                    │  (available_at <= now, not claimed) │
                    └─────────────────┬───────────────────┘
                                      │ Worker polls
                                      ▼
                    ┌─────────────────────────────────────┐
                    │              CLAIMED                │
                    │  (claimed_by, claimed_at set)       │
                    │  TTL: 5 minutes (auto-release)      │
                    └─────────────────┬───────────────────┘
                                      │ Execute
                            ┌─────────┴─────────┐
                            │                   │
                            ▼                   ▼
              ┌───────────────────┐   ┌───────────────────┐
              │    COMPLETED      │   │      FAILED       │
              │ (result recorded) │   │ (error logged)    │
              └───────────────────┘   └─────────┬─────────┘
                                                │
                            ┌───────────────────┼───────────────────┐
                            │                   │                   │
                            ▼                   ▼                   ▼
              ┌───────────────────┐   ┌───────────────────┐   ┌──────────┐
              │   Auto-retry      │   │  Max retries      │   │ Permanent│
              │ (backoff +        │   │  exceeded?        │   │ failure  │
              │  re-queue)        │   │                   │   │          │
              └───────────────────┘   └─────────┬─────────┘   └──────────┘
                                                │
                                                ▼
                                    ┌───────────────────┐
                                    │   DEAD LETTER     │
                                    │ (manual review)   │
                                    └───────────────────┘
```

---

## 5. API Design

### 5.1 Outbox Service Interface

```rust
// crates/core/src/services/outbox.rs

#[async_trait]
pub trait OutboxService: Send + Sync {
    /// Enqueue a side effect for guaranteed delivery
    async fn enqueue(&self, quote_id: &QuoteId, operation: OutboxOperation) -> Result<ExecutionTaskId>;
    
    /// Get status of a specific operation
    async fn get_status(&self, task_id: &ExecutionTaskId) -> Result<Option<OutboxStatus>>;
    
    /// List pending operations for a quote
    async fn list_pending(&self, quote_id: &QuoteId) -> Result<Vec<OutboxEntry>>;
    
    /// List failed operations (for admin dashboard)
    async fn list_failed(&self, limit: usize) -> Result<Vec<DeadLetterEntry>>;
    
    /// Manually replay a failed operation
    async fn replay(&self, dead_letter_id: &str) -> Result<ExecutionTaskId>;
    
    /// Abandon a failed operation (manual override)
    async fn abandon(&self, dead_letter_id: &str, reason: &str) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct OutboxStatus {
    pub task_id: ExecutionTaskId,
    pub state: ExecutionTaskState,
    pub operation_kind: String,
    pub retry_count: u32,
    pub max_retries: u32,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}
```

### 5.2 Admin HTTP Endpoints

```rust
// crates/server/src/admin_outbox.rs

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/outbox/pending", get(list_pending))
        .route("/admin/outbox/failed", get(list_failed))
        .route("/admin/outbox/failed/:id/replay", post(replay_failed))
        .route("/admin/outbox/failed/:id/abandon", post(abandon_failed))
        .route("/admin/outbox/stats", get(outbox_stats))
}

// GET /admin/outbox/failed?limit=50
// Response:
#[derive(Serialize)]
struct FailedOperationResponse {
    id: String,
    quote_id: String,
    operation_kind: String,
    failed_at: String,
    failure_reason: String,
    retry_count: i32,
}

// POST /admin/outbox/failed/:id/replay
// Response: { "new_task_id": "..." }
```

---

## 6. Retry Policy

### 6.1 Exponential Backoff with Jitter

```rust
// crates/core/src/services/outbox_retry.rs

pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub jitter_factor: f64, // 0.0 - 1.0
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 5,
            base_delay: Duration::seconds(30),
            max_delay: Duration::hours(1),
            jitter_factor: 0.2,
        }
    }
}

impl RetryPolicy {
    pub fn next_retry_at(&self, attempt: u32) -> Option<DateTime<Utc>> {
        if attempt >= self.max_retries {
            return None;
        }
        
        // Exponential: 30s, 60s, 120s, 240s, 480s (capped at 1 hour)
        let exponential = self.base_delay * 2_u32.pow(attempt);
        let delay = std::cmp::min(exponential, self.max_delay);
        
        // Add jitter (±20%)
        let jitter_secs = delay.num_seconds() as f64 * self.jitter_factor;
        let jitter_range = -jitter_secs..=jitter_secs;
        let jitter = rand::random::<f64>() * (jitter_range.end() - jitter_range.start()) 
                   + jitter_range.start();
        let final_delay = Duration::seconds((delay.num_seconds() as f64 + jitter) as i64);
        
        Some(Utc::now() + final_delay)
    }
}

// Per-operation-type defaults
impl OutboxOperation {
    pub fn retry_policy(&self) -> RetryPolicy {
        match self {
            // Slack: retry quickly (user-facing)
            Self::SlackPostMessage { .. } => RetryPolicy {
                max_retries: 3,
                base_delay: Duration::seconds(5),
                max_delay: Duration::minutes(5),
                ..Default::default()
            },
            
            // CRM: retry more aggressively (business-critical)
            Self::CrmSyncQuote { .. } => RetryPolicy {
                max_retries: 10,
                base_delay: Duration::seconds(30),
                max_delay: Duration::hours(2),
                ..Default::default()
            },
            
            // Email: standard retry
            Self::EmailSend { .. } => RetryPolicy::default(),
            
            // Webhook: longer backoff (external dependency)
            Self::WebhookCall { .. } => RetryPolicy {
                max_retries: 8,
                base_delay: Duration::minutes(1),
                max_delay: Duration::hours(4),
                ..Default::default()
            },
            
            _ => RetryPolicy::default(),
        }
    }
}
```

---

## 7. Worker Implementation

### 7.1 Outbox Worker (Tokio Task)

```rust
// crates/server/src/outbox_worker.rs

pub struct OutboxWorker {
    db: DbPool,
    outbox: Arc<dyn OutboxService>,
    slack: Arc<dyn SlackClient>,
    crm: Arc<dyn CrmClient>,
    pdf: Arc<dyn PdfGenerator>,
    email: Arc<dyn EmailClient>,
}

impl OutboxWorker {
    pub async fn run(&self, shutdown: ShutdownSignal) {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.process_batch().await {
                        error!("Outbox worker error: {}", e);
                    }
                }
                _ = shutdown.recv() => {
                    info!("Outbox worker shutting down");
                    break;
                }
            }
        }
    }
    
    async fn process_batch(&self) -> Result<()> {
        // 1. Claim pending tasks (atomic)
        let tasks = self.outbox.claim_pending(BATCH_SIZE).await?;
        
        // 2. Process in parallel (with concurrency limit)
        let stream = futures::stream::iter(tasks)
            .map(|task| self.process_task(task))
            .buffer_unordered(CONCURRENCY_LIMIT);
        
        let results: Vec<_> = stream.collect().await;
        
        // 3. Log summary
        let succeeded = results.iter().filter(|r| r.is_ok()).count();
        let failed = results.len() - succeeded;
        info!("Outbox batch: {} succeeded, {} failed", succeeded, failed);
        
        Ok(())
    }
    
    async fn process_task(&self, task: ExecutionTask) -> Result<()> {
        let operation: OutboxOperation = serde_json::from_str(&task.payload_json)?;
        
        let result = match &operation {
            OutboxOperation::SlackPostMessage { .. } => {
                self.slack.execute(operation).await
            }
            OutboxOperation::CrmSyncQuote { .. } | OutboxOperation::CrmCreateDeal { .. } => {
                self.crm.execute(operation).await
            }
            OutboxOperation::PdfGenerate { .. } => {
                self.pdf.execute(operation).await
            }
            OutboxOperation::EmailSend { .. } => {
                self.email.execute(operation).await
            }
            OutboxOperation::WebhookCall { .. } => {
                self.execute_webhook(operation).await
            }
            _ => Err(Error::UnsupportedOperation(operation.kind())),
        };
        
        // Record result
        match result {
            Ok(output) => {
                self.outbox.complete(&task.id, output).await?;
            }
            Err(e) => {
                self.outbox.fail(&task.id, &e).await?;
            }
        }
        
        Ok(())
    }
}
```

---

## 8. Migration Strategy

### 8.1 Phase 1: Instrumentation (Week 1)
- Add outbox enqueue calls alongside existing side effects
- Log when operations would be enqueued (dry-run mode)
- Monitor for idempotency key collisions

### 8.2 Phase 2: Shadow Mode (Week 2)
- Enqueue to outbox AND execute synchronously
- Compare results, log discrepancies
- Fix any idempotency issues

### 8.3 Phase 3: Async Cutover (Week 3)
- CRM sync moves to async (already close!)
- Slack notifications move to async
- PDF generation stays sync (user-facing)

### 8.4 Phase 4: Cleanup (Week 4)
- Remove synchronous fallback paths
- Add admin dashboard
- Document operational runbooks

---

## 9. Observability

### 9.1 Metrics

```rust
// crates/core/src/metrics.rs

pub struct OutboxMetrics {
    /// Total operations enqueued
    pub enqueue_total: Counter,
    
    /// Operations completed successfully
    pub complete_total: Counter,
    
    /// Operations failed (will retry)
    pub fail_retryable_total: Counter,
    
    /// Operations moved to dead letter
    pub dead_letter_total: Counter,
    
    /// Current queue depth by state
    pub queue_depth: GaugeVec,
    
    /// Processing latency histogram
    pub processing_duration: HistogramVec,
    
    /// Time spent in queue before processing
    pub queue_wait_duration: Histogram,
}
```

### 9.2 Alerting Rules

```yaml
# Alert when outbox has stale items
- alert: OutboxStaleItems
  expr: outbox_queue_depth{state="pending"} > 0
  for: 30m
  labels:
    severity: warning
  annotations:
    summary: "Outbox has items pending for > 30 minutes"
    
# Alert when dead letter queue grows
- alert: OutboxDeadLetterGrowing
  expr: rate(outbox_dead_letter_total[1h]) > 0.1
  for: 5m
  labels:
    severity: critical
  annotations:
    summary: "Outbox dead letter queue is growing"
```

---

## 10. Security Considerations

1. **Idempotency keys** must be deterministic but unguessable (use HMAC)
2. **Dead letter access** restricted to admin role only
3. **Payload encryption** for sensitive fields (PII in email addresses)
4. **Audit trail** for all manual replay/abandon actions

---

## 11. Acceptance Criteria

- [ ] All CRM sync operations use outbox
- [ ] Slack notifications use outbox
- [ ] Failed operations auto-retry with exponential backoff
- [ ] Max retry exceeded → dead letter queue
- [ ] Admin API for listing/replaying failed operations
- [ ] Metrics and alerting configured
- [ ] Documentation updated

---

## 12. Appendix: Idempotency Key Algorithm

```rust
/// Deterministic, collision-resistant idempotency key
/// 
/// Format: {quote_id}:{operation_kind}:{hash}
/// Example: Q-2026-0042:slack.post_message:a3f7b2...
pub fn generate_idempotency_key(
    quote_id: &QuoteId,
    operation: &OutboxOperation,
) -> OperationKey {
    use blake3::Hasher;
    
    let mut hasher = Hasher::new();
    
    // Domain separation
    hasher.update(b"quotey:outbox:v1");
    
    // Quote scope
    hasher.update(quote_id.0.as_bytes());
    
    // Operation type
    hasher.update(operation.kind().as_bytes());
    
    // Canonical JSON of payload (sorted keys)
    let canonical_json = serde_json::to_string(operation).unwrap();
    hasher.update(canonical_json.as_bytes());
    
    let hash = hasher.finalize();
    
    OperationKey(format!(
        "{}:{}:{:.16}",
        quote_id.0,
        operation.kind(),
        hash.to_hex()
    ))
}
```

---

**End of Design Document**
