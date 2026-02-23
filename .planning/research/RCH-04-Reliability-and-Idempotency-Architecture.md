# RCH-04: Reliability and Idempotency Architecture

**Research Task:** `bd-3d8.11.5`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** LilacMountain (Codex)  
**Primary Inputs:** `.planning/PROJECT.md`, `.planning/ARCHITECTURE_DECISION_RESEARCH.md`

---

## Executive Summary

This document defines Quotey's reliability model across Slack ingress, deterministic CPQ operations, approval actions, and external adapters.

Core decisions:

1. Use a durable SQLite idempotency ledger at command/action execution boundary.
2. Reserve operation key before business side effects.
3. Use operation-class-specific delivery semantics:
   - exactly-once effect for state mutation commands,
   - at-least-once for notifications and retries with dedupe keys.
4. Require explicit retry/backoff/dead-letter policy per adapter operation class.

This design satisfies deterministic behavior, auditability, and recoverability without requiring distributed infrastructure.

---

## 1. Objective and Acceptance Mapping

Required outputs from `bd-3d8.11.5`:

1. Failure mode catalog by adapter and operation class.
2. Idempotency key strategy and dedupe semantics.
3. Retry policy matrix with backoff and dead-letter behavior.

Acceptance mapping:

- user-visible and auditable recovery paths: Sections 5 and 7.
- explicit exactly-once/at-least-once semantics: Section 4.
- fault injection testing strategy: Section 8.

---

## 2. Reliability Boundary Model

### 2.1 Layers

1. Ingress layer (Slack Socket Mode events, interactive callbacks, slash commands)
2. Command execution layer (flow, pricing, policy, approvals)
3. Side-effect layer (Slack API writes, CRM writes, PDF generation, LLM calls)
4. Persistence layer (SQLite domain + audit + idempotency ledger)

### 2.2 Reliability Rule

No side effect is executed before operation key reservation is persisted.

Order of operations:

1. Normalize incoming request into deterministic command.
2. Derive operation key.
3. Begin transaction and reserve key in idempotency ledger.
4. Execute deterministic business mutation.
5. Commit domain + audit state.
6. Execute external side effects (or enqueue with durable intent).
7. Mark operation completion and response snapshot.

---

## 3. Idempotency Key Strategy and Dedupe Semantics

### 3.1 Operation Key Contract

Canonical key dimensions:

1. `source` (`slack_slash`, `slack_interactive`, `slack_event`, `cli`, `scheduler`)
2. `source_request_id` (envelope/request identifier)
3. `action_kind` (semantic command/action name)
4. `aggregate_id` (quote id or approval request id)
5. `aggregate_version` (quote version or flow version)
6. `semantic_payload_hash` (normalized payload hash)

Derived key:

`operation_key = hash(source|source_request_id|action_kind|aggregate_id|aggregate_version|semantic_payload_hash)`

### 3.2 Dedupe Outcomes

For repeated key:

1. If state=`completed`, return stored response snapshot (idempotent replay success).
2. If state=`in_progress`, return "operation pending" with correlation id.
3. If state=`failed_retryable`, allow controlled retry path by retry policy.
4. If state=`failed_terminal`, return terminal error and remediation guidance.

### 3.3 Payload Normalization Rules

1. Canonicalize JSON keys ordering before hashing.
2. Exclude transport-only metadata (retry counter, receive timestamp).
3. Include semantic version of payload schema in hash scope.

This avoids false misses and key collisions during schema evolution.

---

## 4. Delivery Semantics by Operation Class

| Operation Class | Examples | Required Semantics | Notes |
|---|---|---|---|
| Domain state mutation | create/update quote, route approval, record decision | Exactly-once business effect | Enforced by operation ledger + transactional mutation |
| Pricing/policy evaluation for a version | price quote, evaluate policy | Exactly-once persisted snapshot per key | Re-eval with new key/version allowed |
| Slack status updates | update thread status card | At-least-once with idempotent target (`message_ts`) | Safe replay should converge to same state |
| Slack append notifications | reminders, FYI notices | At-least-once with dedupe token in metadata | Prevent duplicate spam in same window |
| CRM sync/writeback | write quote summary to CRM | At-least-once with external idempotency reference | Reconciled via sync state |
| PDF generation/upload | render document, upload to thread | At-least-once with content checksum dedupe | Reuse existing artifact if checksum matches |

Exactly-once scope clarification:

- Exactly-once is guaranteed for local business state transitions.
- External systems get at-least-once delivery with deterministic dedupe metadata.

---

## 5. Failure Mode Catalog (Adapter x Operation Class)

### 5.1 Slack Ingress and Slack API

| Failure Mode | Trigger | Impact | Recovery Path |
|---|---|---|---|
| Duplicate slash/event delivery | Slack retry behavior | duplicate command execution risk | operation key dedupe at command boundary |
| Slow callback ack | handler blocks | retries/amplification | ack-fast path + deferred execution |
| Slack API 429 | rate limits | delayed user updates | bounded retry with jitter and queue backpressure |
| socket disconnect | network churn | temporary event ingestion gap | reconnect loop + resume with dedupe |

