# RCH-07: Observability KPI and SLO Framework

**Research Task:** `bd-3d8.11.8`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** LilacMountain (Codex)  
**Primary Inputs:** `.planning/PROJECT.md`, `.planning/ARCHITECTURE_DECISION_RESEARCH.md`

---

## Executive Summary

This document defines Quotey's observability baseline for alpha:

1. KPI catalog with formulas, targets, and owners.
2. Event/spans schema contract for consistent telemetry.
3. Initial SLI/SLO definitions with alert thresholds.
4. Dashboard and runbook blueprints that can be implemented directly.

Decision:

- Use structured telemetry centered on correlation continuity (`trace_id`, `operation_id`, `quote_id`).
- Treat observability as a deterministic correctness feature, not only operations tooling.
- Enforce required fields to avoid ambiguous or non-reconstructable incident timelines.

This aligns with ADR-0015 and downstream reliability/security requirements.

---

## 1. Objective and Acceptance Mapping

Required outputs from `bd-3d8.11.8`:

1. KPI list with formulas and owners.
2. Event schema for audit and operational telemetry.
3. Initial SLOs and alert thresholds.

Acceptance mapping:

- before/after feature evaluation enabled: Section 3.
- schema avoids ambiguity/duplicates: Section 4.
- dashboards/runbooks directly implementable: Sections 6 and 7.

---

## 2. Observability Principles

1. Every critical operation is traceable end-to-end by correlation identifiers.
2. Business correctness signals (pricing/approval determinism) are first-class KPIs.
3. Required telemetry fields are immutable contract, not optional conventions.
4. Secrets/PII are excluded or redacted at source.
5. Alerting should be actionable and mapped to runbook steps.

---

## 3. KPI Catalog (Formula + Owner + Target)

### 3.1 Determinism KPIs

| KPI | Formula | Target | Owner |
|---|---|---|---|
| replay_consistency_rate | identical_replay_outcomes / replay_attempts | 100% | CPQ Core |
| pricing_trace_completeness | finalized_quotes_with_complete_trace / finalized_quotes | 100% | CPQ Core |
| flow_transition_legality_rate | legal_transitions / all_transitions | 100% | Flow Engine |

### 3.2 Reliability KPIs

| KPI | Formula | Target | Owner |
|---|---|---|---|
| slack_ack_p95_ms | p95(ack_latency_ms) | <= 1000ms | Slack Adapter |
| duplicate_side_effect_incidents | count(duplicate_business_effect) | 0 | Runtime/Platform |
| retry_recovery_success_rate | recovered_after_retry / retry_attempted_ops | >= 99% | Integrations |
| dead_letter_rate | dead_letter_ops / total_external_ops | <= 0.5% | Integrations |

### 3.3 Quality and Delivery KPIs

| KPI | Formula | Target | Owner |
|---|---|---|---|
| protected_branch_clippy_warnings | count(clippy_warnings_main) | 0 | Engineering |
| migration_failure_rate | failed_migration_runs / migration_runs | 0 | DB/Platform |
| integration_test_pass_rate | passed_critical_scenarios / total_critical_scenarios | 100% | QA/Engineering |

### 3.4 Security and Compliance KPIs

| KPI | Formula | Target | Owner |
|---|---|---|---|
| secret_leak_incidents | count(secret_pattern_matches_in_logs) | 0 | Security |
| audit_coverage_rate | audited_mutations / mutation_commands | 100% | Platform |
| redaction_compliance_rate | redacted_secret_fields / detected_secret_fields | 100% | Security/Platform |

### 3.5 Operability KPIs

| KPI | Formula | Target | Owner |
|---|---|---|---|
| mttd_quote_failure_minutes | avg(detect_ts - incident_start_ts) | <= 10m | Ops |
| mttr_quote_failure_minutes | avg(resolve_ts - detect_ts) | <= 60m | Ops/Engineering |
| local_onboarding_time_minutes | avg(time_to_successful_local_start) | downward trend | Developer Experience |

---

## 4. Event and Span Schema Contract

### 4.1 Required Correlation Fields (all critical spans/events)

1. `trace_id`
2. `operation_id`
3. `correlation_id`
4. `quote_id` (when in quote context)
5. `actor` and `actor_type`
6. `component` / `service`
7. `event_type` or `span_name`
8. `timestamp_utc`

### 4.2 Operational Span Fields

Required span fields:

1. `span_kind` (`ingress`, `internal`, `egress`)
2. `start_ts`, `end_ts`
3. `duration_ms`
4. `status` (`ok`, `error`, `retry`, `timeout`)
5. `retry_count` (if applicable)

Optional diagnostic fields:

1. `db_tx_id`
2. `provider`
3. `endpoint`
4. `payload_version`

### 4.3 Audit Event Taxonomy (canonical categories)

Use planning categories:

1. `quote`
2. `pricing`
3. `approval`
4. `configuration`
5. `catalog`
6. `crm`
7. `system`

