# RCH-08: Security and Compliance Baseline

**Research Task:** `bd-3d8.11.9`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** LilacMountain (Codex)  
**Primary Inputs:** `.planning/PROJECT.md`, `.planning/ARCHITECTURE_DECISION_RESEARCH.md`

---

## Executive Summary

This document establishes Quotey's alpha security baseline across:

1. threat model for Slack, local storage, adapters, and operator actions,
2. control mapping for authn/authz, secrets, audit integrity, and PII handling,
3. acceptance criteria, tamper-evidence hooks, and incident response requirements.

Decision:

- adopt a minimum enforceable baseline (not optional guidance),
- fail closed for critical insecurity (secret/config posture, identity ambiguity),
- keep complete audit lineage for high-impact commercial decisions.

This aligns with ADR-0016 and supports enterprise trust requirements for CPQ workflows.

---

## 1. Objective and Acceptance Mapping

Required outputs from `bd-3d8.11.9`:

1. Threat model covering Slack, storage, adapters, and operator actions.
2. Control map for authn/authz, secrets, audit integrity, and PII handling.
3. Security acceptance criteria for future features.

Acceptance mapping:

- ranked risks with likelihood/impact: Section 3.
- mitigation ownership + verification plan: Sections 4 and 8.
- tamper-evidence + incident hooks: Sections 6 and 7.

---

## 2. Security Scope and Trust Boundaries

### 2.1 In-Scope Assets

1. quote/pricing/approval data in SQLite
2. Slack app tokens and bot identity
3. LLM and CRM provider credentials
4. approval authority actions and audit records
5. generated quote artifacts (PDFs and references)

### 2.2 Trust Boundaries

1. Slack ingress boundary (external requests into runtime)
2. Adapter egress boundary (Composio/LLM/provider APIs)
3. Local persistence boundary (SQLite + files)
4. Operator boundary (CLI/admin commands, local process environment)

### 2.3 Assumptions

1. Local host OS is managed by the operator but not implicitly trusted for perfect hygiene.
2. Network failures and provider anomalies are expected.
3. Human actors can make mistakes; least-privilege and auditable workflows are required.

---

## 3. Threat Model and Ranked Risk Register

Scoring model:

- Likelihood: `Low`, `Medium`, `High`
- Impact: `Low`, `Medium`, `High`, `Critical`

| Threat | Surface | Likelihood | Impact | Priority | Baseline Mitigation |
|---|---|---|---|---|---|
| secret leakage in logs/errors | runtime/logging | Medium | Critical | P0 | redaction-by-default + log scrub tests |
| token misuse/compromise | Slack/LLM/CRM credentials | Medium | High | P0 | env-only secrets + rotation/runbook |
| unauthorized approval action | Slack interactive actions | Medium | High | P0 | actor role validation + idempotent decision keys |
| audit tampering | local DB/operator path | Low | Critical | P0 | append-only audit model + integrity checks |
| PII over-logging | logs/telemetry | Medium | High | P0 | structured field allowlist + PII redaction policy |
| stale approval reused after quote change | approval workflow | Medium | High | P1 | quote-version binding + stale action rejection |
| insecure SQLite file permissions | local storage | Medium | Medium | P1 | doctor permission checks + startup warnings/failures |
| adapter schema drift causing silent corruption | CRM/LLM adapters | Medium | High | P1 | strict mapping validation + explicit error categories |
| replayed Slack callbacks causing duplicate decisions | Slack retry behavior | Medium | High | P1 | idempotency ledger + action dedupe keys |
| misconfigured security posture in deployment | operator config | Medium | Medium | P1 | startup config validation + doctor checks |

---

## 4. Control Mapping

### 4.1 Authentication and Authorization Controls

1. Validate Slack actor identity on all interactive approval actions.
2. Enforce role-based approval authority from deterministic matrix, not UI claims.
3. Bind decisions to quote version and approval request id.
4. Reject stale or conflicting actions deterministically.

Verification:

- approval authorization tests across threshold boundaries,
- replay/race tests for duplicate callbacks.

Owner: Approval Engine + Slack Adapter.

### 4.2 Secrets Management Controls

1. Store secrets only via environment variables.
2. Represent secrets with typed wrappers (`SecretString` policy).
3. Prevent secret display in debug/error serialization.
4. Validate required secrets at startup for enabled providers.

Verification:

- redaction tests,
- secret-misconfiguration startup tests.

Owner: Config/Platform.

### 4.3 Audit Integrity Controls

