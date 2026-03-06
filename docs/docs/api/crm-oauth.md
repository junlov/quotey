# CRM OAuth Flow

Quotey exposes deterministic OAuth endpoints for CRM providers:

- `GET /api/v1/crm/connect/{provider}`
- `GET /api/v1/crm/oauth/{provider}/callback`

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
