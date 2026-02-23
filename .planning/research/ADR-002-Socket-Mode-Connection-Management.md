# ADR-002: Slack Socket Mode Connection Management

**Status:** Accepted  
**Date:** 2026-02-23  
**Related Bead:** `bd-256v.2`

## Context

Quotey depends on Slack Socket Mode as a primary ingress channel. Socket connections refresh regularly and may disconnect without warning. Slack payloads require acknowledgement and can be retried.

Without explicit connection and idempotency architecture, we risk:
- duplicate business mutations,
- avoidable downtime during refresh cycles,
- unstable user-facing responsiveness under rate limiting.

## Decision

Adopt the following connection management model:

1. Maintain **two active Socket Mode connections** by default.
2. Handle `warning`/`refresh_requested` disconnect reasons by pre-opening replacement connections before retiring old connections.
3. Use **ack-fast** ingress and durable idempotency ledger before business processing.
4. Enforce method-aware outbound retry with `Retry-After` handling for API rate limits.
5. Treat SQLite as local operational source of truth for command processing and replay safety.

## Rationale

- Slack explicitly documents periodic connection refresh and multi-connection support.
- Envelope-level ack requirement and retry possibility imply at-least-once delivery characteristics at transport layer.
- Deterministic domain processing requires application-level idempotency.
- Slack API rate limits and 2025/2026 changes make proactive throttling and local-state-first behavior necessary.

## Consequences

### Positive
- Graceful refresh with minimal downtime risk.
- Deterministic behavior under duplicate deliveries.
- Better resilience under transient API constraints.
- Clear observability model for ingress health.

### Negative
- More operational complexity than single connection.
- Need additional queue/idempotency infrastructure.
- Requires careful alert tuning to avoid noise.

## Guardrails

1. No business mutation before idempotency reservation.
2. No assumption of global event ordering across socket connections.
3. Always include correlation and operation IDs in trace and audit records.
4. No infinite reconnect loops; bounded backoff is mandatory.

## Verification Plan

1. Simulate disconnect/refresh and verify continuous processing.
2. Replay duplicate envelopes and verify exactly-once business effect.
3. Inject 429 responses and verify `Retry-After` compliance.
4. Load test ack latency while downstream processing is intentionally delayed.

## Revisit Triggers

- Sustained reconnect storms.
- Frequent idempotency conflict anomalies.
- New Slack platform behavior changing Socket Mode reliability assumptions.
- Scale shift requiring connection sharding beyond dual-connection baseline.

## References

- https://docs.slack.dev/apis/events-api/using-socket-mode
- https://docs.slack.dev/reference/methods/apps.connections.open/
- https://docs.slack.dev/apis/web-api/rate-limits/
- https://slack-rust.abdolence.dev/socket-mode.html

