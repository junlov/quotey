//! Quotey MCP Server Binary
//!
//! This is the entry point for running the Quotey MCP server.
//!
//! ## Usage
//!
//! ```bash
//! # Run with default database
//! quotey-mcp
//!
//! # Run with specific database
//! DATABASE_URL=sqlite://quotey.db quotey-mcp
//! ```

use anyhow::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Quotey MCP Server");

    // Get database URL from environment or use default
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://quotey.db".to_string());

    info!("Connecting to database: {}", database_url);

    // Connect to database
    let db_pool = quotey_db::connect(&database_url).await?;

    // Create and run MCP server
    let server = quotey_mcp::QuoteyMcpServer::new(db_pool);
    server.run_stdio().await?;

    Ok(())
}
