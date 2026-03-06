# Security & Reliability Hardening Plan

**Document Version:** 1.0  
**Last Updated:** 2025-03-06  
**Status:** Draft for Review  

## Executive Summary

This plan addresses 20+ security and reliability issues identified through comprehensive code review of the Quotey CPQ system. Issues span memory exhaustion vectors, integer overflow risks, DoS vulnerabilities, and architectural gaps.

---

## 1. Threat Model

### 1.1 Attack Vectors

| Vector | Risk Level | Description |
|--------|-----------|-------------|
| Resource Exhaustion | 🔴 Critical | Unbounded collections, unvalidated inputs |
| Timing Attacks | 🟠 High | Side-channel leakage in string comparison |
| Integer Overflow | 🔴 Critical | Retry backoff calculation, dimension calculations |
| Injection | 🟡 Medium | SQL injection in dynamic filters (audit required) |
| Panic Propagation | 🟠 High | Unwrapped results in async contexts |

### 1.2 Trust Boundaries

```
┌─────────────────────────────────────────────────────────────┐
│                        Slack API                             │
│                   (Untrusted - Rate Limit)                   │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│                    Slack Bot Handler                         │
│              (Input Validation, Sanitization)                │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│                    Agent Runtime                             │
│         (Intent Extraction, Guardrails, Quotas)              │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│              Deterministic Flow Engine                       │
│      (State Machine, Validation, Policy Engine)              │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│                     CPQ Core                                 │
│    (Pricing Engine, Constraint Engine - Deterministic)       │
└───────────────────────┬─────────────────────────────────────┘
                        │
┌───────────────────────▼─────────────────────────────────────┐
│                     Data Layer                               │
│           (SQLite, Connection Pooling, WAL)                  │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Issue Registry

### 2.1 Critical Issues (P0 - Fix Immediately)

| ID | Issue | Location | Impact | Effort |
|----|-------|----------|--------|--------|
| **SEC-001** | Unbounded audit event storage | `audit.rs` | Memory exhaustion, OOM kill | 2h |
| **SEC-002** | Integer overflow in backoff calc | `execution_engine.rs` | Retry panic, stuck workflows | 1h |
| **SEC-003** | Unbounded metadata in audit events | `audit.rs` | DoS via memory exhaustion | 2h |
| **SEC-004** | No graceful shutdown handling | `server/` | Request loss on deploy | 4h |

### 2.2 High Priority Issues (P1 - Fix This Week)

| ID | Issue | Location | Impact | Effort |
|----|-------|----------|--------|--------|
| **SEC-101** | Blocking file I/O in async context | `mcp/server.rs` | Thread pool starvation | 3h |
| **SEC-102** | Silent data loss in version cast | `repositories/quote.rs` | Data corruption | 1h |
| **SEC-103** | Levenshtein allocation per char | `slack/commands.rs` | Quadratic performance | 2h |
| **SEC-104** | Unbounded quote line items | `domain/quote.rs` | DoS via large quotes | 2h |
| **SEC-105** | Panic risk in error handlers | `server/health.rs` | Service crash | 1h |

### 2.3 Medium Priority Issues (P2 - Fix Next Sprint)

| ID | Issue | Location | Impact | Effort |
|----|-------|----------|--------|--------|
| **SEC-201** | SQL injection in dynamic filters | `repositories/` | Data exfiltration | 8h |
| **SEC-202** | Error message information leakage | Throughout | Info disclosure | 4h |
| **SEC-203** | No circuit breaker pattern | External calls | Cascading failures | 8h |
| **SEC-204** | Inconsistent error types | Throughout | Maintainability | 4h |
| **SEC-205** | No resource quotas per tenant | Multi-tenant | Fairness violations | 16h |

### 2.4 Low Priority Issues (P3 - Backlog)

| ID | Issue | Location | Impact | Effort |
|----|-------|----------|--------|--------|
| **SEC-301** | Missing security headers | HTTP responses | XSS vectors | 2h |
| **SEC-302** | No request ID propagation | Logging | Debug difficulty | 4h |
| **SEC-303** | Timing side-channels | String comparison | Crypto risks | 4h |

---

## 3. Implementation Phases

### Phase 1: Resource Limits (Week 1)

**Goal:** Prevent resource exhaustion attacks

#### 3.1.1 Audit System Hardening

```rust
// crates/core/src/audit.rs

