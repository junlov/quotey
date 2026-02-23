# W1 REL Execution Queue - Operator Runbook

## Overview

The Resilient Execution Queue (REL) provides durable, idempotent execution for quote-scoped actions including Slack notifications, CRM sync, and PDF generation. This runbook covers common operational scenarios.

---

## Architecture Summary

### State Machine

```
queued → running → completed
           ↓
    retryable_failed → running (retry)
           ↓ (max retries exceeded)
    failed_terminal
```

### Key Components

| Component | Purpose | Location |
|-----------|---------|----------|
| `DeterministicExecutionEngine` | State machine logic | `crates/core/src/execution_engine.rs` |
| `ExecutionTask` | Task state | SQLite `execution_queue_task` table |
| `IdempotencyRecord` | Duplicate prevention | SQLite `execution_idempotency_ledger` table |
| `ExecutionTransitionEvent` | Audit trail | SQLite `execution_queue_transition_audit` table |
| Block Kit Templates | Slack UX | `crates/slack/src/blocks.rs` |

---

## KPIs and Alerting

### Key Metrics

| Metric | Target | Alert Threshold |
|--------|--------|-----------------|
| Failed action rate (transient) | ≤ 2.0% | > 5.0% |
| Recovery success within 5 min | ≥ 95% | < 90% |
| Duplicate side-effect rate | ≤ 0.1% | > 0.5% |
| Mean status latency | ≤ 1.5s | > 3.0s |

### Querying Metrics

```sql
-- Failed action rate (last hour)
SELECT 
    COUNT(CASE WHEN to_state = 'failed_terminal' THEN 1 END) * 100.0 / COUNT(*) as fail_rate
FROM execution_queue_transition_audit
WHERE occurred_at > datetime('now', '-1 hour');

-- Stuck tasks (claimed but not updated)
SELECT id, quote_id, claimed_by, claimed_at
FROM execution_queue_task
WHERE state = 'running'
  AND claimed_at < datetime('now', '-5 minutes');

-- Tasks by state
SELECT state, COUNT(*) as count
FROM execution_queue_task
GROUP BY state;
```

---

## Common Scenarios

### 1. Task Stuck in Running State

**Symptoms:**
- Task shows as "Processing" in Slack for > 5 minutes
- Worker may have crashed

**Investigation:**
```sql
-- Find stuck tasks
SELECT 
    id, 
    quote_id, 
    operation_kind,
    claimed_by,
    claimed_at,
    datetime('now') - claimed_at as minutes_stuck
FROM execution_queue_task
WHERE state = 'running'
  AND claimed_at < datetime('now', '-5 minutes');
```

**Resolution:**
1. Check worker logs: `journalctl -u quotey-worker -f`
2. If worker crashed, recovery happens automatically on next sweep
3. For manual recovery, update task state:
```sql
-- Force retry (use with caution)
UPDATE execution_queue_task
SET state = 'retryable_failed',
    retry_count = retry_count + 1,
    available_at = datetime('now'),
    claimed_by = NULL,
    claimed_at = NULL,
    state_version = state_version + 1
WHERE id = 'TASK-ID-HERE';
```

### 2. Duplicate Side Effects Detected

**Symptoms:**
- Multiple Slack messages sent for same action
- Duplicate CRM records created

**Investigation:**
```sql
-- Check idempotency records
SELECT 
    operation_key,
    quote_id,
    state,
    attempt_count,
    first_seen_at,
    last_seen_at
FROM execution_idempotency_ledger
WHERE quote_id = 'QUOTE-ID-HERE'
ORDER BY first_seen_at;
```

**Resolution:**
1. Review duplicate records for patterns
2. Check if idempotency key generation is consistent
3. If needed, manually mark duplicates:
```sql
-- Mark duplicate as terminal
UPDATE execution_idempotency_ledger
SET state = 'failed_terminal',
    error_snapshot_json = '{"reason": "duplicate_detected"}'
WHERE operation_key = 'DUPLICATE-KEY-HERE';
```

### 3. High Failure Rate

**Symptoms:**
- Many tasks failing
- Users reporting errors

**Investigation:**
```sql
-- Failure patterns
SELECT 
    operation_kind,
    to_state,
    error_class,
    COUNT(*) as count
FROM execution_queue_transition_audit
WHERE occurred_at > datetime('now', '-1 hour')
GROUP BY operation_kind, to_state, error_class
ORDER BY count DESC;
```

**Common Causes:**

| Error Class | Likely Cause | Resolution |
|-------------|--------------|------------|
| `NetworkError` | External service unavailable | Check Slack/CRM connectivity |
| `TimeoutError` | Slow external API | Increase timeout in config |
| `ValidationError` | Invalid task payload | Check task payload format |
| `DatabaseError` | SQLite issues | Check disk space, WAL mode |

### 4. Recovery After Worker Crash

**Symptoms:**
- Worker process died
- Tasks marked as "running" but not progressing

**Investigation:**
```bash
# Check worker status
systemctl status quotey-worker

# Check recent crashes
journalctl -u quotey-worker --since "1 hour ago" | grep -i "panic\|error\|killed"
```

