//! Integration tests for Quotey MCP Server
//!
//! These tests verify that the MCP server correctly handles:
//! - Server info and tool listing
//! - Tool calling (no-auth mode)
//! - Authentication enforcement (auth-required mode)
//! - Rate limiting

use quotey_mcp::{ApiKeyConfig, AuthConfig, AuthManager, QuoteyMcpServer};
use rmcp::ServerHandler;

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
