# ADR: CRM Integration Architecture (Composio + Deterministic Local Core)

**Status:** Accepted  
**Date:** 2026-02-23  
**Related Bead:** `bd-256v.7`

## Context

Quotey must integrate with Salesforce and HubSpot to support account lookup, deal context,
and quote writeback, while preserving the project's local-first and deterministic CPQ guarantees.

Key constraints:

1. CRM APIs and auth models vary significantly by provider.
2. User-facing quote workflows must remain responsive even during CRM degradation.
3. CPQ correctness (pricing, constraint validity, policy/approval) must never depend on nondeterministic external state.
4. Development and demos need a reliable offline path.

Without a strict architecture boundary, CRM coupling can leak into domain logic, create replay drift,
and turn transient integration failures into quote lifecycle failures.

## Decision

Adopt an adapter-based CRM architecture with Composio as the primary execution/auth control plane:

1. **Domain boundary**
   - Define a provider-neutral `CrmAdapter` trait and canonical DTOs.
   - Keep vendor/toolkit payloads confined to adapter implementations.

2. **Execution model**
   - Use asynchronous, eventually consistent sync.
   - Keep quote command path independent from full CRM round-trips.
   - Drive outbound writeback from deterministic quote lifecycle transitions.

3. **Ownership and conflicts**
   - Enforce field-level ownership policy (`crm_wins`, `local_wins`, `merge_with_audit`, `immutable_after_bind`).
   - Resolve conflicts deterministically; quarantine unresolved/terminal cases for reconciliation.

4. **Adapter parity**
   - Ship `StubCrmAdapter` and `ComposioCrmAdapter` behind the same trait contract.
   - Require semantic parity for contract-required fields across adapters.

5. **Reliability controls**
   - Classify errors by retryability (transient, auth/config, mapping, stale-write).
   - Apply bounded retry with jitter only to transient failures.
   - Persist reconciliation queue items for non-retryable failures.

## Rationale

1. Preserves deterministic CPQ behavior by preventing external API semantics from entering core logic.
2. Supports multi-CRM integration without rewriting domain code.
3. Improves operator resilience through explicit retry, quarantine, and reconciliation paths.
4. Enables deterministic CI/demo workflows using a fully contract-compatible stub.

## Consequences

### Positive

1. Clear separation of concerns between CPQ core and integration concerns.
2. Better testability through adapter contract tests and fixture-driven stub behavior.
3. Reduced blast radius when provider APIs, limits, or auth flows change.
4. Faster iteration on CRM features with stable domain interfaces.

### Negative

1. Additional mapping/normalization layer introduces implementation overhead.
2. Requires ongoing maintenance of provider-specific schema mappings.
3. Eventual consistency means users may briefly see CRM lag behind local quote state.

## Guardrails

1. CRM failures must not mutate authoritative quote pricing/policy state.
2. Every outbound CRM write attempt must include correlation id, quote id, and quote version.
3. Writeback must be idempotent by `(quote_id, quote_version, operation_kind)`.
4. Tokens and sensitive fields must never be logged in plaintext.
5. Adapter implementations may not bypass ownership/conflict policy evaluation.

## Verification Plan

1. Contract tests asserting required DTO parity between stub and Composio adapters.
2. Integration tests covering retry classification and dead-letter/reconciliation behavior.
3. Conflict tests for stale writes and immutable-after-bind violations.
4. Load tests for rate-limit handling and queued backpressure behavior.
5. Replay tests validating identical results for identical local event streams regardless of CRM availability.

## Revisit Triggers

1. Need for provider-native transactions beyond current adapter abstraction.
2. Sustained reconciliation backlog growth due to mapping drift.
3. Requirement for strict near-real-time sync SLOs that exceed eventual-consistency model.
4. Introduction of new CRM providers requiring broader canonical model changes.

## References

1. https://docs.composio.dev/reference/authentication
2. https://docs.composio.dev/docs/authenticating-tools
3. https://docs.composio.dev/docs/executing-tools
4. https://docs.composio.dev/reference/rate-limits
5. https://developer.salesforce.com/blogs/2024/04/accessing-object-data-with-salesforce-platform-apis
6. https://developer.salesforce.com/docs/platform/pub-sub-api/guide/intro.html
7. https://developers.hubspot.com/docs/developer-tooling/platform/usage-guidelines
8. https://developers.hubspot.com/docs/api-reference/crm-deals-v3/guide
