# RCH-07a: Event Taxonomy and Dashboard Blueprint

**Research Task:** `bd-3d8.11.8.1`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** LilacMountain (Codex)  
**Primary Inputs:** `.planning/research/RCH-07-Observability-KPI-and-SLO-Framework.md`, `.planning/research/RCH-09-Decision-Freeze-and-Phased-Execution-Plan.md`, current workspace crate boundaries

---

## Executive Summary

This document converts the `RCH-07` observability baseline into a directly implementable event contract:

1. canonical event naming taxonomy,
2. required dimensions and type contracts,
3. dashboard panel blueprint tied to concrete queries,
4. instrumentation task mapping to foundation implementation seams.

Outcome: latency, failure, approval, and conversion slices are explicitly defined and ready for logging/audit instrumentation work.

---

## 1. Event Contract Scope

In scope:

1. canonical event names and categories,
2. shared required dimensions for all critical events,
3. event-level optional fields for diagnostics,
4. dashboard panels and query definitions,
5. mapping from event producers to crate/module seams.

Out of scope:

1. final log storage backend schema details,
2. visualization tool choice (Grafana/DataFusion/etc.),
3. long-term retention policy.

---

## 2. Naming and Versioning Rules

## 2.1 Event Naming Convention

Format:

`<category>.<subject>.<action>`

Examples:

1. `ingress.slack.envelope_received`
2. `quote.lifecycle.transition_applied`
3. `approval.request.decision_recorded`
4. `integration.crm.sync_failed`

Rules:

1. snake_case segments only.
2. Stable semantic meaning per event name (no polymorphic overload).
3. Breaking payload changes require `schema_version` increment.

## 2.2 Required Metadata on Every Critical Event

1. `event_name`
2. `schema_version`
3. `timestamp_utc`
4. `trace_id`
5. `operation_id`
6. `correlation_id`
7. `component`
8. `environment`
9. `severity`

Quote-context events additionally require:

1. `quote_id`
2. `quote_version`
3. `flow_step` (if applicable)

---

## 3. Canonical Event Taxonomy

## 3.1 Ingress and Routing Events

| Event Name | Purpose | Key Fields |
|---|---|---|
| `ingress.slack.envelope_received` | Track incoming Socket Mode envelope volume/latency | `slack_envelope_id`, `event_type`, `channel_id`, `thread_ts` |
| `ingress.slack.ack_sent` | Measure ack latency SLO | `ack_latency_ms`, `ack_status` |
| `ingress.command.parsed` | Track slash/command parse outcomes | `command_family`, `parse_result`, `parse_error_code` |
| `ingress.command.rejected` | Monitor unsupported or ambiguous commands | `reject_reason`, `raw_command_hash` |

## 3.2 Flow and Quote Lifecycle Events

| Event Name | Purpose | Key Fields |
|---|---|---|
| `quote.lifecycle.created` | Quote creation funnel start | `account_id`, `deal_type`, `source` |
| `quote.lifecycle.transition_requested` | Detect illegal transition attempts | `from_status`, `to_status`, `requested_by` |
| `quote.lifecycle.transition_applied` | Canonical state progression | `from_status`, `to_status`, `is_legal=true` |
| `quote.lifecycle.transition_rejected` | Invalid transition observability | `from_status`, `to_status`, `rejection_code` |

## 3.3 Pricing and Determinism Events

| Event Name | Purpose | Key Fields |
|---|---|---|
| `pricing.evaluate.started` | Pricing path latency tracing | `ruleset_version`, `line_count` |
| `pricing.evaluate.completed` | Success timing and total context | `duration_ms`, `total_amount`, `currency` |
| `pricing.evaluate.failed` | Failure slice by error class | `error_class`, `error_code` |
| `pricing.trace.persisted` | Ensure replay/audit completeness | `trace_id_ref`, `stage_count`, `completeness_pct` |
| `pricing.replay.consistency_checked` | Determinism validation | `is_consistent`, `mismatch_count` |

## 3.4 Approval and Policy Events

