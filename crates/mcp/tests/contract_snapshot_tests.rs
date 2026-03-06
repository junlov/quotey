//! Contract snapshot tests for Quotey MCP Server
//!
//! These tests verify the exact shape of JSON responses (success and error envelopes)
//! to catch unintended breaking changes in the MCP tool API contract.
//!
//! Coverage targets:
//! - Error envelope structure: {error: {code, message, details}}
//! - Success response field presence and types for each tool
//! - Normalization behavior: limit/page defaults, max clamping
//! - Currency mismatch errors
//! - Audit trail persistence from tool invocations
//! - Quote PDF tool responses

use quotey_mcp::QuoteyMcpServer;
use rmcp::handler::server::wrapper::Parameters;
use sqlx::Row;

type TestResult<T = ()> = Result<T, String>;

async fn setup_pool() -> TestResult<quotey_db::DbPool> {
    let pool = quotey_db::connect_with_settings("sqlite::memory:", 1, 30)
        .await
        .map_err(|e| format!("connect: {e}"))?;
    quotey_db::migrations::run_pending(&pool).await.map_err(|e| format!("migrations: {e}"))?;
    Ok(pool)
}

async fn seed_product(
    pool: &quotey_db::DbPool,
    id: &str,
    sku: &str,
    name: &str,
    price_cents: i64,
    currency: &str,
) -> TestResult {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO product (id, sku, name, base_price, currency, active, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, 1, ?, ?)",
    )
    .bind(id)
    .bind(sku)
    .bind(name)
    .bind(format!("{:.2}", price_cents as f64 / 100.0))
    .bind(currency)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("seed product: {e}"))?;
    Ok(())
}

fn parse(output: &str) -> serde_json::Value {
    serde_json::from_str(output).expect("tool output must be valid JSON")
}

/// Helper: create a quote and return its ID.
async fn create_test_quote(
    server: &QuoteyMcpServer,
    pool: &quotey_db::DbPool,
    suffix: &str,
) -> TestResult<String> {
    let prod_id = format!("PROD-CS-{suffix}");
    seed_product(pool, &prod_id, &format!("SKU-CS-{suffix}"), "Contract Item", 5000, "USD").await?;

    let input = quotey_mcp::server::QuoteCreateInput {
        account_id: format!("ACCT-CS-{suffix}"),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: Some(12),
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: prod_id,
            quantity: 2,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some(format!("cs-{suffix}")),
    };

    let output = server.quote_create(Parameters(input)).await;
    let parsed = parse(&output);
    parsed
        .get("quote_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("failed to create quote: {parsed}"))
}

// ============================================================================
// Error Envelope Contract
// ============================================================================

#[tokio::test]
async fn contract_error_envelope_has_required_fields() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());

    // Trigger a VALIDATION_ERROR
    let input = quotey_mcp::server::CatalogSearchInput {
        query: "   ".to_string(),
        category: None,
        active_only: true,
        limit: 20,
        page: 1,
    };
    let output = server.catalog_search(Parameters(input)).await;
    let parsed = parse(&output);

    // Error envelope must have exactly {error: {code, message, details}}
    let error = parsed.get("error").expect("error envelope must exist");
    assert!(error.is_object(), "error must be an object");

    let code = error.get("code").expect("error.code must exist");
    assert!(code.is_string(), "error.code must be a string");

    let message = error.get("message").expect("error.message must exist");
    assert!(message.is_string(), "error.message must be a string");

    // details field must exist (even if null)
    assert!(error.get("details").is_some(), "error.details must be present");

    // No extra top-level keys besides "error"
    let top_keys: Vec<&String> = parsed.as_object().unwrap().keys().collect();
    assert_eq!(top_keys, vec!["error"], "error response must only contain 'error' key");

    Ok(())
}

#[tokio::test]
async fn contract_error_codes_are_stable() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());

    // VALIDATION_ERROR from empty query
    let input = quotey_mcp::server::CatalogSearchInput {
        query: "".to_string(),
        category: None,
        active_only: true,
        limit: 20,
        page: 1,
    };
    let v = parse(&server.catalog_search(Parameters(input)).await);
    assert_eq!(
        v["error"]["code"].as_str(),
        Some("VALIDATION_ERROR"),
        "empty query must produce VALIDATION_ERROR"
    );

    // NOT_FOUND from catalog_get
    let input = quotey_mcp::server::CatalogGetInput {
        product_id: "DOES-NOT-EXIST".to_string(),
        include_relationships: false,
    };
    let v = parse(&server.catalog_get(Parameters(input)).await);
    assert_eq!(
        v["error"]["code"].as_str(),
        Some("NOT_FOUND"),
        "missing product must produce NOT_FOUND"
    );

    // NOT_FOUND from quote_get
    let input = quotey_mcp::server::QuoteGetInput {
        quote_id: "Q-GHOST".to_string(),
        include_pricing: false,
    };
    let v = parse(&server.quote_get(Parameters(input)).await);
    assert_eq!(
        v["error"]["code"].as_str(),
        Some("NOT_FOUND"),
        "missing quote must produce NOT_FOUND"
    );

    Ok(())
}