/// Maximum audit events in memory before rotation
pub const MAX_AUDIT_EVENTS: usize = 100_000;

/// Maximum metadata entries per event
pub const MAX_METADATA_ENTRIES: usize = 100;

/// Maximum metadata key length
pub const MAX_METADATA_KEY_LEN: usize = 256;

/// Maximum metadata value length  
pub const MAX_METADATA_VALUE_LEN: usize = 4096;

/// Ring buffer for bounded memory usage
pub struct BoundedAuditSink {
    events: Arc<Mutex<VecDeque<AuditEvent>>>,
    max_events: usize,
    dropped_events: AtomicU64, // Metrics
}

impl AuditSink for BoundedAuditSink {
    fn emit(&self, event: AuditEvent) {
        if let Ok(mut events) = self.events.lock() {
            if events.len() >= self.max_events {
                let dropped = events.pop_front();
                self.dropped_events.fetch_add(1, Ordering::Relaxed);
                
                // Log rotation event for observability
                tracing::warn!(?dropped, "audit event rotated out of buffer");
            }
            events.push_back(event);
        }
    }
}
```

**Validation:**
- [ ] Memory usage flat at 100k events
- [ ] Rotation events logged
- [ ] Metrics endpoint exposes drop count

#### 3.1.2 Metadata Size Limits

```rust
impl AuditEvent {
    pub fn with_metadata(
        mut self, 
        key: impl Into<String>, 
        value: impl Into<String>
    ) -> Self {
        let key = key.into();
        let value = value.into();
        
        // Validate key
        if key.len() > MAX_METADATA_KEY_LEN {
            tracing::warn!(key_len = key.len(), "metadata key truncated");
            return self;
        }
        
        // Validate value
        let value = if value.len() > MAX_METADATA_VALUE_LEN {
            tracing::warn!(value_len = value.len(), "metadata value truncated");
            value[..MAX_METADATA_VALUE_LEN].to_string()
        } else {
            value
        };
        
        // Check entry limit
        if self.metadata.len() >= MAX_METADATA_ENTRIES {
            tracing::warn!("metadata entry limit reached, dropping key={}", key);
            return self;
        }
        
        self.metadata.insert(key, value);
        self
    }
}
```

**Validation:**
- [ ] Oversized keys rejected with warning
- [ ] Oversized values truncated
- [ ] Entry limit enforced

#### 3.1.3 Integer Overflow Protection

```rust
// crates/core/src/execution_engine.rs

/// Maximum backoff delay (24 hours)
const MAX_BACKOFF_SECONDS: i64 = 24 * 60 * 60;

fn calculate_backoff(retry_count: u32, config: &RetryConfig) -> time::Duration {
    let base_delay = config.retry_base_delay_seconds.max(0) as u64;
    let exponent = retry_count.min(20); // Cap exponent to prevent huge numbers
    
    // Use checked operations throughout
    let multiplier = u64::from(config.retry_backoff_multiplier)
        .checked_pow(exponent)
        .unwrap_or(u64::MAX);
    
    let backoff_secs = base_delay
        .checked_mul(multiplier)
        .unwrap_or(MAX_BACKOFF_SECONDS as u64)
        .min(MAX_BACKOFF_SECONDS as u64);
    
    // Safe conversion with explicit bounds
    let backoff_secs = backoff_secs.min(i64::MAX as u64) as i64;
    
    time::Duration::seconds(backoff_secs)
}
```

**Validation:**
- [ ] retry_count=100 doesn't panic
- [ ] Backoff capped at 24 hours
- [ ] Unit tests for edge cases

---

### Phase 2: Async Safety (Week 2)

**Goal:** Eliminate blocking operations and improve reliability

#### 3.2.1 Non-blocking File Operations

```rust
// crates/mcp/src/server.rs

use tokio::fs;

