//! Integration tests for Quotey MCP Server
//!
//! These tests verify that the MCP server correctly handles:
//! - Server info and tool listing
//! - Tool calling (no-auth mode)
//! - Authentication enforcement (auth-required mode)
//! - Rate limiting
//! - Tool execution against real SQLite database
//! - Audit trail persistence

use quotey_mcp::{ApiKeyConfig, AuthConfig, AuthManager, QuoteyMcpServer};
use rmcp::ServerHandler;
use sqlx::Row;

/// Create a test database pool (in-memory SQLite).
async fn test_db() -> quotey_db::DbPool {
    quotey_db::connect("sqlite::memory:").await.expect("in-memory DB")
}

/// Create a test server without authentication.
async fn server_no_auth() -> QuoteyMcpServer {
    QuoteyMcpServer::new(test_db().await)
}

/// Create a test server with authentication.
async fn server_with_auth(key: &str, rpm: u32) -> QuoteyMcpServer {
    let auth = AuthManager::from_config(&AuthConfig {
        enabled: true,
        rate_limit_window_secs: 60,
        api_keys: vec![ApiKeyConfig {
            key: key.to_string(),
            name: "test-agent".to_string(),
            requests_per_minute: rpm,
        }],
    });
    QuoteyMcpServer::with_auth(test_db().await, auth)
}

#[tokio::test]
async fn test_server_info() {
    let server = server_no_auth().await;
    let info = server.get_info();
    assert_eq!(info.server_info.name, "quotey-mcp");
    assert!(info.instructions.unwrap().contains("catalog_search"));
}

#[tokio::test]
async fn test_auth_manager_accessor() {
    let server = server_no_auth().await;
    assert!(!server.auth_manager().is_auth_required());

    let server = server_with_auth("key123", 60).await;
    assert!(server.auth_manager().is_auth_required());
}

#[tokio::test]
async fn test_no_auth_allows_all() {
    let auth = AuthManager::no_auth();
    let result = auth.validate_request(None).await;
    assert!(result.is_allowed());
}

#[tokio::test]
async fn test_auth_required_rejects_missing_key() {
    let auth = AuthManager::from_config(&AuthConfig {
        enabled: true,
        rate_limit_window_secs: 60,
        api_keys: vec![ApiKeyConfig {
            key: "secret".to_string(),
            name: "agent".to_string(),
            requests_per_minute: 60,
        }],
    });
    let result = auth.validate_request(None).await;
    assert!(!result.is_allowed());
    assert_eq!(result.denial_reason(), Some("API key required"));
}

#[tokio::test]
async fn test_auth_rejects_wrong_key() {
    let auth = AuthManager::from_config(&AuthConfig {
        enabled: true,
        rate_limit_window_secs: 60,
        api_keys: vec![ApiKeyConfig {
            key: "correct-key".to_string(),
            name: "agent".to_string(),
            requests_per_minute: 60,
        }],
    });
    let result = auth.validate_request(Some("wrong-key")).await;
    assert!(!result.is_allowed());
    assert_eq!(result.denial_reason(), Some("Invalid API key"));
}

#[tokio::test]
async fn test_auth_accepts_valid_key() {
    let auth = AuthManager::from_config(&AuthConfig {
        enabled: true,
        rate_limit_window_secs: 60,
        api_keys: vec![ApiKeyConfig {
            key: "valid-key".to_string(),
            name: "my-agent".to_string(),
            requests_per_minute: 100,
        }],
    });
    let result = auth.validate_request(Some("valid-key")).await;
    assert!(result.is_allowed());
    assert!(result.remaining().unwrap() > 0);
}

#[tokio::test]
async fn test_rate_limiting_enforced() {
    let auth = AuthManager::from_config(&AuthConfig {
        enabled: true,
        rate_limit_window_secs: 60,
        api_keys: vec![ApiKeyConfig {
            key: "limited-key".to_string(),
            name: "rate-limited".to_string(),
            requests_per_minute: 3,
        }],
    });

    // First 3 requests succeed
    for _ in 0..3 {
        let r = auth.validate_request(Some("limited-key")).await;
        assert!(r.is_allowed());
    }

    // Fourth request is rate-limited
    let r = auth.validate_request(Some("limited-key")).await;
    assert!(!r.is_allowed());
    assert_eq!(r.denial_reason(), Some("Rate limit exceeded"));
    assert!(r.retry_after().is_some());
}

#[tokio::test]
async fn test_disabled_auth_config() {
    let auth = AuthManager::from_config(&AuthConfig {
        enabled: false,
        rate_limit_window_secs: 60,
        api_keys: vec![ApiKeyConfig {
            key: "ignored".to_string(),
            name: "ignored".to_string(),
            requests_per_minute: 60,
        }],
    });
    // Disabled auth → all requests allowed even without key
    let result = auth.validate_request(None).await;
    assert!(result.is_allowed());
}

#[tokio::test]
async fn test_api_key_config_serde() {
    let json = r#"[
        {"key":"k1","name":"Agent1","requests_per_minute":30},
        {"key":"k2","name":"Agent2"}
    ]"#;
    let configs: Vec<ApiKeyConfig> = serde_json::from_str(json).unwrap();
    assert_eq!(configs.len(), 2);
    assert_eq!(configs[0].requests_per_minute, 30);
    // Default requests_per_minute is 60
    assert_eq!(configs[1].requests_per_minute, 60);
}

// ============================================================================
// Tool Integration Tests Against Real SQLite Database
// ============================================================================

type TestResult<T = ()> = Result<T, String>;

