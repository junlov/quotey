//! Quotey MCP (Model Context Protocol) Server
//!
//! This crate provides an MCP server implementation that allows AI agents
//! to programmatically interact with Quotey for quote creation, pricing,
//! approval workflows, and catalog management.
//!
//! ## Architecture
//!
//! - `QuoteyMcpServer`: Main server implementing MCP protocol
//! - `tools/`: Individual tool implementations (catalog, quote, approval, pdf)
//! - `transport/`: Transport layer (stdio, TCP, WebSocket)
//!
//! ## Example Usage
//!
//! ```no_run
//! use quotey_mcp::QuoteyMcpServer;
//! use std::io::{stdin, stdout};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let server = QuoteyMcpServer::new().await?;
//!     server.run_stdio().await
//! }
//! ```

mod server;
mod tools;

pub use server::QuoteyMcpServer;
pub use tools::*;

use thiserror::Error;

/// Errors specific to MCP server operations
#[derive(Error, Debug)]
pub enum McpError {
    #[error("database error: {0}")]
    Database(#[from] db::RepositoryError),

    #[error("domain error: {0}")]
    Domain(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl McpError {
    /// Convert to JSON-RPC error code
    pub fn error_code(&self) -> i32 {
        match self {
            McpError::NotFound(_) => -32602, // Invalid params (resource not found)
            McpError::Validation(_) => -32602, // Invalid params
            McpError::PermissionDenied(_) => -32001, // Server error (permission)
            McpError::Domain(_) => -32600,   // Invalid request
            McpError::Database(_) | McpError::Internal(_) => -32603, // Internal error
        }
    }
}

/// Result type for MCP operations
pub type McpResult<T> = Result<T, McpError>;