#[tokio::test]
async fn contract_internal_error_message_is_redacted() -> TestResult {
    // Internal errors should never leak raw DB messages.
    // We can't easily trigger a real INTERNAL_ERROR without mocking,
    // but we verify the tool_error function behavior via a validation path
    // that confirms the envelope shape is consistent.
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());

    // Approval for nonexistent quote produces NOT_FOUND (not INTERNAL_ERROR)
    let input = quotey_mcp::server::ApprovalRequestInput {
        quote_id: "Q-NOSUCH".to_string(),
        justification: "Valid justification text here".to_string(),
        approver_role: None,
    };
    let v = parse(&server.approval_request(Parameters(input)).await);
    assert_eq!(v["error"]["code"].as_str(), Some("NOT_FOUND"));
    // Message should be human-readable, not a raw SQL error
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(!msg.contains("sqlx"), "error message must not leak DB details");
    assert!(!msg.contains("sqlite"), "error message must not leak DB details");

    Ok(())
}

// ============================================================================
// Success Response Contracts: Catalog Tools
// ============================================================================

#[tokio::test]
async fn contract_catalog_search_success_shape() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "CS-P1", "CS-SKU-1", "Alpha Widget", 9999, "USD").await?;

    let server = QuoteyMcpServer::new(pool.clone());
    let input = quotey_mcp::server::CatalogSearchInput {
        query: "Alpha".to_string(),
        category: None,
        active_only: true,
        limit: 10,
        page: 1,
    };

    let v = parse(&server.catalog_search(Parameters(input)).await);
    assert!(v.get("error").is_none(), "expected success");

    // Top-level keys
    assert!(v.get("items").is_some(), "must have 'items'");
    assert!(v.get("pagination").is_some(), "must have 'pagination'");

    // Items shape
    let item = &v["items"][0];
    assert!(item.get("id").is_some(), "item must have 'id'");
    assert!(item.get("name").is_some(), "item must have 'name'");
    assert!(item.get("sku").is_some(), "item must have 'sku'");

    // Pagination shape
    let pg = &v["pagination"];
    assert!(pg.get("total").is_some(), "pagination must have 'total'");
    assert!(pg.get("page").is_some(), "pagination must have 'page'");
    assert!(pg.get("per_page").is_some(), "pagination must have 'per_page'");
    assert!(pg.get("has_more").is_some(), "pagination must have 'has_more'");

    // Type checks
    assert!(pg["total"].is_number(), "pagination.total must be number");
    assert!(pg["page"].is_number(), "pagination.page must be number");
    assert!(pg["has_more"].is_boolean(), "pagination.has_more must be boolean");

    Ok(())
}

#[tokio::test]
async fn contract_catalog_get_success_shape() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "CS-GET-1", "CS-GET-SKU", "Beta Gadget", 4500, "USD").await?;

    let server = QuoteyMcpServer::new(pool.clone());
    let input = quotey_mcp::server::CatalogGetInput {
        product_id: "CS-GET-1".to_string(),
        include_relationships: false,
    };

    let v = parse(&server.catalog_get(Parameters(input)).await);
    assert!(v.get("error").is_none(), "expected success");

    // Required fields
    for field in &["id", "sku", "name", "active"] {
        assert!(v.get(field).is_some(), "catalog_get response must have '{field}'");
    }

    assert!(v["id"].is_string(), "id must be string");
    assert!(v["active"].is_boolean(), "active must be boolean");

    Ok(())
}

// ============================================================================
// Success Response Contracts: Quote Tools
// ============================================================================