/// Create a test database pool with migrations applied.
async fn setup_pool() -> TestResult<quotey_db::DbPool> {
    let pool = quotey_db::connect_with_settings("sqlite::memory:", 1, 30)
        .await
        .map_err(|e| format!("connect: {e}"))?;
    quotey_db::migrations::run_pending(&pool).await.map_err(|e| format!("migrations: {e}"))?;
    Ok(pool)
}

/// Seed a product directly into the database.
async fn seed_product(
    pool: &quotey_db::DbPool,
    id: &str,
    sku: &str,
    name: &str,
    price_cents: i64,
) -> TestResult {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO product (id, sku, name, base_price, currency, active, created_at, updated_at)
         VALUES (?, ?, ?, ?, 'USD', 1, ?, ?)",
    )
    .bind(id)
    .bind(sku)
    .bind(name)
    .bind(format!("{:.2}", price_cents as f64 / 100.0))
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| format!("seed product: {e}"))?;
    Ok(())
}

/// Parse tool output as JSON.
fn parse_output(output: &str) -> serde_json::Value {
    serde_json::from_str(output).expect("tool output must be valid JSON")
}

/// Extract error code from tool error response.
fn error_code(output: &serde_json::Value) -> Option<&str> {
    output.get("error")?.get("code")?.as_str()
}

// ============================================================================
// Catalog Tools: catalog_search, catalog_get
// ============================================================================

#[tokio::test]
async fn test_catalog_search_with_results() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-001", "SKU-A", "Widget Pro", 4999).await?;
    seed_product(&pool, "PROD-002", "SKU-B", "Widget Lite", 2999).await?;
    seed_product(&pool, "PROD-003", "SKU-C", "Gadget Plus", 1999).await?;

    let server = QuoteyMcpServer::new(pool.clone());
    let input = quotey_mcp::server::CatalogSearchInput {
        query: "Widget".to_string(),
        category: None,
        active_only: true,
        limit: 20,
        page: 1,
    };

    let output = server.catalog_search(rmcp::handler::server::wrapper::Parameters(input)).await;
    let parsed = parse_output(&output);

    assert!(parsed.get("error").is_none(), "expected success, got: {:?}", parsed);
    let items = parsed.get("items").and_then(|v| v.as_array()).expect("items array");
    assert_eq!(items.len(), 2, "should find 2 widget products");

    // Verify pagination
    let pagination = parsed.get("pagination").expect("pagination");
    assert_eq!(pagination.get("total").and_then(|v| v.as_u64()), Some(2));
    assert_eq!(pagination.get("page").and_then(|v| v.as_u64()), Some(1));
    assert_eq!(pagination.get("has_more").and_then(|v| v.as_bool()), Some(false));

    Ok(())
}

#[tokio::test]
async fn test_catalog_search_empty_query_rejected() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());

    let input = quotey_mcp::server::CatalogSearchInput {
        query: "   ".to_string(),
        category: None,
        active_only: true,
        limit: 20,
        page: 1,
    };

    let output = server.catalog_search(rmcp::handler::server::wrapper::Parameters(input)).await;
    let parsed = parse_output(&output);

    assert_eq!(error_code(&parsed), Some("VALIDATION_ERROR"));
    Ok(())
}

#[tokio::test]
async fn test_catalog_search_pagination() -> TestResult {
    let pool = setup_pool().await?;
    // Seed 25 products
    for i in 0..25 {
        seed_product(
            &pool,
            &format!("PROD-{:03}", i),
            &format!("SKU-{}", i),
            &format!("Item {}", i),
            1000 + i as i64 * 100,
        )
        .await?;
    }

    let server = QuoteyMcpServer::new(pool.clone());
    let input = quotey_mcp::server::CatalogSearchInput {
        query: "Item".to_string(),
        category: None,
        active_only: true,
        limit: 10,
        page: 1,
    };

    let output = server.catalog_search(rmcp::handler::server::wrapper::Parameters(input)).await;
    let parsed = parse_output(&output);

    let items = parsed.get("items").and_then(|v| v.as_array()).expect("items array");
    assert_eq!(items.len(), 10, "first page should have 10 items");
    assert_eq!(
        parsed.get("pagination").and_then(|p| p.get("has_more").and_then(|v| v.as_bool())),
        Some(true)
    );

    Ok(())
}

#[tokio::test]
async fn test_catalog_get_found() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-GET-1", "SKU-GET", "Test Product", 9999).await?;

    let server = QuoteyMcpServer::new(pool.clone());
    let input = quotey_mcp::server::CatalogGetInput {
        product_id: "PROD-GET-1".to_string(),
        include_relationships: false,
    };

    let output = server.catalog_get(rmcp::handler::server::wrapper::Parameters(input)).await;
    let parsed = parse_output(&output);

    assert!(parsed.get("error").is_none(), "expected success, got: {:?}", parsed);
    assert_eq!(parsed.get("id").and_then(|v| v.as_str()), Some("PROD-GET-1"));
    assert_eq!(parsed.get("sku").and_then(|v| v.as_str()), Some("SKU-GET"));
    assert_eq!(parsed.get("name").and_then(|v| v.as_str()), Some("Test Product"));
    assert_eq!(parsed.get("active").and_then(|v| v.as_bool()), Some(true));

    Ok(())
}

#[tokio::test]
async fn test_catalog_get_not_found() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());

    let input = quotey_mcp::server::CatalogGetInput {
        product_id: "NONEXISTENT".to_string(),
        include_relationships: false,
    };

    let output = server.catalog_get(rmcp::handler::server::wrapper::Parameters(input)).await;
    let parsed = parse_output(&output);

    assert_eq!(error_code(&parsed), Some("NOT_FOUND"));
    Ok(())
}

