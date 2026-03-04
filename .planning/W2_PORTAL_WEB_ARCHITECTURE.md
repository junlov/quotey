# W2 Portal Web Architecture

## Scope
This document defines the web portal architecture for `quotey-003-1` and maps the design to the current implementation in:
- `crates/server/src/health.rs`
- `crates/server/src/portal.rs`
- `templates/portal/quote_viewer.html`
- `templates/portal/index.html`
- `migrations/0023_portal_link.up.sql`
- `migrations/0024_portal_comment.up.sql`

The portal is a v2 customer-facing extension layered onto Quotey's Slack-first runtime.

## Architecture Decision
Decision: embedded Axum portal routes inside the existing server process.

Rationale:
1. Single-binary deployment remains intact.
2. Portal and Slack paths share the same SQLite state and audit trail.
3. No separate service discovery, auth domain, or deployment topology is required.

Concrete wiring:
1. `crates/server/src/health.rs` builds the HTTP router and merges `portal::router(...)`.
2. `crates/server/src/main.rs` starts the health/API listener via `health::spawn(...)`.
3. `crates/server/src/portal.rs` owns HTML + JSON portal endpoints.

## Route Surface
HTML endpoints:
1. `GET /portal` -> quote index page.
2. `GET /quote/{token}` -> quote viewer.
3. `GET /quote/{token}/download` -> quote PDF (or fallback output from PDF generator).

JSON endpoints:
1. `POST /quote/{token}/approve`
2. `POST /quote/{token}/reject`
3. `POST /quote/{token}/comment`
4. `GET /quote/{token}/comments`
5. `POST /quote/{token}/line/{line_id}/comment`
6. `POST /quote/{token}/assumptions`
7. `POST /api/v1/portal/links`
8. `POST /api/v1/portal/links/revoke`
9. `GET /api/v1/portal/links/{quote_id}`

## Data Model and Persistence
Primary tables:
1. `portal_link` (share token lifecycle, expiry, revoke flag).
2. `portal_comment` (quote-level and line-level customer comments, threaded via `parent_id`).
3. `approval_request` (customer approve/reject events persisted as approval records).
4. `quote_pricing_snapshot` (authoritative totals, including assumption-update recalculations).
5. `audit_event` (all portal actions and token-denied attempts).

Token lifecycle:
1. Link creation mints a UUID-based token and expiry window (default 30 days, clamp 1..365).
2. New link creation auto-revokes previous active links for the same quote.
3. Revocation flips `revoked=1`.
4. Resolution allows only active, non-expired, non-revoked tokens.

## Security Model
Access model:
1. Customer access is token-based; no customer login required.
2. `resolve_quote_by_token(...)` enforces strict token-only access.
3. Raw quote IDs are rejected (no quote-id fallback path).
4. Revoked/expired/unknown token attempts are audit-logged with `portal.token_denied`.

Current hardening in code:
1. UUIDv4 token generation for non-guessable links.
2. Expiration checks on every token resolution.
3. Revocation checks on every token resolution.
4. Input validation for approve/reject/comment/assumption payloads.
5. Token redaction in logs (`redact_token`).

Gaps intentionally called out for follow-up implementation:
1. No CSRF token mechanism on POST endpoints yet.
2. No explicit rate-limiter middleware bound to portal routes yet.
3. Optional password protection for sensitive links is not yet implemented.

## Deterministic Boundary and Safety
Portal behavior preserves Quotey's deterministic CPQ principle:
1. Portal inputs capture human/customer intent only.
2. Pricing and totals are read from or written to deterministic quote/pricing snapshot state.
3. Approval/rejection/comment actions persist as auditable domain events.
4. No LLM is required in portal request handlers.

## Request Flow (Reference)
Approve flow:
1. Resolve token -> quote id.
2. Validate approver name/email.
3. Insert `approval_request` record with approved status.
4. Transition quote status to `approved`.
5. Write `audit_event` `portal.approval`.

