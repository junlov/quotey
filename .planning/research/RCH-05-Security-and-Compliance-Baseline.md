# RCH-08: Security and Compliance Baseline

**Bead:** `bd-3d8.11.9`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** IvoryBear (Codex)

---

## Executive Summary

Quotey handles pricing decisions, approval workflows, and customer/deal context. This creates meaningful security and compliance exposure even in local-first deployments.

This baseline defines:
1. a practical threat model,
2. prioritized controls by risk,
3. security acceptance criteria for implementation beads,
4. tamper-evidence and incident hooks.

Top conclusions:
- Highest-impact risks are secret leakage, unauthorized approval actions, tampered audit trails, and adapter-side data exfiltration.
- Security architecture must enforce deterministic authority boundaries and append-only audit semantics.
- Local-first does not eliminate enterprise compliance requirements; it changes control implementation details.

---

## 1. Scope and Assets

## 1.1 In-Scope Surfaces

1. Slack ingress and interactive callbacks.
2. Local SQLite datastore and local filesystem artifacts.
3. LLM provider integrations.
4. CRM/Composio adapter boundary.
5. CLI/operator workflows.
6. Audit and observability pipelines.

## 1.2 High-Value Assets

1. Slack bot/app tokens and provider API keys.
2. Quote pricing outputs and policy decisions.
3. Approval decision records and authority metadata.
4. Customer/deal context (PII and sensitive commercial data).
5. Audit events proving quote lineage and decision causality.
6. Policy/rules definitions controlling discount/approval behavior.

## 1.3 Data Classification (Baseline)

- **Class A (High):** secrets, credentials, approval authority mappings.
- **Class B (Medium-High):** quote financials, discount rationale, customer identifiers.
- **Class C (Medium):** operational telemetry with business references.
- **Class D (Low):** non-sensitive metadata and static documentation.

---

## 2. Threat Model (STRIDE-oriented)

## 2.1 Slack Interface Threats

1. Spoofed or replayed interaction payloads.
2. Unauthorized user attempts to trigger approval/finalization actions.
3. Thread context confusion causing operations on wrong quote.

Controls:
- signature/token validation via Slack SDK/client security primitives,
- strict idempotency keys for callback actions,
- quote-thread binding validation before mutation,
- role/authority checks before approval actions.

## 2.2 Local Storage Threats (SQLite + Files)

1. Unauthorized local read of DB or generated artifacts.
2. Local tampering with quote/audit rows.
3. Direct DB edits bypassing migration and policy pathways.

Controls:
- least-privilege filesystem permissions,
- startup/doctor checks for insecure file modes,
- append-only audit policy with hash chaining option,
- migration-only schema mutation policy,
- periodic integrity checks.

## 2.3 Adapter Threats (LLM + CRM)

1. Prompt injection and exfiltration via untrusted inputs.
2. Over-broad credentials to external providers.
3. Response poisoning or schema drift from external APIs.

Controls:
- strict schema validation and bounded context injection,
- provider-specific least-privilege credential scopes,
- anti-corruption mapping layer,
- typed error taxonomy + fail-closed behavior for high-risk operations.

## 2.4 Operator/CLI Threats

1. Accidental destructive changes to policies.
2. Secret leakage via command output.
3. Misuse of override actions without approval provenance.

Controls:
- redacted-by-default diagnostics,
- explicit confirmation gates for high-impact operations,
- immutable audit records for override operations,
- role-aware CLI command restrictions (where deploy environment supports identity mapping).

## 2.5 Observability Threats

1. Sensitive data leakage in logs.
2. Missing audit events for high-impact mutations.
3. Log/audit divergence making forensic reconstruction impossible.

Controls:
- structured logging schema with PII/secret denylist,
- mandatory event emission invariants for critical operations,
- correlation IDs across ingress, mutation, and outbound effects.

---

## 3. Risk Register (Likelihood x Impact)

Scale:
- Likelihood: Low/Med/High
- Impact: Low/Med/High/Critical

| Risk ID | Threat | Likelihood | Impact | Priority | Mitigation Owner |
|---|---|---|---|---|---|
| SEC-01 | Secret leakage in logs/errors | Med | Critical | P0 | runtime + config |
| SEC-02 | Unauthorized approval action | Med | Critical | P0 | approval service |
| SEC-03 | Duplicate/replayed callbacks mutate state | Med | High | P0 | idempotency layer |
| SEC-04 | Audit trail tampering or silent gaps | Low-Med | Critical | P0 | audit service + DB layer |
| SEC-05 | Prompt injection causing policy bypass attempt | High | High | P0 | agent runtime guardrails |
| SEC-06 | Overprivileged adapter credentials | Med | High | P1 | integration layer |
| SEC-07 | PII leakage via telemetry | Med | High | P1 | observability layer |
| SEC-08 | Local DB/file theft on compromised host | Med | High | P1 | deployment hardening |
| SEC-09 | Policy/rules unauthorized edits | Low-Med | High | P1 | admin tooling + audit |
| SEC-10 | Incident detection delayed (no hooks/alerts) | Med | High | P1 | ops/telemetry |

---

## 4. Control Baseline (Control Mapping)

## 4.1 Authentication and Authorization Controls

1. Validate Slack transport/auth context for each inbound event.
2. Map Slack user identity to internal actor identity before approval actions.
3. Enforce policy-driven authority checks for approve/reject/finalize operations.
4. Deny unknown/unmapped actors for privileged operations.