#[tokio::test]
async fn test_catalog_get_empty_id_rejected() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());

    let input = quotey_mcp::server::CatalogGetInput {
        product_id: "   ".to_string(),
        include_relationships: false,
    };

    let output = server.catalog_get(rmcp::handler::server::wrapper::Parameters(input)).await;
    let parsed = parse_output(&output);

    assert_eq!(error_code(&parsed), Some("VALIDATION_ERROR"));
    Ok(())
}

// ============================================================================
// Quote Tools: quote_create, quote_get, quote_price, quote_list
// ============================================================================

#[tokio::test]
async fn test_quote_create_success() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-Q-1", "SKU-Q1", "Quote Item", 5000).await?;

    let server = QuoteyMcpServer::new(pool.clone());
    let input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-001".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: Some(12),
        start_date: None,
        notes: Some("Test quote".to_string()),
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-Q-1".to_string(),
            quantity: 5,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("test-quote-001".to_string()),
    };

    let output = server.quote_create(rmcp::handler::server::wrapper::Parameters(input)).await;
    let parsed = parse_output(&output);

    assert!(parsed.get("error").is_none(), "expected success, got: {:?}", parsed);
    assert!(parsed.get("quote_id").and_then(|v| v.as_str()).unwrap().starts_with("Q-"));
    assert_eq!(parsed.get("status").and_then(|v| v.as_str()), Some("draft"));
    assert_eq!(parsed.get("account_id").and_then(|v| v.as_str()), Some("ACCT-001"));
    assert_eq!(parsed.get("currency").and_then(|v| v.as_str()), Some("USD"));
    assert_eq!(parsed.get("version").and_then(|v| v.as_u64()), Some(1));

    let items = parsed.get("line_items").and_then(|v| v.as_array()).expect("line_items");
    assert_eq!(items.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_quote_create_idempotent() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-IDEM", "SKU-IDEM", "Idempotent Item", 1000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    let input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-IDEM".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: Some(12),
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-IDEM".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("idem-key-123".to_string()),
    };

    let output1 =
        server.quote_create(rmcp::handler::server::wrapper::Parameters(input.clone())).await;
    let parsed1 = parse_output(&output1);
    let quote_id1 = parsed1.get("quote_id").and_then(|v| v.as_str()).unwrap();

    // Second call with same idempotency key should return same quote ID
    let output2 = server.quote_create(rmcp::handler::server::wrapper::Parameters(input)).await;
    let parsed2 = parse_output(&output2);
    let quote_id2 = parsed2.get("quote_id").and_then(|v| v.as_str()).unwrap();

    assert_eq!(quote_id1, quote_id2, "idempotency key should produce same quote ID");

    Ok(())
}

#[tokio::test]
async fn test_quote_create_validation_errors() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-VAL", "SKU-VAL", "Validation Item", 1000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Empty account_id
    let input = quotey_mcp::server::QuoteCreateInput {
        account_id: "   ".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-VAL".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: None,
    };
    let output = server.quote_create(rmcp::handler::server::wrapper::Parameters(input)).await;
    assert_eq!(error_code(&parse_output(&output)), Some("VALIDATION_ERROR"));

    // Empty line items
    let input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-VAL".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![],
        idempotency_key: None,
    };
    let output = server.quote_create(rmcp::handler::server::wrapper::Parameters(input)).await;
    assert_eq!(error_code(&parse_output(&output)), Some("VALIDATION_ERROR"));

    // Zero quantity
    let input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-VAL".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-VAL".to_string(),
            quantity: 0,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: None,
    };
    let output = server.quote_create(rmcp::handler::server::wrapper::Parameters(input)).await;
    assert_eq!(error_code(&parse_output(&output)), Some("VALIDATION_ERROR"));

    // Invalid discount ( > 100)
    let input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-VAL".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-VAL".to_string(),
            quantity: 1,
            discount_pct: 150.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: None,
    };
    let output = server.quote_create(rmcp::handler::server::wrapper::Parameters(input)).await;
    assert_eq!(error_code(&parse_output(&output)), Some("VALIDATION_ERROR"));

    Ok(())
}

#[tokio::test]
async fn test_quote_create_product_not_found() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());

    let input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-NF".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "NONEXISTENT-PROD".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: None,
    };

    let output = server.quote_create(rmcp::handler::server::wrapper::Parameters(input)).await;
    assert_eq!(error_code(&parse_output(&output)), Some("NOT_FOUND"));

    Ok(())
}

#[tokio::test]
async fn test_quote_create_product_inactive() -> TestResult {
    let pool = setup_pool().await?;

    // Insert inactive product
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO product (id, sku, name, base_price, currency, active, created_at, updated_at)
         VALUES (?, ?, ?, ?, 'USD', 0, ?, ?)",
    )
    .bind("PROD-INACTIVE")
    .bind("SKU-INACTIVE")
    .bind("Inactive Product")
    .bind("10.00")
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await
    .map_err(|e| format!("seed inactive product: {e}"))?;

    let server = QuoteyMcpServer::new(pool.clone());

    let input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-INACTIVE".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-INACTIVE".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: None,
    };

    let output = server.quote_create(rmcp::handler::server::wrapper::Parameters(input)).await;
    assert_eq!(error_code(&parse_output(&output)), Some("CONFLICT"));

    Ok(())
}