Assumption update flow:
1. Resolve token -> quote id.
2. Validate tax rate + payment terms.
3. Persist explicit assumption values to `quote`.
4. Recompute totals from quote lines.
5. Replace current-version pricing snapshot in `quote_pricing_snapshot`.
6. Write `audit_event` `portal.assumptions_updated`.

Link-management flow:
1. Verify quote exists.
2. Create new tokenized `portal_link` row with expiry.
3. Revoke older active links for same quote.
4. Return `link_id`, token, expiry.

## Templates and Rendering
Rendering model:
1. Tera loads filesystem templates under `templates/portal/**/*`.
2. Embedded fallback templates are registered to avoid hard-failure when files are absent.
3. `quote_viewer.html` receives quote/customer/rep/comments/branding context.
4. `index.html` receives stats, filtered quote list, and branding context.

The viewer uses pricing snapshot totals as the authoritative source when available, with line-item computation as fallback.

## Test Coverage Snapshot
`crates/server/src/portal.rs` includes tests for:
1. approve/reject/comment request validation and persistence.
2. link creation/revocation/listing behavior.
3. token resolution for active/revoked/expired/unknown tokens.
4. audit-event coverage for token denial events.
5. line-item comment threading and parent validation.
6. PDF subtotal regression for pre-discount line subtotals.

## Acceptance Mapping for quotey-003-1
1. Architecture approach is explicit: embedded Axum route set in existing server runtime.
2. URL scheme is defined and matches current endpoints.
3. Security model is documented (token, expiry, revocation, strict token resolution).
4. Persistence/audit topology is documented across portal tables and audit events.
5. Known hardening gaps are explicitly listed for follow-on beads.

## PWA Architecture Addendum (quotey-008-1)

This addendum defines the mobile approval PWA architecture aligned to the existing portal/Axum surface.

### URL and Asset Surface
1. `GET /approvals` -> mobile-optimized pending approval list (PWA shell entrypoint).
2. `GET /approvals/{id}` -> approval detail view for one decision packet.
3. `GET /settings` -> notification + cache/reset preferences.
4. `GET /manifest.webmanifest` -> install metadata.
5. `GET /sw.js` -> service worker script.

### Runtime Integration Strategy
1. Keep one server process: PWA routes are merged into existing `portal::router(...)`.
2. Reuse existing token model from `portal_link` for customer-manager link access.
3. Reuse existing approval mutation endpoints (`/quote/{token}/approve|reject`) to avoid parallel write paths.
4. PWA read models consume the same deterministic quote + pricing snapshot state used by the portal.

### Offline and Caching Model
1. Service worker cache-first for static shell assets (HTML/CSS/JS/icons).
2. Network-first with fallback cache for read endpoints (`/approvals*`) to avoid stale approvals.
3. Mutation endpoints are online-only; when offline, UI must block submit and show deterministic recovery guidance.
4. Cache keys include quote/approval IDs and short TTL to reduce stale decision risk.

### Notification Model
1. Web Push subscription bound to approver identity + routeable token scope.
2. Notification payload includes quote id, customer, amount, discount, and deep link to `/approvals/{id}`.
3. Push receipt + open actions are audit-logged in `audit_event` (`portal.pwa.push_sent`, `portal.pwa.push_opened`).

### Security and Session Constraints
1. Link-based access remains default, but PWA session token must be scoped and expiring.
2. No local persistence of raw approval tokens; store opaque short-lived session ids only.
3. Service worker must never cache approval POST responses or sensitive token-bearing payloads.
4. CSRF + route-level rate limiting are mandatory before production PWA rollout.

### Deterministic Safety Boundary
1. PWA can collect intent and display state; deterministic engines still decide approvals, policy, and pricing.
2. All approve/reject/need-info actions emit audit events with actor, timestamp, and request id.
3. Any offline attempt to approve/reject must be explicitly rejected client-side and retried online.

### Acceptance Mapping for quotey-008-1
1. PWA architecture and route structure are explicit.
2. Integration with existing Axum/portal runtime is explicit.
3. Offline/service worker constraints are explicit.
4. Notification, auth/session, and audit requirements are explicit.
