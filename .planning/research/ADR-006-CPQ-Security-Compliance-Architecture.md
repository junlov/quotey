# ADR-006: CPQ Security and Compliance Architecture

**Status:** Accepted  
**Date:** 2026-02-23  
**Related Bead:** `bd-256v.5`

## Context

CPQ systems encode commercial commitments. Inaccurate or unauthorized quote outcomes can create financial, legal, and trust consequences.

Quotey needs a practical architecture that supports both security best practices and compliance evidence needs while remaining local-first and developer-operable.

## Decision

Adopt a security/compliance architecture with these pillars:

1. **Deterministic control authority**
- pricing/policy/approval logic remains deterministic and auditable.

2. **Identity-bound privileged actions**
- all approval/finalization/override actions require actor authentication and authorization checks.

3. **Tamper-evident operational records**
- append-only audit events with correlation metadata,
- optional hash-chain integrity mode.

4. **Layered data protection**
- encrypted transport for adapters,
- protected storage and encrypted backups,
- profile-based encrypted SQLite option for stricter environments.

5. **Operational security lifecycle**
- key rotation/revocation playbooks,
- security-aware diagnostics,
- alerting on suspicious behavior and integrity failures.

## Rationale

- Aligns CPQ reliability with enterprise security expectations.
- Supports evidence generation for audits and incident response.
- Preserves local-first deployment flexibility with profile-based hardening levels.

## Consequences

### Positive
- Stronger assurance for pricing/approval integrity.
- Better readiness for enterprise procurement/security reviews.
- Reduced mean time to investigate security incidents.

### Negative
- Additional implementation and ops complexity.
- Need sustained discipline in log redaction and role governance.

## Guardrails

1. No direct adapter path to authoritative business mutations.
2. No privileged business action without explicit actor verification.
3. No release that fails security acceptance checklist.
4. No silent audit gaps for high-impact operations.

## Verification Plan

1. Security integration tests (unauthorized action, replay, tamper simulation).
2. Audit integrity verification checks in CI and runtime diagnostics.
3. Periodic tabletop incident-response exercises.

## Revisit Triggers

- Multi-tenant architecture introduction.
- New compliance obligations from target customer segments.
- Repeated incidents indicating baseline control insufficiency.

## References

- https://api.slack.com/authentication
- https://www.sqlite.org/security.html
- https://owasp.org/www-project-application-security-verification-standard/
- https://www.nist.gov/cyberframework