async fn render_pdf_template(
    tera: &Tera,
    template_name: &str,
    context: &Context,
) -> Result<String> {
    // Use tokio::fs for all file operations
    // Tera rendering is CPU-bound, use spawn_blocking
    
    let tera = tera.clone();
    let template_name = template_name.to_string();
    let context = context.clone();
    
    tokio::task::spawn_blocking(move || {
        tera.render(&template_name, &context)
            .map_err(|e| anyhow!("Template render failed: {}", e))
    })
    .await
    .context("PDF template rendering panicked")?
}
```

**Validation:**
- [ ] No blocking syscalls in async path
- [ ] PDF generation doesn't stall other requests

#### 3.2.2 Graceful Shutdown

```rust
// crates/server/src/shutdown.rs

use tokio::sync::watch;
use std::time::Duration;

pub struct GracefulShutdown {
    signal: watch::Sender<()>,
    receiver: watch::Receiver<()>,
}

impl GracefulShutdown {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(());
        Self {
            signal: tx,
            receiver: rx,
        }
    }
    
    pub fn shutdown(&self) {
        let _ = self.signal.send(());
    }
    
    pub async fn run_with_shutdown<F, Fut>(
        &self,
        server_future: F,
        timeout: Duration,
    ) where
        F: FnOnce(watch::Receiver<()>) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let mut sigterm = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate()
        ).expect("Failed to create signal handler");
        
        let mut sigint = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::interrupt()
        ).expect("Failed to create signal handler");
        
        tokio::select! {
            _ = sigterm.recv() => {
                tracing::info!("SIGTERM received, initiating graceful shutdown");
                self.shutdown();
            }
            _ = sigint.recv() => {
                tracing::info!("SIGINT received, initiating graceful shutdown");
                self.shutdown();
            }
        }
        
        // Wait for graceful shutdown with timeout
        tokio::time::timeout(timeout, server_future(self.receiver.clone()))
            .await
            .unwrap_or_else(|_| {
                tracing::warn!("Graceful shutdown timed out, forcing exit");
            });
    }
}
```

**Validation:**
- [ ] SIGTERM triggers shutdown
- [ ] In-flight requests complete
- [ ] Timeout respected

---

### Phase 3: Data Integrity (Week 3)

**Goal:** Prevent data corruption and silent failures

#### 3.3.1 Repository Input Validation

```rust
// crates/db/src/repositories/quote.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("data integrity violation: {0}")]
    DataIntegrity(&'static str),
    #[error("input validation failed: {0}")]
    Validation(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

// Safe conversion with validation
fn row_to_quote(row: QuoteRow) -> Result<Quote, RepositoryError> {
    Ok(Quote {
        id: QuoteId(row.id),
        account_id: AccountId(row.account_id),
        version: row.version.try_into()
            .map_err(|_| RepositoryError::DataIntegrity("negative version"))?,
        status: row.status.parse()
            .map_err(|e| RepositoryError::DataIntegrity("invalid status"))?,
        total: Decimal::from_str(&row.total)
            .map_err(|_| RepositoryError::DataIntegrity("invalid decimal"))?,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}
```

**Validation:**
- [ ] Negative versions rejected
- [ ] Invalid decimals caught
- [ ] Error messages don't leak internals

#### 3.3.2 Domain Model Invariants

```rust
// crates/core/src/domain/quote.rs

pub const MAX_LINE_ITEMS: usize = 500;
pub const MAX_LINE_ITEM_DESCRIPTION_LEN: usize = 10_000;

impl Quote {
    pub fn add_line(&mut self, line: QuoteLine) -> Result<(), QuoteError> {
        // Check line item limit
        if self.lines.len() >= MAX_LINE_ITEMS {
            return Err(QuoteError::TooManyLineItems {
                current: self.lines.len(),
                maximum: MAX_LINE_ITEMS,
            });
        }
        
        // Validate description length
        if line.description.len() > MAX_LINE_ITEM_DESCRIPTION_LEN {
            return Err(QuoteError::DescriptionTooLong {
                len: line.description.len(),
                max: MAX_LINE_ITEM_DESCRIPTION_LEN,
            });
        }
        
        // Validate quantity
        if line.quantity <= Decimal::ZERO {
            return Err(QuoteError::InvalidQuantity);
        }
        
        self.lines.push(line);
        self.recalculate_total();
        
        Ok(())
    }
}
```

**Validation:**
- [ ] 501st line item rejected
- [ ] Negative quantities rejected
- [ ] Descriptions truncated at limit

---

### Phase 4: Performance Hardening (Week 4)

**Goal:** Eliminate performance cliffs and DoS vectors

#### 3.4.1 Levenshtein Optimization

```rust
// crates/slack/src/commands.rs

/// Optimized Levenshtein with buffer reuse
fn levenshtein_distance(a: &str, b: &str) -> usize {
    if a.is_empty() { return b.len(); }
    if b.is_empty() { return a.len(); }
    
    // Use byte slices for ASCII-fast path
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    
    // Pre-allocate buffers
    let mut prev: Vec<usize> = (0..=b_bytes.len()).collect();
    let mut curr: Vec<usize> = vec![0; b_bytes.len() + 1];
    
    for (i, &ca) in a_bytes.iter().enumerate() {
        curr[0] = i + 1;
        
        for (j, &cb) in b_bytes.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            
            curr[j + 1] = (prev[j + 1] + 1)     // deletion
                .min(curr[j] + 1)                // insertion
                .min(prev[j] + cost);           // substitution
        }
        
        // Reuse buffers instead of allocating
        std::mem::swap(&mut prev, &mut curr);
    }
    
    prev[b_bytes.len()]
}

/// Early exit variant for suggestion matching
fn levenshtein_distance_limited(a: &str, b: &str, limit: usize) -> Option<usize> {
    let dist = levenshtein_distance(a, b);
    if dist <= limit { Some(dist) } else { None }
}
```

**Validation:**
- [ ] Benchmark: 10x improvement on 1000 char strings
- [ ] No allocations per character
- [ ] Results identical to previous implementation

---

### Phase 5: Security Architecture (Ongoing)

#### 3.5.1 Circuit Breaker Pattern

```rust
// crates/core/src/circuit_breaker.rs

pub struct CircuitBreaker {
    state: Arc<RwLock<State>>,
    failure_threshold: u32,
    reset_timeout: Duration,
    half_open_max_calls: u32,
}

enum State {
    Closed,           // Normal operation
    Open { until: Instant },           // Failing, reject fast
    HalfOpen { attempts: u32 },        // Testing recovery
}

impl CircuitBreaker {
    pub async fn call<F, Fut, T>(&self, f: F) -> Result<T, CircuitBreakerError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, Error>>,
    {
        // Check state
        match *self.state.read().await {
            State::Open { until } if until > Instant::now() => {
                return Err(CircuitBreakerError::Open);
            }
            State::Open { .. } => {
                // Transition to half-open
                *self.state.write().await = State::HalfOpen { attempts: 0 };
            }
            _ => {}
        }
        
        // Execute call
        match f().await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(e) => {
                self.on_failure().await;
                Err(CircuitBreakerError::Underlying(e))
            }
        }
    }
}
```

#### 3.5.2 Request Quotas

```rust
// crates/core/src/quotas.rs

pub struct ResourceQuota {
    max_line_items_per_quote: usize,
    max_quotes_per_hour: u32,
    max_pdf_size_bytes: usize,
    max_concurrent_calculations: usize,
}

impl ResourceQuota {
    pub fn default_tenant() -> Self {
        Self {
            max_line_items_per_quote: 500,
            max_quotes_per_hour: 100,
            max_pdf_size_bytes: 10 * 1024 * 1024, // 10MB
            max_concurrent_calculations: 10,
        }
    }
    
    pub fn enterprise() -> Self {
        Self {
            max_line_items_per_quote: 2000,
            max_quotes_per_hour: 1000,
            max_pdf_size_bytes: 50 * 1024 * 1024, // 50MB
            max_concurrent_calculations: 50,
        }
    }
}
```

---

## 4. Testing Strategy

### 4.1 Unit Tests

```rust
#[cfg(test)]
mod security_tests {
    use super::*;
    
    #[test]
    fn test_audit_buffer_rotation() {
        let sink = BoundedAuditSink::new(10);
        
        for i in 0..15 {
            sink.emit(AuditEvent::new("test").with_field("i", i));
        }
        
        let events = sink.events.lock().unwrap();
        assert_eq!(events.len(), 10);
        assert_eq!(events.front().unwrap().get_field("i"), 5); // First 5 rotated out
    }
    
    #[test]
    fn test_backoff_overflow_protection() {
        let config = RetryConfig {
            retry_base_delay_seconds: 1,
            retry_backoff_multiplier: 2,
        };
        
        // retry_count=100 would overflow without protection
        let backoff = calculate_backoff(100, &config);
        assert!(backoff <= Duration::hours(24));
    }
    
    #[test]
    fn test_metadata_size_limits() {
        let event = AuditEvent::new("test")
            .with_metadata("key", "x".repeat(10_000)); // Too long
        
        assert!(event.metadata.get("key").unwrap().len() <= MAX_METADATA_VALUE_LEN);
    }
}
```

### 4.2 Integration Tests

```rust
#[tokio::test]
async fn test_graceful_shutdown() {
    let shutdown = GracefulShutdown::new();
    let server = spawn_test_server(shutdown.receiver());
    
    // Send request
    let client = reqwest::Client::new();
    let response = client.get("http://localhost:8080/health").send().await;
    assert!(response.is_ok());
    
    // Trigger shutdown
    shutdown.shutdown();
    
    // Wait for completion
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("Shutdown timed out");
}
```

### 4.3 Fuzzing Targets

```rust
// fuzz_targets/quote_parsing.rs

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Should not panic on any input
        let _ = QuoteId::parse(s);
        let _ = parse_requirements(s);
    }
});
```

---

## 5. Monitoring & Alerting

### 5.1 Key Metrics

| Metric | Type | Threshold | Alert |
|--------|------|-----------|-------|
| `audit_events_dropped_total` | Counter | > 0 | Warning |
| `audit_buffer_utilization` | Gauge | > 80% | Warning |
| `circuit_breaker_state` | Gauge | == 1 (Open) | Critical |
| `backoff_calculation_errors` | Counter | > 0 | Critical |
| `graceful_shutdown_duration` | Histogram | > 30s | Warning |
| `line_item_limit_hits` | Counter | > 10/hr | Warning |

### 5.2 Alerting Rules

```yaml
# prometheus/alerts.yml
groups:
  - name: quotey_security
    rules:
      - alert: AuditEventsBeingDropped
        expr: rate(audit_events_dropped_total[5m]) > 0
        for: 1m
        labels:
          severity: warning
        annotations:
          summary: "Audit events are being dropped"
          
      - alert: CircuitBreakerOpen
        expr: circuit_breaker_state == 1
        for: 30s
        labels:
          severity: critical
        annotations:
          summary: "Circuit breaker is open"