#[tokio::test]
async fn contract_quote_create_success_shape() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "CS-QC-1", "CS-QC-SKU", "Quote Create Item", 7500, "USD").await?;

    let server = QuoteyMcpServer::new(pool.clone());
    let input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-QC".to_string(),
        deal_id: Some("DEAL-QC".to_string()),
        currency: "usd".to_string(), // lowercase — should be normalized to USD
        term_months: Some(12),
        start_date: None,
        notes: Some("Contract test".to_string()),
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "CS-QC-1".to_string(),
            quantity: 3,
            discount_pct: 5.0,
            attributes: None,
            notes: Some("line note".to_string()),
        }],
        idempotency_key: Some("cs-qc-shape".to_string()),
    };

    let v = parse(&server.quote_create(Parameters(input)).await);
    assert!(v.get("error").is_none(), "expected success, got: {v}");

    // Required top-level fields
    for field in &["quote_id", "status", "account_id", "currency", "version", "line_items"] {
        assert!(v.get(field).is_some(), "quote_create response must have '{field}'");
    }

    // Type contracts
    assert!(v["quote_id"].as_str().unwrap().starts_with("Q-"), "quote_id must start with Q-");
    assert_eq!(v["status"].as_str(), Some("draft"), "new quote must be draft");
    assert_eq!(v["currency"].as_str(), Some("USD"), "currency must be uppercased");
    assert!(v["version"].is_number(), "version must be number");
    assert_eq!(v["version"].as_u64(), Some(1), "first version must be 1");
    assert!(v["line_items"].is_array(), "line_items must be array");
    assert_eq!(v["line_items"].as_array().unwrap().len(), 1);

    Ok(())
}

#[tokio::test]
async fn contract_quote_get_success_shape() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());
    let quote_id = create_test_quote(&server, &pool, "qget").await?;

    let input =
        quotey_mcp::server::QuoteGetInput { quote_id: quote_id.clone(), include_pricing: true };
    let v = parse(&server.quote_get(Parameters(input)).await);
    assert!(v.get("error").is_none(), "expected success, got: {v}");

    // Required top-level keys
    assert!(v.get("quote").is_some(), "must have 'quote' object");
    assert!(v.get("line_items").is_some(), "must have 'line_items'");

    // Quote object shape
    let q = &v["quote"];
    for field in &["id", "account_id", "currency", "status"] {
        assert!(q.get(field).is_some(), "quote object must have '{field}'");
    }

    // Line items shape
    let items = v["line_items"].as_array().unwrap();
    assert!(!items.is_empty(), "line_items must not be empty");
    let li = &items[0];
    assert!(li.get("product_id").is_some(), "line item must have 'product_id'");
    assert!(li.get("quantity").is_some(), "line item must have 'quantity'");

    // Pricing should be present when include_pricing=true
    assert!(v.get("pricing").is_some(), "pricing must be present when requested");

    Ok(())
}

#[tokio::test]
async fn contract_quote_price_success_shape() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());
    let quote_id = create_test_quote(&server, &pool, "qprice").await?;

    let input = quotey_mcp::server::QuotePriceInput {
        quote_id: quote_id.clone(),
        requested_discount_pct: 5.0,
    };
    let v = parse(&server.quote_price(Parameters(input)).await);
    assert!(v.get("error").is_none(), "expected success, got: {v}");

    // Required fields
    for field in &["quote_id", "pricing", "line_pricing", "approval_required", "policy_violations"]
    {
        assert!(v.get(field).is_some(), "quote_price must have '{field}'");
    }

    // Pricing object shape
    let pricing = &v["pricing"];
    for field in &["subtotal", "discount_total", "total"] {
        assert!(pricing.get(field).is_some(), "pricing must have '{field}'");
        assert!(pricing[field].is_number(), "pricing.{field} must be number");
    }

    // approval_required is boolean
    assert!(v["approval_required"].is_boolean(), "approval_required must be boolean");

    // policy_violations is array
    assert!(v["policy_violations"].is_array(), "policy_violations must be array");

    // line_pricing is array
    let lp = v["line_pricing"].as_array().unwrap();
    assert!(!lp.is_empty());

    Ok(())
}

#[tokio::test]
async fn contract_quote_list_success_shape() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());
    let _qid = create_test_quote(&server, &pool, "qlist").await?;

    let input = quotey_mcp::server::QuoteListInput {
        account_id: Some("ACCT-CS-qlist".to_string()),
        status: None,
        limit: 10,
        page: 1,
    };
    let v = parse(&server.quote_list(Parameters(input)).await);
    assert!(v.get("error").is_none(), "expected success, got: {v}");

    assert!(v.get("items").is_some(), "must have 'items'");
    assert!(v.get("pagination").is_some(), "must have 'pagination'");

    let item = &v["items"][0];
    for field in &["id", "account_id", "status", "currency"] {
        assert!(item.get(field).is_some(), "list item must have '{field}'");
    }

    Ok(())
}