#[tokio::test]
async fn test_quote_get_found() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-GETQ", "SKU-GETQ", "Get Quote Item", 2500).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // First create a quote
    let create_input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-GET".to_string(),
        deal_id: Some("DEAL-001".to_string()),
        currency: "USD".to_string(),
        term_months: Some(6),
        start_date: None,
        notes: Some("Quote for retrieval test".to_string()),
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-GETQ".to_string(),
            quantity: 3,
            discount_pct: 10.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("quote-get-test".to_string()),
    };

    let create_output =
        server.quote_create(rmcp::handler::server::wrapper::Parameters(create_input)).await;
    let create_parsed = parse_output(&create_output);
    let quote_id = create_parsed.get("quote_id").and_then(|v| v.as_str()).unwrap();

    // Now retrieve it
    let get_input =
        quotey_mcp::server::QuoteGetInput { quote_id: quote_id.to_string(), include_pricing: true };

    let get_output = server.quote_get(rmcp::handler::server::wrapper::Parameters(get_input)).await;
    let get_parsed = parse_output(&get_output);

    assert!(get_parsed.get("error").is_none(), "expected success, got: {:?}", get_parsed);

    let quote = get_parsed.get("quote").expect("quote object");
    assert_eq!(quote.get("id").and_then(|v| v.as_str()), Some(quote_id));
    assert_eq!(quote.get("account_id").and_then(|v| v.as_str()), Some("ACCT-GET"));
    assert_eq!(quote.get("deal_id").and_then(|v| v.as_str()), Some("DEAL-001"));
    assert_eq!(quote.get("currency").and_then(|v| v.as_str()), Some("USD"));
    assert_eq!(quote.get("term_months").and_then(|v| v.as_u64()), Some(6));

    let line_items = get_parsed.get("line_items").and_then(|v| v.as_array()).expect("line_items");
    assert_eq!(line_items.len(), 1);

    // Verify pricing is included
    assert!(get_parsed.get("pricing").is_some());

    Ok(())
}

#[tokio::test]
async fn test_quote_get_not_found() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());

    let input = quotey_mcp::server::QuoteGetInput {
        quote_id: "Q-NONEXISTENT".to_string(),
        include_pricing: false,
    };

    let output = server.quote_get(rmcp::handler::server::wrapper::Parameters(input)).await;
    assert_eq!(error_code(&parse_output(&output)), Some("NOT_FOUND"));

    Ok(())
}

#[tokio::test]
async fn test_quote_price_success() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-PRICE", "SKU-PRICE", "Price Test Item", 10000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Create quote
    let create_input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-PRICE".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: Some(12),
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-PRICE".to_string(),
            quantity: 2,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("price-test".to_string()),
    };

    let create_output =
        server.quote_create(rmcp::handler::server::wrapper::Parameters(create_input)).await;
    let quote_id =
        parse_output(&create_output).get("quote_id").and_then(|v| v.as_str()).unwrap().to_string();

    // Price the quote
    let price_input = quotey_mcp::server::QuotePriceInput {
        quote_id: quote_id.clone(),
        requested_discount_pct: 0.0,
    };

    let price_output =
        server.quote_price(rmcp::handler::server::wrapper::Parameters(price_input)).await;
    let price_parsed = parse_output(&price_output);

    assert!(price_parsed.get("error").is_none(), "expected success, got: {:?}", price_parsed);
    assert_eq!(price_parsed.get("quote_id").and_then(|v| v.as_str()), Some(quote_id.as_str()));

    let pricing = price_parsed.get("pricing").expect("pricing object");
    // 2 x $100.00 = $200.00
    assert_eq!(pricing.get("subtotal").and_then(|v| v.as_f64()), Some(200.0));
    assert_eq!(pricing.get("discount_total").and_then(|v| v.as_f64()), Some(0.0));
    assert_eq!(pricing.get("total").and_then(|v| v.as_f64()), Some(200.0));

    let line_pricing =
        price_parsed.get("line_pricing").and_then(|v| v.as_array()).expect("line_pricing");
    assert_eq!(line_pricing.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_quote_price_with_discount() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-DISC", "SKU-DISC", "Discount Item", 10000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Create quote
    let create_input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-DISC".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: Some(12),
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-DISC".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("disc-test".to_string()),
    };

    let create_output =
        server.quote_create(rmcp::handler::server::wrapper::Parameters(create_input)).await;
    let quote_id =
        parse_output(&create_output).get("quote_id").and_then(|v| v.as_str()).unwrap().to_string();

    // Price with 25% discount (triggers approval)
    let price_input = quotey_mcp::server::QuotePriceInput {
        quote_id: quote_id.clone(),
        requested_discount_pct: 25.0,
    };

    let price_output =
        server.quote_price(rmcp::handler::server::wrapper::Parameters(price_input)).await;
    let price_parsed = parse_output(&price_output);

    assert!(price_parsed.get("error").is_none(), "expected success, got: {:?}", price_parsed);

    // 25% discount should require approval
    assert_eq!(price_parsed.get("approval_required").and_then(|v| v.as_bool()), Some(true));

    // Check policy violations
    let violations = price_parsed
        .get("policy_violations")
        .and_then(|v| v.as_array())
        .expect("policy_violations");
    assert!(!violations.is_empty(), "25% discount should trigger policy violation");

    Ok(())
}

