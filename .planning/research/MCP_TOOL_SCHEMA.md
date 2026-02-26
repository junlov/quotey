# MCP Tool Schema for Quotey

**Task:** quotey-001-2: Define MCP tool schema for Quotey  
**Date:** 2026-02-26  
**Author:** Kimi (AI Agent)  
**Version:** 1.0.0

---

## Overview

This document defines the complete MCP tool schema for Quotey. These tools allow AI agents to programmatically interact with the CPQ system.

**Total Tools:** 10
- 2 Catalog tools
- 4 Quote tools  
- 3 Approval tools
- 1 PDF tool

---

## Common Types

### Error Response

All tools return errors in this format:

```json
{
  "error": true,
  "code": "NOT_FOUND",
  "message": "Product with ID 'prod_123' not found",
  "details": { ... }
}
```

**Error Codes:**
- `NOT_FOUND` - Resource doesn't exist
- `VALIDATION_ERROR` - Input validation failed
- `POLICY_VIOLATION` - Business rule violation
- `PERMISSION_DENIED` - Insufficient permissions
- `INTERNAL_ERROR` - Server error
- `CONFLICT` - Resource state conflict

### Pagination

List endpoints support pagination:

```json
{
  "items": [...],
  "pagination": {
    "total": 100,
    "page": 1,
    "per_page": 20,
    "has_more": true
  }
}
```

---

## 1. Catalog Tools

### 1.1 `catalog_search`

Search products by name, SKU, description, or category.

**Input:**

```json
{
  "query": "Pro Plan",
  "category": "saas",           // optional
  "active_only": true,          // default: true
  "limit": 20,                  // default: 20, max: 100
  "page": 1                     // default: 1
}
```