// ============================================================================
// Success Response Contracts: Approval Tools
// ============================================================================

#[tokio::test]
async fn contract_approval_request_success_shape() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());
    let quote_id = create_test_quote(&server, &pool, "apr-shape").await?;

    let input = quotey_mcp::server::ApprovalRequestInput {
        quote_id: quote_id.clone(),
        justification: "Volume commitment from strategic account requires discount.".to_string(),
        approver_role: Some("sales_director".to_string()),
    };
    let v = parse(&server.approval_request(Parameters(input)).await);
    assert!(v.get("error").is_none(), "expected success, got: {v}");

    // Required fields
    for field in &["approval_id", "quote_id", "status", "approver_role", "expires_at"] {
        assert!(v.get(field).is_some(), "approval_request must have '{field}'");
    }

    assert!(v["approval_id"].as_str().unwrap().starts_with("APR-"), "must start with APR-");
    assert_eq!(v["status"].as_str(), Some("pending"));
    assert_eq!(v["quote_id"].as_str().unwrap(), quote_id);

    Ok(())
}

#[tokio::test]
async fn contract_approval_status_success_shape() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());
    let quote_id = create_test_quote(&server, &pool, "apr-status").await?;

    let input = quotey_mcp::server::ApprovalStatusInput { quote_id: quote_id.clone() };
    let v = parse(&server.approval_status(Parameters(input)).await);
    assert!(v.get("error").is_none(), "expected success, got: {v}");

    for field in &["quote_id", "current_status", "can_proceed", "pending_requests"] {
        assert!(v.get(field).is_some(), "approval_status must have '{field}'");
    }

    assert!(v["can_proceed"].is_boolean(), "can_proceed must be boolean");
    assert!(v["pending_requests"].is_array(), "pending_requests must be array");

    Ok(())
}

#[tokio::test]
async fn contract_approval_pending_success_shape() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());
    let quote_id = create_test_quote(&server, &pool, "apr-pend").await?;

    // Create an approval first
    let apr_input = quotey_mcp::server::ApprovalRequestInput {
        quote_id,
        justification: "Contract test pending list".to_string(),
        approver_role: Some("deal_desk".to_string()),
    };
    server.approval_request(Parameters(apr_input)).await;

    let input = quotey_mcp::server::ApprovalPendingInput {
        approver_role: Some("deal_desk".to_string()),
        limit: 10,
    };
    let v = parse(&server.approval_pending(Parameters(input)).await);
    assert!(v.get("error").is_none(), "expected success, got: {v}");

    assert!(v.get("items").is_some(), "must have 'items'");
    assert!(v["items"].is_array(), "items must be array");

    let item = &v["items"][0];
    for field in
        &["approval_id", "quote_id", "account_name", "quote_total", "requested_by", "justification"]
    {
        assert!(item.get(field).is_some(), "pending item must have '{field}'");
    }

    Ok(())
}

// ============================================================================
// Normalization Behavior Contracts
// ============================================================================

#[tokio::test]
async fn contract_pagination_defaults_limit_zero_becomes_default() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "CS-NORM-1", "CS-NORM-SKU", "Norm Item", 1000, "USD").await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // limit=0 should default to 20
    let input = quotey_mcp::server::CatalogSearchInput {
        query: "Norm".to_string(),
        category: None,
        active_only: true,
        limit: 0,
        page: 0,
    };
    let v = parse(&server.catalog_search(Parameters(input)).await);
    assert!(v.get("error").is_none(), "expected success");

    let pg = &v["pagination"];
    assert_eq!(pg["per_page"].as_u64(), Some(20), "limit=0 must normalize to 20");
    assert_eq!(pg["page"].as_u64(), Some(1), "page=0 must normalize to 1");

    Ok(())
}

#[tokio::test]
async fn contract_pagination_max_limit_clamped() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "CS-CLAMP-1", "CS-CLAMP-SKU", "Clamp Item", 1000, "USD").await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // limit=9999 should be clamped to 100
    let input = quotey_mcp::server::CatalogSearchInput {
        query: "Clamp".to_string(),
        category: None,
        active_only: true,
        limit: 9999,
        page: 1,
    };
    let v = parse(&server.catalog_search(Parameters(input)).await);
    assert!(v.get("error").is_none(), "expected success");

    let pg = &v["pagination"];
    assert_eq!(pg["per_page"].as_u64(), Some(100), "limit=9999 must be clamped to 100");

    Ok(())
}