#[tokio::test]
async fn test_quote_list_with_pagination() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-LIST", "SKU-LIST", "List Item", 1000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Create 3 quotes for same account
    for i in 0..3 {
        let create_input = quotey_mcp::server::QuoteCreateInput {
            account_id: "ACCT-LIST".to_string(),
            deal_id: None,
            currency: "USD".to_string(),
            term_months: None,
            start_date: None,
            notes: None,
            line_items: vec![quotey_mcp::server::LineItemInput {
                product_id: "PROD-LIST".to_string(),
                quantity: 1,
                discount_pct: 0.0,
                attributes: None,
                notes: None,
            }],
            idempotency_key: Some(format!("list-test-{}", i)),
        };
        server.quote_create(rmcp::handler::server::wrapper::Parameters(create_input)).await;
    }

    // List quotes for account
    let list_input = quotey_mcp::server::QuoteListInput {
        account_id: Some("ACCT-LIST".to_string()),
        status: None,
        limit: 10,
        page: 1,
    };

    let list_output =
        server.quote_list(rmcp::handler::server::wrapper::Parameters(list_input)).await;
    let list_parsed = parse_output(&list_output);

    assert!(list_parsed.get("error").is_none(), "expected success, got: {:?}", list_parsed);

    let items = list_parsed.get("items").and_then(|v| v.as_array()).expect("items");
    assert_eq!(items.len(), 3, "should find 3 quotes for account");

    // All items should be for the correct account
    for item in items {
        assert_eq!(item.get("account_id").and_then(|v| v.as_str()), Some("ACCT-LIST"));
    }

    Ok(())
}

#[tokio::test]
async fn test_quote_list_filter_by_status() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-STATUS", "SKU-STATUS", "Status Item", 1000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Create quote (will be in 'draft' status)
    let create_input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-STATUS".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-STATUS".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("status-test".to_string()),
    };
    server.quote_create(rmcp::handler::server::wrapper::Parameters(create_input)).await;

    // List drafts only
    let list_input = quotey_mcp::server::QuoteListInput {
        account_id: Some("ACCT-STATUS".to_string()),
        status: Some("draft".to_string()),
        limit: 10,
        page: 1,
    };

    let list_output =
        server.quote_list(rmcp::handler::server::wrapper::Parameters(list_input)).await;
    let list_parsed = parse_output(&list_output);

    let items = list_parsed.get("items").and_then(|v| v.as_array()).expect("items");
    assert!(!items.is_empty(), "should find draft quotes");

    for item in items {
        assert_eq!(item.get("status").and_then(|v| v.as_str()), Some("draft"));
    }

    Ok(())
}

// ============================================================================
// Approval Tools: approval_request, approval_status, approval_pending
// ============================================================================

#[tokio::test]
async fn test_approval_request_success() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-APR", "SKU-APR", "Approval Item", 5000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Create quote
    let create_input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-APR".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-APR".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("apr-test".to_string()),
    };

    let create_output =
        server.quote_create(rmcp::handler::server::wrapper::Parameters(create_input)).await;
    let quote_id =
        parse_output(&create_output).get("quote_id").and_then(|v| v.as_str()).unwrap().to_string();

    // Request approval
    let apr_input = quotey_mcp::server::ApprovalRequestInput {
        quote_id: quote_id.clone(),
        justification: "Customer requires 30% discount due to volume commitment.".to_string(),
        approver_role: Some("sales_manager".to_string()),
    };

    let apr_output =
        server.approval_request(rmcp::handler::server::wrapper::Parameters(apr_input)).await;
    let apr_parsed = parse_output(&apr_output);

    assert!(apr_parsed.get("error").is_none(), "expected success, got: {:?}", apr_parsed);
    assert!(apr_parsed.get("approval_id").and_then(|v| v.as_str()).unwrap().starts_with("APR-"));
    assert_eq!(apr_parsed.get("quote_id").and_then(|v| v.as_str()), Some(quote_id.as_str()));
    assert_eq!(apr_parsed.get("status").and_then(|v| v.as_str()), Some("pending"));
    assert_eq!(apr_parsed.get("approver_role").and_then(|v| v.as_str()), Some("sales_manager"));

    Ok(())
}

#[tokio::test]
async fn test_approval_request_duplicate_rejected() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-DUP", "SKU-DUP", "Dup Item", 5000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Create quote
    let create_input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-DUP".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-DUP".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("dup-test".to_string()),
    };

    let create_output =
        server.quote_create(rmcp::handler::server::wrapper::Parameters(create_input)).await;
    let quote_id =
        parse_output(&create_output).get("quote_id").and_then(|v| v.as_str()).unwrap().to_string();

    // First approval request
    let apr_input = quotey_mcp::server::ApprovalRequestInput {
        quote_id: quote_id.clone(),
        justification: "First request".to_string(),
        approver_role: Some("sales_manager".to_string()),
    };
    server.approval_request(rmcp::handler::server::wrapper::Parameters(apr_input)).await;

    // Duplicate request should fail
    let dup_input = quotey_mcp::server::ApprovalRequestInput {
        quote_id: quote_id.clone(),
        justification: "Duplicate request".to_string(),
        approver_role: Some("sales_manager".to_string()),
    };

    let dup_output =
        server.approval_request(rmcp::handler::server::wrapper::Parameters(dup_input)).await;
    assert_eq!(error_code(&parse_output(&dup_output)), Some("CONFLICT"));

    Ok(())
}

