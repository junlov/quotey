# RCH-08a: Threat Model Worksheet and Control Map

**Bead:** `bd-3d8.11.9.1`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** IvoryBear (Codex)

## 1. Objective

Convert the security baseline into an implementation-ready worksheet that connects:

1. concrete abuse cases,
2. deterministic controls,
3. verification evidence,
4. logging and incident-response obligations.

This artifact is intended to be directly consumed by implementation beads and decision-freeze planning.

## 2. Inputs and Traceability

Primary references:

1. `.planning/research/RCH-05-Security-and-Compliance-Baseline.md`
2. `.planning/research/RCH-05-Security-Audit-Compliance-Research.md`
3. `.planning/research/ADR-005-Security-Baseline-Architecture.md`
4. `.planning/research/ADR-006-CPQ-Security-Compliance-Architecture.md`
5. `.planning/PROJECT.md` (safety principle, local-first constraints)

Control authority reminder:

- LLMs and adapters may transform or summarize inputs.
- Deterministic services remain authoritative for pricing, policy, approval, and finalization.

## 3. Scope and Trust Boundaries

Boundary map:

1. **Ingress boundary**: Slack Socket Mode events and interactive callbacks.
2. **Runtime boundary**: agent runtime, flow engine, CPQ core, approval service.
3. **Persistence boundary**: SQLite state + filesystem artifacts.
4. **Egress boundary**: CRM/LLM/PDF adapters and outbound APIs.
5. **Operator boundary**: CLI/admin operations and runtime configuration.

High-value assets:

1. Secrets and tokens (Slack, CRM, LLM, storage).
2. Quote/pricing outputs and policy evaluation results.
3. Approval decisions and authority lineage.
4. Audit events and integrity metadata.
5. PII-bearing customer/deal context and generated artifacts.

## 4. Control Catalog

Control IDs are referenced from the threat worksheet and verification matrix.

| Control ID | Control Name | Type | Boundary | Description |
|---|---|---|---|---|
| AUTH-01 | Actor Identity Validation | Preventive | Ingress/Runtime | Map Slack actor to internal principal; reject unknown actors. |
| AUTH-02 | Approval Authority Enforcement | Preventive | Runtime | Validate actor against deterministic approval matrix and quote version. |
| IDEMP-01 | Callback Idempotency Ledger | Preventive | Ingress/Runtime | Deduplicate replayed Slack callbacks via operation key and TTL. |
| FLOW-01 | Thread-to-Quote Binding Guard | Preventive | Runtime | Reject operations where thread context and quote identity mismatch. |
| LLM-01 | Prompt Boundary Guard | Preventive | Egress/Runtime | Restrict model input to sanitized, minimum context and schema-bound tasks. |
| INTEG-01 | Anti-Corruption Mapping | Preventive | Egress | Validate external payloads into strict domain DTOs; fail closed on mismatch. |
| SEC-01 | Secret Redaction and Typed Secret Wrappers | Preventive/Detective | Runtime/Operator | Prevent secret values from entering logs/errors and debug serialization. |
| DB-01 | Migration-Only Schema Change Policy | Preventive | Persistence/Operator | Disallow ad hoc schema changes and direct mutation pathways. |
| AUD-01 | Append-Only Audit Event Contract | Preventive | Runtime/Persistence | Emit immutable audit events for all high-impact mutations. |
| AUD-02 | Audit Integrity Verification | Detective | Persistence/Operator | Validate event continuity and required-field presence on schedule and demand. |
| LOG-01 | Structured Security Event Taxonomy | Detective | Runtime/Ops | Emit security events with stable codes and correlation metadata. |
| PII-01 | PII Field Allowlist and Masking | Preventive/Detective | Runtime/Ops | Permit only approved sensitive fields in logs/telemetry with masking rules. |
| BKP-01 | Encrypted Backup and Restore Validation | Preventive/Detective | Persistence/Ops | Encrypt backups and routinely verify restore + integrity checks. |
| IR-01 | Incident Runbooks and Escalation Policy | Corrective | Ops | Enforce severity model, containment path, and post-incident evidence capture. |

## 5. Threat Worksheet (Abuse Cases to Controls)

Scoring:

- Likelihood: `L`, `M`, `H`
- Impact: `M`, `H`, `C` (critical)
- Priority: `P0`, `P1`, `P2`