| Event Name | Purpose | Key Fields |
|---|---|---|
| `approval.request.created` | Approval funnel entry | `approval_id`, `required_role`, `threshold_reason` |
| `approval.request.decision_recorded` | Approval outcome tracking | `decision`, `actor_id`, `decision_latency_ms` |
| `approval.request.stale_action_rejected` | Security and correctness signal | `actor_id`, `quote_version_at_action`, `expected_version` |
| `approval.request.escalated` | Escalation policy observability | `from_role`, `to_role`, `escalation_reason` |

## 3.5 Idempotency and Reliability Events

| Event Name | Purpose | Key Fields |
|---|---|---|
| `reliability.idempotency.reserved` | Reserve-before-side-effect audit | `idempotency_key`, `operation_kind` |
| `reliability.idempotency.hit` | Duplicate/replay suppression tracking | `idempotency_key`, `dedupe_mode` |
| `reliability.retry.scheduled` | Backoff and retry volume tracking | `adapter`, `operation_kind`, `retry_count`, `delay_ms` |
| `reliability.dead_letter.queued` | Dead-letter backlog signal | `adapter`, `operation_kind`, `error_class` |

## 3.6 Integration and CRM Events

| Event Name | Purpose | Key Fields |
|---|---|---|
| `integration.crm.sync_started` | Sync cycle observability | `provider`, `sync_mode` |
| `integration.crm.sync_completed` | Success counters and duration | `provider`, `duration_ms`, `records_written` |
| `integration.crm.sync_failed` | Failure classification and alerting | `provider`, `error_class`, `http_status` |
| `integration.crm.conflict_detected` | Conflict policy effectiveness | `field_name`, `conflict_class`, `resolution_policy` |

## 3.7 Security and Compliance Events

| Event Name | Purpose | Key Fields |
|---|---|---|
| `security.authz.denied` | Unauthorized action monitoring | `actor_id`, `resource`, `action` |
| `security.redaction.violation_detected` | PII/secret leakage detection | `field_name`, `detector`, `severity` |
| `security.audit.coverage_gap` | Missing audit hooks detection | `mutation_path`, `missing_event_name` |
| `security.integrity.check_failed` | Tamper-evidence alerting | `check_name`, `failed_record_count` |

---

## 4. Dimension Dictionary (Canonical Keys)

| Dimension | Type | Required | Notes |
|---|---|---|---|
| `trace_id` | string(UUID/ULID) | Yes | global trace continuity |
| `operation_id` | string | Yes | one logical mutation/command |
| `correlation_id` | string | Yes | cross-component linkage |
| `quote_id` | string | Contextual | mandatory for quote-context events |
| `quote_version` | integer | Contextual | required on approval/pricing events |
| `actor_id` | string | Contextual | required for user action events |
| `actor_type` | enum | Contextual | `user`, `system`, `adapter`, `scheduler` |
| `component` | string | Yes | emitting crate/module |
| `duration_ms` | integer | Optional | required for completed timed ops |
| `status` | enum | Optional | `ok`, `error`, `retry`, `timeout`, `rejected` |
| `error_class` | enum | Optional | `validation`, `auth`, `rate_limit`, `timeout`, `mapping`, `internal` |
| `schema_version` | integer | Yes | payload compatibility contract |

---

## 5. Dashboard Blueprint (Panels + Queries)

Query notation assumes event records are available in an analytics table `events`.

## 5.1 Dashboard A: Runtime and Ingress Health

Panels:

1. Ingress volume by event type (5m buckets)
2. Ack latency p50/p95/p99
3. Retry scheduled count by adapter
4. Dead-letter queue count + max age

Example query snippets:

```sql
-- Ack latency percentile
SELECT percentile_cont(0.95) WITHIN GROUP (ORDER BY duration_ms) AS p95_ack_ms
FROM events
WHERE event_name = 'ingress.slack.ack_sent'
  AND timestamp_utc >= datetime('now', '-15 minutes');

-- Retry volume by adapter
SELECT json_extract(payload, '$.adapter') AS adapter, count(*) AS retries
FROM events
WHERE event_name = 'reliability.retry.scheduled'
  AND timestamp_utc >= datetime('now', '-1 hour')
GROUP BY adapter;
```

## 5.2 Dashboard B: Deterministic CPQ Correctness

Panels:

1. Pricing success vs failure
2. Replay consistency pass rate
3. Flow transition rejection counts
4. Pricing trace completeness trend