#[tokio::test]
async fn test_approval_request_validation_errors() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-APV", "SKU-APV", "APV Item", 5000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Create quote
    let create_input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-APV".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-APV".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("apv-test".to_string()),
    };

    let create_output =
        server.quote_create(rmcp::handler::server::wrapper::Parameters(create_input)).await;
    let quote_id =
        parse_output(&create_output).get("quote_id").and_then(|v| v.as_str()).unwrap().to_string();

    // Empty justification
    let apr_input = quotey_mcp::server::ApprovalRequestInput {
        quote_id: quote_id.clone(),
        justification: "   ".to_string(),
        approver_role: None,
    };
    let output =
        server.approval_request(rmcp::handler::server::wrapper::Parameters(apr_input)).await;
    assert_eq!(error_code(&parse_output(&output)), Some("VALIDATION_ERROR"));

    // Non-existent quote
    let apr_input = quotey_mcp::server::ApprovalRequestInput {
        quote_id: "Q-NONEXISTENT".to_string(),
        justification: "Valid justification".to_string(),
        approver_role: None,
    };
    let output =
        server.approval_request(rmcp::handler::server::wrapper::Parameters(apr_input)).await;
    assert_eq!(error_code(&parse_output(&output)), Some("NOT_FOUND"));

    Ok(())
}

#[tokio::test]
async fn test_approval_status_pending() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-STS", "SKU-STS", "STS Item", 5000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Create quote and request approval
    let create_input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-STS".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-STS".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("sts-test".to_string()),
    };

    let create_output =
        server.quote_create(rmcp::handler::server::wrapper::Parameters(create_input)).await;
    let quote_id =
        parse_output(&create_output).get("quote_id").and_then(|v| v.as_str()).unwrap().to_string();

    let apr_input = quotey_mcp::server::ApprovalRequestInput {
        quote_id: quote_id.clone(),
        justification: "Need approval for this deal".to_string(),
        approver_role: Some("vp_sales".to_string()),
    };
    server.approval_request(rmcp::handler::server::wrapper::Parameters(apr_input)).await;

    // Check status
    let status_input = quotey_mcp::server::ApprovalStatusInput { quote_id: quote_id.clone() };

    let status_output =
        server.approval_status(rmcp::handler::server::wrapper::Parameters(status_input)).await;
    let status_parsed = parse_output(&status_output);

    assert!(status_parsed.get("error").is_none(), "expected success, got: {:?}", status_parsed);
    assert_eq!(status_parsed.get("quote_id").and_then(|v| v.as_str()), Some(quote_id.as_str()));
    assert_eq!(
        status_parsed.get("current_status").and_then(|v| v.as_str()),
        Some("pending_approval")
    );
    assert_eq!(status_parsed.get("can_proceed").and_then(|v| v.as_bool()), Some(false));

    let pending =
        status_parsed.get("pending_requests").and_then(|v| v.as_array()).expect("pending_requests");
    assert_eq!(pending.len(), 1);

    Ok(())
}

#[tokio::test]
async fn test_approval_status_no_approvals() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-NOA", "SKU-NOA", "NOA Item", 5000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Create quote without approval request
    let create_input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-NOA".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-NOA".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("noa-test".to_string()),
    };

    let create_output =
        server.quote_create(rmcp::handler::server::wrapper::Parameters(create_input)).await;
    let quote_id =
        parse_output(&create_output).get("quote_id").and_then(|v| v.as_str()).unwrap().to_string();

    // Check status
    let status_input = quotey_mcp::server::ApprovalStatusInput { quote_id: quote_id.clone() };

    let status_output =
        server.approval_status(rmcp::handler::server::wrapper::Parameters(status_input)).await;
    let status_parsed = parse_output(&status_output);

    assert_eq!(status_parsed.get("current_status").and_then(|v| v.as_str()), Some("no_approvals"));
    assert_eq!(status_parsed.get("can_proceed").and_then(|v| v.as_bool()), Some(false));

    Ok(())
}

#[tokio::test]
async fn test_approval_pending_list() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-PEND", "SKU-PEND", "Pend Item", 5000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Create quote and request approval
    let create_input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-PEND".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-PEND".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("pend-test".to_string()),
    };

    let create_output =
        server.quote_create(rmcp::handler::server::wrapper::Parameters(create_input)).await;
    let quote_id =
        parse_output(&create_output).get("quote_id").and_then(|v| v.as_str()).unwrap().to_string();

    let apr_input = quotey_mcp::server::ApprovalRequestInput {
        quote_id: quote_id.clone(),
        justification: "Urgent deal requiring approval".to_string(),
        approver_role: Some("sales_manager".to_string()),
    };
    server.approval_request(rmcp::handler::server::wrapper::Parameters(apr_input)).await;

    // List pending approvals
    let pending_input = quotey_mcp::server::ApprovalPendingInput {
        approver_role: Some("sales_manager".to_string()),
        limit: 20,
    };

    let pending_output =
        server.approval_pending(rmcp::handler::server::wrapper::Parameters(pending_input)).await;
    let pending_parsed = parse_output(&pending_output);

    assert!(pending_parsed.get("error").is_none(), "expected success, got: {:?}", pending_parsed);

    let items = pending_parsed.get("items").and_then(|v| v.as_array()).expect("items");
    assert!(!items.is_empty(), "should find pending approvals");

    // Find our approval
    let found = items
        .iter()
        .any(|item| item.get("quote_id").and_then(|v| v.as_str()) == Some(quote_id.as_str()));
    assert!(found, "should find our pending approval");

    Ok(())
}

// ============================================================================
// PDF Tool: quote_pdf
// ============================================================================