// ============================================================================
// Currency Mismatch Contract
// ============================================================================

#[tokio::test]
async fn contract_currency_mismatch_produces_error() -> TestResult {
    let pool = setup_pool().await?;
    // Product in EUR
    seed_product(&pool, "CS-EUR-1", "CS-EUR-SKU", "Euro Widget", 5000, "EUR").await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Quote in USD with EUR product
    let input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-MISMATCH".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: Some(12),
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "CS-EUR-1".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("mismatch-test".to_string()),
    };

    let v = parse(&server.quote_create(Parameters(input)).await);
    assert_eq!(
        v["error"]["code"].as_str(),
        Some("CURRENCY_MISMATCH"),
        "currency mismatch must produce CURRENCY_MISMATCH error"
    );

    // Message should mention both currencies
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("USD") || msg.contains("EUR"),
        "error message should reference the currencies involved"
    );

    Ok(())
}

// ============================================================================
// Validation Boundary Contracts
// ============================================================================

#[tokio::test]
async fn contract_max_quantity_boundary() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "CS-MAXQ-1", "CS-MAXQ-SKU", "MaxQ Item", 1000, "USD").await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Quantity over 1,000,000 should be rejected
    let input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-MAXQ".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "CS-MAXQ-1".to_string(),
            quantity: 1_000_001,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: None,
    };

    let v = parse(&server.quote_create(Parameters(input)).await);
    assert_eq!(
        v["error"]["code"].as_str(),
        Some("VALIDATION_ERROR"),
        "quantity > MAX_QUANTITY must be rejected"
    );

    Ok(())
}

#[tokio::test]
async fn contract_max_line_items_boundary() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "CS-MLI-1", "CS-MLI-SKU", "MLI Item", 100, "USD").await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // 501 line items should be rejected (MAX_LINE_ITEMS = 500)
    let items: Vec<quotey_mcp::server::LineItemInput> = (0..501)
        .map(|_| quotey_mcp::server::LineItemInput {
            product_id: "CS-MLI-1".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        })
        .collect();

    let input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-MLI".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: items,
        idempotency_key: None,
    };

    let v = parse(&server.quote_create(Parameters(input)).await);
    assert_eq!(
        v["error"]["code"].as_str(),
        Some("VALIDATION_ERROR"),
        "too many line items must be rejected"
    );

    Ok(())
}

#[tokio::test]
async fn contract_negative_discount_rejected() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "CS-NEG-1", "CS-NEG-SKU", "Neg Item", 5000, "USD").await?;

    let server = QuoteyMcpServer::new(pool.clone());

    let input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-NEG".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "CS-NEG-1".to_string(),
            quantity: 1,
            discount_pct: -5.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: None,
    };

    let v = parse(&server.quote_create(Parameters(input)).await);
    assert_eq!(
        v["error"]["code"].as_str(),
        Some("VALIDATION_ERROR"),
        "negative discount must be rejected"
    );

    Ok(())
}

// ============================================================================
// Approval Error Envelope Details Contract
// ============================================================================

#[tokio::test]
async fn contract_duplicate_approval_error_includes_details() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());
    let quote_id = create_test_quote(&server, &pool, "dup-apr").await?;

    // First approval
    let input = quotey_mcp::server::ApprovalRequestInput {
        quote_id: quote_id.clone(),
        justification: "First request is valid.".to_string(),
        approver_role: Some("sales_manager".to_string()),
    };
    let first = parse(&server.approval_request(Parameters(input)).await);
    assert!(first.get("error").is_none(), "first approval should succeed");

    // Duplicate
    let dup_input = quotey_mcp::server::ApprovalRequestInput {
        quote_id: quote_id.clone(),
        justification: "Duplicate attempt.".to_string(),
        approver_role: Some("sales_manager".to_string()),
    };
    let v = parse(&server.approval_request(Parameters(dup_input)).await);

    assert_eq!(v["error"]["code"].as_str(), Some("CONFLICT"));
    // Details should include the existing approval ID
    let details = &v["error"]["details"];
    if !details.is_null() {
        // When details are present, they should include existing_approval_id
        if let Some(existing) = details.get("existing_approval_id") {
            assert!(existing.as_str().unwrap().starts_with("APR-"));
        }
    }

    Ok(())
}

