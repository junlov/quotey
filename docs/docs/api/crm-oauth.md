# CRM OAuth Flow

Quotey exposes deterministic OAuth endpoints for CRM providers:

- `GET /api/v1/crm/connect/{provider}`
- `GET /api/v1/crm/oauth/{provider}/callback`
- `POST /api/v1/crm/sync/{quote_id}` (Quotey -> CRM outbound sync)
- `POST /api/v1/crm/sync/batch` (batch fallback replay for outbound sync events)
- `POST /api/v1/crm/sync/inbound/batch` (batch fallback replay for inbound sync events)
- `POST /api/v1/crm/events/{event_id}/retry` (manual replay)
- `GET /api/v1/crm/outbox/tasks` (execution outbox visibility)
- `POST /api/v1/crm/outbox/tasks/{task_id}/replay` (dead-letter/task replay via correlated event)
- `POST /api/v1/crm/outbox/recover-stale` (recover stuck running outbox tasks)

Supported providers:

- `salesforce`
- `hubspot`

## Salesforce Connection Flow

1. Call `GET /api/v1/crm/connect/salesforce`.
2. Quotey creates a short-lived OAuth state token in `crm_oauth_state`.
3. Response includes:
   - `authorization_url`
   - `state_token`
   - `state_expires_at`
4. Redirect the user to `authorization_url`.
5. Salesforce redirects to `/api/v1/crm/oauth/salesforce/callback` with `code` and `state`.
6. Quotey exchanges `code` for tokens and upserts `crm_integration`.

## HubSpot Connection Flow

1. Call `GET /api/v1/crm/connect/hubspot`.
2. Quotey stores a short-lived, one-time state token for provider `hubspot`.
3. Redirect the user to the returned HubSpot authorization URL.
4. HubSpot redirects to `/api/v1/crm/oauth/hubspot/callback` with `code` and `state`.
5. Quotey validates provider-bound state, exchanges the code, then upserts `crm_integration`.

## Security Invariants

- OAuth state tokens are one-time use (`used = 0 -> 1` on reservation).
- State tokens are provider-bound:
  - Salesforce callback only accepts state rows where `provider = "salesforce"`.
  - HubSpot callback only accepts state rows where `provider = "hubspot"`.
- Provider mismatch is rejected with `400 invalid or expired oauth state token`.
- Expired state tokens are rejected.

## Response Contract

`GET /api/v1/crm/connect/salesforce`:

```json
{
  "provider": "salesforce",
  "authorization_url": "https://login.salesforce.com/services/oauth2/authorize?...",
  "state_token": "8ef1...",
  "state_expires_at": "2026-03-06T02:00:00Z"
}
```

`GET /api/v1/crm/oauth/salesforce/callback?...`:

```json
{
  "provider": "salesforce",
  "status": "connected",
  "crm_account_id": "00D...",
  "crm_object_id": "00D...",
  "connected": true,
  "updated_at": "2026-03-06T02:00:05Z"
}
```

## Quote Sync API (Quotey -> CRM)

Use `POST /api/v1/crm/sync/{quote_id}` to push the latest quote snapshot to connected CRM providers.

Query params:

- `provider` (optional): `salesforce` or `hubspot`
- `direction` (optional): must be `quotey_to_crm` when provided
- `event_type` (optional): override default lifecycle-derived event type

### Lifecycle Event Mapping

By default, outbound `event_type` and `sync_action` are derived from quote status:

- `draft` -> `quote_created` (`create_opportunity`)
- `finalized` -> `quote_finalized` (`update_amount_stage`)
- `approved` -> `quote_approved` (`post_approval`)
- `rejected` -> `quote_rejected` (`post_rejection`)
- `expired` -> `quote_expired` (`update_stage`)
- any other status -> `quote_updated` (`update_quote`)

### Conflict Resolution

Outbound retries and batch replays enforce **Quotey wins**:

- latest quote snapshot is reloaded from SQLite before replay,
- stored sync payload is refreshed before execution,
- payload includes:
  - `"conflict_resolution": "quotey_wins"`
  - `"source_of_truth": "quotey"`

## Batch Fallback (Hourly)

Use `POST /api/v1/crm/sync/batch` to replay retryable outbound events.

Query params:

- `provider` (optional): limit to one provider
- `limit` (optional): max events to replay (`1..200`, default `50`)

The batch endpoint replays outbound events with status in:

- `queued`
- `failed`
- `retrying`
- `skipped`

Recommended operation: trigger this endpoint from an hourly scheduler as fallback for missed/failed real-time sync pushes.

## CRM -> Quotey Inbound Sync

Real-time inbound updates are ingested via:

- `POST /api/v1/crm/webhook/{provider}`

Supported inbound update classes:

- account updates -> `quote.account_id`
- contact updates -> contact snapshot in `quote.notes`
- opportunity/deal stage updates -> quote context/status update

### Deduplication by CRM ID

Inbound webhook events are deduplicated by:

- `provider`
- `event_type`
- `crm_object_type`
- `crm_object_id`

within a 30-minute window. Duplicate events are recorded as `skipped` with:

- `"duplicate crm object event ignored"`

### Polling Fallback

Use `POST /api/v1/crm/sync/inbound/batch` to replay retryable inbound events (`crm_to_quotey`) when webhook delivery is degraded.

Query params:

- `provider` (optional): `salesforce` or `hubspot`
- `limit` (optional): max events to replay (`1..200`, default `50`)

## CRM Outbox Visibility + Replay

The CRM execution path writes deterministic queue state into:

- `execution_queue_task`
- `execution_idempotency_ledger`
- `execution_queue_transition_audit`

Use `GET /api/v1/crm/outbox/tasks` to inspect replay posture with correlated CRM event context.

Query params:

- `state` (optional): `queued`, `running`, `retryable_failed`, `failed_terminal`, `completed`, or `dead_letter`
- `provider` (optional): `salesforce` or `hubspot`
- `quote_id` (optional)
- `limit` (optional): `1..200` (default `50`)

Use `POST /api/v1/crm/outbox/tasks/{task_id}/replay` for deterministic replay of eligible queue tasks.

Replay invariants:

- Task must be in `queued`, `retryable_failed`, or `failed_terminal`.
- Task must have a correlated CRM event ID (`correlation_id` from idempotency ledger).
- Correlated CRM event must still be replayable (`queued|failed|retrying|skipped` and below retry cap).

The outbox replay endpoint delegates to the canonical event retry flow, so payload refresh and Quotey-wins conflict resolution remain unchanged.

### Stale Task Recovery

Use `POST /api/v1/crm/outbox/recover-stale` to recover tasks stuck in `running` past the configured claim timeout.

Query params:

- `limit` (optional): max stale tasks to recover (`1..200`, default `50`)

Recovery behavior:

- Selects stale `running` tasks for `operation_kind = crm.quote_sync`.
- Applies deterministic failure transition with retry policy (`running -> retryable_failed` or `running -> failed_terminal` when retry budget is exhausted).
- Persists transition audit + idempotency ledger updates.
- If a correlated CRM sync event exists, updates event status to match queue state (`retrying` or `failed`).
