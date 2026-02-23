# RCH-05: Security & Audit Compliance Research

**Bead:** `bd-256v.5`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** IvoryBear (Codex)

---

## Executive Summary

CPQ systems are high-impact business systems: quote values, discount decisions, and approval actions can create contractual obligations.

For Quotey, security and compliance should be designed as **deterministic control layers**:
1. strong authn/authz for privileged actions,
2. tamper-evident quote and audit lineage,
3. encryption + key hygiene for local-first deployments,
4. role-aware operational controls and incident response,
5. evidence-ready audit records for compliance narratives.

This research delivers:
- threat model,
- audit trail requirements,
- encryption strategy,
- authentication/authorization flow,
- SOX/GDPR-oriented checklist,
- security hardening guide,
- ADR for security architecture.

---

## 1. Threat Model (Extended)

This section extends the foundation threat model (`RCH-05-Security-and-Compliance-Baseline.md`) with compliance-oriented attack paths.

## 1.1 Primary Threat Actors

1. External attacker targeting Slack/integration credentials.
2. Internal unauthorized user attempting discount/approval abuse.
3. Compromised endpoint host with filesystem access.
4. Malicious or malfunctioning integration endpoint.
5. Operator error causing insecure deployment or data leakage.

## 1.2 Critical Abuse Scenarios

1. Approve discount without proper authority.
2. Alter quote totals or approval records after finalization.
3. Delete/modify audit events to hide unauthorized changes.
4. Exfiltrate customer/quote data via logs, prompts, or backups.
5. Replay Slack interactive actions to duplicate business effects.
6. Abuse provider credentials to query or mutate external systems.

## 1.3 Threat Prioritization

Highest priority (P0):
- unauthorized approval/finalization,
- audit tampering,
- secret leakage,
- replay-driven duplicate mutations.

High priority (P1):
- PII leakage in telemetry,
- overprivileged integration credentials,
- insecure artifact/backups.

---

## 2. Audit Trail Requirements Document

## 2.1 Non-Negotiable Audit Events

Must emit immutable audit events for:
1. Quote create/update/delete-intent (soft delete only where applicable).
2. Pricing run start/result/failure with ruleset snapshot references.
3. Policy evaluation result and violated thresholds.
4. Approval request create/escalate/approve/reject/delegate/expire.
5. Finalization and document generation outcomes.
6. High-impact operator commands and overrides.
7. External adapter side effects and reconciliation outcomes.

## 2.2 Required Audit Fields

Each critical event must include:
- `audit_event_id`
- `event_ts`
- `event_type`
- `actor_type` + `actor_id`
- `quote_id` + `quote_version` (if applicable)
- `operation_id`
- `correlation_id`
- `policy_snapshot_id` / `ruleset_snapshot_id` (where relevant)
- `result_code`
- `event_payload_hash`
- `prev_event_hash` (if hash chain enabled)

## 2.3 Tamper-Evidence Requirements

1. Append-only event semantics.
2. Optional event hash chain for strong tamper-evidence.
3. Scheduled integrity verification job comparing chain continuity.
4. Alert on gaps, out-of-order writes, or hash mismatches.

## 2.4 Retention Requirements

Baseline recommendations:
- Audit events: retain per enterprise policy (commonly multi-year retention for commercial decision records).
- Operational logs: shorter retention with redaction.
- LLM raw payload retention: minimize and redact sensitive content.

Retention policy must be configurable by deployment profile.

---

## 3. Encryption Strategy

## 3.1 Data at Rest

1. **SQLite file protection**
- minimum: strict filesystem permissions + disk encryption at host/volume level.
- higher assurance: SQLCipher or equivalent encrypted SQLite variant where required.

2. **Artifact encryption**
- quote PDFs and exports should be stored in protected directories,
- optional per-artifact encryption for high-sensitivity deployments.

3. **Backup encryption**
- backups must be encrypted in transit and at rest.
- backup key access limited to operations principals.

## 3.2 Data in Transit

1. TLS enforced for all cloud adapter calls.
2. Certificate validation must not be disabled.
3. Retry logic must not leak payloads in plaintext logs.

## 3.3 Key Management

1. No keys in code or committed files.
2. Rotation schedule for Slack, LLM, and CRM credentials.
3. Fast revocation path documented and tested.
4. Least-privilege scopes for each integration token.

## 3.4 Local-first Nuance

Local-first architecture reduces central attack surface but increases endpoint responsibility.
Security posture depends heavily on endpoint hardening and operational discipline.

---

## 4. Authentication and Authorization Flow Design

## 4.1 Authentication Sources

1. Slack user identity for interactive actions.
2. Bot/app credentials for Slack API access.
3. Provider credentials for LLM/CRM adapters.

## 4.2 Authorization Model