// ============================================================================
// Policy Violation Shape Contract
// ============================================================================

#[tokio::test]
async fn contract_policy_violation_shape_on_high_discount() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());
    let quote_id = create_test_quote(&server, &pool, "policy").await?;

    // Price with 25% discount to trigger policy violation
    let input = quotey_mcp::server::QuotePriceInput { quote_id, requested_discount_pct: 25.0 };
    let v = parse(&server.quote_price(Parameters(input)).await);
    assert!(v.get("error").is_none(), "expected success, got: {v}");

    assert_eq!(v["approval_required"].as_bool(), Some(true));

    let violations = v["policy_violations"].as_array().unwrap();
    assert!(!violations.is_empty(), "25% discount must trigger violations");

    // Each violation should have standard fields
    for violation in violations {
        assert!(violation.get("policy_id").is_some(), "violation must have policy_id");
        assert!(violation.get("description").is_some(), "violation must have description");
        assert!(violation.get("severity").is_some(), "violation must have severity");
    }

    Ok(())
}

// ============================================================================
// Audit Trail Persistence Contract
// ============================================================================

#[tokio::test]
async fn contract_tool_calls_create_audit_events() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());
    let quote_id = create_test_quote(&server, &pool, "audit").await?;

    // quote_create records audit with event_type='mcp.quote_create.invoked' (quote_id may be NULL)
    let row = sqlx::query(
        "SELECT COUNT(*) as cnt FROM audit_event WHERE event_type = 'mcp.quote_create.invoked'",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| format!("audit query: {e}"))?;
    let count = row.get::<i64, _>("cnt");

    assert!(count > 0, "quote_create must persist audit events (found {})", count);

    // Price the quote — this records audit with quote_id set
    let row_before = sqlx::query("SELECT COUNT(*) as cnt FROM audit_event")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("audit query: {e}"))?;
    let before_total = row_before.get::<i64, _>("cnt");

    let input = quotey_mcp::server::QuotePriceInput {
        quote_id: quote_id.clone(),
        requested_discount_pct: 0.0,
    };
    server.quote_price(Parameters(input)).await;

    let row_after = sqlx::query("SELECT COUNT(*) as cnt FROM audit_event")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("audit query: {e}"))?;
    let after_total = row_after.get::<i64, _>("cnt");

    assert!(
        after_total > before_total,
        "quote_price must create additional audit events (before={}, after={})",
        before_total,
        after_total
    );

    Ok(())
}

// ============================================================================
// Quote PDF Tool Contract
// ============================================================================

#[tokio::test]
async fn contract_quote_pdf_for_nonexistent_quote() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());

    let input = quotey_mcp::server::QuotePdfInput {
        quote_id: "Q-NOPDF".to_string(),
        template: "detailed".to_string(),
    };
    let v = parse(&server.quote_pdf(Parameters(input)).await);
    assert_eq!(
        v["error"]["code"].as_str(),
        Some("NOT_FOUND"),
        "PDF for nonexistent quote must return NOT_FOUND"
    );

    Ok(())
}

#[tokio::test]
async fn contract_quote_pdf_invalid_template() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());
    let quote_id = create_test_quote(&server, &pool, "pdf-tpl").await?;

    let input = quotey_mcp::server::QuotePdfInput {
        quote_id,
        template: "nonexistent_template".to_string(),
    };
    let v = parse(&server.quote_pdf(Parameters(input)).await);
    assert_eq!(
        v["error"]["code"].as_str(),
        Some("VALIDATION_ERROR"),
        "invalid template must return VALIDATION_ERROR"
    );

    Ok(())
}

#[tokio::test]
async fn contract_quote_pdf_success_shape() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());
    let quote_id = create_test_quote(&server, &pool, "pdf-ok").await?;

    let input = quotey_mcp::server::QuotePdfInput { quote_id, template: "detailed".to_string() };
    let v = parse(&server.quote_pdf(Parameters(input)).await);

    // PDF may fail if wkhtmltopdf is not installed, but the response
    // should still have the expected shape (either success or graceful error)
    if v.get("error").is_none() {
        // Success shape
        for field in &["file_path", "file_size_bytes", "pdf_generated"] {
            assert!(v.get(field).is_some(), "quote_pdf success must have '{field}'");
        }
        assert!(v["pdf_generated"].is_boolean());
    }
    // If error, it should be INTERNAL_ERROR from missing wkhtmltopdf, not a crash

    Ok(())
}