1. Audit events are append-only.
2. Every mutation path emits an audit event with actor and correlation metadata.
3. Approval decisions include rationale and policy trigger references.
4. Integrity check utility validates event continuity and required field presence.

Verification:

- audit coverage tests,
- continuity checks in integration scenarios.

Owner: Audit/Platform.

### 4.4 PII Handling Controls

1. Classify PII-bearing fields and avoid default logging.
2. Permit only approved telemetry fields (allowlist model).
3. Redact or hash sensitive identifiers where full value is unnecessary.
4. Include explicit `pii_class` metadata for structured event schemas where relevant.

Verification:

- log scanning checks,
- field allowlist conformance tests.

Owner: Security + Observability.

---

## 5. Sensitive Data Boundary and Classification

### 5.1 Data Classes

1. `C0 Public`: non-sensitive operational metadata.
2. `C1 Internal`: standard business metadata (non-sensitive by default).
3. `C2 Confidential`: deal values, discount strategy, approval rationale.
4. `C3 Restricted`: credentials, tokens, personal identifiers.

### 5.2 Handling Policy by Class

| Class | Storage | Logging | External Transmission |
|---|---|---|---|
| C0 | plain | allowed | allowed |
| C1 | plain | allowlisted only | allowed with purpose |
| C2 | encrypted transport, controlled local access | summary only | minimal necessary |
| C3 | secret wrappers, no plaintext persistence in config | never raw | only to required provider endpoints |

---

## 6. Tamper-Evidence Hooks

### 6.1 Audit Tamper-Evidence Baseline

1. Append-only event semantics.
2. Event continuity checks (`previous_event_ref` or equivalent sequence integrity strategy).
3. Periodic integrity snapshot (hash over event windows).
4. Explicit alert on continuity break or missing required events.

### 6.2 Pricing/Approval Integrity Anchors

1. Pricing snapshot immutability.
2. Approval decision immutability for resolved requests.
3. Version-linked references between quote, pricing snapshot, and approvals.

---

## 7. Incident Response Hooks

### 7.1 Required Detection Hooks

1. secret leak pattern detection in logs
2. repeated auth failures by provider
3. unauthorized/stale approval action attempts
4. audit integrity check failures

### 7.2 Severity Classification (alpha)

1. `SEV-1`: credential exposure, audit tamper evidence failure, unauthorized approval accepted.
2. `SEV-2`: prolonged CRM/LLM auth failures, unresolved reconciliation affecting quote delivery.
3. `SEV-3`: transient mapping or timeout issues with safe fallback.

### 7.3 Response Workflow

1. create incident record with correlation references,
2. contain impact (disable compromised integration/token),
3. rotate secrets and revalidate config posture,
4. run integrity checks and replay critical workflows,
5. document post-incident action items linked to beads.

---

## 8. Security Acceptance Criteria for Future Features

No feature is "ready" unless all criteria pass:

1. threat delta documented (new assets, boundaries, attack paths),
2. required controls implemented for new surfaces,
3. redaction and authorization tests added/updated,
4. audit coverage remains 100% for new mutation paths,
5. incident hooks include the new feature failure modes.

Release gate for security-critical changes:

1. no known secret leakage path in normal/exception flow,
2. startup and doctor checks pass for security posture,
3. approval authority and stale-action protections verified.

---

## 9. Implementation Handoff Notes

### For `bd-3d8.11.9.1` (threat worksheet/control map)

1. expand Section 3 into detailed STRIDE-style worksheet per boundary.
2. map each risk to concrete test cases and monitoring queries.
3. track residual risk and accepted exceptions explicitly.

### For platform/security implementation tasks

1. enforce secret wrappers and serialization redaction policy,
2. add doctor checks for DB permissions and insecure config posture,
3. add audit integrity and log scanning regression tests.

### For `bd-3d8.11.10` (decision freeze)

Freeze:

1. threat model scope and priority register,
2. control baseline (authz, secrets, audit, PII),
3. severity model and incident response hooks.

---

## 10. Done Criteria Mapping

Deliverable: Threat model across critical surfaces  
Completed: Sections 2 and 3.

Deliverable: Control mapping for authn/authz, secrets, audit, PII  
Completed: Section 4 and 5.

Deliverable: Security acceptance criteria  
Completed: Section 8.

Acceptance: Ranked risk model  
Completed: Section 3.

Acceptance: Mitigation ownership and verification  
Completed: Section 4.

Acceptance: Tamper-evidence and incident hooks  
Completed: Sections 6 and 7.