1. Map Slack actor -> internal role/authority profile.
2. Evaluate action permissions against policy matrix.
3. Block privileged actions when identity/authority cannot be established.
4. Require explicit role checks for approval/finalization/override actions.

## 4.3 Approval-Specific AuthZ

1. Approval action must verify actor belongs to current required approver set.
2. Delegation must preserve authority chain and audit evidence.
3. Expired approvals cannot be accepted.

## 4.4 Session and Token Handling

1. Securely store runtime tokens in process memory only.
2. Redact tokens in diagnostics.
3. Trigger degraded-mode alerts if token refresh/auth repeatedly fails.

---

## 5. Compliance Checklist (SOX/GDPR-Oriented)

This is a practical engineering checklist, not legal advice.

## 5.1 SOX-style Financial Control Readiness

- [ ] Deterministic pricing logic with versioned policy snapshots.
- [ ] Approval authority and threshold controls enforced by system.
- [ ] Immutable audit trail for pricing/approval/finalization changes.
- [ ] Segregation of duties support (requestor vs approver role separation).
- [ ] Change control process for rule/policy modifications.
- [ ] Evidence extraction path for quote decision lineage.

## 5.2 GDPR-style Privacy Readiness

- [ ] Data minimization for collected/stored personal data.
- [ ] Purpose limitation documented for each PII field.
- [ ] Retention and deletion procedures defined.
- [ ] Access controls for PII-bearing views/exports.
- [ ] Export/reporting pathways for data subject requests (as applicable).
- [ ] Redaction strategy for logs/telemetry containing personal identifiers.

## 5.3 Operational Governance

- [ ] Incident response runbooks maintained and tested.
- [ ] Credential rotation schedule enforced.
- [ ] Security monitoring and alert thresholds documented.
- [ ] Periodic access review for approval roles and integration scopes.

---

## 6. Security Hardening Guide (Deployment)

## 6.1 Host and Runtime

1. Run under least-privileged service account.
2. Restrict filesystem access to DB/config/artifact paths.
3. Keep host packages and runtime patched.
4. Disable unnecessary outbound network routes where possible.

## 6.2 Application Configuration

1. Strict startup validation (no placeholder secrets in non-dev profiles).
2. Secure default logging level and redaction filters.
3. Enforce deterministic quality gates before deployment.

## 6.3 Database and Backups

1. Enable integrity checks in operational routine.
2. Protect backup keys and destinations.
3. Test restore procedures regularly.

## 6.4 Integration Hardening

1. Scope tokens minimally.
2. Enforce bounded retry and timeout policies.
3. Log adapter failures with correlation IDs, not sensitive payload dumps.

## 6.5 Monitoring and Alerting

1. Alert on auth failures and suspicious approval attempts.
2. Alert on tamper-check anomalies.
3. Alert on repeated replay/idempotency anomalies.
4. Alert on abnormal data export volume.

---

## 7. Security Architecture Guidance for Upcoming Implementation

1. Add explicit security event taxonomy (`SEC_AUTHZ_DENY`, `SEC_TAMPER_ALERT`, etc.).
2. Add hash-chain optional mode to audit tables.
3. Add CLI commands:
- `quotey doctor --security`
- `quotey audit verify-integrity`
- `quotey security rotate-keys` (or runbook references)
4. Add integration tests for unauthorized approvals and replay abuse.

---

## 8. Residual Risks and Open Questions

1. Should encrypted SQLite (SQLCipher) be mandatory or profile-driven?
2. What minimum retention period is required by target customer segment?
3. Which quote/document artifacts require application-layer encryption vs storage-layer encryption?
4. How should cross-region data residency constraints be handled for cloud providers?

These should become follow-up beads if needed by target deployment profile.

---

## 9. Acceptance Criteria Traceability (`bd-256v.5`)

- **Security threat model:** Sections 1 and 2 complete.
- **Audit trail requirements doc:** Section 2 complete.
- **Encryption strategy:** Section 3 complete.
- **Authentication flow design:** Section 4 complete.
- **Compliance checklist (SOX/GDPR):** Section 5 complete.
- **ADR security architecture:** companion ADR delivered.
- **Security hardening guide:** Section 6 complete.

Result: bead acceptance criteria satisfied.

---

## 10. References

1. Slack authentication/security: https://api.slack.com/authentication  
2. Slack interactions security context: https://docs.slack.dev/interactivity/handling-user-interaction/  
3. SQLite security notes: https://www.sqlite.org/security.html  
4. SQLCipher project: https://www.zetetic.net/sqlcipher/  
5. OWASP ASVS: https://owasp.org/www-project-application-security-verification-standard/  
6. NIST Cybersecurity Framework 2.0: https://www.nist.gov/cyberframework  
7. EU GDPR regulation text hub: https://gdpr.eu/