| Threat ID | STRIDE | Abuse Case | Boundary | Likelihood | Impact | Priority | Primary Controls | Residual Risk Notes |
|---|---|---|---|---|---|---|---|---|
| TM-01 | Spoofing | Adversary submits forged interactive approval payload as privileged user. | Ingress | M | C | P0 | AUTH-01, AUTH-02, LOG-01 | Residual risk if identity mapping table stale. |
| TM-02 | Tampering | Replayed callback creates duplicate approvals or quote transitions. | Ingress/Runtime | M | H | P0 | IDEMP-01, AUTH-02, AUD-01 | Residual risk from misconfigured idempotency TTL. |
| TM-03 | Tampering | Wrong thread controls wrong quote due to context confusion. | Runtime | M | H | P0 | FLOW-01, AUTH-02, AUD-01 | Residual risk during concurrent thread migrations. |
| TM-04 | Elevation | Prompt injection attempts to coerce LLM into bypassing policy checks. | Runtime/Egress | H | H | P0 | LLM-01, AUTH-02, AUD-01 | Residual risk if model outputs treated as authority. |
| TM-05 | Information Disclosure | Over-scoped CRM token leaks or is abused to exfiltrate customer data. | Egress/Ops | M | H | P1 | SEC-01, INTEG-01, IR-01 | Residual risk until token scope governance is automated. |
| TM-06 | Tampering | Local DB rows modified directly to alter quote totals or approval state. | Persistence | M | C | P0 | DB-01, AUD-01, AUD-02 | Residual risk on compromised host with root access. |
| TM-07 | Repudiation | High-impact action occurs without corresponding audit event. | Runtime/Persistence | L-M | C | P0 | AUD-01, AUD-02, LOG-01 | Residual risk if event emission path not enforced transactionally. |
| TM-08 | Tampering | Unauthorized policy/rules change weakens approval thresholds. | Operator/Persistence | L-M | H | P1 | AUTH-02, DB-01, AUD-01, IR-01 | Residual risk from shared operator credentials. |
| TM-09 | Information Disclosure | PII and deal-sensitive fields leak through logs/telemetry. | Runtime/Ops | M | H | P1 | PII-01, SEC-01, LOG-01 | Residual risk from third-party logger defaults. |
| TM-10 | Information Disclosure | Panic/error chain exposes secret values. | Runtime/Ops | M | C | P0 | SEC-01, LOG-01, IR-01 | Residual risk if new adapters bypass redaction wrappers. |
| TM-11 | Information Disclosure | Backup artifact theft reveals quote history and approvals. | Persistence/Ops | M | H | P1 | BKP-01, SEC-01, IR-01 | Residual risk if host-level disk encryption disabled. |
| TM-12 | Denial/Detection Failure | Security anomalies occur but no actionable alerts fire. | Ops | M | H | P1 | LOG-01, AUD-02, IR-01 | Residual risk if on-call routing not maintained. |

## 6. Verification Matrix (Controls to Evidence)

Each control has mandatory verification modes:

- `U`: Unit tests
- `I`: Integration tests
- `O`: Operational checks (`doctor`, audit verify, alert simulation)
- `R`: Reviewable artifacts (logs, reports, runbooks)

| Control ID | Verification | Evidence Artifact | Pass Criteria |
|---|---|---|---|
| AUTH-01 | I, O | AuthZ deny test logs; principal resolution report | Unknown actor denied with `SEC_AUTHZ_DENY`; no side effects persisted. |
| AUTH-02 | I | Approval matrix tests across thresholds and roles | Unauthorized and stale actors rejected; authorized path succeeds once. |
| IDEMP-01 | I, O | Replay harness output; idempotency ledger dump | Duplicate callbacks produce one mutation and one `SEC_REPLAY_BLOCKED`. |
| FLOW-01 | I | Thread mismatch tests and audit trail | Cross-thread mutation rejected with stable error code and audit record. |
| LLM-01 | U, I | Prompt sanitizer tests; schema validation traces | Untrusted prompt directives cannot alter deterministic action decisions. |
| INTEG-01 | U, I | Adapter contract tests; schema mismatch logs | Invalid external payloads fail closed without domain mutation. |
| SEC-01 | U, I, O | Redaction regression tests; `doctor --security` output | No secret material appears in logs/errors in success or failure paths. |
| DB-01 | O, R | Migration-only workflow docs; schema drift checks | No direct schema mutations outside migrations in CI and ops routines. |
| AUD-01 | I | Critical mutation integration suite | Every high-impact action emits mandatory audit event set. |
| AUD-02 | O, R | `audit verify-integrity` report; scheduled check logs | No continuity gaps or missing required fields; failures alert immediately. |
| LOG-01 | U, O | Event schema contract tests; SIEM query pack | All security events include mandatory fields and stable event codes. |
| PII-01 | U, O | Log scan report with denylist checks | No raw restricted fields emitted; masked output validates format policy. |
| BKP-01 | O, R | Encrypted backup restore drill output | Restores succeed, integrity checks pass, and keys are rotation-capable. |
| IR-01 | O, R | Incident tabletop notes and runbooks | Runbooks are executable, owners assigned, and escalation works by severity. |

