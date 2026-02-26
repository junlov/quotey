//! Quotey MCP (Model Context Protocol) Server
//!
//! This crate provides an MCP server implementation that allows AI agents
//! to programmatically interact with Quotey for quote creation, pricing,
//! approval workflows, and catalog management.
//!
//! ## Tools Available
//!
//! ### Catalog Tools
//! - `catalog_search`: Search products by name, SKU, or description
//! - `catalog_get`: Get detailed product information by ID
//!
//! ### Quote Tools
//! - `quote_create`: Create a new quote for a customer
//! - `quote_get`: Get detailed quote information
//! - `quote_price`: Run pricing engine on a quote
//! - `quote_list`: List quotes with optional filters
//!
//! ### Approval Tools
//! - `approval_request`: Submit a quote for approval
//! - `approval_status`: Check approval status for a quote
//! - `approval_pending`: List all pending approval requests
//!
//! ### PDF Tools
//! - `quote_pdf`: Generate PDF for a quote
//!
//! ## Example Usage
//!
//! ```no_run
//! use quotey_mcp::QuoteyMcpServer;
//! use db::connect;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let db_pool = connect("sqlite://quotey.db").await?;
//!     let server = QuoteyMcpServer::new(db_pool);
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
    Database(#[from] quotey_db::repositories::RepositoryError),

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

/// Version of the MCP server
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