For each category, event names must be unique and stable (`<category>.<verb_or_state_change>`).

### 4.4 Ambiguity and Duplicate Prevention Rules

1. One event type name maps to one semantic meaning only.
2. Event payload versions must be explicit (`schema_version`).
3. Duplicate events from replay must include dedupe marker (`idempotency_hit=true`) instead of pretending first-run behavior.
4. Unknown fields are retained in raw diagnostics but excluded from canonical metric aggregation until mapped.

---

## 5. Initial SLI/SLO Baselines and Alert Thresholds

### 5.1 SLI/SLO Table

| SLI | Measurement Window | SLO Target | Warn Threshold | Page Threshold |
|---|---|---|---|---|
| Slack ack latency p95 | 15m rolling | <= 1000ms | > 1200ms | > 2000ms |
| Duplicate business effects | daily | 0 | >= 1 | >= 1 immediate |
| Pricing trace completeness | daily | 100% | < 100% | < 99% |
| Approval routing success before timeout | daily | >= 99% | < 98% | < 95% |
| CRM sync success rate | 1h rolling | >= 99% | < 97% | < 93% |
| Dead-letter backlog age | 1h rolling | <= 30m max age | > 45m | > 90m |
| Audit coverage rate | daily | 100% | < 100% | < 99% |

### 5.2 Error Budget Policy (alpha)

1. Any SLO with target 100% has zero budget; violations trigger immediate remediation.
2. For percentile/rate SLOs, use rolling 7-day error budget tracking.
3. If budget exhaustion projected within 48h, freeze non-critical feature rollout until stabilized.

---

## 6. Dashboard Blueprint (Implementable)

### 6.1 Dashboard A: Runtime Health

Panels:

1. ingress event volume and ack latency percentiles
2. retry rate by adapter/operation class
3. dead-letter count and age
4. error class breakdown (timeout, mapping, auth, rate_limit)

### 6.2 Dashboard B: Deterministic CPQ Correctness

Panels:

1. replay consistency rate
2. pricing trace completeness
3. flow transition legality violations
4. approval stale-action conflict count

### 6.3 Dashboard C: Integration and CRM

Panels:

1. sync success/failure trend by provider
2. conflict class counts
3. reconciliation queue size/age
4. writeback latency and timeout trend

### 6.4 Dashboard D: Security/Compliance Signals

Panels:

1. redaction compliance rate
2. secret leak detections
3. audit coverage rate
4. permission/config doctor findings trend

---

## 7. Runbook Blueprint (Implementable)

### 7.1 Runbook RB-01: Slack Ingress Degradation

Trigger:

- ack latency SLO breached.

Steps:

1. verify recent deploy/config changes,
2. inspect ingress spans by `trace_id` and `operation_id`,
3. identify blocking pre-ack work,
4. apply mitigation (queue/defer heavy path),
5. validate recovery via p95 trend normalization.

### 7.2 Runbook RB-02: Duplicate Side Effect Incident

Trigger:

- duplicate business effect count > 0.

Steps:

1. query operation ledger for duplicated semantic payload,
2. verify idempotency key construction and hash normalization,
3. check reserve-before-side-effect ordering in traces,
4. patch and replay safe test scenario.

### 7.3 Runbook RB-03: CRM Sync Degradation

Trigger:

- CRM sync success rate below threshold.

Steps:

1. classify errors (timeout/auth/mapping/rate limit),
2. inspect reconciliation queue growth,
3. remediate provider auth or mapping drift,
4. execute controlled retry and verify queue drain.

### 7.4 Runbook RB-04: Audit Coverage Violation

Trigger:

- audit coverage < 100%.

Steps:

1. identify mutation paths missing audit hooks,
2. block release for affected path,
3. add required event emission and regression tests,
4. verify restored coverage.

---

## 8. Implementation Handoff Notes

### For `bd-3d8.11.8.1` (event taxonomy + dashboard blueprint)

1. convert Section 4 taxonomy into formal schema files.
2. define dashboard queries for each KPI in Sections 3 and 6.
3. include schema versioning and compatibility guidance.

### For `bd-3d8.11.10` (decision freeze)

Freeze:

1. required correlation field contract,
2. baseline KPI formulas/owners,
3. initial SLO thresholds and runbook triggers.

### For observability implementation tasks

1. enforce required span fields at instrumentation wrappers.
2. add trace completeness and audit coverage CI checks.
3. ensure redaction tests gate structured logging changes.

---

## 9. Done Criteria Mapping

Deliverable: KPI list with formulas and owners  
Completed: Section 3.

Deliverable: Event schema for audit and operational telemetry  
Completed: Section 4.

Deliverable: Initial SLOs and alert thresholds  
Completed: Section 5.

Acceptance: before/after feature evaluation support  
Completed: Sections 3 and 5.

Acceptance: unambiguous schema and duplicate controls  
Completed: Section 4.

Acceptance: dashboard/runbook implementation readiness  
Completed: Sections 6 and 7.