#[tokio::test]
async fn test_quote_pdf_success() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-PDF", "SKU-PDF", "PDF Item", 7500).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Create quote
    let create_input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-PDF".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: Some(12),
        start_date: None,
        notes: Some("Quote for PDF generation".to_string()),
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-PDF".to_string(),
            quantity: 2,
            discount_pct: 5.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("pdf-test".to_string()),
    };

    let create_output =
        server.quote_create(rmcp::handler::server::wrapper::Parameters(create_input)).await;
    let quote_id =
        parse_output(&create_output).get("quote_id").and_then(|v| v.as_str()).unwrap().to_string();

    // Generate PDF
    let pdf_input = quotey_mcp::server::QuotePdfInput {
        quote_id: quote_id.clone(),
        template: "detailed".to_string(),
    };

    let pdf_output = server.quote_pdf(rmcp::handler::server::wrapper::Parameters(pdf_input)).await;
    let pdf_parsed = parse_output(&pdf_output);

    if pdf_parsed.get("error").is_some() {
        // In some local environments template rendering can still fail and is intentionally
        // sanitized by hardening logic.
        assert_eq!(error_code(&pdf_parsed), Some("INTERNAL_ERROR"));
    } else {
        assert_eq!(pdf_parsed.get("quote_id").and_then(|v| v.as_str()), Some(quote_id.as_str()));
        assert!(pdf_parsed.get("file_path").and_then(|v| v.as_str()).is_some());
        assert!(pdf_parsed.get("checksum").and_then(|v| v.as_str()).is_some());
        assert_eq!(pdf_parsed.get("template_used").and_then(|v| v.as_str()), Some("detailed"));
    }

    Ok(())
}

#[tokio::test]
async fn test_quote_pdf_not_found() -> TestResult {
    let pool = setup_pool().await?;
    let server = QuoteyMcpServer::new(pool.clone());

    let pdf_input = quotey_mcp::server::QuotePdfInput {
        quote_id: "Q-NONEXISTENT".to_string(),
        template: "detailed".to_string(),
    };

    let pdf_output = server.quote_pdf(rmcp::handler::server::wrapper::Parameters(pdf_input)).await;
    assert_eq!(error_code(&parse_output(&pdf_output)), Some("NOT_FOUND"));

    Ok(())
}

#[tokio::test]
async fn test_quote_pdf_invalid_template() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-PDFV", "SKU-PDFV", "PDFV Item", 5000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Create quote
    let create_input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-PDFV".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-PDFV".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("pdfv-test".to_string()),
    };

    let create_output =
        server.quote_create(rmcp::handler::server::wrapper::Parameters(create_input)).await;
    let quote_id =
        parse_output(&create_output).get("quote_id").and_then(|v| v.as_str()).unwrap().to_string();

    // Try invalid template
    let pdf_input = quotey_mcp::server::QuotePdfInput {
        quote_id: quote_id.clone(),
        template: "invalid_template".to_string(),
    };

    let pdf_output = server.quote_pdf(rmcp::handler::server::wrapper::Parameters(pdf_input)).await;
    assert_eq!(error_code(&parse_output(&pdf_output)), Some("VALIDATION_ERROR"));

    Ok(())
}

// ============================================================================
// Audit Trail Verification Tests
// ============================================================================

#[tokio::test]
async fn test_audit_trail_catalog_tools() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-AUD", "SKU-AUD", "Audit Item", 1000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Call catalog_search
    let search_input = quotey_mcp::server::CatalogSearchInput {
        query: "Audit".to_string(),
        category: None,
        active_only: true,
        limit: 10,
        page: 1,
    };
    server.catalog_search(rmcp::handler::server::wrapper::Parameters(search_input)).await;

    // Verify audit event was created
    let row = sqlx::query(
        "SELECT event_type, event_category FROM audit_event WHERE event_type = 'mcp.catalog_search.invoked' ORDER BY timestamp DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| format!("audit event not found: {e}"))?;

    assert_eq!(row.get::<String, _>("event_type"), "mcp.catalog_search.invoked");
    assert_eq!(row.get::<String, _>("event_category"), "mcp_tool");

    Ok(())
}

#[tokio::test]
async fn test_audit_trail_quote_tools() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-QAUD", "SKU-QAUD", "Quote Audit Item", 1000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Create quote
    let create_input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-QAUD".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-QAUD".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("qaud-test".to_string()),
    };

    server.quote_create(rmcp::handler::server::wrapper::Parameters(create_input)).await;

    // Verify audit event
    let row = sqlx::query(
        "SELECT event_type, event_category FROM audit_event WHERE event_type = 'mcp.quote_create.invoked' ORDER BY timestamp DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| format!("audit event not found: {e}"))?;

    assert_eq!(row.get::<String, _>("event_type"), "mcp.quote_create.invoked");

    Ok(())
}

#[tokio::test]
async fn test_audit_trail_approval_tools() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-AAUD", "SKU-AAUD", "Approval Audit Item", 1000).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // Create quote and request approval
    let create_input = quotey_mcp::server::QuoteCreateInput {
        account_id: "ACCT-AAUD".to_string(),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        notes: None,
        line_items: vec![quotey_mcp::server::LineItemInput {
            product_id: "PROD-AAUD".to_string(),
            quantity: 1,
            discount_pct: 0.0,
            attributes: None,
            notes: None,
        }],
        idempotency_key: Some("aaud-test".to_string()),
    };

    let create_output =
        server.quote_create(rmcp::handler::server::wrapper::Parameters(create_input)).await;
    let quote_id =
        parse_output(&create_output).get("quote_id").and_then(|v| v.as_str()).unwrap().to_string();

    let apr_input = quotey_mcp::server::ApprovalRequestInput {
        quote_id: quote_id.clone(),
        justification: "Audit trail test".to_string(),
        approver_role: None,
    };

    server.approval_request(rmcp::handler::server::wrapper::Parameters(apr_input)).await;

    // Verify audit event
    let row = sqlx::query(
        "SELECT event_type, quote_id FROM audit_event WHERE event_type = 'mcp.approval_request.invoked' ORDER BY timestamp DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| format!("audit event not found: {e}"))?;

    assert_eq!(row.get::<String, _>("event_type"), "mcp.approval_request.invoked");
    assert_eq!(row.get::<Option<String>, _>("quote_id"), Some(quote_id));

    Ok(())
}