## 7. Logging Requirements

### 7.1 Mandatory Security Event Codes

| Event Code | Trigger | Minimum Severity | Required Action |
|---|---|---|---|
| `SEC_AUTHZ_DENY` | Actor denied for privileged action | Warning | Count by actor/thread; alert on repeated spikes. |
| `SEC_REPLAY_BLOCKED` | Duplicate callback blocked by idempotency layer | Info/Warning | Track frequency; alert if threshold exceeded. |
| `SEC_THREAD_BINDING_FAIL` | Thread and quote identity mismatch | Warning | Trigger investigation if repeated by same actor. |
| `SEC_PROMPT_GUARD_TRIGGERED` | Prompt guard strips or rejects unsafe directives | Warning | Log reason class; monitor for prompt abuse campaigns. |
| `SEC_SCHEMA_REJECT` | External adapter payload rejected by mapping contract | Warning | Alert on sustained rate to detect integration drift. |
| `SEC_SECRET_REDACTION_FAIL` | Redaction invariant violation | Critical | Page on-call and enter containment workflow. |
| `SEC_AUDIT_GAP` | Required audit event missing or invalid continuity | Critical | Freeze high-impact mutations until resolved. |
| `SEC_POLICY_CHANGE` | Policy/rules mutation committed | Info/Warning | Require actor + change rationale + approval reference. |
| `SEC_PII_LOG_BLOCKED` | PII logging attempt blocked | Warning | Track source; patch offending code path quickly. |
| `SEC_BACKUP_VERIFY_FAIL` | Backup restore or integrity check failed | High | Escalate to operations and security owner. |

### 7.2 Common Event Schema (Required Fields)

Every security event must include:

1. `event_code`
2. `event_ts`
3. `severity`
4. `correlation_id`
5. `operation_id` (if applicable)
6. `actor_id` and `actor_type` (if applicable)
7. `quote_id` and `quote_version` (if applicable)
8. `component`
9. `result` (`allowed`, `denied`, `blocked`, `failed`)
10. `reason_code` (stable enum, not free text)

Redaction rule:

- Sensitive values never appear raw in event payloads.
- Allowed representation is masked token or irreversible hash where needed for joinability.

## 8. Incident Response Requirements

### 8.1 Severity Model

| Severity | Example Triggers | Initial Response Target |
|---|---|---|
| `SEV-1` | Unauthorized approval accepted, audit gap, secret redaction failure | Immediate containment |
| `SEV-2` | Sustained adapter schema rejects, repeated replay attacks, backup verify failures | Same working day |
| `SEV-3` | Isolated denied auth attempts, transient security warning patterns | Scheduled triage |

### 8.2 Minimum Runbooks (Must Exist)

1. **Credential compromise**: revoke, rotate, validate, replay-safe recovery.
2. **Unauthorized approval suspicion**: freeze finalize path, verify lineage, restore from audit truth.
3. **Audit integrity failure**: pause high-impact mutations, run integrity diagnostics, recover chain continuity.
4. **PII/secret leak**: contain logs/artifacts, rotate keys if exposed, legal/privacy escalation path.
5. **Integration abuse surge**: constrain adapter scopes/rate limits and investigate source patterns.

### 8.3 Post-Incident Evidence Requirements

1. Incident timeline with UTC timestamps.
2. Security events and audit entries by `correlation_id`.
3. Root cause category (people, process, code, config, dependency).
4. Corrective actions mapped to beads with owners and due dates.

## 9. Implementation Mapping to Beads

This worksheet is a direct input for downstream execution:

1. `bd-3d8.8.1` layered error taxonomy should include security reason codes from Section 7.
2. `bd-3d8.9.1` doctor checks should include secret/config/file-permission assertions tied to Controls `SEC-01`, `DB-01`, `BKP-01`.
3. `bd-3d8.11.10` decision freeze should adopt the threat/control set and severity model as accepted baseline.
4. `bd-256v.8` async test strategy should include security integration harnesses for `TM-01` to `TM-10`.

## 10. Exit Criteria for This Subtask

`bd-3d8.11.9.1` acceptance criteria mapping:

1. **Contains abuse cases and control verifications**: Sections 5 and 6.
2. **Includes logging and incident response requirements**: Sections 7 and 8.

This artifact is ready for closure and decision-freeze consumption.