```sql
SELECT
  sum(CASE WHEN json_extract(payload, '$.is_consistent') = 1 THEN 1 ELSE 0 END) * 1.0 / count(*) AS replay_consistency_rate
FROM events
WHERE event_name = 'pricing.replay.consistency_checked'
  AND timestamp_utc >= datetime('now', '-1 day');
```

## 5.3 Dashboard C: Approval and Policy Throughput

Panels:

1. Approval requests created vs resolved
2. Median approval decision latency
3. Stale-action rejection counts
4. Escalation volume by role transition

```sql
SELECT
  percentile_cont(0.5) WITHIN GROUP (ORDER BY json_extract(payload, '$.decision_latency_ms')) AS p50_approval_latency_ms
FROM events
WHERE event_name = 'approval.request.decision_recorded'
  AND timestamp_utc >= datetime('now', '-1 day');
```

## 5.4 Dashboard D: Integration and Conversion Signals

Panels:

1. CRM sync success rate by provider
2. CRM conflict class distribution
3. Quote created -> priced -> approved -> exported conversion funnel
4. Writeback timeout/error trend

```sql
SELECT
  provider,
  sum(CASE WHEN event_name = 'integration.crm.sync_completed' THEN 1 ELSE 0 END) * 1.0 /
  NULLIF(sum(CASE WHEN event_name IN ('integration.crm.sync_completed','integration.crm.sync_failed') THEN 1 ELSE 0 END), 0) AS success_rate
FROM (
  SELECT event_name, json_extract(payload, '$.provider') AS provider
  FROM events
  WHERE event_name IN ('integration.crm.sync_completed','integration.crm.sync_failed')
    AND timestamp_utc >= datetime('now', '-1 hour')
)
GROUP BY provider;
```

## 5.5 Dashboard E: Security and Compliance

Panels:

1. AuthZ denial count by action/resource
2. Redaction violation count
3. Audit coverage gap count
4. Integrity check failure trend

---

## 6. Mapping to Instrumentation Tasks (Current Workspace Seams)

| Area | Emitting Module Seam | Initial Bead Link |
|---|---|---|
| Slack ingress/ack | `crates/slack/src/socket.rs`, `crates/slack/src/events.rs`, `crates/slack/src/commands.rs` | `bd-3d8.5`, `bd-3d8.5.1`, `bd-3d8.5.2` |
| Flow lifecycle | `crates/core/src/flows/engine.rs`, `crates/core/src/flows/states.rs` | `bd-3d8.6` |
| Pricing/policy | `crates/core/src/cpq/pricing.rs`, `crates/core/src/cpq/policy.rs`, `crates/core/src/cpq/constraints.rs` | `bd-3d8.7` |
| Audit event capture | `crates/core/src/audit.rs`, `crates/db/src/repositories/*` | `bd-3d8.8`, `bd-3d8.8.1` |
| Correlated tracing | `crates/server/src/bootstrap.rs`, `crates/agent/src/runtime.rs`, `crates/slack/src/*` | `bd-3d8.8.2` |
| CLI diagnostics/doctor | `crates/cli/src/commands/doctor.rs`, `crates/cli/src/commands/config.rs` | `bd-3d8.9.1`, `bd-3d8.9.2` |
| CRM sync/integration | adapter seam under `crates/agent` + `crates/db` sync tables | `bd-3d8.11.7.1`, future CRM implementation beads |

---

## 7. Verification Checklist

1. Every critical command path emits at least one ingress, one domain, and one outcome event.
2. Correlation fields are present on all critical events.
3. Duplicate/replay events are represented with explicit idempotency markers.
4. Dashboard queries produce stable outputs with schema-versioned payloads.
5. Alert thresholds from `RCH-07` map directly to at least one panel/query each.

---

## 8. Acceptance Mapping for `bd-3d8.11.8.1`

Deliverable: event names and dimensions  
Completed: Sections 2, 3, and 4.

Deliverable: dashboard panels blueprint  
Completed: Section 5.

Deliverable: direct mapping to instrumentation tasks  
Completed: Section 6.

Acceptance: includes latency/failure/approval/conversion slices  
Completed: Section 5.

Acceptance: directly maps to logging instrumentation tasks  
Completed: Section 6.