### 5.2 CRM/Composio

| Failure Mode | Trigger | Impact | Recovery Path |
|---|---|---|---|
| timeout | provider latency | blocked writeback | retry with bounded timeout + dead-letter |
| mapping error | schema mismatch | failed sync for specific record | mark terminal failure with actionable mapping error |
| auth expiration | token lifecycle | repeated failures | surface auth remediation + pause retries |

### 5.3 PDF/Document Pipeline

| Failure Mode | Trigger | Impact | Recovery Path |
|---|---|---|---|
| renderer failure | converter error | no deliverable artifact | retry if transient, otherwise terminal error with diagnostics |
| upload failure | Slack API/network | artifact exists but undelivered | retry upload with checksum reference |

### 5.4 LLM Adapter

| Failure Mode | Trigger | Impact | Recovery Path |
|---|---|---|---|
| timeout/provider error | remote API issues | missing extraction/summary | fallback extraction path + retry with cap |
| invalid structured output | malformed response | command parse uncertainty | deterministic validation failure prompt to user |

---

## 6. Retry/Backoff/Dead-Letter Matrix

| Operation Class | Retry Count | Backoff | Jitter | Dead-Letter Trigger |
|---|---|---|---|---|
| Slack ingress ack path | 0 inline | none | n/a | never (must ack fast or fail fast) |
| Slack API message update/post | 5 | exponential (base 250ms, cap 30s) | full jitter | exhausted retries or 4xx terminal |
| CRM writeback | 6 | exponential (base 500ms, cap 60s) | full jitter | exhausted retries or mapping/auth terminal |
| PDF render | 2 | fixed 1s then 3s | none | repeated deterministic renderer failure |
| LLM extraction/summarization | 3 | exponential (base 300ms, cap 10s) | bounded jitter | repeated timeout/provider hard fail |

Dead-letter behavior:

1. Persist failed payload + operation key + error classification.
2. Emit audit event with `event_type=system.error` and operation correlation.
3. Surface actionable user/operator message (where user-visible path exists).
4. Provide CLI replay command for safe manual retry.

---

## 7. User-Visible and Auditable Recovery Paths

### 7.1 User-visible Responses

For recoverable failures:

1. "Operation pending retry" message with correlation id.
2. Next valid actions shown (`Retry`, `Check status`, `Cancel` where allowed).

For terminal failures:

1. concise reason ("CRM mapping mismatch", "approval action already processed", etc.)
2. safe fallback instruction (`/quote status <id>`, or "contact deal desk").

### 7.2 Audit Trail Requirements

Each recovery-relevant event must include:

1. `operation_key`
2. `correlation_id`
3. `retry_count`
4. `failure_category`
5. `recovery_action`
6. `actor`/`actor_type`

This ensures deterministic post-incident reconstruction.

---

## 8. Fault Injection and Verification Strategy

### 8.1 Required Failure Injection Cases

1. Duplicate slash command envelope replay.
2. Duplicate interactive approval callback replay.
3. Crash after key reservation before domain commit.
4. Crash after domain commit before side-effect completion.
5. Slack 429 and transient 5xx behavior under retry policy.
6. CRM timeout and auth expiry behavior.

### 8.2 Assertions

1. No duplicate business mutations for same operation key.
2. Domain state remains valid after crash/restart.
3. Retried side effects are deduped or converge deterministically.
4. Audit events capture retry and recovery sequence.
5. Dead-letter records are queryable and replay-safe.

### 8.3 Observability Metrics

Minimum metrics:

1. idempotency hit rate
2. duplicate delivery dedupe count
3. retry attempts by adapter + operation class
4. dead-letter count by failure category
5. ack latency distribution for ingress path

---

## 9. Implementation Handoff Notes

### For `bd-3d8.6` and `bd-3d8.7`

1. Wrap all mutation commands with operation ledger reserve/complete lifecycle.
2. Persist response snapshot for completed operation keys.
3. Reject mutation if dedupe indicates already completed.

### For `bd-3d8.5.1` (Failure injection test matrix)

1. Use Section 8 as baseline test inventory.
2. Include crash-window tests for reserve-before-side-effect ordering.
3. Verify retry matrix behavior matches Section 6.

### For `bd-3d8.11.10` (Decision freeze)

Freeze these reliability contracts:

1. durable idempotency ledger in SQLite,
2. operation-key dimensions and normalization,
3. explicit operation-class delivery semantics,
4. dead-letter with replayable payload capture.

---

## 10. Done Criteria Mapping

Deliverable: Failure mode catalog  
Completed: Section 5.

Deliverable: Idempotency key strategy and dedupe semantics  
Completed: Section 3.

Deliverable: Retry matrix with backoff and dead-letter behavior  
Completed: Section 6.

Acceptance: User-visible and auditable recovery paths  
Completed: Section 7.

Acceptance: Exactly-once/at-least-once semantics explicit  
Completed: Section 4.

Acceptance: Fault-injection testing strategy  
Completed: Section 8.