## 4.2 Secret Management Controls

1. Store secrets in env/runtime secret stores; never hardcode.
2. Use typed secret wrappers in code (`SecretString` style patterns).
3. Redact secret-bearing fields in logs and diagnostics.
4. Add `doctor` checks for insecure placeholder configs in non-dev modes.

## 4.3 Data Integrity and Tamper-Evidence Controls

1. Append-only audit records for business-critical events.
2. Include causal metadata (`operation_id`, `correlation_id`, actor).
3. Consider per-event hash chain (`prev_hash`, `event_hash`) for tamper evidence in v1.1.
4. Block quote finalization when required audit artifacts are missing.

## 4.4 Privacy and PII Controls

1. Classify and tag PII fields in model inputs/outputs and logs.
2. Minimize context payload to LLM/adapters (need-to-know snippets only).
3. Redact or tokenize sensitive identifiers in lower environment artifacts.
4. Define retention/deletion strategy for generated docs and debug artifacts.

## 4.5 Integration Boundary Controls

1. Anti-corruption mapping between external schemas and domain DTOs.
2. Fail-closed for schema mismatch on high-impact operations.
3. Rate-limit, timeout, and retry bounds to prevent abuse and cascade failures.
4. Store adapter outcomes in integration records (auditable success/failure).

## 4.6 Operational Controls

1. Security-focused health checks in `doctor` (permissions, missing secrets, migration posture).
2. Alerting on suspicious spikes (failed auth, repeated replays, tamper-check failures).
3. Incident runbooks for credential compromise and suspicious quote mutations.
4. Explicit key rotation and revocation procedure.

---

## 5. Security Acceptance Criteria for Future Features

Any new feature touching quote, approval, or integration flows must satisfy:

1. Threat model delta documented.
2. Data classification impact documented.
3. Secrets/PII exposure analysis completed.
4. Authorization checks implemented for privileged actions.
5. Audit event coverage added for all new high-impact mutations.
6. Negative tests for unauthorized/replay/malformed payloads.
7. Monitoring/alert updates if new attack surface introduced.

A feature is not release-ready if any of these are missing.

---

## 6. Security Hooks and Incident Response Baseline

## 6.1 Detection Hooks

Emit high-priority security events for:
- failed actor authorization on approval operations,
- repeated idempotency collision anomalies,
- missing required audit event invariant,
- secret redaction failure detection,
- tamper-evidence check mismatch,
- unusual adapter error rates that suggest abuse.

## 6.2 Incident Workflow (Minimum)

1. Detect and classify severity.
2. Contain (disable integration path / revoke credentials / freeze risky actions).
3. Investigate with audit + correlation traces.
4. Recover with validated config/keys and replay-safe reconciliation.
5. Post-incident review with control updates and bead follow-ups.

## 6.3 Required Playbooks

1. Credential compromise response.
2. Unauthorized approval action response.
3. Suspected audit tampering response.
4. Sensitive-data leak response.

---

## 7. Compliance Alignment Notes (Pragmatic)

This baseline is designed to support common enterprise expectations (SOX-style auditability, privacy handling, access control discipline) without claiming certification.

Compliance posture depends on deployment context. For each target environment, map controls to required frameworks and evidence expectations.

---

## 8. Verification Plan

## 8.1 Test Categories

1. AuthZ negative tests: unauthorized actor denies.
2. Replay tests: duplicate callbacks do not duplicate state changes.
3. Redaction tests: no secrets in logs/errors.
4. Audit completeness tests: every critical operation emits expected event set.
5. Tamper simulation tests: detect modified audit row/hash mismatch.
6. Adapter schema mismatch tests: fail-closed behavior validated.

## 8.2 Operational Checks

1. `doctor` security checks pass in baseline environment.
2. File permissions checks pass for DB/artifact paths.
3. Alert rules trigger on injected suspicious events.

---

## 9. Implementation Handoff (Bead Mapping)

- `bd-3d8.8` (errors/telemetry): adopt security event taxonomy and required fields.
- `bd-3d8.9` (CLI): implement secure diagnostics and redaction defaults.
- `bd-3d8.10` (foundation gate): add security smoke checks to gate matrix.
- `bd-3d8.11.9.1` (threat model worksheet): derive directly from Sections 2-4.
- `bd-256v.5` (security/compliance research): can reuse and deepen this baseline for broader product track.

---

## 10. Acceptance Criteria Traceability (`bd-3d8.11.9`)

- **Threat model covering Slack/storage/adapters/operator actions:** Sections 2 and 3 complete.
- **Control mapping for authn/authz, secrets, audit integrity, PII:** Section 4 complete.
- **Security acceptance criteria for future features:** Section 5 complete.
- **Risks ranked by likelihood and impact:** Section 3 complete.
- **Mitigation ownership + verification plan:** Sections 3, 8, 9 complete.
- **Tamper-evidence and incident response hooks:** Sections 4.3 and 6 complete.

Result: bead acceptance criteria satisfied.

---

## 11. References

1. Slack app security resources: https://api.slack.com/authentication  
2. Slack interactivity handling: https://docs.slack.dev/interactivity/handling-user-interaction/  
3. SQLite security considerations: https://www.sqlite.org/security.html  
4. OWASP ASVS project overview: https://owasp.org/www-project-application-security-verification-standard/  
5. NIST CSF 2.0: https://www.nist.gov/cyberframework