```

---

## 6. Rollback Plan

For each phase:

1. **Feature flags** - All changes behind `#[cfg(feature = "hardening")]`
2. **Canary deployment** - Deploy to 1% of traffic first
3. **Automatic rollback** - Revert if error rate increases > 0.1%
4. **Data compatibility** - Ensure no schema changes without migration

---

## 7. Success Criteria

| Criterion | Target | Measurement |
|-----------|--------|-------------|
| Memory stability | Flat for 7 days | `container_memory_working_set_bytes` |
| No panics | 0 in production | Error tracking |
| P99 latency | < 100ms | Request duration |
| Graceful shutdown | < 10s | Deployment time |
| Security scan | 0 critical issues | `cargo audit` |

---

## 8. Appendix

### 8.1 Related Documents

- `.planning/PROJECT.md` - Project architecture
- `.planning/W2_CLO_CLOSED_LOOP_POLICY_OPTIMIZER_SPEC.md` - Audit requirements
- `CONTRIBUTING.md` - Development guidelines

### 8.2 Beads Created

| Bead ID | Issue | Status |
|---------|-------|--------|
| quotey-1i9 | Blocking file operations | P2 |
| quotey-175 | SQL injection audit | P1 |
| quotey-zaz | Error message redaction | P2 |
| quotey-1r1 | Exponential backoff overflow | P2 |
| quotey-ti6 | Quote ID extraction limits | P2 |

---

## Approval

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Security Lead | | | |
| Tech Lead | | | |
| Product Owner | | | |