**Resolution:**
1. Restart worker: `systemctl restart quotey-worker`
2. Recovery sweep runs automatically on startup
3. Verify recovery:
```sql
-- Check recovered tasks
SELECT 
    id,
    quote_id,
    state,
    retry_count,
    updated_at
FROM execution_queue_task
WHERE updated_at > datetime('now', '-5 minutes')
ORDER BY updated_at DESC;
```

---

## Maintenance Procedures

### Daily Health Check

```bash
#!/bin/bash
# daily_health_check.sh

echo "=== Execution Queue Health ==="

# Task counts by state
sqlite3 quotey.db "
SELECT 
    state,
    COUNT(*) as count,
    MAX(updated_at) as latest
FROM execution_queue_task
GROUP BY state;
"

# Stuck tasks
sqlite3 quotey.db "
SELECT COUNT(*) as stuck_count
FROM execution_queue_task
WHERE state = 'running'
  AND claimed_at < datetime('now', '-5 minutes');
"

# Recent failures
sqlite3 quotey.db "
SELECT 
    operation_kind,
    to_state,
    COUNT(*) as count
FROM execution_queue_transition_audit
WHERE occurred_at > datetime('now', '-24 hours')
  AND to_state IN ('retryable_failed', 'failed_terminal')
GROUP BY operation_kind, to_state;
"
```

### Cleanup Old Completed Tasks

**WARNING:** Only run after confirming audit retention requirements.

```sql
-- Archive old completed tasks (keeps 30 days)
BEGIN TRANSACTION;

-- Archive to separate table (create if not exists)
CREATE TABLE IF NOT EXISTS execution_queue_task_archive AS 
SELECT * FROM execution_queue_task WHERE 0;

-- Move old records
INSERT INTO execution_queue_task_archive
SELECT * FROM execution_queue_task
WHERE state = 'completed'
  AND updated_at < datetime('now', '-30 days');

-- Delete from main table
DELETE FROM execution_queue_task
WHERE state = 'completed'
  AND updated_at < datetime('now', '-30 days');

COMMIT;
```

---

## Configuration

### Engine Config

```rust
ExecutionEngineConfig {
    claim_timeout_seconds: 300,      // 5 minutes
    default_max_retries: 3,          // 3 attempts
    retry_backoff_multiplier: 2,     // 2x exponential
    retry_base_delay_seconds: 5,     // 5s base delay
}
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `QUOTEY_EXEC_CLAIM_TIMEOUT_SEC` | 300 | Stale claim timeout |
| `QUOTEY_EXEC_MAX_RETRIES` | 3 | Default max retries |
| `QUOTEY_EXEC_BACKOFF_BASE_SEC` | 5 | Base retry delay |

---

## Debugging

### Enable Debug Logging

```bash
export RUST_LOG=quotey_core::execution_engine=debug,quotey_db=debug
```

### Trace a Specific Task

```sql
-- Full audit trail for a task
SELECT 
    occurred_at,
    from_state,
    to_state,
    transition_reason,
    error_class,
    actor_id,
    correlation_id
FROM execution_queue_transition_audit
WHERE task_id = 'TASK-ID-HERE'
ORDER BY occurred_at;
```

### Check Idempotency Key Collisions

```sql
-- Find duplicate keys
SELECT 
    operation_key,
    COUNT(*) as count,
    GROUP_CONCAT(quote_id) as quotes
FROM execution_idempotency_ledger
GROUP BY operation_key
HAVING count > 1;
```

---

## Emergency Procedures

### Pause All Processing

```sql
-- Set all queued tasks to retryable_failed with long delay
UPDATE execution_queue_task
SET state = 'retryable_failed',
    available_at = datetime('now', '+1 hour'),
    last_error = 'EMERGENCY: Processing paused by operator'
WHERE state = 'queued';
```

### Resume Processing

```sql
-- Make all retryable_failed tasks available immediately
UPDATE execution_queue_task
SET state = 'queued',
    available_at = datetime('now')
WHERE state = 'retryable_failed'
  AND last_error LIKE 'EMERGENCY:%';
```

### Force Retry All Failed Tasks

```sql
-- Reset all retryable_failed to queued
UPDATE execution_queue_task
SET state = 'queued',
    available_at = datetime('now'),
    retry_count = 0
WHERE state = 'retryable_failed';
```

---

## Contacts

| Role | Contact | Escalation |
|------|---------|------------|
| On-call Engineer | #alerts-quotey | @sre-manager |
| Product Owner | @product-quotey | @cto |
| Determinism Review | #cpq-determinism | @architect |

---

## References

- Spec: `.planning/W1_REL_EXECUTION_QUEUE_SPEC.md`
- Implementation: `crates/core/src/execution_engine.rs`
- Slack UX: `crates/slack/src/blocks.rs`
- Database: `migrations/0012_execution_queue_rel.up.sql`

---

*Last Updated: 2026-02-23*
*Version: 1.0*