**Output:**

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
      "attributes": {
        "seats": { "type": "integer", "min": 1, "max": 1000 },
        "billing": { "type": "enum", "values": ["monthly", "annual"] }
      },
      "active": true,
      "base_price": 10.00,
      "currency": "USD"
    }
  ],
  "pagination": {
    "total": 5,
    "page": 1,
    "per_page": 20,
    "has_more": false
  }
}
```

---

### 1.2 `catalog_get`

Get detailed product information by ID.

**Input:**

```json
{
  "product_id": "prod_pro_v2",
  "include_relationships": true  // optional, default: false
}
```

**Output:**

```json
{
  "id": "prod_pro_v2",
  "sku": "PLAN-PRO-001",
  "name": "Pro Plan",
  "description": "Professional tier with advanced features",
  "product_type": "configurable",
  "category": "saas",
  "attributes": { ... },
  "active": true,
  "created_at": "2025-01-15T10:00:00Z",
  "updated_at": "2025-06-20T14:30:00Z",
  "relationships": {
    "requires": [
      { "product_id": "prod_support_basic", "name": "Basic Support" }
    ],
    "excludes": [
      { "product_id": "prod_starter", "name": "Starter Plan" }
    ],
    "recommends": [
      { "product_id": "prod_sso", "name": "SSO Add-on" }
    ]
  },
  "pricing": {
    "base_price": 10.00,
    "currency": "USD",
    "unit": "per_seat_per_month",
    "volume_tiers": [
      { "min": 1, "max": 49, "price": 10.00 },
      { "min": 50, "max": 99, "price": 8.00 },
      { "min": 100, "max": null, "price": 6.00 }
    ]
  }
}
```

---

## 2. Quote Tools

### 2.1 `quote_create`

Create a new quote.

**Input:**

```json
{
  "account_id": "acct_acme_001",
  "deal_id": "deal_123",              // optional
  "currency": "USD",                   // default: USD
  "term_months": 12,                   // optional
  "start_date": "2026-03-01",         // optional
  "billing_frequency": "monthly",      // default: monthly
  "payment_terms": "net_30",           // default: net_30
  "valid_until": "2026-04-01",        // optional, auto-calculated if not provided
  "notes": "Initial quote for Q1",    // optional
  "line_items": [
    {
      "product_id": "prod_pro_v2",
      "quantity": 150,
      "attributes": { "billing": "annual" },
      "discount_pct": 0,               // optional
      "notes": "Primary license"       // optional
    },
    {
      "product_id": "prod_sso",
      "quantity": 150,
      "attributes": {},
      "discount_pct": 0
    }
  ]
}
```

**Output:**

```json
{
  "quote": {
    "id": "Q-2026-0042",
    "version": 1,
    "account_id": "acct_acme_001",
    "deal_id": "deal_123",
    "status": "draft",
    "currency": "USD",
    "term_months": 12,
    "start_date": "2026-03-01",
    "end_date": "2027-02-28",
    "billing_frequency": "monthly",
    "payment_terms": "net_30",
    "valid_until": "2026-04-01",
    "notes": "Initial quote for Q1",
    "created_at": "2026-02-26T11:30:00Z",
    "created_by": "agent:kimi"
  },
  "line_items": [
    {
      "id": "ql_001",
      "quote_id": "Q-2026-0042",
      "product_id": "prod_pro_v2",
      "product_name": "Pro Plan",
      "quantity": 150,
      "unit_price": null,              // Set during pricing
      "discount_pct": 0,
      "discount_amount": 0,
      "subtotal": null,                // Set during pricing
      "attributes": { "billing": "annual" },
      "sort_order": 1
    },
    {
      "id": "ql_002",
      "quote_id": "Q-2026-0042",
      "product_id": "prod_sso",
      "product_name": "SSO Add-on",
      "quantity": 150,
      "unit_price": null,
      "discount_pct": 0,
      "discount_amount": 0,
      "subtotal": null,
      "attributes": {},
      "sort_order": 2
    }
  ],
  "validation": {
    "valid": false,                    // Not valid until priced
    "missing_fields": [],
    "constraint_violations": []
  }
}
```

**Errors:**
- `NOT_FOUND` - account_id or product_id doesn't exist
- `VALIDATION_ERROR` - invalid quantity, dates, etc.
- `CONFLICT` - inactive product specified

---

### 2.2 `quote_get`

Get quote details including line items and pricing (if available).

**Input:**

```json
{
  "quote_id": "Q-2026-0042",
  "include_pricing": true,        // default: true
  "include_audit": false          // default: false
}
```

**Output:**

```json
{
  "quote": {
    "id": "Q-2026-0042",
    "version": 1,
    "account_id": "acct_acme_001",
    "account_name": "Acme Corp",
    "deal_id": "deal_123",
    "status": "priced",
    "currency": "USD",
    "term_months": 12,
    "start_date": "2026-03-01",
    "end_date": "2027-02-28",
    "billing_frequency": "monthly",
    "payment_terms": "net_30",
    "valid_until": "2026-04-01",
    "notes": "Initial quote for Q1",
    "created_at": "2026-02-26T11:30:00Z",
    "created_by": "agent:kimi",
    "updated_at": "2026-02-26T11:35:00Z",
    "finalized_at": null,
    "sent_at": null
  },
  "line_items": [
    {
      "id": "ql_001",
      "product_id": "prod_pro_v2",
      "product_name": "Pro Plan",
      "quantity": 150,
      "unit_price": 6.00,
      "discount_pct": 0,
      "discount_amount": 0,
      "subtotal": 10800.00,
      "attributes": { "billing": "annual" },
      "sort_order": 1
    }
  ],
  "pricing": {
    "subtotal": 14400.00,
    "discount_total": 1440.00,
    "tax_total": 0.00,
    "total": 12960.00,
    "priced_at": "2026-02-26T11:35:00Z",
    "pricing_trace_id": "pt_12345"
  },
  "policy_evaluation": {
    "status": "APPROVAL_REQUIRED",
    "violations": [
      {
        "policy_id": "pol_discount_cap_smb",
        "policy_name": "SMB Discount Cap",
        "severity": "approval_required",
        "description": "15% discount exceeds 10% cap for SMB segment",
        "threshold": 10.0,
        "actual": 15.0
      }
    ]
  }
}
```

---

### 2.3 `quote_price`

Run pricing engine on a quote. This validates configuration and calculates totals.

**Input:**

```json
{
  "quote_id": "Q-2026-0042",
  "requested_discount_pct": 10,       // optional
  "price_book_id": "pb_enterprise_us" // optional, auto-selected if not provided
}
```

**Output:**

```json
{
  "quote_id": "Q-2026-0042",
  "version": 1,
  "status": "priced",
  "pricing": {
    "subtotal": 14400.00,
    "discount_total": 1440.00,
    "tax_total": 0.00,
    "total": 12960.00,
    "currency": "USD",
    "price_book_id": "pb_enterprise_us",
    "price_book_name": "Enterprise US 2026"
  },
  "line_pricing": [
    {
      "line_id": "ql_001",
      "product_id": "prod_pro_v2",
      "product_name": "Pro Plan",
      "quantity": 150,
      "base_unit_price": 10.00,
      "volume_tier_applied": { "min": 100, "price": 6.00 },
      "unit_price": 6.00,
      "subtotal_before_discount": 14400.00,
      "discount_pct": 10.0,
      "discount_amount": 1440.00,
      "line_total": 12960.00
    }
  ],
  "policy_evaluation": {
    "status": "APPROVAL_REQUIRED",
    "violations": [
      {
        "policy_id": "pol_discount_cap_smb",
        "severity": "approval_required",
        "description": "10% discount exceeds 0% auto-approval cap for Enterprise segment",
        "required_approver_role": "sales_manager"
      }
    ],
    "auto_approved": false
  },
  "approval_required": true,
  "next_actions": ["request_approval", "reduce_discount", "edit_quote"]
}
```

**Errors:**
- `NOT_FOUND` - quote_id doesn't exist
- `VALIDATION_ERROR` - constraint violations, missing required fields
- `POLICY_VIOLATION` - discount exceeds hard cap (cannot be approved)

---

### 2.4 `quote_list`

List quotes with optional filters.

**Input:**

```json
{
  "account_id": "acct_acme_001",      // optional
  "status": "priced",                  // optional: draft, priced, approval, approved, etc.
  "created_after": "2026-01-01",      // optional
  "created_before": "2026-03-01",     // optional
  "limit": 20,                         // default: 20, max: 100
  "page": 1,                           // default: 1
  "sort_by": "created_at",            // default: created_at
  "sort_order": "desc"                 // default: desc
}
```

**Output:**

```json
{
  "items": [
    {
      "id": "Q-2026-0042",
      "version": 1,
      "account_id": "acct_acme_001",
      "account_name": "Acme Corp",
      "deal_id": "deal_123",
      "status": "priced",
      "currency": "USD",
      "total": 12960.00,
      "valid_until": "2026-04-01",
      "created_at": "2026-02-26T11:30:00Z",
      "created_by": "agent:kimi"
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

---

## 3. Approval Tools

### 3.1 `approval_request`

Submit a quote for approval.

**Input:**

```json
{
  "quote_id": "Q-2026-0042",
  "justification": "Customer has been with us for 2 years with 100% on-time payment. Competitor is offering 20% discount.",
  "escalation": false              // Force escalation to next level
}
```

**Output:**

```json
{
  "approval_request": {
    "id": "APR-2026-0089",
    "quote_id": "Q-2026-0042",
    "status": "pending",
    "requested_by": "agent:kimi",
    "approver_role": "sales_manager",
    "approver_id": null,
    "justification": "Customer has been with us for 2 years...",
    "context": {
      "customer_tenure_months": 24,
      "payment_history": "100% on-time",
      "competitor_discount": "20%",
      "requested_discount": "10%",
      "annual_value": 12960.00
    },
    "created_at": "2026-02-26T11:40:00Z",
    "expires_at": "2026-02-26T15:40:00Z"
  },
  "notification_sent": true,
  "slack_message_ts": "1234567890.123456",
  "slack_channel": "#deal-desk"
}
```

**Errors:**
- `NOT_FOUND` - quote_id doesn't exist
- `VALIDATION_ERROR` - quote not in priced status
- `POLICY_VIOLATION` - discount exceeds hard maximum

---

### 3.2 `approval_status`

Check approval status for a quote.

**Input:**

```json
{
  "quote_id": "Q-2026-0042",
  "include_history": true           // Include past approval requests
}
```

**Output:**

```json
{
  "quote_id": "Q-2026-0042",
  "current_status": "pending_approval",
  "pending_requests": [
    {
      "id": "APR-2026-0089",
      "status": "pending",
      "approver_role": "sales_manager",
      "approver_name": null,
      "justification": "Customer has been with us for 2 years...",
      "created_at": "2026-02-26T11:40:00Z",
      "expires_at": "2026-02-26T15:40:00Z"
    }
  ],
  "history": [
    {
      "id": "APR-2026-0075",
      "status": "approved",
      "approver_role": "sales_manager",
      "approver_name": "John Smith",
      "decision_comment": "Good customer, approved",
      "decided_at": "2026-02-20T14:30:00Z"
    }
  ],
  "can_proceed": false              // Can quote be finalized?
}
```

---

### 3.3 `approval_pending`

List all pending approval requests (for approvers).

**Input:**

```json
{
  "approver_role": "sales_manager",  // optional - filter by role
  "quote_id": null,                  // optional - filter by quote
  "limit": 20,
  "page": 1
}
```

**Output:**

```json
{
  "items": [
    {
      "id": "APR-2026-0089",
      "quote_id": "Q-2026-0042",
      "quote_total": 12960.00,
      "account_name": "Acme Corp",
      "status": "pending",
      "requested_by": "agent:kimi",
      "approver_role": "sales_manager",
      "justification": "Customer has been with us for 2 years...",
      "context_summary": {
        "discount_pct": 10,
        "margin_pct": 62,
        "deal_size": 12960
      },
      "created_at": "2026-02-26T11:40:00Z",
      "expires_at": "2026-02-26T15:40:00Z"
    }
  ],
  "pagination": {
    "total": 5,
    "page": 1,
    "per_page": 20,
    "has_more": true
  }
}
```

---

## 4. PDF Tools

### 4.1 `quote_pdf`

Generate PDF for a quote.

**Input:**

```json
{
  "quote_id": "Q-2026-0042",
  "template": "standard",            // standard, executive, detailed, renewal
  "include_pricing_trace": false,   // Include detailed pricing breakdown
  "watermark": null                  // Optional: "DRAFT", "APPROVED"
}
```

**Output:**

```json
{
  "quote_id": "Q-2026-0042",
  "pdf_generated": true,
  "file_path": "/data/quotey/output/quotes/Acme_Corp_Q-2026-0042_v1.pdf",
  "file_size_bytes": 45678,
  "checksum": "sha256:abc123...",
  "template_used": "standard",
  "generated_at": "2026-02-26T11:45:00Z",
  "download_url": "file:///data/quotey/output/quotes/Acme_Corp_Q-2026-0042_v1.pdf"
}
```

**Errors:**
- `NOT_FOUND` - quote_id doesn't exist
- `VALIDATION_ERROR` - quote must be priced or approved
- `INTERNAL_ERROR` - PDF generation failed

---

## JSON Schema Definitions

### Complete Schema (for rmcp/schemars)

```rust
// catalog.rs
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CatalogSearchInput {
    #[schemars(description = "Search query for product name, SKU, or description")]
    pub query: String,
    
    #[schemars(description = "Filter by product category")]
    pub category: Option<String>,
    
    #[schemars(description = "Only return active products", default = "default_true")]
    #[serde(default = "default_true")]
    pub active_only: bool,
    
    #[schemars(description = "Maximum results to return", default = "default_20", maximum = 100)]
    #[serde(default = "default_20")]
    pub limit: u32,
    
    #[schemars(description = "Page number for pagination", default = "default_1")]
    #[serde(default = "default_1")]
    pub page: u32,
}

// quote.rs
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct QuoteCreateInput {
    #[schemars(description = "Account/Customer ID")]
    pub account_id: String,
    
    #[schemars(description = "Optional associated deal ID")]
    pub deal_id: Option<String>,
    
    #[schemars(description = "Currency code (ISO 4217)", default = "default_currency")]
    #[serde(default = "default_currency")]
    pub currency: String,
    
    #[schemars(description = "Contract term in months")]
    pub term_months: Option<u32>,
    
    #[schemars(description = "Contract start date (YYYY-MM-DD)")]
    pub start_date: Option<String>,
    
    #[schemars(description = "Billing frequency", default = "default_billing")]
    #[serde(default = "default_billing")]
    pub billing_frequency: String,
    
    #[schemars(description = "Payment terms", default = "default_payment_terms")]
    #[serde(default = "default_payment_terms")]
    pub payment_terms: String,
    
    #[schemars(description = "Quote expiration date (YYYY-MM-DD)")]
    pub valid_until: Option<String>,
    
    #[schemars(description = "Internal notes")]
    pub notes: Option<String>,
    
    #[schemars(description = "Line items for the quote")]
    pub line_items: Vec<LineItemInput>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LineItemInput {
    #[schemars(description = "Product ID")]
    pub product_id: String,
    
    #[schemars(description = "Quantity", minimum = 1)]
    pub quantity: u32,
    
    #[schemars(description = "Product-specific attributes")]
    pub attributes: Option<serde_json::Value>,
    
    #[schemars(description = "Requested discount percentage", default = "default_zero_f64")]
    #[serde(default)]
    pub discount_pct: f64,
    
    #[schemars(description = "Line item notes")]
    pub notes: Option<String>,
}

// approval.rs
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ApprovalRequestInput {
    #[schemars(description = "Quote ID to request approval for")]
    pub quote_id: String,
    
    #[schemars(description = "Business justification for the discount/exception")]
    pub justification: String,
    
    #[schemars(description = "Force escalation to next approval level", default = "default_false")]
    #[serde(default)]
    pub escalation: bool,
}

// pdf.rs
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct QuotePdfInput {
    #[schemars(description = "Quote ID")]
    pub quote_id: String,
    
    #[schemars(description = "PDF template to use", default = "default_template")]
    #[serde(default = "default_template")]
    pub template: String,
    
    #[schemars(description = "Include detailed pricing trace", default = "default_false")]
    #[serde(default)]
    pub include_pricing_trace: bool,
    
    #[schemars(description = "Optional watermark (DRAFT, APPROVED)")]
    pub watermark: Option<String>,
}

// Helper functions for defaults
fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_20() -> u32 { 20 }
fn default_1() -> u32 { 1 }
fn default_zero_f64() -> f64 { 0.0 }
fn default_currency() -> String { "USD".to_string() }
fn default_billing() -> String { "monthly".to_string() }
fn default_payment_terms() -> String { "net_30".to_string() }
fn default_template() -> String { "standard".to_string() }
```

---

## Implementation Checklist

- [ ] Define all input/output structs with schemars derive
- [ ] Implement `catalog_search` tool
- [ ] Implement `catalog_get` tool
- [ ] Implement `quote_create` tool
- [ ] Implement `quote_get` tool
- [ ] Implement `quote_price` tool
- [ ] Implement `quote_list` tool
- [ ] Implement `approval_request` tool
- [ ] Implement `approval_status` tool
- [ ] Implement `approval_pending` tool
- [ ] Implement `quote_pdf` tool
- [ ] Add comprehensive error handling
- [ ] Add audit logging for all operations
- [ ] Write integration tests
- [ ] Document all tools for agent users
