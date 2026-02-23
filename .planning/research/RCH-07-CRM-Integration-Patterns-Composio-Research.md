# RCH-07: CRM Integration Patterns (Composio) Research

**Bead:** `bd-256v.7`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** IvoryBear (Codex)

## 1. Executive Summary

Recommended CRM integration strategy for Quotey:

1. Use **Composio as the authentication and tool-execution control plane**.
2. Keep **Quotey as deterministic source of truth** for quote lifecycle/pricing/approvals.
3. Use **field-level ownership policies** for bidirectional sync (`crm_wins`, `local_wins`, `merge_with_audit`, `immutable_after_bind`).
4. Use **eventual consistency + reconciliation queue**, not synchronous hard coupling to CRM on user request path.
5. Ship with **offline stub adapter parity** for deterministic demos and CI.

This aligns with local-first reliability and avoids fragile coupling to vendor-specific API semantics.

## 2. Composio Integration Guide

## 2.1 Authentication and User Scoping

Composio requirements from docs:

1. API authentication via `x-api-key`.
2. Tool execution scoped by `user_id` and connected account context.
3. Auth config defines toolkit auth method/scopes and can use managed auth or custom credentials.

Practical guidance:

1. Create per-toolkit Auth Configs for Salesforce and HubSpot.
2. Persist `connected_account_id` mapping in Quotey local DB per workspace user.
3. Treat missing/expired connected accounts as recoverable integration state, not fatal quote state.

## 2.2 Tool Execution Model

Use Composio for:

1. discovery of available toolkit actions,
2. controlled execution with typed arguments,
3. user-scoped credential injection.

Guidelines:

1. Tool execution requests should carry Quotey `correlation_id` in metadata.
2. Normalize responses immediately into internal DTOs.
3. Do not allow toolkit-specific payloads to cross into core domain.

## 2.3 Rate Limits and Backpressure

Composio limits are organization-wide and include tool execution and connected-account APIs.

Engineering policy:

1. inspect `X-RateLimit-Remaining` and `Retry-After`,
2. apply bounded retries with jitter on `429`,
3. prefetch/cache static tool metadata to reduce call volume.

## 3. Salesforce Integration Patterns

## 3.1 Preferred Data Access Pattern

For core CRM sync workflows:

1. Use REST object resources for single-record CRUD.
2. Use Composite resources when multiple dependent updates should be grouped.
3. Use Limits endpoint + response headers for consumption monitoring.

Why:

1. Composite can reduce round trips and keep business operation grouping explicit.
2. Request counting behavior must be considered in capacity planning.

## 3.2 Event-Driven Inbound Pattern (Optional/Advanced)

For near-real-time synchronization:

1. use Salesforce Change Data Capture feeds via Pub/Sub API where available,
2. persist replay cursor and process events idempotently,
3. account for 72-hour event durability window.

Guidance:

1. keep polling fallback available where event plumbing is unavailable,
2. decode CDC metadata safely before field application,
3. run replay recovery workflows after disconnects.

## 4. HubSpot Integration Patterns

## 4.1 Deals and Core Objects

For deals workflow:

1. create via `/crm/v3/objects/deals`,
2. required create fields include `dealname`, `dealstage`, and `pipeline` when needed,
3. patch updates via deal ID or unique property route.

## 4.2 Rate-Limit Aware Design

HubSpot constraints (plan-specific and endpoint-specific) require:

1. short-window throttling by token/app,
2. explicit 429 handling and delayed retries,
3. special treatment for search endpoints with stricter limits.

Engineering policy:

1. batch operations where supported,
2. cache stable metadata (properties, pipelines, association labels),
3. route heavy sync tasks through scheduled worker with budget controls.

## 5. Data Mapping Examples (Provider-Aware)

The canonical mapping contract remains adapter-neutral; provider specifics are adapter-local.

| Canonical Field | Salesforce Example | HubSpot Example | Owner Policy |
|---|---|---|---|
| `deal.crm_ref` | `Opportunity.Id` | `deal.hs_object_id` | `immutable_after_bind` |
| `deal.stage` | `Opportunity.StageName` | `deal.dealstage` | `crm_wins` |
| `deal.close_date` | `Opportunity.CloseDate` | `deal.closedate` | `crm_wins` |
| `quote.id` | custom `Quote__c.Quotey_Id__c` | custom `deal.quotey_quote_id` | `local_wins` |
| `quote.version` | custom `Quote__c.Quotey_Version__c` | custom `deal.quotey_quote_version` | `local_wins` |
| `quote.total` | custom amount field | custom amount field | `local_wins` |

