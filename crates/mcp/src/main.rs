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
//!
//! # Run with auth from quotey.toml ([mcp.auth] section)
//! QUOTEY_CONFIG_PATH=config/quotey.dev.toml quotey-mcp
//!
//! # Customize auth rate-limit defaults
//! MCP_RATE_LIMIT_WINDOW_SECS=120 MCP_DEFAULT_REQUESTS_PER_MINUTE=90 MCP_API_KEY=secret quotey-mcp
//!
//! # Pin MCP protocol version for legacy clients (defaults to latest supported)
//! QUOTEY_MCP_PROTOCOL_VERSION=2024-11-05 quotey-mcp
//! ```

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;
use std::env::VarError;
use std::path::{Path, PathBuf};
use tracing::info;

fn redact_database_url(database_url: &str) -> String {
    let Some((scheme, remainder)) = database_url.split_once("://") else {
        return database_url.to_string();
    };
    let Some((authority, tail)) = remainder.split_once('@') else {
        return database_url.to_string();
    };
    if authority.contains(':') {
        format!("{scheme}://***:***@{tail}")
    } else {
        format!("{scheme}://***@{tail}")
    }
}

fn read_positive_u64_env(key: &str, default: u64) -> Result<u64> {
    match std::env::var(key) {
        Ok(raw) => {
            let parsed = raw
                .trim()
                .parse::<u64>()
                .map_err(|_| anyhow!("{key} must be a positive integer, got `{raw}`"))?;
            if parsed == 0 {
                bail!("{key} must be >= 1");
            }
            Ok(parsed)
        }
        Err(VarError::NotPresent) => Ok(default),
        Err(VarError::NotUnicode(_)) => bail!("{key} is not valid UTF-8"),
    }
}

fn read_positive_u32_env(key: &str, default: u32) -> Result<u32> {
    match std::env::var(key) {
        Ok(raw) => {
            let parsed = raw
                .trim()
                .parse::<u32>()
                .map_err(|_| anyhow!("{key} must be a positive integer, got `{raw}`"))?;
            if parsed == 0 {
                bail!("{key} must be >= 1");
            }
            Ok(parsed)
        }
        Err(VarError::NotPresent) => Ok(default),
        Err(VarError::NotUnicode(_)) => bail!("{key} is not valid UTF-8"),
    }
}

#[derive(Debug, Deserialize, Default)]
struct McpConfigFile {
    mcp: Option<McpSection>,
}

#[derive(Debug, Deserialize, Default)]
struct McpSection {
    auth: Option<quotey_mcp::AuthConfig>,
}

fn parse_auth_config_from_toml(raw: &str) -> Result<Option<quotey_mcp::AuthConfig>> {
    let parsed: McpConfigFile = toml::from_str(raw)?;
    Ok(parsed.mcp.and_then(|mcp| mcp.auth))
}

fn discover_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(config_path) = std::env::var("QUOTEY_CONFIG_PATH") {
        let trimmed = config_path.trim();
        if !trimmed.is_empty() {
            paths.push(PathBuf::from(trimmed));
        }
    }
    paths.push(PathBuf::from("quotey.toml"));
    paths.push(PathBuf::from("config/quotey.toml"));
    paths.push(PathBuf::from("config/quotey.dev.toml"));
    paths
}

async fn load_auth_config_from_file() -> Result<Option<(PathBuf, quotey_mcp::AuthConfig)>> {
    for path in discover_config_paths() {
        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            continue;
        }
        let raw = tokio::fs::read_to_string(&path).await?;
        if let Some(mut auth) = parse_auth_config_from_toml(&raw)? {
            auth.rate_limit_window_secs = auth.rate_limit_window_secs.max(1);
            for key in &mut auth.api_keys {
                key.requests_per_minute = key.requests_per_minute.max(1);
            }
            return Ok(Some((path, auth)));
        }
    }
    Ok(None)
}

fn auth_source_label(path: &Path) -> String {
    format!("config:{}", path.display())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Quotey MCP Server");

    // Get database URL from environment or use default
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://quotey.db".to_string());

    info!("Connecting to database: {}", redact_database_url(&database_url));

    // Connect to database
    let db_pool = quotey_db::connect(&database_url).await?;

    let rate_limit_window_secs = read_positive_u64_env("MCP_RATE_LIMIT_WINDOW_SECS", 60)?;
    let default_requests_per_minute = read_positive_u32_env("MCP_DEFAULT_REQUESTS_PER_MINUTE", 60)?;

    let mut auth_source = "none".to_string();
    let auth_config = if let Ok(api_keys_json) = std::env::var("MCP_API_KEYS") {
        auth_source = "env:MCP_API_KEYS".to_string();
        let key_configs: Vec<quotey_mcp::ApiKeyConfig> = serde_json::from_str(&api_keys_json)?;
        Some(quotey_mcp::AuthConfig {
            enabled: true,
            rate_limit_window_secs,
            api_keys: key_configs,
        })
    } else if let Ok(single_key) = std::env::var("MCP_API_KEY") {
        auth_source = "env:MCP_API_KEY".to_string();
        Some(quotey_mcp::AuthConfig {
            enabled: true,
            rate_limit_window_secs,
            api_keys: vec![quotey_mcp::ApiKeyConfig {
                key: single_key,
                name: "default".to_string(),
                requests_per_minute: default_requests_per_minute,
            }],
        })
    } else if let Some((path, config)) = load_auth_config_from_file().await? {
        auth_source = auth_source_label(&path);
        Some(config)
    } else {
        None
    };

    let server = if let Some(auth_config) = auth_config {
        let auth_manager = quotey_mcp::AuthManager::from_config(&auth_config);
        if auth_manager.is_auth_required() {
            info!(
                source = %auth_source,
                key_count = auth_config.api_keys.len(),
                rate_limit_window_secs = auth_config.rate_limit_window_secs,
                "Loading API key authentication"
            );
        } else {
            info!(source = %auth_source, "Auth configuration resolved to no-auth mode");
        }
        quotey_mcp::QuoteyMcpServer::with_auth(db_pool, auth_manager)
    } else {
        info!("Running without authentication");
        quotey_mcp::QuoteyMcpServer::new(db_pool)
    };

    // Run MCP server
    server.run_stdio().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_parse_auth_config_from_toml_mcp_section() {
        let toml = r#"
            [mcp.auth]
            enabled = true
            rate_limit_window_secs = 45

            [[mcp.auth.api_keys]]
            key = "key-1"
            name = "Agent One"
            requests_per_minute = 12
        "#;

        let parsed = parse_auth_config_from_toml(toml).expect("parse toml").expect("mcp auth");
        assert!(parsed.enabled);
        assert_eq!(parsed.rate_limit_window_secs, 45);
        assert_eq!(parsed.api_keys.len(), 1);
        assert_eq!(parsed.api_keys[0].name, "Agent One");
        assert_eq!(parsed.api_keys[0].requests_per_minute, 12);
    }

    #[test]
    fn test_parse_auth_config_from_toml_missing_mcp_section() {
        let toml = r#"
            [database]
            url = "sqlite://quotey.db"
        "#;
        let parsed = parse_auth_config_from_toml(toml).expect("parse toml");
        assert!(parsed.is_none());
    }
}
