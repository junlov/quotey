//! Quotey MCP Server Binary
//!
//! This is the entry point for running the Quotey MCP server.
//!
//! ## Usage
//!
//! ```bash
//! # Run with default database (no auth)
//! quotey-mcp
//!
//! # Run with specific database
//! DATABASE_URL=sqlite://quotey.db quotey-mcp
//!
//! # Run with API key authentication
//! MCP_API_KEY=your-secret-key quotey-mcp
//!
//! # Run with multiple API keys and rate limiting
//! MCP_API_KEYS='[{"key":"key1","name":"Agent1","requests_per_minute":60}]' quotey-mcp
//! ```

use anyhow::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Quotey MCP Server");

    // Get database URL from environment or use default
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://quotey.db".to_string());

    info!("Connecting to database: {}", database_url);

    // Connect to database
    let db_pool = quotey_db::connect(&database_url).await?;

    // Check for authentication configuration
    let server = if let Ok(api_keys_json) = std::env::var("MCP_API_KEYS") {
        // Load API keys from JSON configuration
        info!("Loading API key authentication");
        let key_configs: Vec<quotey_mcp::ApiKeyConfig> = serde_json::from_str(&api_keys_json)?;
        let auth_manager = quotey_mcp::AuthManager::from_config(&quotey_mcp::AuthConfig {
            enabled: true,
            rate_limit_window_secs: 60,
            api_keys: key_configs,
        });
        quotey_mcp::QuoteyMcpServer::with_auth(db_pool, auth_manager)
    } else if let Ok(single_key) = std::env::var("MCP_API_KEY") {
        // Single API key mode
        info!("Using single API key authentication");
        let auth_manager = quotey_mcp::AuthManager::from_config(&quotey_mcp::AuthConfig {
            enabled: true,
            rate_limit_window_secs: 60,
            api_keys: vec![quotey_mcp::ApiKeyConfig {
                key: single_key,
                name: "default".to_string(),
                requests_per_minute: 60,
            }],
        });
        quotey_mcp::QuoteyMcpServer::with_auth(db_pool, auth_manager)
    } else {
        // No authentication
        info!("Running without authentication");
        quotey_mcp::QuoteyMcpServer::new(db_pool)
    };

    // Run MCP server
    server.run_stdio().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_api_keys_json() {
        let json = r#"[
            {"key":"test-key-123","name":"Test Agent","requests_per_minute":30}
        ]"#;

        let configs: Vec<quotey_mcp::ApiKeyConfig> = serde_json::from_str(json).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].name, "Test Agent");
        assert_eq!(configs[0].requests_per_minute, 30);
    }
}