Provider-specific rules:

1. Salesforce often supports richer transaction composition (Composite).
2. HubSpot requires explicit pipeline/dealstage handling and strict rate-aware batching.

## 6. Sync and Conflict Strategy

Recommended sync model:

1. inbound CRM refresh (scheduled/event-driven),
2. outbound quote writeback (state transition-driven),
3. reconciliation queue for non-terminal mismatches.

Conflict resolution:

1. apply field ownership policy deterministically,
2. reject immutable ID conflicts immediately,
3. quarantine schema/type mismatches as terminal mapping errors,
4. keep unresolved conflicts operator-visible with actionable payload.

## 7. Error Handling and Reliability

Error classes:

1. transport transient (`timeout`, `5xx`, `429`) -> retry with backoff/jitter,
2. auth/config (`401`, `403`, missing connection) -> no blind retry, prompt reconnect,
3. mapping/contract (`schema mismatch`, missing required field) -> terminal + reconciliation item,
4. stale write/version conflict -> non-retryable conflict with operator action.

Retry policy baseline:

1. exponential backoff + jitter,
2. bounded max attempts,
3. dead-letter + replay path for exhausted retries.

Observability:

1. emit `integration.crm.sync_started/completed/failed`,
2. emit conflict-class metrics and queue age,
3. attach `quote_id`, `operation_id`, `correlation_id` to every writeback event.

## 8. Offline Stub Implementation Plan

Stub requirements:

1. Same trait interface as Composio adapter.
2. Deterministic fixtures for account/contact/deal data.
3. Simulated error modes (`timeout`, `rate_limit`, `mapping_error`) for testability.
4. Replayable sync cursor behavior for CI scenarios.

Suggested structure:

1. fixture files under `config/crm/stub/*.json`,
2. deterministic ID and timestamp generation,
3. toggles for forced error injections in tests.

## 9. Security and Compliance Notes

1. Scope OAuth credentials minimally in Auth Configs.
2. Never log raw access tokens or sensitive payload fields.
3. Maintain audit trail for outbound write intents and outcomes.
4. Support token revocation and reconnect flow without losing local quote continuity.

## 10. Rollout Plan

1. Implement adapter trait and DTO normalization boundary first.
2. Land stub adapter + fixtures for CI and demo flows.
3. Add Composio Salesforce writeback path.
4. Add HubSpot writeback path.
5. Add inbound sync and reconciliation UI/CLI tools.
6. Add optional CDC/PubSub subscription workflow for Salesforce advanced deployments.

## 11. Deliverable Coverage vs Bead Requirements

`bd-256v.7` requested:

1. **Composio integration guide**: Section 2.
2. **Salesforce sync patterns**: Section 3.
3. **HubSpot sync patterns**: Section 4.
4. **Data mapping examples**: Section 5.
5. **Error handling strategies**: Section 7.
6. **Stub implementation for offline demos**: Section 8.
7. **ADR**: `.planning/research/ADR-RCH07-CRM-Integration-Architecture.md`.

## 12. Primary Sources

1. Composio authentication: https://docs.composio.dev/reference/authentication
2. Composio auth configs / managed auth: https://docs.composio.dev/docs/authenticating-tools
3. Composio tool execution: https://docs.composio.dev/docs/executing-tools
4. Composio rate limits: https://docs.composio.dev/reference/rate-limits
5. Composio Salesforce toolkit: https://docs.composio.dev/tools/salesforce
6. Salesforce API usage and limits: https://developer.salesforce.com/blogs/2024/11/api-limits-and-monitoring-your-api-usage
7. Salesforce object-data API patterns: https://developer.salesforce.com/blogs/2024/04/accessing-object-data-with-salesforce-platform-apis
8. Salesforce Pub/Sub intro: https://developer.salesforce.com/docs/platform/pub-sub-api/guide/intro.html
9. Salesforce event durability: https://developer.salesforce.com/docs/platform/pub-sub-api/guide/event-message-durability.html
10. HubSpot API usage guidelines: https://developers.hubspot.com/docs/developer-tooling/platform/usage-guidelines
11. HubSpot deals API guide: https://developers.hubspot.com/docs/api-reference/crm-deals-v3/guide
