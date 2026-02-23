# RCH-04a: Failure Injection Test Matrix

**Research Task:** `bd-3d8.11.5.1`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** LilacMountain (Codex)  
**Primary Inputs:** `.planning/research/RCH-04-Reliability-and-Idempotency-Architecture.md`, `.planning/research/RCH-03-Slack-Command-Grammar-and-Thread-Lifecycle.md`, `.planning/research/RCH-09-Decision-Freeze-and-Phased-Execution-Plan.md`

---

## Executive Summary

This matrix operationalizes reliability design into deterministic fault-injection scenarios spanning Slack, CRM, PDF, LLM, and persistence boundaries.

For each failure case, it specifies:

1. injection point and method,
2. expected system behavior (deterministic + auditable),
3. user-visible behavior,
4. operator remediation workflow.

Outcome: a concrete verification inventory for foundation and post-foundation reliability hardening.

---

## 1. Test Matrix Scope

Critical flows covered:

1. net-new quote flow,
2. renewal/expansion flow,
3. discount exception + approval flow,
4. document generation and outbound writeback path.

Operation classes covered:

1. domain state mutations (exactly-once business effect),
2. side effects (Slack updates, CRM writes, PDF upload) with at-least-once + dedupe,
3. retry and dead-letter behavior,
4. crash-window recovery around idempotency reservation and commit boundaries.

---

## 2. Injection Harness Contract (Implementation Target)

## 2.1 Injection Modes

1. `fail_once`: inject one failure then recover.
2. `fail_n`: fail first `N` attempts then recover.
3. `always_fail`: terminal failure.
4. `timeout`: operation exceeds configured timeout.
5. `crash_after(step)`: process abort after specified checkpoint.
6. `duplicate_replay`: replay same inbound payload/key.

## 2.2 Checkpoints for Crash-Window Testing

1. `after_idempotency_reserve`
2. `after_domain_commit`
3. `before_side_effect_dispatch`
4. `after_side_effect_partial_success`

---

## 3. Failure Injection Matrix

Legend:

1. Severity: `P0` critical correctness/security, `P1` high reliability, `P2` degraded UX/ops.
2. Behavior codes:
   1. `DEDUPE` = replay deduped safely
   2. `RETRY` = bounded retry with backoff
   3. `DLQ` = dead-letter queue capture
   4. `REJECT` = deterministic reject/no mutation

### 3.1 Slack Ingress and Callback Failures

| ID | Scenario | Injection | Expected Deterministic Behavior | User/Operator Behavior | Severity |
|---|---|---|---|---|---|
| FI-001 | Duplicate slash envelope replay | `duplicate_replay` on `/quote new` envelope | first execution mutates; replay hits idempotency (`DEDUPE`), no second quote | thread indicates existing operation reference; operator sees idempotency-hit metric | P0 |
| FI-002 | Duplicate approval button callback replay | `duplicate_replay` on approval action payload | exactly one decision persisted; replay returns already-processed response (`DEDUPE`) | approver gets stable confirmation; no duplicate approval rows | P0 |
| FI-003 | Slow pre-ack path | inject blocking delay before ack | command processing path must ack-fast or fail-fast; no heavy work pre-ack | alert on ack latency breach; operator runbook RB-01 | P1 |
| FI-004 | Unknown command verb storm | burst of unsupported verbs | parser rejects deterministically (`REJECT`) and emits structured reject events | user receives help response; no domain mutation | P2 |
| FI-005 | Thread drift message with no quote context | inject unrelated message in active thread | ignore/route-safe behavior; no unintended mutation (`REJECT`) | user gets optional guidance if mention present | P1 |

### 3.2 Slack API Egress Failures

| ID | Scenario | Injection | Expected Deterministic Behavior | User/Operator Behavior | Severity |
|---|---|---|---|---|---|
| FI-006 | Slack API 429 on status update | `fail_n=3` with 429 + `Retry-After` | bounded `RETRY` obeying `Retry-After`; converges to final message state | delayed status update; retry telemetry visible | P1 |
| FI-007 | Slack API 5xx transient | `fail_once` with 500 | retries with jitter, final success (`RETRY`) | user sees eventual update, no duplicate status cards | P1 |
| FI-008 | Slack API permanent 4xx (invalid channel) | `always_fail` 4xx | stop retries and persist terminal error (`DLQ` for side effect only) | user gets actionable failure message; operator remaps channel | P1 |

### 3.3 CRM Integration Failures

| ID | Scenario | Injection | Expected Deterministic Behavior | User/Operator Behavior | Severity |
|---|---|---|---|---|---|
| FI-009 | CRM timeout on writeback | `timeout` on `write_quote` | bounded `RETRY`; if exhausted -> `DLQ` with replay payload | user told "sync pending"; operator replay command available | P1 |
| FI-010 | CRM auth expired | `always_fail` auth/401 | classify terminal auth failure; retries paused; no local rollback | operator prompted for auth remediation, sync pause visible | P0 |
| FI-011 | CRM field mapping mismatch | `always_fail` mapping error | deterministic failure class, no silent partial domain mutation (`REJECT`) | user sees sync blocked reason; operator updates mapping | P0 |
| FI-012 | Partial sync success then failure | `fail_once` after subset write | partial writes auditable; reconciliation state persisted; safe replay idempotent | operator can replay unresolved records only | P1 |

### 3.4 PDF and Artifact Pipeline Failures