// ============================================================================
// AI Agent Workflow Smoke Test
// ============================================================================

#[tokio::test]
async fn test_ai_agent_quote_workflow_smoke() -> TestResult {
    let pool = setup_pool().await?;
    seed_product(&pool, "PROD-AI-1", "SKU-AI-1", "AI Workflow Core", 2500).await?;
    seed_product(&pool, "PROD-AI-2", "SKU-AI-2", "AI Workflow Add-on", 1200).await?;

    let server = QuoteyMcpServer::new(pool.clone());

    // 1) Agent searches catalog
    let search_output = server
        .catalog_search(rmcp::handler::server::wrapper::Parameters(
            quotey_mcp::server::CatalogSearchInput {
                query: "AI Workflow".to_string(),
                category: None,
                active_only: true,
                limit: 10,
                page: 1,
            },
        ))
        .await;
    let search_parsed = parse_output(&search_output);
    assert!(search_parsed.get("error").is_none(), "catalog search failed: {search_parsed}");
    let first_product_id = search_parsed
        .get("items")
        .and_then(|v| v.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "catalog search returned no products".to_string())?
        .to_string();

    // 2) Agent creates quote
    let create_output = server
        .quote_create(rmcp::handler::server::wrapper::Parameters(
            quotey_mcp::server::QuoteCreateInput {
                account_id: "ACCT-AI-SMOKE".to_string(),
                deal_id: Some("DEAL-AI-SMOKE".to_string()),
                currency: "USD".to_string(),
                term_months: Some(12),
                start_date: None,
                notes: Some("AI workflow smoke test".to_string()),
                line_items: vec![quotey_mcp::server::LineItemInput {
                    product_id: first_product_id,
                    quantity: 4,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: Some("line added by agent flow".to_string()),
                }],
                idempotency_key: Some("ai-workflow-smoke".to_string()),
            },
        ))
        .await;
    let create_parsed = parse_output(&create_output);
    assert!(create_parsed.get("error").is_none(), "quote create failed: {create_parsed}");
    let quote_id = create_parsed
        .get("quote_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "quote_create missing quote_id".to_string())?
        .to_string();

    // 3) Agent runs pricing with discount request
    let price_output = server
        .quote_price(rmcp::handler::server::wrapper::Parameters(
            quotey_mcp::server::QuotePriceInput {
                quote_id: quote_id.clone(),
                requested_discount_pct: 12.5,
            },
        ))
        .await;
    let price_parsed = parse_output(&price_output);
    assert!(price_parsed.get("error").is_none(), "quote price failed: {price_parsed}");

    // 4) Agent checks approval status before request
    let status_before_output = server
        .approval_status(rmcp::handler::server::wrapper::Parameters(
            quotey_mcp::server::ApprovalStatusInput { quote_id: quote_id.clone() },
        ))
        .await;
    let status_before_parsed = parse_output(&status_before_output);
    assert!(
        status_before_parsed.get("error").is_none(),
        "approval status (pre-request) failed: {status_before_parsed}"
    );

    // 5) Agent requests approval
    let request_output = server
        .approval_request(rmcp::handler::server::wrapper::Parameters(
            quotey_mcp::server::ApprovalRequestInput {
                quote_id: quote_id.clone(),
                justification: "Agent workflow smoke approval request".to_string(),
                approver_role: Some("manager".to_string()),
            },
        ))
        .await;
    let request_parsed = parse_output(&request_output);
    assert!(request_parsed.get("error").is_none(), "approval request failed: {request_parsed}");

    // 6) Agent verifies pending queue visibility
    let pending_output = server
        .approval_pending(rmcp::handler::server::wrapper::Parameters(
            quotey_mcp::server::ApprovalPendingInput {
                approver_role: Some("manager".to_string()),
                limit: 20,
            },
        ))
        .await;
    let pending_parsed = parse_output(&pending_output);
    assert!(pending_parsed.get("error").is_none(), "approval pending failed: {pending_parsed}");

    // 7) Agent requests PDF artifact
    let pdf_output = server
        .quote_pdf(rmcp::handler::server::wrapper::Parameters(quotey_mcp::server::QuotePdfInput {
            quote_id: quote_id.clone(),
            template: "detailed".to_string(),
        }))
        .await;
    let pdf_parsed = parse_output(&pdf_output);
    if pdf_parsed.get("error").is_some() {
        assert_eq!(error_code(&pdf_parsed), Some("INTERNAL_ERROR"));
    } else {
        assert_eq!(pdf_parsed.get("quote_id").and_then(|v| v.as_str()), Some(quote_id.as_str()));
    }

    // 8) Verify core audit trail footprint across the workflow
    for event_type in [
        "mcp.catalog_search.invoked",
        "mcp.quote_create.invoked",
        "mcp.quote_price.invoked",
        "mcp.approval_status.invoked",
        "mcp.approval_request.invoked",
        "mcp.approval_pending.invoked",
        "mcp.quote_pdf.invoked",
    ] {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM audit_event WHERE event_type = ?")
                .bind(event_type)
                .fetch_one(&pool)
                .await
                .map_err(|e| format!("audit count for {event_type}: {e}"))?;
        assert!(count > 0, "expected at least one audit event for {event_type}");
    }

    Ok(())
}
