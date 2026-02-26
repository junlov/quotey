# MCP Tool Schema for Quotey (v1.1.0)

**Task:** quotey-001-2: Define MCP tool schema for Quotey  
**Date:** 2026-02-26  
**Author:** Kimi (AI Agent)  
**Status:** Aligned to implementation in `crates/mcp/src/server.rs`

---

## Why this doc exists

Quotey MCP tools are the contract between AI agents and Quotey’s CPQ functions. This document defines that contract and codifies behavior we want to keep stable:

- deterministic names/types across all clients
- explicit validation rules
- predictable error model
- minimal assumptions about tool-call transport details

---

## Transport and Error Contract

All tool handlers are implemented via `rmcp` stdio transport in `crates/mcp/src/main.rs`.

All user-facing tool failures currently return JSON payloads shaped as:

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "quote_id is required",
    "details": null
  }
}
```

### Error Codes in Scope

- `VALIDATION_ERROR`: bad input shape or value
- `NOT_FOUND`: resource missing
- `CONFLICT`: state/capacity conflict (for example duplicate pending approval)
- `CURRENCY_MISMATCH`: quote line currency mismatch
- `INTERNAL_ERROR`: unexpected internal failure
- `INTERNAL_ERROR`: internal failures
- `POLICY`-related validation failures are represented via `policy_violations` in `quote_price` output for now.

The shared helper is `tool_error()` in `crates/mcp/src/server.rs`.

---

## Canonical Tool List

The MCP server currently exposes 10 tools:

- `catalog_search`
- `catalog_get`
- `quote_create`
- `quote_get`
- `quote_price`
- `quote_list`
- `approval_request`
- `approval_status`
- `approval_pending`
- `quote_pdf`

All names are stable as of this revision and should be treated as API-compatible without breaking consumers.

---

## Shared Conventions

- All identifier arguments are trimmed; blank values are rejected.
- Optional identifiers are validated as non-empty strings after trim.
- Monetary fields are represented as JSON numbers.
- Dates are ISO-8601 strings where returned.
- Pagination uses `page` + `limit` (1-indexed page, default 1).
- `limit` is capped by server at `MAX_PAGE_LIMIT = 100`.

### Pagination object

Most list-like outputs use:

```json
{
  "total": 12,
  "page": 1,
  "per_page": 20,
  "has_more": false
}
```

`total` currently reflects the returned page size, not necessarily total table rows.

---

## 1. Catalog Tools

### 1.1 `catalog_search`

Search products by text and/or category.

#### Input

```json
{
  "query": "Pro Plan",
  "category": "saas",
  "active_only": true,
  "limit": 20,
  "page": 1
}
```

- At least one of `query` or `category` must be non-empty after trim.
- `active_only` defaults to `true`.
- `limit` defaults to `20`, max `100`.
- `page` defaults to `1`.

#### Output

```json
{
  "items": [
    {
      "id": "prod_pro_v2",
      "sku": "PLAN-PRO-001",
      "name": "Pro Plan",
      "description": "Professional tier with advanced features",
      "product_type": "configurable",
      "category": "saas",
      "active": true
    }
  ],
  "pagination": {
    "total": 1,
    "page": 1,
    "per_page": 20,
    "has_more": false
  }
}
```

#### Notes

- `include_relationships` is not implemented for this endpoint and is intentionally ignored when provided.
- Category filtering is currently applied post-search to keep implementation simple.

---

### 1.2 `catalog_get`

Load a specific product.

#### Input

```json
{
  "product_id": "prod_pro_v2",
  "include_relationships": false
}
```

- `product_id` required and trimmed.

#### Output

```json
{
  "id": "prod_pro_v2",
  "sku": "PLAN-PRO-001",
  "name": "Pro Plan",
  "description": "Professional tier with advanced features",
  "product_type": "configurable",
  "category": "saas",
  "attributes": {},
  "active": true,
  "created_at": "2026-02-26T11:30:00Z",
  "updated_at": "2026-06-20T14:30:00Z"
}
```

#### Notes

- `include_relationships` is defined in schema compatibility, but currently not populated in output.

---

## 2. Quote Tools

### 2.1 `quote_create`

Create a new draft quote and persist it in the DB.

#### Input

```json
{
  "account_id": "acct_acme_001",
  "deal_id": "deal_123",
  "currency": "USD",
  "term_months": 12,
  "start_date": "2026-03-01",
  "notes": "Initial quote for Q1",
  "line_items": [
    {
      "product_id": "prod_pro_v2",
      "quantity": 150,
      "discount_pct": 0,
      "attributes": { "billing": "annual" },
      "notes": "Primary license"
    }
  ],
  "idempotency_key": "client-key-abc"
}
```

- `account_id` required.
- `currency` defaults to `USD` and is normalized to upper-case ASCII.
- At least one line item required.
- `term_months` optional; when provided it must be `> 0`.
- `product_id` and `line_items[n].quantity` must be valid.
- `discount_pct` for each line must be in `[0, 100]`.

#### Output

```json
{
  "quote_id": "Q-0000000000000000",
  "version": 1,
  "status": "draft",
  "account_id": "acct_acme_001",
  "currency": "USD",
  "idempotency_key": "client-key-abc",
  "line_items": [
    {
      "line_id": "Q-...-ql-1",
      "product_id": "prod_pro_v2",
      "product_name": "Pro Plan",
      "quantity": 150,
      "unit_price": 10,
      "discount_pct": 0,
      "subtotal": 1500
    }
  ],
  "created_at": "2026-02-26T11:30:00Z",
  "message": "Quote created successfully"
}
```

#### Error behavior

- `NOT_FOUND` if any line product missing.
- `CONFLICT` if any product is inactive.
- `CURRENCY_MISMATCH` if a line product currency does not match quote currency.

---

### 2.2 `quote_get`

Fetch full quote state.

#### Input

```json
{
  "quote_id": "Q-...",
  "include_pricing": true
}
```

- `quote_id` required.

#### Output

```json
{
  "quote": {
    "id": "Q-...",
    "version": 1,
    "account_id": "acct_acme_001",
    "account_name": null,
    "deal_id": "deal_123",
    "status": "draft",
    "currency": "USD",
    "term_months": 12,
    "start_date": "2026-03-01",
    "end_date": null,
    "valid_until": null,
    "notes": "Initial quote",
    "created_at": "2026-02-26T11:30:00Z",
    "created_by": "agent:mcp"
  },
  "line_items": [
    {
      "line_id": "Q-...-ql-1",
      "product_id": "prod_pro_v2",
      "product_name": "Pro Plan",
      "quantity": 150,
      "unit_price": 10,
      "discount_pct": 0,
      "discount_amount": null,
      "subtotal": 1500
    }
  ],
  "pricing": {
    "subtotal": 1500,
    "discount_total": 0,
    "tax_total": 0,
    "total": 1500,
    "priced_at": null
  }
}
```

`pricing` is omitted when `include_pricing=false`.

---

### 2.3 `quote_price`

Run deterministic pricing/policy evaluation.

#### Input

```json
{
  "quote_id": "Q-...",
  "requested_discount_pct": 10
}
```

- `requested_discount_pct` must be finite and within `[0, 100]`.

#### Output

```json
{
  "quote_id": "Q-...",
  "version": 1,
  "status": "draft",
  "pricing": {
    "subtotal": 1500,
    "discount_total": 150,
    "tax_total": 0,
    "total": 1350,
    "priced_at": "2026-02-26T11:35:00Z"
  },
  "line_pricing": [
    {
      "line_id": "Q-...-ql-1",
      "product_id": "prod_pro_v2",
      "product_name": "Pro Plan",
      "quantity": 150,
      "base_unit_price": 10,
      "unit_price": 9,
      "subtotal_before_discount": 1500,
      "discount_pct": 10,
      "discount_amount": 150,
      "line_total": 1350
    }
  ],
  "approval_required": true,
  "policy_violations": [
    {
      "policy_id": "discount_cap_sales_manager",
      "policy_name": "discount cap sales manager",
      "severity": "approval_required",
      "description": "Requested discount exceeds configured cap",
      "threshold": null,
      "actual": 10,
      "required_approver_role": "sales_manager"
    }
  ]
}
```

---

### 2.4 `quote_list`

Search quotes with filters.

#### Input

```json
{
  "account_id": "acct_acme_001",
  "status": "draft",
  "limit": 20,
  "page": 1
}
```

#### Output

```json
{
  "items": [
    {
      "id": "Q-...",
      "version": 1,
      "account_id": "acct_acme_001",
      "account_name": null,
      "status": "draft",
      "currency": "USD",
      "total": 1500,
      "valid_until": null,
      "created_at": "2026-02-26T11:30:00Z"
    }
  ],
  "pagination": {
    "total": 1,
    "page": 1,
    "per_page": 20,
    "has_more": false
  }
}
```

`account_name` is not currently enriched and is returned as `null`.

---

## 3. Approval Tools

### 3.1 `approval_request`

Create a new approval request for a quote.

#### Input

```json
{
  "quote_id": "Q-...",
  "justification": "Special pricing requested due to competitive displacement.",
  "approver_role": "sales_manager"
}
```

- `approver_role` defaults to `sales_manager` when omitted.
- `justification` is required and capped at 2000 chars.
- Requests are rejected if quote is already in terminal states (`approved`, `sent`, `expired`, `cancelled`).
- Duplicate pending approval for same `quote_id + approver_role` is rejected.

#### Output

```json
{
  "approval_id": "APR-...",
  "quote_id": "Q-...",
  "status": "pending",
  "approver_role": "sales_manager",
  "requested_by": "agent:mcp",
  "justification": "Special pricing requested due to competitive displacement.",
  "created_at": "2026-02-26T11:40:00Z",
  "expires_at": "2026-02-26T15:40:00Z",
  "message": "Approval request submitted and persisted"
}
```

---

### 3.2 `approval_status`

Retrieve current approval state for a quote.

#### Input

```json
{
  "quote_id": "Q-..."
}
```

#### Output

```json
{
  "quote_id": "Q-...",
  "current_status": "pending_approval",
  "pending_requests": [
    {
      "approval_id": "APR-...",
      "status": "pending",
      "approver_role": "sales_manager",
      "requested_at": "2026-02-26T11:40:00Z",
      "expires_at": "2026-02-26T15:40:00Z"
    }
  ],
  "can_proceed": false
}
```

`current_status` is derived from approval history: rejected > pending > approved > none.

---

### 3.3 `approval_pending`

List pending approvals.

#### Input

```json
{
  "approver_role": "sales_manager",
  "limit": 20
}
```

- `approver_role` optional.

#### Output

```json
{
  "items": [
    {
      "approval_id": "APR-...",
      "quote_id": "Q-...",
      "account_name": "Acme Corp",
      "quote_total": 1500,
      "requested_by": "agent:mcp",
      "justification": "Special pricing requested due to competitive displacement.",
      "requested_at": "2026-02-26T11:40:00Z"
    }
  ],
  "total": 1
}
```

---

## 4. PDF Tool

### 4.1 `quote_pdf`

Generate a quote artifact (mock PDF placeholder for now).

#### Input

```json
{
  "quote_id": "Q-...",
  "template": "standard"
}
```

- `template` defaults to `standard`.
- Allowed values: `standard`, `compact`, `detailed`.

#### Output

```json
{
  "quote_id": "Q-...",
  "pdf_generated": true,
  "file_path": "/tmp/quotey-mcp/pdf/quote-Q-....pdf",
  "file_size_bytes": 3120,
  "checksum": "mock-checksum:4f3a...",
  "template_used": "standard",
  "generated_at": "2026-02-26T11:45:00Z"
}
```

#### Validation in scope

- quote must exist
- template must be in allowlist

---

## Implementation Checklist (Current)

- [x] Tool discovery and list wiring through `rmcp`
- [x] catalog tools with validation and pagination defaults
- [x] quote creation/get/price/list
- [x] approval request/status/pending
- [x] quote PDF artifact generation
- [x] auth and rate limiting entry points on transport
- [x] consistent user-facing error wrapper
- [ ] account/counterparty enrichment (`account_name`, customer lookups)
- [ ] persist quote price result / quote status transitions
- [ ] actual PDF rendering (template engine)
- [ ] approvals actions (`approve` / `reject`) as dedicated MCP tools
- [ ] audit event records for every MCP tool invocation

---

## Practical Client Contract Notes

### Recommended call ordering
1. Use `catalog_search` + `catalog_get` to validate and inspect lines.
2. Build a draft via `quote_create`.
3. Run `quote_get` and `quote_price` to validate economics.
4. If approval needed, call `approval_request`.
5. Poll `approval_status` and/or `approval_pending`.
6. Generate client artifact via `quote_pdf` once stable.

### Extensibility guidance

- New error codes should be added to the same wrapper shape.
- Keep response fields additive to avoid breaking existing callers.
- For any new tool with asynchronous side effects, include a deterministic `request_id` and idempotency key semantics early.