| ID | Scenario | Injection | Expected Deterministic Behavior | User/Operator Behavior | Severity |
|---|---|---|---|---|---|
| FI-013 | Renderer transient failure | `fail_once` in render process | retry render; if success, one canonical artifact checksum | user receives delayed but single PDF result | P1 |
| FI-014 | Renderer deterministic invalid template failure | `always_fail` template parse | no infinite retries; terminal error captured (`DLQ`) | user receives actionable template error; operator fixes template | P1 |
| FI-015 | Upload failure after successful render | `fail_n=2` on Slack upload | reuse artifact checksum, retry upload only; no duplicate render | user sees retry status; eventually one uploaded file | P1 |

### 3.5 Persistence and Crash-Window Failures

| ID | Scenario | Injection | Expected Deterministic Behavior | User/Operator Behavior | Severity |
|---|---|---|---|---|---|
| FI-016 | Crash after idempotency reserve, before domain commit | `crash_after(after_idempotency_reserve)` | restart detects reserved/incomplete op; safe retry resumes without double effect | operator sees recoverable pending operation | P0 |
| FI-017 | Crash after domain commit, before side effects | `crash_after(after_domain_commit)` | domain mutation persists once; side effects replay-safe and resumed | user sees eventual status convergence post-restart | P0 |
| FI-018 | Crash after side-effect partial success | `crash_after(after_side_effect_partial_success)` | replay dedupe by external ref/checksum; no duplicate business mutation | operator verifies via correlation trace + dedupe counters | P0 |
| FI-019 | DB lock contention spike | inject busy/lock timeout pressure | retries respect timeout policy; no inconsistent partial writes | SLO alert on latency; operator tunes pool/pragma | P1 |

### 3.6 LLM/Agent Runtime Failures

| ID | Scenario | Injection | Expected Deterministic Behavior | User/Operator Behavior | Severity |
|---|---|---|---|---|---|
| FI-020 | LLM provider timeout | `timeout` extraction call | bounded retries then deterministic fallback path | user asked clarifying input if needed; no guessed mutation | P1 |
| FI-021 | Invalid structured output from LLM | inject malformed schema payload | deterministic validation failure (`REJECT`), no mutation | user prompt requests clarification; audit logs reason | P0 |
| FI-022 | Prompt-injection style malicious content | inject adversarial user text | guardrails prevent policy/price authority bypass | user sees safe constrained response; security event emitted | P0 |

---

## 4. Assertions Per Scenario

Every FI scenario must assert:

1. No duplicate business mutation for same semantic operation key.
2. Domain state and audit trail remain coherent after failure/retry/restart.
3. Expected retry/dead-letter path is followed exactly (bounded, no infinite loops).
4. User-visible response is deterministic and actionable.
5. Correlation fields (`trace_id`, `operation_id`, `correlation_id`) are present on emitted events.

Additional assertions for P0 scenarios:

1. approval decisions remain quote-version-bound and stale actions rejected,
2. no missing mandatory audit events on mutation paths,
3. replay path never changes final commercial decision.

---

## 5. Operator Action Playbooks by Failure Class

| Failure Class | Detection Signal | Immediate Action | Follow-up |
|---|---|---|---|
| `idempotency_anomaly` | duplicate dedupe spikes, inconsistent reserve/complete pairs | pause affected worker lane, inspect operation ledger | add key-dimension regression tests |
| `ingress_ack_degradation` | ack p95 over threshold | inspect pre-ack code path and queue depth | adjust backpressure and handler split |
| `crm_auth_or_mapping_failure` | sustained 401/mapping failures | suspend retries, remediate credentials/mappings | replay queued operations in controlled batch |
| `pdf_pipeline_failure` | renderer/upload DLQ growth | identify deterministic vs transient class; retry safe cases | patch template or converter config |
| `security_guardrail_trigger` | authz denied/prompt-injection events | verify actor/resource context and rule coverage | add/adjust guardrail fixtures and alerts |

---

## 6. Test Execution Cadence

1. PR-level deterministic subset:
   1. FI-001, FI-002, FI-016, FI-017, FI-021
2. Daily CI reliability pack:
   1. FI-003, FI-006, FI-009, FI-013, FI-019, FI-020
3. Weekly chaos-style full matrix run:
   1. all FI-001..FI-022 with runbook verification sampling.

Entry/exit gating:

1. Any P0 failure blocks merge for affected mutation path.
2. P1/P2 failures require tracked remediation bead with owner and due target.

---

## 7. Mapping to Foundation Beads

| Matrix Area | Primary Implementation Beads |
|---|---|
| Slack ingress/callback cases | `bd-3d8.5`, `bd-3d8.5.1`, `bd-3d8.5.2` |
| Idempotency/cross-layer reliability | `bd-3d8.6`, `bd-3d8.7`, `bd-3d8.8.1` |
| Audit/telemetry signals | `bd-3d8.8`, `bd-3d8.8.2`, `bd-3d8.11.8.1` |
| CLI/operator recovery paths | `bd-3d8.9`, `bd-3d8.9.1`, `bd-3d8.9.2` |
| Security failure hooks | `bd-3d8.11.9.1`, `bd-3d8.10` |

---

## 8. Acceptance Mapping for `bd-3d8.11.5.1`

Deliverable: deterministic fault-injection matrix across Slack/CRM/PDF  
Completed: Section 3.

Deliverable: expected behavior and operator actions  
Completed: Sections 4 and 5.

Acceptance: includes Slack, CRM, and PDF adapter failures  
Completed: Sections 3.1, 3.3, and 3.4.

Acceptance: defines expected system behavior and operator actions  
Completed: Sections 4, 5, and 6.
