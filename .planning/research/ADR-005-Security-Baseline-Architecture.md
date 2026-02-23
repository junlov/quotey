# ADR-005: Security Baseline Architecture for Foundation

**Status:** Accepted  
**Date:** 2026-02-23  
**Related Bead:** `bd-3d8.11.9`

## Context

Quotey manages sensitive commercial operations (pricing, approvals, customer context) and must remain auditable and deterministic. Security failures can invalidate quote integrity and create contractual or compliance risk.

Foundation needs a baseline security architecture before wider feature expansion.

## Decision

Adopt a baseline security architecture with these pillars:

1. **Deterministic authority boundaries**
- LLM/adapters cannot directly decide pricing, policy, or approval outcomes.

2. **Strong identity and authorization checks**
- approval and finalize operations require explicit actor validation.

3. **Secret and PII protection by default**
- secrets redacted in logs/diagnostics,
- minimal external context exposure.

4. **Audit integrity and tamper-evidence**
- append-only critical events,
- correlation/operation metadata required.

5. **Operational detection and response hooks**
- security event taxonomy,
- alerting and incident playbooks for high-impact anomalies.

## Rationale

- Prevents highest-impact risks (unauthorized approvals, secret leaks, audit gaps).
- Preserves legal/commercial defensibility of quote lifecycle decisions.
- Creates reusable guardrails for all subsequent feature work.

## Consequences

### Positive
- Clear minimum security bar for all teams.
- Better incident detectability and forensic reconstruction.
- Reduced chance of silent policy/security regressions.

### Negative
- Adds implementation overhead (tests, redaction logic, security checks).
- Requires continuous maintenance of playbooks and controls.

## Guardrails

1. No high-impact mutation without audit event emission.
2. No privileged action without actor authorization check.
3. No secret-bearing value in logs, debug output, or error chains.
4. No direct adapter-to-domain mutation bypassing service validations.

## Verification Plan

1. Security-focused integration tests (unauthorized, replay, tamper, redaction).
2. `doctor` command security checks in CI and runtime readiness.
3. Alert simulation drills for key incident classes.

## Revisit Triggers

- Significant deployment-model change (single-tenant local -> multi-tenant).
- New regulatory/compliance obligations.
- Repeated incidents indicating control gaps.

## References

- https://www.sqlite.org/security.html
- https://api.slack.com/authentication
- https://owasp.org/www-project-application-security-verification-standard/

